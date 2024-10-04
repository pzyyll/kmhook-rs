use crate::types::ID;
use crate::utils::gen_id;
use crate::windows::worker::{KeyboardSysMsg, MouseSysMsg, WorkerMsg};
use crate::windows::WM_USER_RECHECK_HOOK;
use crate::Listener;

use lazy_static::lazy_static;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};
use std::thread;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::{
    GetCurrentThread, GetCurrentThreadId, SetThreadPriority, THREAD_PRIORITY_HIGHEST, THREAD_PRIORITY_TIME_CRITICAL,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, PostThreadMessageW, SetWindowsHookExW,
    TranslateMessage, UnhookWindowsHookEx, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT,
    WH_KEYBOARD_LL, WH_MOUSE_LL, WM_QUIT, WM_USER,
};

thread_local! {
    static LOCAL_KEYBOARD_HHOOK: RefCell<HashMap<ID, HHOOK>> = RefCell::new(HashMap::new());
    static LOCAL_MOUSE_HHOOK: RefCell<HashMap<ID, HHOOK>> = RefCell::new(HashMap::new());
    static LOCAL_KEY_LAST_TIME: RefCell<u32> = RefCell::new(0);
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

    unsafe extern "system" fn keyboard_hook_proc(
        ncode: i32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if ncode != HC_ACTION.try_into().unwrap() {
            return CallNextHookEx(None, ncode, wparam, lparam);
        }

        let kb = &*(lparam.0 as *const usize as *const KBDLLHOOKSTRUCT);
        let mut is_repeat = false;
        LOCAL_KEY_LAST_TIME.with(|last_time| {
            let mut last_time = last_time.borrow_mut();
            let current_time = kb.time;
            if *last_time == current_time {
                is_repeat = true;
            }
            *last_time = current_time;
        });
        if is_repeat {
            println!(
                "{:?} keyboard_hook_proc is repeat {:?}",
                std::thread::current().id(),
                kb
            );
            return CallNextHookEx(None, ncode, wparam, lparam);
        }

        #[cfg(feature = "Debug")]
        println!(
            "{:?} keyboard_hook_proc trigger {:?}",
            std::thread::current().id(),
            kb
        );

        let msg = WorkerMsg::KeyboardEvent(KeyboardSysMsg::new(wparam.0 as u32, *kb));

        let event_loops = { EVENT_LOOP_MANAGER.lock().unwrap().get_keyboard_event_loop() };
        for event_loop in event_loops.iter() {
            event_loop.post_msg_to_worker(msg.clone());
        }

        #[cfg(feature = "Debug")]
        println!(
            "{:?} keyboard_hook_proc trigger end call next",
            std::thread::current().id()
        );

        CallNextHookEx(None, ncode, wparam, lparam)
    }

    unsafe extern "system" fn mouse_hook_proc(
        ncode: i32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if ncode == HC_ACTION.try_into().unwrap() {
            let mtype = wparam.0 as u32;
            let minfo = &*(lparam.0 as *const usize as *const MSLLHOOKSTRUCT);

            #[cfg(feature = "Debug")]
            println!(
                "{:?} mouse_hook_proc trigger {:?}",
                std::thread::current().id(),
                minfo
            );

            let msg = WorkerMsg::MouseEvent(MouseSysMsg::new(mtype, *minfo));

            let event_loops = { EVENT_LOOP_MANAGER.lock().unwrap().get_mouse_event_loop() };
            for event_loop in event_loops.iter() {
                event_loop.post_msg_to_worker(msg.clone());
            }

            #[cfg(feature = "Debug")]
            println!(
                "{:?} mouse_hook_proc trigger end call next",
                std::thread::current().id()
            );
        }
        CallNextHookEx(None, ncode, wparam, lparam)
    }

    fn set_keyboard_hook(&self) {
        if LOCAL_KEYBOARD_HHOOK.with_borrow(|ids| ids.contains_key(&self.id)) {
            return;
        }
        if let Ok(hhook) =
            unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(Self::keyboard_hook_proc), None, 0) }
        {
            #[cfg(feature = "Debug")]
            println!(
                "{:?} set_keyboard_hook {:?}",
                std::thread::current().id(),
                hhook
            );

            LOCAL_KEYBOARD_HHOOK.with_borrow_mut(|ids| {
                ids.insert(self.id, hhook);
            });
            EVENT_LOOP_MANAGER
                .lock()
                .unwrap()
                .add_keyboard_event(self.id);
        }
    }

    fn set_mouse_hook(&self) {
        if LOCAL_MOUSE_HHOOK.with_borrow(|ids| ids.contains_key(&self.id)) {
            return;
        }
        if let Ok(hhook) =
            unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(Self::mouse_hook_proc), None, 0) }
        {
            #[cfg(feature = "Debug")]
            println!(
                "{:?} set_mouse_hook {:?}",
                std::thread::current().id(),
                hhook
            );

            LOCAL_MOUSE_HHOOK.with_borrow_mut(|ids| {
                ids.insert(self.id, hhook);
            });
            EVENT_LOOP_MANAGER.lock().unwrap().add_mouse_event(self.id);
        }
    }

    fn unhook_keyboard(&self) {
        LOCAL_KEYBOARD_HHOOK.with_borrow_mut(|ids| {
            if let Some(hhook) = ids.remove(&self.id) {
                unsafe {
                    println!("unhook_keyboard {:?}", hhook);
                    let _ = UnhookWindowsHookEx(hhook);
                    EVENT_LOOP_MANAGER
                        .lock()
                        .unwrap()
                        .del_keyboard_event(self.id);
                }
            }
        });
    }

    fn unhook_mouse(&self) {
        LOCAL_MOUSE_HHOOK.with_borrow_mut(|ids| {
            if let Some(hhook) = ids.remove(&self.id) {
                unsafe {
                    println!("unhook_mouse {:?}", hhook);
                    let _ = UnhookWindowsHookEx(hhook);
                    EVENT_LOOP_MANAGER.lock().unwrap().del_mouse_event(self.id);
                }
            }
        });
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
            let _ = PostThreadMessageW(thread_id, WM_USER, WPARAM(msg_type as usize), None);
        }
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
            let _ = PostThreadMessageW(loop_thread_id, WM_QUIT, None, None);
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

    fn del_keyboard_event(&mut self, id: ID) {
        self.keyboard_event_ids.retain(|&x| x != id);
    }

    fn add_mouse_event(&mut self, id: ID) {
        self.mouse_event_ids.push(id);
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
