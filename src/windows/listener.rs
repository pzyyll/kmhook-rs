//! Copyright: 2024 Lizc. All rights reserved.
//! License: MIT License
//! You may obtain a copy of the License at https://opensource.org/licenses/MIT
//!
//! Author: Lizc
//! Created Data: 2024-09-29
//!
//! Description: add msg listener

use super::worker::{KeyboardSysMsg, MouseSysMsg, Worker, WorkerMsg};

use std::cell::RefCell;
use std::collections::HashMap;
use std::result::Result;
use std::sync::{Arc, Mutex, Weak};
use std::thread::{self};
use std::time::Instant;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, PostThreadMessageW, SetWindowsHookExW,
    TranslateMessage, UnhookWindowsHookEx, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT,
    WH_KEYBOARD_LL, WH_MOUSE_LL, WM_QUIT, WM_USER,
};

use crate::types::{EventType, KeyState, KeyboardState, Shortcut, ID};
// use crate::windows::KeyIdFrom;
use crate::consts;
use crate::types::{EventListener, JoinHandleType};
use lazy_static::lazy_static;

type FnEvent = Arc<Box<dyn Fn(EventType) + Send + Sync + 'static>>;
type FnShourtcut = Arc<Box<dyn Fn() + Send + Sync + 'static>>;

thread_local! {
    static LOCAL_KEYBOARD_HHOOK: RefCell<HashMap<ID, HHOOK>> = RefCell::new(HashMap::new());
    static LOCAL_MOUSE_HHOOK: RefCell<HashMap<ID, HHOOK>> = RefCell::new(HashMap::new());
    static LOCAL_KEYBOARD_STATE: RefCell<KeyboardState> = RefCell::new(KeyboardState::new(Some(consts::MAX_KEYS)));
}

fn gen_id() -> ID {
    static mut ID: ID = 0;
    unsafe {
        ID += 1;
        ID
    }
}

const WM_USER_RECHECK_HOOK: u32 = 1;
// const WM_USER_SET_KEYBOARD_HOOK: u32 = WM_USER + 1;
// const WM_USER_SET_MOUSE_HOOK: u32 = WM_USER + 2;
// const WM_USER_UNSET_KEYBOARD_HOOK: u32 = WM_USER + 3;
// const WM_USER_UNSET_MOUSE_HOOK: u32 = WM_USER + 4;

#[derive(Debug)]
struct EventLoop {
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
        // println!("keyboard_hook_proc {:?}", kb);

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

        self.listener
            .upgrade()
            .unwrap()
            .worker
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .post_msg(msg);
    }

    fn post_msg_to_loop(&self, msg_type: u32) {
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

    fn stop(&self) {
        let loop_thread_id = *self.loop_thread_id.lock().unwrap();
        if loop_thread_id == 0 {
            return;
        }
        unsafe {
            let _ = PostThreadMessageW(loop_thread_id, WM_QUIT, None, None);
        }
        *self.loop_thread_id.lock().unwrap() = 0;
    }

    fn run_with_thread(self: &Arc<Self>) {
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
struct EventLoopManager {
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

    fn new_event_loop(&mut self, listener: &Arc<Listener>) -> Arc<EventLoop> {
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
    static ref EVENT_LOOP_MANAGER: Mutex<EventLoopManager> = Mutex::new(EventLoopManager::new());
}

#[derive(Debug)]
struct ShortcutTriggerInfo {
    trigger: u32,
    last_trigger_time: Instant,
}

impl ShortcutTriggerInfo {
    fn new() -> Self {
        Self {
            trigger: 0,
            last_trigger_time: Instant::now(),
        }
    }

    fn reset(&mut self) {
        self.trigger = 0;
        self.last_trigger_time = Instant::now();
    }

    fn increase(&mut self) {
        self.trigger += 1;
        self.last_trigger_time = Instant::now();
    }
}

pub struct Listener {
    listener_event_loop: Mutex<Option<Arc<EventLoop>>>,
    worker: Mutex<Option<Arc<Worker>>>,
    event_map: Mutex<HashMap<ID, (EventType, FnEvent)>>,
    shortcut_map: Mutex<HashMap<ID, (Shortcut, FnShourtcut)>>,
}

impl Listener {
    fn get_worker(&self) -> Option<Arc<Worker>> {
        self.worker.lock().unwrap().clone()
    }

    fn get_event_loop(&self) -> Option<Arc<EventLoop>> {
        self.listener_event_loop.lock().unwrap().clone()
    }

    fn filter_events(&self, event_type: &EventType) -> Vec<(EventType, FnEvent)> {
        let binding = self.event_map.lock().unwrap();
        binding
            .iter()
            .filter_map(|(_, (et, cb))| {
                if matches!(et, EventType::All)
                    || std::mem::discriminant(et) == std::mem::discriminant(event_type)
                {
                    Some((et.clone(), cb.clone()))
                } else {
                    None
                }
            })
            .collect()
    }

    fn filter_shortcut(&self, et: &EventType) -> Option<FnShourtcut> {
        match et {
            EventType::KeyboardEvent(Some(key_info)) => {
                if key_info.state != KeyState::Pressed {
                    return None;
                }
                if let Some(keyboard_state) = &key_info.keyboard_state {
                    let binding = self.shortcut_map.lock().unwrap();
                    let usb_input = keyboard_state.clone().usb_input_report().to_vec();
                    for (_, (shortcut, cb)) in binding.iter() {
                        if shortcut.is_input_match(&usb_input) {
                            // Check if the modifier key is pressed, and when used with other keys,
                            // the last key pressed must not be a modifier key.
                            if shortcut.has_modifier()
                                & shortcut.has_normal_key()
                                & key_info.key_id.is_modifier()
                            {
                                return None;
                            }
                            return Some(cb.clone());
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn on_event(&self, event_type: EventType) {
        #[cfg(feature = "Debug")]
        println!(
            "{:?} on_event {:?}",
            std::thread::current().id(),
            event_type
        );

        let events = self.filter_events(&event_type);
        for (et, cb) in events.iter() {
            if matches!(et, EventType::All)
                || std::mem::discriminant(et) == std::mem::discriminant(&event_type)
            {
                cb(event_type.clone());
            }
        }

        if let Some(cb) = self.filter_shortcut(&event_type) {
            cb();
        }

        #[cfg(feature = "Debug")]
        println!(
            "{:?} event_type: {:?}\n ----------------on_event Finish ",
            std::thread::current().id(),
            event_type
        );
    }

    fn gen_id(&self) -> ID {
        gen_id()
    }

    fn post_recheck_hook(&self) {
        self.listener_event_loop
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .post_msg_to_loop(WM_USER_RECHECK_HOOK);
    }

    fn has_keyboard_event(&self) -> bool {
        {
            if !self.shortcut_map.lock().unwrap().is_empty() {
                return true;
            }
        }

        let binding = self.event_map.lock().unwrap();
        for (_, (et, _)) in binding.iter() {
            if matches!(et, EventType::KeyboardEvent(_) | EventType::All) {
                return true;
            }
        }
        false
    }

    fn has_mouse_event(&self) -> bool {
        let binding = self.event_map.lock().unwrap();
        for (_, (et, _)) in binding.iter() {
            if matches!(et, EventType::MouseEvent(_) | EventType::All) {
                return true;
            }
        }
        false
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        println!("Listener drop");
        self.shutdown();
    }
}

impl EventListener for Listener {
    fn new() -> Arc<Self> {
        let listener = Self {
            listener_event_loop: Mutex::new(None),
            event_map: Mutex::new(HashMap::new()),
            shortcut_map: Mutex::new(HashMap::new()),
            worker: Mutex::new(None),
        };
        let rc = Arc::new(listener);
        rc.listener_event_loop
            .lock()
            .unwrap()
            .replace(EVENT_LOOP_MANAGER.lock().unwrap().new_event_loop(&rc));
        rc.worker.lock().unwrap().replace(Arc::new(Worker::new()));
        rc
    }

    /// `work_thread`:
    /// Handle event callbacks in a separate thread. Default is `true`.
    /// return: `Option<JoinHandleType>` if `work_thread` is `true`, else `None`.
    fn startup(self: &Arc<Self>, work_thread: Option<bool>) -> Option<JoinHandleType> {
        if let Some(event_loop) = self.get_event_loop().as_ref() {
            event_loop.run_with_thread();
        }

        if let Some(w) = self.get_worker() {
            let _self = self.clone();
            w.run(
                move |event_type| {
                    _self.on_event(event_type);
                },
                work_thread,
            )
        } else {
            None
        }
    }

    fn shutdown(&self) {
        self.del_all_events();
        if let Some(worker) = self.get_worker() {
            worker.post_msg(WorkerMsg::Stop);
        }
        if let Some(event_loop) = self.listener_event_loop.lock().unwrap().as_ref() {
            event_loop.stop();
        }
    }

    fn add_event_listener<F>(&self, cb: F, event_type: Option<EventType>) -> Result<ID, String>
    where
        F: Fn(EventType) + Send + Sync + 'static,
    {
        let id = self.gen_id();
        let et = event_type.unwrap_or(EventType::All);
        self.event_map
            .lock()
            .unwrap()
            .insert(id, (et, Arc::new(Box::new(cb))));
        self.post_recheck_hook();
        Ok(id)
    }

    fn add_global_shortcut<F>(&self, shortcut: Shortcut, cb: F) -> std::result::Result<ID, String>
    where
        F: Fn() + Send + Sync + 'static,
    {
        let id = self.gen_id();
        {
            let mut binding = self.shortcut_map.lock().unwrap();
            for (_, (sc, _)) in binding.iter() {
                // println!("sc usb_input: {:?}", sc.usb_input());
                // println!("shortcut usb_input: {:?}", shortcut.usb_input());
                if sc.is_input_match(shortcut.usb_input()) {
                    return Err("Shortcut already exists".to_string());
                }
            }
            binding.insert(id, (shortcut, Arc::new(Box::new(cb))));
        }

        self.post_recheck_hook();
        Ok(id)
    }

    fn add_global_shortcut_trigger<F>(
        &self,
        shortcut: Shortcut,
        cb: F,
        trigger: u32,
        internal: Option<u32>,
    ) -> std::result::Result<ID, String>
    where
        F: Fn() + Send + Sync + 'static,
    {
        let trigger_info = Arc::new(Mutex::new(ShortcutTriggerInfo::new()));
        let next_internal = internal.unwrap_or(consts::DEFAULT_SHORTCUT_TRIGGER_INTERVAL) as u128;

        self.add_global_shortcut(shortcut, move || {
            #[cfg(feature = "Debug")]
            println!("global_shortcut trigger: {:?}", Instant::now());

            let need_trigger = {
                let mut mtrigger_info = trigger_info.lock().unwrap();

                let elapsed = mtrigger_info.last_trigger_time.elapsed().as_millis();
                #[cfg(feature = "Debug")]
                println!(
                    "trigger times: {:?}, elapsed: {:?}",
                    mtrigger_info.trigger, elapsed
                );

                if mtrigger_info.trigger == 0 || elapsed < next_internal {
                    mtrigger_info.increase();
                } else {
                    mtrigger_info.reset();
                    mtrigger_info.increase();
                }
                if mtrigger_info.trigger >= trigger {
                    mtrigger_info.reset();
                    true
                } else {
                    false
                }
            };
            if need_trigger {
                cb();
                #[cfg(feature = "Debug")]
                println!(
                    "------------------------Trigger------------------------{:?}",
                    Instant::now()
                );
            }
        })
    }

    fn del_all_events(&self) {
        self.event_map.lock().unwrap().clear();
        self.shortcut_map.lock().unwrap().clear();
        self.post_recheck_hook();
    }

    fn del_event_by_id(&self, id: ID) {
        self.event_map.lock().unwrap().remove(&id);
        self.shortcut_map.lock().unwrap().remove(&id);
        self.post_recheck_hook();
        println!("del_event_by_id finish {:?}", id);
    }
}
