//! Copyright: 2024 Lizc. All rights reserved.
//! License: MIT License
//! You may obtain a copy of the License at https://opensource.org/licenses/MIT
//!
//! Author: Lizc
//! Created Data: 2024-09-29
//!
//! Description: @todo add msg listener

use std::cell::RefCell;
use std::collections::HashMap;
use std::result::Result;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, Weak};
use std::thread::{self};
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, PostThreadMessageW, SetWindowsHookExW,
    TranslateMessage, UnhookWindowsHookEx, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT,
    WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN,
    WM_MBUTTONUP, WM_MOUSEMOVE, WM_QUIT, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYDOWN, WM_USER,
};

use crate::types::{
    EventType, KeyId, KeyInfo, KeyState, KeyboardState, MouseButton, MouseInfo, MouseStateFlags,
    Pos, Shortcut, ID,
};
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
        if ncode == HC_ACTION.try_into().unwrap() {
            let kb = &*(lparam.0 as *const usize as *const KBDLLHOOKSTRUCT);
            // println!("keyboard_hook_proc {:?}", kb);

            let keyid = match KeyId::try_from(*kb) {
                Ok(keyid) => keyid,
                Err(_) => {
                    println!("keyid convert err {:?}", kb);
                    return CallNextHookEx(None, ncode, wparam, lparam);
                }
            };

            let key_state = match wparam.0 as u32 {
                WM_KEYDOWN | WM_SYSKEYDOWN => KeyState::Pressed,
                _ => KeyState::Released,
            };

            let mut key = KeyInfo::new(keyid, key_state);
            let mut old_state: Option<KeyboardState> = None;
            LOCAL_KEYBOARD_STATE.with_borrow_mut(|state| {
                old_state.replace(state.clone());
                state.update_key(keyid.into(), key_state);
                key.keyboard_state = Some(state.clone());
                // println!("keyboard_state: {:?}", state);
            });

            if old_state == key.keyboard_state {
                // println!("keyboard_hook_proc same state {:?}", key);
                return CallNextHookEx(None, ncode, wparam, lparam);
            }

            let event_type = EventType::KeyboardEvent(Some(key));

            let event_loops = EVENT_LOOP_MANAGER.lock().unwrap().get_reg_msg_event_loop();

            for event_loop in event_loops.iter() {
                event_loop.post_msg_to_worker(event_type.clone());
            }
        }
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

            let pos = Pos {
                x: minfo.pt.x,
                y: minfo.pt.y,
            };

            let button = match mtype {
                WM_LBUTTONDOWN => {
                    println!("mouse_hook_proc ldown {:?}", minfo);
                    Some((MouseButton::Left(MouseStateFlags::PRESSED),))
                }
                WM_LBUTTONUP => {
                    // println!("mouse_hook_proc lup {:?}", minfo);
                    Some((MouseButton::Left(MouseStateFlags::RELEASED),))
                }
                WM_RBUTTONDOWN => {
                    // println!("mouse_hook_proc rdown {:?}", minfo);
                    Some((MouseButton::Right(MouseStateFlags::PRESSED),))
                }
                WM_RBUTTONUP => {
                    // println!("mouse_hook_proc rup {:?}", minfo);
                    Some((MouseButton::Right(MouseStateFlags::RELEASED),))
                }
                WM_MBUTTONDOWN => {
                    // println!("mouse_hook_proc mdown {:?}", minfo);
                    Some((MouseButton::Middle(MouseStateFlags::PRESSED),))
                }
                WM_MBUTTONUP => {
                    // println!("mouse_hook_proc mup {:?}", minfo);
                    Some((MouseButton::Middle(MouseStateFlags::RELEASED),))
                }
                WM_MOUSEMOVE => {
                    // println!("mouse_hook_proc move {:?}", minfo);
                    Some((MouseButton::Move(MouseStateFlags::MOVEING),))
                }
                _ => None,
            };

            if let Some((button,)) = button {
                // Handle the button event here
                // println!("mouse_hook_proc {:?}", button);
                let event_type = EventType::MouseEvent(Some(MouseInfo { button, pos }));
                let event_loops = EVENT_LOOP_MANAGER.lock().unwrap().get_reg_msg_event_loop();
                for event_loop in event_loops.iter() {
                    event_loop.post_msg_to_worker(event_type.clone());
                }
            }
        }
        CallNextHookEx(None, ncode, wparam, lparam)
    }

    fn set_keyboard_hook(&self) {
        unsafe {
            if let Ok(hhook) =
                SetWindowsHookExW(WH_KEYBOARD_LL, Some(Self::keyboard_hook_proc), None, 0)
            {
                LOCAL_KEYBOARD_HHOOK.with_borrow_mut(|ids| {
                    ids.insert(self.id, hhook);
                });
            }
        }
    }

    fn set_mouse_hook(&self) {
        unsafe {
            if let Ok(hhook) = SetWindowsHookExW(WH_MOUSE_LL, Some(Self::mouse_hook_proc), None, 0)
            {
                LOCAL_MOUSE_HHOOK.with_borrow_mut(|ids| {
                    ids.insert(self.id, hhook);
                });
            }
        }
    }

    fn unhook_keyboard(&self) {
        LOCAL_KEYBOARD_HHOOK.with_borrow_mut(|ids| {
            if let Some(hhook) = ids.remove(&self.id) {
                unsafe {
                    println!("unhook_keyboard {:?}", hhook);
                    let _ = UnhookWindowsHookEx(hhook);
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
                }
            }
        });
    }

    fn recheck_hook(&self) {
        if let Some(listener) = self.listener.upgrade().as_ref() {
            let (mut set_keyboard_flag, mut set_mouse_flag) = (false, false);

            if let Ok(event_map) = listener.event_map.lock().as_ref() {
                for (_, (etype, _)) in event_map.iter() {
                    match etype {
                        EventType::All => {
                            set_keyboard_flag = true;
                            set_mouse_flag = true;
                            break;
                        }
                        EventType::KeyboardEvent(_) => set_keyboard_flag = true,
                        EventType::MouseEvent(_) => set_mouse_flag = true,
                    }
                    if set_keyboard_flag & set_mouse_flag {
                        break;
                    }
                }
            }

            if set_keyboard_flag {
                self.set_keyboard_hook();
            } else {
                self.unhook_keyboard();
            }

            if set_mouse_flag {
                self.set_mouse_hook();
            } else {
                self.unhook_mouse();
            }
        }
    }

    fn post_msg_to_worker(&self, event_type: EventType) {
        // println!("{:?} post_msg {:?}", unsafe{GetCurrentThreadId()}, event_type);
        self.listener
            .upgrade()
            .unwrap()
            .worker
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .post_msg(Some(event_type));
    }

    fn post_msg_to_loop(&self, msg_type: u32) {
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
        EVENT_LOOP_MANAGER
            .lock()
            .unwrap()
            .reg_msg_event(event_loop.id);
        let handle = thread::spawn(move || {
            event_loop.recheck_hook();
            event_loop.run();
        });
        self.thread_handle.lock().unwrap().replace(Arc::new(handle));
    }
}

#[derive(Debug)]
struct EventLoopManager {
    event_loops: HashMap<ID, Arc<EventLoop>>,
    reg_msg_event_ids: Vec<ID>,
}

impl EventLoopManager {
    fn new() -> Self {
        Self {
            event_loops: HashMap::new(),
            reg_msg_event_ids: Vec::new(),
        }
    }

    fn new_event_loop(&mut self, listener: &Arc<Listener>) -> Arc<EventLoop> {
        let event_loop = Arc::new(EventLoop::new(listener));
        self.event_loops.insert(event_loop.id, event_loop.clone());
        event_loop
    }

    fn reg_msg_event(&mut self, id: ID) {
        self.reg_msg_event_ids.push(id);
    }

    fn del_msg_event(&mut self, id: ID) {
        self.reg_msg_event_ids.retain(|&x| x != id);
    }

    fn get_reg_msg_event_loop(&self) -> Vec<Arc<EventLoop>> {
        let mut event_loops = Vec::new();
        for id in self.reg_msg_event_ids.iter() {
            if let Some(event_loop) = self.event_loops.get(id) {
                event_loops.push(event_loop.clone());
            }
        }
        event_loops
    }

    fn del_event_loop(&mut self, id: ID) {
        self.event_loops.remove(&id);
        self.del_msg_event(id);
    }
}

lazy_static! {
    static ref EVENT_LOOP_MANAGER: Mutex<EventLoopManager> = Mutex::new(EventLoopManager::new());
}

struct Worker {
    msg_sender: Mutex<Option<Sender<Option<EventType>>>>,
    listener: Weak<Listener>,
}

impl Drop for Worker {
    fn drop(&mut self) {
        println!("Worker drop");
    }
}

impl Worker {
    fn new(listener: &Arc<Listener>) -> Self {
        Self {
            msg_sender: Mutex::new(None),
            listener: Arc::downgrade(listener),
        }
    }

    fn run(self: &Arc<Self>, with_thread: Option<bool>) -> Option<JoinHandleType> {
        let (tx, rx) = std::sync::mpsc::channel();
        *self.msg_sender.lock().unwrap() = Some(tx);
        let threading = with_thread.unwrap_or(false);

        let listener = self.listener.clone();
        let worker_loop = move || {
            while let Ok(Some(event_type)) = rx.recv() {
                if let Some(listener) = listener.upgrade() {
                    listener.on_event(event_type);
                }
            }
            println!("Worker exit");
        };

        if threading {
            Some(thread::spawn(worker_loop))
        } else {
            worker_loop();
            None
        }
    }

    fn post_msg(&self, event_type: Option<EventType>) {
        if let Some(tx) = self.msg_sender.lock().unwrap().as_ref() {
            let _ = tx.send(event_type);
        }
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

    fn filter_shortcuts(&self, et: &EventType) -> Vec<(Shortcut, FnShourtcut)> {
        let mut resutl = Vec::new();
        resutl
        // todo
    }

    fn on_event(&self, event_type: EventType) {
        for (et, cb) in self.filter_events(&event_type).iter() {
            if matches!(et, EventType::All)
                || std::mem::discriminant(et) == std::mem::discriminant(&event_type)
            {
                cb(event_type.clone());
            }
        }

        for (shortcut, cb) in self.filter_shortcuts(&event_type).iter() {
            cb();
        }
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
        rc.worker
            .lock()
            .unwrap()
            .replace(Arc::new(Worker::new(&rc)));
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
            w.run(work_thread)
        } else {
            None
        }
    }

    fn shutdown(&self) {
        self.del_all_events();
        if let Some(worker) = self.get_worker() {
            worker.post_msg(None);
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
        self.shortcut_map
            .lock()
            .unwrap()
            .insert(id, (shortcut, Arc::new(Box::new(cb))));
        self.post_recheck_hook();
        Ok(id)
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
