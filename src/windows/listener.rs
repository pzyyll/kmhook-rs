//! Copyright: 2024 Lizc. All rights reserved.
//! License: MIT License
//! You may obtain a copy of the License at https://opensource.org/licenses/MIT
//!
//! Author: Lizc
//! Created Data: 2024-09-29
//!
//! Description: @todo add msg listener

use std::collections::HashMap;
use std::result::Result;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, Weak};
use std::thread::{self};
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, PostThreadMessageW, SetWindowsHookExW,
    TranslateMessage, HC_ACTION, KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT, WH_KEYBOARD_LL, WH_MOUSE_LL,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEMOVE, WM_QUIT,
    WM_RBUTTONDOWN, WM_RBUTTONUP,
};

use crate::types::{
    EventType, KeyId, KeyboardInfo, MouseButton, MouseEventInfo, MouseStateFlags, Pos, Shortcut, ID,
};
use crate::{types, EventListener, JoinHandleType};
use lazy_static::lazy_static;

fn gen_id() -> ID {
    static mut ID: ID = 0;
    unsafe {
        ID += 1;
        ID
    }
}

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
            // println!("keyboard_hook_proc");
            let kb = &*(lparam.0 as *const usize as *const KBDLLHOOKSTRUCT);
            let vkcode = kb.scanCode;

            println!("keyboard_hook_proc {:?}", kb);

            let keyid = KeyId::from_win(vkcode);

            if keyid.is_err() {
                println!("keyid convert err {:?}", vkcode);
                return CallNextHookEx(None, ncode, wparam, lparam);
            }

            let key = KeyboardInfo {
                key_code: keyid.unwrap(),
            };

            let event_type = EventType::KeyboardEvent(Some(key));

            // get all event loops
            let event_loops = EVENT_LOOP_MANAGER.lock().unwrap().get_reg_msg_event_loop();

            // println!("event_loops: {:?}", event_loops);
            for event_loop in event_loops.iter() {
                // println!("thread_id: {:?}, id:{:?} post_msg {:?}, event_loop {:?}", {
                //     unsafe { GetCurrentThreadId() }
                // },
                // event_loop.id, event_type, event_loop);
                event_loop.post_msg(event_type.clone());
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
                    // println!("mouse_hook_proc ldown {:?}", minfo);
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
                let event_type = EventType::MouseEvent(Some(MouseEventInfo { button, pos }));
                let event_loops = EVENT_LOOP_MANAGER.lock().unwrap().get_reg_msg_event_loop();
                for event_loop in event_loops.iter() {
                    event_loop.post_msg(event_type.clone());
                }
            }
        }
        CallNextHookEx(None, ncode, wparam, lparam)
    }

    fn set_keyboard_hook(&self) {
        unsafe {
            let _ = SetWindowsHookExW(WH_KEYBOARD_LL, Some(Self::keyboard_hook_proc), None, 0);
        }
    }

    fn set_mouse_hook(&self) {
        unsafe {
            let _ = SetWindowsHookExW(WH_MOUSE_LL, Some(Self::mouse_hook_proc), None, 0);
        }
    }

    fn post_msg(&self, event_type: EventType) {
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

    fn run(&self) {
        *self.loop_thread_id.lock().unwrap() = unsafe { GetCurrentThreadId() };
        println!(
            "run loop_thread_id: {:?}",
            *self.loop_thread_id.lock().unwrap()
        );
        let mut msg = MSG::default();
        unsafe {
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
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
            event_loop.set_keyboard_hook();
            event_loop.set_mouse_hook();
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
    event_map: Mutex<HashMap<ID, (EventType, Box<dyn Fn(EventType) + Send + Sync + 'static>)>>,
    shortcut_map: Mutex<HashMap<ID, (Shortcut, Box<dyn Fn() + Send + Sync + 'static>)>>,
    mouse_map: Mutex<
        HashMap<
            ID,
            (
                MouseButton,
                Box<dyn Fn(MouseEventInfo) + Send + Sync + 'static>,
            ),
        >,
    >,
}

impl Listener {
    fn on_event(&self, event_type: EventType) {
        self.event_map
            .lock()
            .unwrap()
            .values()
            .for_each(|(et, cb)| {
                if matches!(et, EventType::All)
                    || std::mem::discriminant(et) == std::mem::discriminant(&event_type)
                {
                    cb(event_type.clone());
                }
            });

        if self.shortcut_map.lock().unwrap().len() > 0 {
            if let EventType::KeyboardEvent(Some(info)) = event_type {
                // todo
                println!("on_event {:?}", info);
            }
        }
    }

    fn gen_id(&self) -> ID {
        gen_id()
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
            mouse_map: Mutex::new(HashMap::new()),
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
        if let Some(event_loop) = self.listener_event_loop.lock().unwrap().as_ref() {
            event_loop.run_with_thread();
        }

        let w = {
            let worker = self.worker.lock().unwrap();
            worker.as_ref().unwrap().clone()
        };
        w.run(work_thread)
    }

    fn shutdown(&self) {
        self.listener_event_loop
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .stop();
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
            .insert(id, (et, Box::new(cb)));
        Ok(id)
    }

    fn add_global_shortcut<F>(
        &self,
        shortcut: types::Shortcut,
        cb: F,
    ) -> std::result::Result<types::ID, String>
    where
        F: Fn() + Send + Sync + 'static,
    {
        let id = self.gen_id();
        self.shortcut_map
            .lock()
            .unwrap()
            .insert(id, (shortcut, Box::new(cb)));
        Ok(id)
    }

    fn del_all_events(&self) {
        self.event_map.lock().unwrap().clear();
        self.shortcut_map.lock().unwrap().clear();
        self.mouse_map.lock().unwrap().clear();
    }

    fn del_event_by_id(&self, id: types::ID) {
        self.event_map.lock().unwrap().remove(&id);
        self.shortcut_map.lock().unwrap().remove(&id);
        self.mouse_map.lock().unwrap().remove(&id);
    }
}
