use crate::types::{
    ClickState, KeyId, KeyInfo, KeyState, MouseButton, MouseInfo, Pos, Shortcut, ID,
};
use crate::utils::gen_id;
use crate::windows::focus_tracker::{
    initialize_global_focus_tracker, shutdown_global_focus_tracker,
};
use crate::windows::worker::{KeyboardSysMsg, MouseSysMsg, WorkerMsg};
use crate::windows::WM_USER_RECHECK_HOOK;
use crate::Listener;

use lazy_static::lazy_static;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};
use std::thread;
use windows::core::PCWSTR;
use windows::Win32::Devices::HumanInterfaceDevice::{
    HID_USAGE_GENERIC_KEYBOARD, HID_USAGE_GENERIC_MOUSE, HID_USAGE_PAGE_GENERIC,
    KEYBOARD_OVERRUN_MAKE_CODE,
};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Globalization::UCHAR_MAX_VALUE;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{
    GetCurrentThread, GetCurrentThreadId, SetThreadPriority, THREAD_PRIORITY_TIME_CRITICAL,
};
use windows::Win32::UI::Input::{
    GetRawInputData, RegisterRawInputDevices, HRAWINPUT, MOUSE_MOVE_ABSOLUTE,
    MOUSE_VIRTUAL_DESKTOP, RAWINPUT, RAWINPUTDEVICE, RAWINPUTHEADER, RIDEV_INPUTSINK,
    RID_DEVICE_INFO_TYPE, RID_INPUT, RIM_TYPEKEYBOARD, RIM_TYPEMOUSE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetCursorPos, GetMessageW,
    GetSystemMetrics, PostThreadMessageW, RegisterClassW, TranslateMessage, CW_USEDEFAULT, HHOOK,
    MSG, RI_KEY_BREAK, RI_MOUSE_BUTTON_4_DOWN, RI_MOUSE_BUTTON_4_UP, RI_MOUSE_BUTTON_5_DOWN,
    RI_MOUSE_BUTTON_5_UP, RI_MOUSE_LEFT_BUTTON_DOWN, RI_MOUSE_LEFT_BUTTON_UP,
    RI_MOUSE_MIDDLE_BUTTON_DOWN, RI_MOUSE_MIDDLE_BUTTON_UP, RI_MOUSE_RIGHT_BUTTON_DOWN,
    RI_MOUSE_RIGHT_BUTTON_UP, SM_CXSCREEN, SM_CXVIRTUALSCREEN, SM_CYSCREEN, SM_CYVIRTUALSCREEN,
    SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, WM_INPUT, WM_QUIT, WM_USER, WNDCLASSW, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT, WS_OVERLAPPED,
};

thread_local! {
    static LOCAL_KEYBOARD_HHOOK: RefCell<HashMap<ID, HHOOK>> = RefCell::new(HashMap::new());
    static LOCAL_MOUSE_HHOOK: RefCell<HashMap<ID, HHOOK>> = RefCell::new(HashMap::new());
    static LOCAL_KEY_LAST_TIME: RefCell<u32> = RefCell::new(0);
    static LOCAL_HWDN: RefCell<HashMap<ID, HWND>> = RefCell::new(HashMap::new());
    // static LOCAL_KEYBOARD_STATE: RefCell<KeyboardState> = RefCell::new(KeyboardState::new(Some(consts::MAX_KEYS)));
    static LOCAL_KEYBOARD_STATE_S: RefCell<Shortcut> = RefCell::new(Shortcut::default());
    static BUFFER_POOL: RefCell<Vec<Vec<u8>>> = RefCell::new(Vec::new());
}

#[derive(Debug)]
pub(crate) struct EventLoop {
    id: ID,
    // main_thread_id: Arc<Mutex<u32>>,
    loop_thread_id: Arc<Mutex<u32>>,
    thread_handle: Mutex<Option<Arc<thread::JoinHandle<()>>>>,
    listener: Weak<Listener>,
}

impl Drop for EventLoop {
    fn drop(&mut self) {
        EVENT_LOOP_MANAGER.lock().unwrap().del_event_loop(self.id);
        self.uninit_fake_win();
    }
}

impl EventLoop {
    fn new(listener: &Arc<Listener>) -> Self {
        Self {
            id: gen_id(),
            loop_thread_id: Arc::new(Mutex::new(0)),
            thread_handle: Mutex::new(None),
            listener: Arc::downgrade(listener),
        }
    }

    fn keyboard_proc(rawinput: &RAWINPUT) {
        let keyboard = unsafe { &rawinput.data.keyboard };
        let key_up = keyboard.Flags as u32 & RI_KEY_BREAK > 0;

        if keyboard.MakeCode as u32 == KEYBOARD_OVERRUN_MAKE_CODE
            || keyboard.VKey as u32 >= UCHAR_MAX_VALUE
        {
            return;
        }

        let key_id: Result<KeyId, _> = KeyId::try_from(*keyboard);
        if key_id.is_err() {
            println!("Get KeyID failed {:?}", keyboard);
            return;
        }
        let key_id = key_id.unwrap();
        let mut key_info = KeyInfo::new(
            key_id,
            if key_up {
                KeyState::Released
            } else {
                KeyState::Pressed
            },
        );

        let mut old_state: Option<Shortcut> = None;
        LOCAL_KEYBOARD_STATE_S.with(|state| {
            old_state.replace(state.borrow().clone());
            if key_info.state == KeyState::Pressed {
                state.borrow_mut().set_key(key_id.into())
            } else {
                state.borrow_mut().remove_key(key_id.into())
            }
            key_info.keyboard_state.replace(state.borrow().clone());
        });

        if old_state == key_info.keyboard_state {
            #[cfg(feature = "Debug")]
            println!("Key State not changed {:?}", key_info);
            return;
        }

        #[cfg(feature = "Debug")]
        println!("kbd: vk_code={:?} key_info={:?}", keyboard.VKey, key_info);

        let msg = WorkerMsg::KeyboardEvent(KeyboardSysMsg::new(key_info));

        let event_loops = { EVENT_LOOP_MANAGER.lock().unwrap().get_keyboard_event_loop() };
        for event_loop in event_loops.iter() {
            event_loop.post_msg_to_worker(msg.clone());
        }
    }

    fn mouse_proc(rawinput: &RAWINPUT) {
        let mouse = unsafe { &rawinput.data.mouse };

        let button_flags = unsafe { mouse.Anonymous.Anonymous.usButtonFlags };
        let pos_flags = mouse.usFlags.0;
        let last_x = mouse.lLastX;
        let last_y = mouse.lLastY;

        let mut lppoint = windows::Win32::Foundation::POINT::default();
        unsafe {
            let _ = GetCursorPos(&mut lppoint);
        }

        let btn = match button_flags as u32 {
            RI_MOUSE_LEFT_BUTTON_DOWN => {
                // println!("Left mouse button down {:?}", lppoint);
                Some(MouseButton::Left(ClickState::Pressed))
            }
            RI_MOUSE_LEFT_BUTTON_UP => {
                // println!("Left mouse button up {:?}", lppoint);
                Some(MouseButton::Left(ClickState::Released))
            }
            RI_MOUSE_RIGHT_BUTTON_DOWN => {
                // println!("Right mouse button down {:?}", lppoint);
                Some(MouseButton::Right(ClickState::Pressed))
            }
            RI_MOUSE_RIGHT_BUTTON_UP => {
                // println!("Right mouse button up {:?}", lppoint);
                Some(MouseButton::Right(ClickState::Released))
            }
            RI_MOUSE_MIDDLE_BUTTON_DOWN => {
                // println!("Middle mouse button down {:?}", lppoint);
                Some(MouseButton::Middle(ClickState::Pressed))
            }
            RI_MOUSE_MIDDLE_BUTTON_UP => {
                // println!("Middle mouse button up {:?}", lppoint);
                Some(MouseButton::Middle(ClickState::Released))
            }
            RI_MOUSE_BUTTON_4_DOWN => {
                // println!("X1 mouse button down {:?}", lppoint);
                Some(MouseButton::X1(ClickState::Pressed))
            }
            RI_MOUSE_BUTTON_4_UP => {
                // println!("X1 mouse button up {:?}", lppoint);
                Some(MouseButton::X1(ClickState::Released))
            }
            RI_MOUSE_BUTTON_5_DOWN => {
                // println!("X2 mouse button down {:?}", lppoint);
                Some(MouseButton::X2(ClickState::Pressed))
            }
            RI_MOUSE_BUTTON_5_UP => {
                // println!("X2 mouse button up {:?}", lppoint);
                Some(MouseButton::X2(ClickState::Released))
            }
            _ => None,
        };

        if btn.is_none() && button_flags != 0 {
            #[cfg(feature = "Debug")]
            println!(
                "Currently, mouse button events are not supported. {:?}",
                button_flags
            );
            return;
        }

        let mut pos = Pos {
            x: lppoint.x,
            y: lppoint.y,
        };
        let mut rel_pos = Pos::default();
        if pos_flags & MOUSE_MOVE_ABSOLUTE.0 > 0 {
            let mut rect = RECT::default();
            if (pos_flags & MOUSE_VIRTUAL_DESKTOP.0) > 0 {
                unsafe {
                    rect.left = GetSystemMetrics(SM_XVIRTUALSCREEN);
                    rect.top = GetSystemMetrics(SM_YVIRTUALSCREEN);
                    rect.right = GetSystemMetrics(SM_CXVIRTUALSCREEN);
                    rect.bottom = GetSystemMetrics(SM_CYVIRTUALSCREEN);
                }
            } else {
                unsafe {
                    rect.left = 0;
                    rect.top = 0;
                    rect.right = GetSystemMetrics(SM_CXSCREEN);
                    rect.bottom = GetSystemMetrics(SM_CYSCREEN);
                }
            }

            let absolute_x = (mouse.lLastX * rect.right / u16::MAX as i32) + rect.left;
            let absolute_y = (mouse.lLastY * rect.bottom / u16::MAX as i32) + rect.top;

            pos.x = absolute_x;
            pos.y = absolute_y;
        } else if last_x != 0 || last_y != 0 {
            pos.x += last_x;
            pos.y += last_y;
            rel_pos.x = last_x;
            rel_pos.y = last_y;
        }

        let minfo = MouseInfo {
            button: btn,
            pos,
            relative_pos: rel_pos,
        };

        let msg = WorkerMsg::MouseEvent(MouseSysMsg::new(minfo));

        let event_loops = { EVENT_LOOP_MANAGER.lock().unwrap().get_mouse_event_loop() };
        for event_loop in event_loops.iter() {
            event_loop.post_msg_to_worker(msg.clone());
        }
    }

    unsafe extern "system" fn fake_win_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_INPUT => {
                // 安全边界检查和错误处理
                if let Err(_e) = Self::handle_raw_input(lparam) {
                    // #[cfg(feature = "Debug")]
                    eprintln!("Error handling raw input: {:?}", _e);
                }
            }
            _ => {}
        }
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }

    fn handle_raw_input(lparam: LPARAM) -> Result<(), &'static str> {
        let mut dw_size: u32 = 0;
        let hrawinput: HRAWINPUT = HRAWINPUT(lparam.0 as *mut std::ffi::c_void);

        // 第一次调用获取数据大小
        let result1 = unsafe {
            GetRawInputData(
                hrawinput,
                RID_INPUT,
                None,
                &mut dw_size,
                std::mem::size_of::<RAWINPUTHEADER>() as u32,
            )
        };

        // GetRawInputData 返回 u32，成功时返回数据大小，失败时返回 u32::MAX
        if result1 == u32::MAX {
            return Err("Failed to get raw input data size");
        }

        // 验证数据大小
        if dw_size == 0 {
            return Err("Raw input data size is zero");
        }

        // 防止过大的缓冲区分配
        const MAX_BUFFER_SIZE: u32 = 1024 * 16; // 16KB 限制
        if dw_size > MAX_BUFFER_SIZE {
            return Err("Raw input data size too large");
        }

        // 确保最小缓冲区大小
        if dw_size < std::mem::size_of::<RAWINPUTHEADER>() as u32 {
            return Err("Raw input data size too small");
        }

        // 使用缓冲区池优化内存分配
        let mut buffer = BUFFER_POOL.with(|pool| {
            let mut pool = pool.borrow_mut();
            pool.pop()
                .unwrap_or_else(|| Vec::with_capacity(dw_size as usize))
        });

        // 调整缓冲区大小
        if buffer.capacity() < dw_size as usize {
            buffer.resize(dw_size as usize, 0);
        } else {
            buffer.clear();
            buffer.resize(dw_size as usize, 0);
        }

        // 第二次调用获取实际数据
        let mut actual_size = dw_size;
        let result2 = unsafe {
            GetRawInputData(
                hrawinput,
                RID_INPUT,
                Some(buffer.as_mut_ptr() as *mut std::ffi::c_void),
                &mut actual_size,
                std::mem::size_of::<RAWINPUTHEADER>() as u32,
            )
        };

        if result2 == u32::MAX {
            // 归还缓冲区到池中
            Self::return_buffer_to_pool(buffer);
            return Err("Failed to get raw input data");
        }

        // 验证实际数据大小
        if actual_size == 0 || actual_size as usize > buffer.len() {
            Self::return_buffer_to_pool(buffer);
            return Err("Invalid actual data size");
        }

        // 安全地获取 RAWINPUT 结构
        let rawinput = unsafe { &*(buffer.as_ptr() as *const RAWINPUT) };

        // 使用 catch_unwind 防止 panic 影响系统
        let result =
            std::panic::catch_unwind(|| match RID_DEVICE_INFO_TYPE(rawinput.header.dwType) {
                RIM_TYPEKEYBOARD => {
                    Self::keyboard_proc(rawinput);
                }
                RIM_TYPEMOUSE => {
                    Self::mouse_proc(rawinput);
                }
                _ => {}
            });

        // 归还缓冲区到池中
        Self::return_buffer_to_pool(buffer);

        if result.is_err() {
            return Err("Panic occurred during input processing");
        }

        Ok(())
    }

    fn return_buffer_to_pool(buffer: Vec<u8>) {
        BUFFER_POOL.with(|pool| {
            let mut pool = pool.borrow_mut();
            // 限制池大小，避免内存泄漏
            const MAX_POOL_SIZE: usize = 4;
            if pool.len() < MAX_POOL_SIZE && buffer.capacity() <= 1024 * 16 {
                pool.push(buffer);
            }
            // 如果缓冲区太大或池已满，则让它自然释放
        });
    }

    fn set_keyboard_hook(&self) {
        {
            if EVENT_LOOP_MANAGER
                .lock()
                .unwrap()
                .has_keyboard_event(&self.id)
            {
                return;
            }
        }

        EVENT_LOOP_MANAGER
            .lock()
            .unwrap()
            .add_keyboard_event(self.id);
    }

    fn set_mouse_hook(&self) {
        {
            if EVENT_LOOP_MANAGER.lock().unwrap().has_mouse_event(&self.id) {
                return;
            }
        }

        EVENT_LOOP_MANAGER.lock().unwrap().add_mouse_event(self.id);
    }

    fn unhook_keyboard(&self) {
        {
            if !EVENT_LOOP_MANAGER
                .lock()
                .unwrap()
                .has_keyboard_event(&self.id)
            {
                return;
            }
        }

        EVENT_LOOP_MANAGER
            .lock()
            .unwrap()
            .del_keyboard_event(self.id);
    }

    fn unhook_mouse(&self) {
        {
            if !EVENT_LOOP_MANAGER.lock().unwrap().has_mouse_event(&self.id) {
                return;
            }
        }

        EVENT_LOOP_MANAGER.lock().unwrap().del_mouse_event(self.id);
    }

    fn set_focus_tracker(&self) {
        if let Some(listener) = self.listener.upgrade() {
            if !listener.is_enable_focus_tracker() {
                return;
            }
            let mut focus_tracker = listener.focus_tracker.lock().unwrap();
            if focus_tracker.is_none() {
                if let Ok(new_trancker) = initialize_global_focus_tracker(None) {
                    *focus_tracker = Some(new_trancker);
                } else {
                    #[cfg(feature = "Debug")]
                    println!("Failed to initialize focus tracker.");
                }
            }
        } else {
            return;
        }
    }

    fn unset_focus_tracker(&self) {
        if let Some(listener) = self.listener.upgrade() {
            if !listener.is_enable_focus_tracker() {
                return;
            }
            let mut focus_tracker = listener.focus_tracker.lock().unwrap();
            if focus_tracker.is_some() {
                shutdown_global_focus_tracker();
                focus_tracker.take(); // 清理实例
            } else {
                #[cfg(feature = "Debug")]
                println!("Focus tracker is already unset.");
            }
        } else {
            return;
        }
    }

    fn recheck_hook(&self) {
        if let Some(listener) = self.listener.upgrade() {
            if listener.has_keyboard_event() {
                self.set_keyboard_hook();
            } else {
                self.unhook_keyboard();
            }

            if listener.has_mouse_event() {
                self.set_mouse_hook();
            } else {
                self.unhook_mouse();
            }

            if listener.is_enable_focus_tracker() {
                self.set_focus_tracker();
            } else {
                self.unset_focus_tracker();
            }
        }
    }

    fn post_msg_to_worker(&self, msg: WorkerMsg) {
        #[cfg(feature = "Debug")]
        println!(
            "{:?} post_msg_to_worker {:?}",
            std::thread::current().id(),
            msg
        );

        if let Some(listener) = self.listener.upgrade() {
            if let Some(worker) = listener.get_worker() {
                worker.post_msg(msg);
            }
        }
    }

    pub fn post_msg_to_loop(&self, msg_type: u32) {
        #[cfg(feature = "Debug")]
        println!(
            "{:?} post_msg_to_loop {:?}",
            std::thread::current().id(),
            msg_type
        );

        let thread_id = {
            let binding = self.loop_thread_id.lock().unwrap();
            *binding
        };
        if thread_id == 0 {
            return;
        }
        unsafe {
            let _ = PostThreadMessageW(thread_id, WM_USER, WPARAM(msg_type as usize), LPARAM(0));
        }
    }

    fn init_fake_win(&self) -> std::result::Result<(), ()> {
        let hinstance = unsafe { GetModuleHandleW(None).unwrap().into() };
        let class_name: Vec<u16> =
            std::os::windows::ffi::OsStrExt::encode_wide(std::ffi::OsStr::new("kmhook_app"))
                .chain(std::iter::once(0))
                .collect();
        let wnd_class = WNDCLASSW {
            lpfnWndProc: Some(Self::fake_win_proc),
            hInstance: hinstance,
            lpszClassName: PCWSTR::from_raw(class_name.as_ptr()),
            ..Default::default()
        };
        unsafe {
            let _ = RegisterClassW(&wnd_class);
            let hwnd = CreateWindowExW(
                WS_EX_NOACTIVATE | WS_EX_TRANSPARENT | WS_EX_LAYERED | WS_EX_TOOLWINDOW,
                PCWSTR(class_name.as_ptr()),
                None,
                WS_OVERLAPPED,
                CW_USEDEFAULT,
                0,
                CW_USEDEFAULT,
                0,
                None,
                None,
                Some(hinstance),
                None,
            );
            if hwnd.is_err() {
                return Err(());
            }
            let hwnd = hwnd.unwrap();
            if hwnd.is_invalid() {
                return Err(());
            }

            self.register_raw_input(hwnd.clone());
            LOCAL_HWDN.with(|hwdn| {
                hwdn.borrow_mut().insert(self.id, hwnd);
            });

            Ok(())
        }
    }

    fn register_raw_input(&self, hwnd: HWND) {
        let rid = RAWINPUTDEVICE {
            usUsagePage: HID_USAGE_PAGE_GENERIC,
            usUsage: HID_USAGE_GENERIC_KEYBOARD,
            dwFlags: RIDEV_INPUTSINK,
            hwndTarget: hwnd,
        };
        let rid_mouse = RAWINPUTDEVICE {
            usUsagePage: HID_USAGE_PAGE_GENERIC,
            usUsage: HID_USAGE_GENERIC_MOUSE,
            dwFlags: RIDEV_INPUTSINK,
            hwndTarget: hwnd,
        };
        unsafe {
            let _ = RegisterRawInputDevices(
                &[rid, rid_mouse],
                std::mem::size_of::<RAWINPUTDEVICE>() as u32,
            );
        }
    }

    fn uninit_fake_win(&self) {
        LOCAL_HWDN.with(|hwdn| {
            if let Some(h) = hwdn.borrow_mut().remove(&self.id) {
                unsafe {
                    let _ = DestroyWindow(h);
                }
            }
        });
    }

    fn run(&self) {
        {
            *self.loop_thread_id.lock().unwrap() = unsafe { GetCurrentThreadId() };
        }
        unsafe {
            let thread_handle = GetCurrentThread();
            if SetThreadPriority(thread_handle, THREAD_PRIORITY_TIME_CRITICAL).is_err() {
                #[cfg(feature = "Debug")]
                println!("SetThreadPriority failed {:?}", thread_handle);
            }
        }

        if let Err(_) = self.init_fake_win() {
            return;
        }

        let mut msg = MSG::default();
        unsafe {
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                #[cfg(feature = "Debug")]
                println!("{:?} GetMessageW {:?}", std::thread::current().id(), msg);

                match msg.message {
                    WM_USER if msg.wParam.0 as u32 == WM_USER_RECHECK_HOOK => self.recheck_hook(),
                    _ => {
                        let _ = TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
            }
        }
    }

    pub fn stop(&self) {
        let loop_thread_id = *self.loop_thread_id.lock().unwrap();
        if loop_thread_id == 0 {
            return;
        }
        unsafe {
            let _ = PostThreadMessageW(loop_thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
        }
        *self.loop_thread_id.lock().unwrap() = 0;
    }

    pub fn run_with_thread(self: &Arc<Self>) {
        let event_loop = Arc::clone(self);
        let handle = thread::spawn(move || {
            #[cfg(feature = "Debug")]
            println!(
                "Event loop thread started with ID: {:?}",
                std::thread::current().id()
            );
            event_loop.recheck_hook();
            event_loop.run();
        });
        self.thread_handle.lock().unwrap().replace(Arc::new(handle));
    }
}

#[derive(Debug)]
pub(crate) struct EventLoopManager {
    event_loops: HashMap<ID, Arc<EventLoop>>,
    keyboard_event_ids: Vec<ID>,
    mouse_event_ids: Vec<ID>,
}

impl EventLoopManager {
    fn new() -> Self {
        Self {
            event_loops: HashMap::new(),
            keyboard_event_ids: Vec::new(),
            mouse_event_ids: Vec::new(),
        }
    }

    pub fn new_event_loop(&mut self, listener: &Arc<Listener>) -> Arc<EventLoop> {
        let event_loop = Arc::new(EventLoop::new(listener));
        self.event_loops.insert(event_loop.id, event_loop.clone());
        event_loop
    }

    fn add_keyboard_event(&mut self, id: ID) {
        self.keyboard_event_ids.push(id);
    }

    fn has_keyboard_event(&self, id: &ID) -> bool {
        self.keyboard_event_ids.contains(id)
    }

    fn del_keyboard_event(&mut self, id: ID) {
        self.keyboard_event_ids.retain(|&x| x != id);
    }

    fn add_mouse_event(&mut self, id: ID) {
        self.mouse_event_ids.push(id);
    }

    fn has_mouse_event(&self, id: &ID) -> bool {
        self.mouse_event_ids.contains(id)
    }

    fn del_mouse_event(&mut self, id: ID) {
        self.mouse_event_ids.retain(|&x| x != id);
    }

    fn get_keyboard_event_loop(&self) -> Vec<Arc<EventLoop>> {
        let mut event_loops = Vec::new();
        for id in self.keyboard_event_ids.iter() {
            if let Some(event_loop) = self.event_loops.get(id) {
                event_loops.push(event_loop.clone());
            }
        }
        event_loops
    }

    fn get_mouse_event_loop(&self) -> Vec<Arc<EventLoop>> {
        let mut event_loops = Vec::new();
        for id in self.mouse_event_ids.iter() {
            if let Some(event_loop) = self.event_loops.get(id) {
                event_loops.push(event_loop.clone());
            }
        }
        event_loops
    }

    fn del_event_loop(&mut self, id: ID) {
        self.event_loops.remove(&id);
        self.del_keyboard_event(id);
        self.del_mouse_event(id);
    }
}

lazy_static! {
    pub(crate) static ref EVENT_LOOP_MANAGER: Mutex<EventLoopManager> =
        Mutex::new(EventLoopManager::new());
}
