#![allow(unused)]

use std::cell::RefCell;
use std::{
    sync::{mpsc::Sender, Arc, Mutex},
    thread,
};
use windows::Win32::UI::WindowsAndMessaging::{
    KBDLLHOOKSTRUCT, MSLLHOOKSTRUCT, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN,
    WM_MBUTTONUP, WM_MOUSEMOVE, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYDOWN,
};

use crate::consts;
use crate::types::{
    EventType, JoinHandleType, KeyId, KeyInfo, KeyState, KeyboardState, MouseButton, MouseInfo,
    MouseStateFlags, Pos,
};

thread_local! {
    static LOCAL_KEYBOARD_STATE: RefCell<KeyboardState> = RefCell::new(KeyboardState::new(Some(consts::MAX_KEYS)));
}

#[derive(Debug, Clone)]
pub(crate) struct KeyboardSysMsg {
    state: u32,
    hook_data: KBDLLHOOKSTRUCT,
}

impl KeyboardSysMsg {
    pub fn new(state: u32, hook_data: KBDLLHOOKSTRUCT) -> Self {
        Self { state, hook_data }
    }

    fn translate_msg(&self) -> Option<EventType> {
        let keyid = KeyId::try_from(self.hook_data).ok()?;
        let key_state = match self.state {
            WM_KEYDOWN | WM_SYSKEYDOWN => KeyState::Pressed,
            _ => KeyState::Released,
        };
        let mut key = KeyInfo::new(keyid, key_state);
        let mut old_state: Option<KeyboardState> = None;
        LOCAL_KEYBOARD_STATE.with(|state| {
            old_state.replace(state.borrow().clone());
            state.borrow_mut().update_key(keyid.into(), key_state);
            key.keyboard_state = Some(state.borrow().clone());
        });

        if old_state == key.keyboard_state {
            return None;
        }

        Some(EventType::KeyboardEvent(Some(key)))
    }
}

#[derive(Debug, Clone)]
pub(crate) struct MouseSysMsg {
    state: u32,
    hook_data: MSLLHOOKSTRUCT,
}

impl MouseSysMsg {
    pub fn new(state: u32, hook_data: MSLLHOOKSTRUCT) -> Self {
        Self { state, hook_data }
    }

    fn translate_msg(&self) -> Option<EventType> {
        let minfo = &self.hook_data;
        let pos = Pos {
            x: minfo.pt.x,
            y: minfo.pt.y,
        };

        let button = match self.state {
            WM_LBUTTONDOWN => Some(MouseButton::Left(MouseStateFlags::PRESSED)),
            WM_LBUTTONUP => Some(MouseButton::Left(MouseStateFlags::RELEASED)),
            WM_RBUTTONDOWN => Some(MouseButton::Right(MouseStateFlags::PRESSED)),
            WM_RBUTTONUP => Some(MouseButton::Right(MouseStateFlags::RELEASED)),
            WM_MBUTTONDOWN => Some(MouseButton::Middle(MouseStateFlags::PRESSED)),
            WM_MBUTTONUP => Some(MouseButton::Middle(MouseStateFlags::RELEASED)),
            WM_MOUSEMOVE => Some(MouseButton::Move(MouseStateFlags::MOVEING)),
            _ => None,
        };

        if let Some(button) = button {
            return Some(EventType::MouseEvent(Some(MouseInfo { button, pos })));
        }
        None
    }
}

#[derive(Debug, Clone)]
pub(crate) enum WorkerMsg {
    KeyboardEvent(KeyboardSysMsg),
    MouseEvent(MouseSysMsg),
    Stop,
}

impl WorkerMsg {
    fn translate_msg(&self) -> Option<EventType> {
        match self {
            WorkerMsg::KeyboardEvent(msg) => msg.translate_msg(),
            WorkerMsg::MouseEvent(msg) => msg.translate_msg(),
            WorkerMsg::Stop => None,
        }
    }
}

pub(crate) struct Worker {
    msg_sender: Mutex<Option<Sender<WorkerMsg>>>,
}

impl Drop for Worker {
    fn drop(&mut self) {
        println!("Worker drop");
    }
}

impl Worker {
    pub fn new() -> Self {
        Self {
            msg_sender: Mutex::new(None),
        }
    }

    pub fn run<F>(self: &Arc<Self>, handle: F, with_thread: Option<bool>) -> Option<JoinHandleType>
    where
        F: Fn(EventType) + Send + Sync + 'static,
    {
        let (tx, rx) = std::sync::mpsc::channel();
        {
            let mut msg_sender = self.msg_sender.lock().unwrap();
            *msg_sender = Some(tx);
        }
        let threading = with_thread.unwrap_or(true);

        let handle = Arc::new(handle);
        let worker_loop = move || {
            #[cfg(feature = "Debug")]
            println!(
                "Worker loop thread started with ID: {:?}",
                std::thread::current().id()
            );
            while let Ok(msg) = rx.recv() {
                if let WorkerMsg::Stop = msg {
                    break;
                }
                if let Some(event) = msg.translate_msg() {
                    let handle = Arc::clone(&handle);
                    thread::spawn(move || handle(event));
                } else {
                    #[cfg(feature = "Debug")]
                    println!(
                        "Worker loop thread({:?}) translate_msg failed. {:?}",
                        std::thread::current().id(),
                        msg
                    );
                }
            }
            #[cfg(feature = "Debug")]
            println!(
                "Worker loop thread({:?}) break.",
                std::thread::current().id()
            );
        };

        if threading {
            Some(thread::spawn(worker_loop))
        } else {
            worker_loop();
            None
        }
    }

    pub fn post_msg(&self, msg: WorkerMsg) {
        if let Some(tx) = self.msg_sender.lock().unwrap().as_ref() {
            let _ = tx.send(msg);
        }
    }
}
