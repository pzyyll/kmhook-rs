#![allow(unused)]
use std::{sync::Arc, thread::JoinHandle};

use bitflags::bitflags;

pub use keycode::{KeyMap, KeyMappingId, KeyState, KeyboardState};

pub type ID = usize;

bitflags! {
    #[derive(Debug, Hash, Eq, PartialEq, Clone)]
    pub struct MouseStateFlags: u32 {
        const PRESSED = 0b0001;
        const RELEASED = 0b0010;
        const MOVEING = 0b0100;
        // const DRAG_START = 0b0100;
        // const DRAG_END = 0b1000;
        // const DRAGGING = 0b10000;
    }
}

#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy)]
pub struct KeyId(pub KeyMappingId);

impl From<KeyMappingId> for KeyId {
    fn from(id: KeyMappingId) -> Self {
        Self(id)
    }
}

impl Into<KeyMappingId> for KeyId {
    fn into(self) -> KeyMappingId {
        self.0
    }
}

impl From<KeyMap> for KeyId {
    fn from(key_map: KeyMap) -> Self {
        Self(key_map.id)
    }
}

impl Into<KeyMap> for KeyId {
    fn into(self) -> KeyMap {
        KeyMap::from(self.0)
    }
}

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub enum MouseButton {
    Left(MouseStateFlags),
    Right(MouseStateFlags),
    Middle(MouseStateFlags),
    Move(MouseStateFlags),
}

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub struct KeyInfo {
    pub key_code: KeyId,
    pub state: KeyState,

    /// All keys state
    pub keyboard_state: Option<KeyboardState>,
}

impl KeyInfo {
    pub fn new(key_code: KeyId, state: KeyState) -> Self {
        Self {
            key_code,
            state,
            keyboard_state: None,
        }
    }
}

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub struct Pos {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub struct MouseInfo {
    pub button: MouseButton,
    pub pos: Pos,
}

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub enum EventType {
    KeyboardEvent(Option<KeyInfo>),
    MouseEvent(Option<MouseInfo>),
    All,
}

#[derive(Debug)]
pub struct Shortcut {
    pub keys: Vec<KeyId>,
}

pub type JoinHandleType = JoinHandle<()>;

pub trait EventListener {
    fn new() -> Arc<Self>;
    fn add_global_shortcut<F>(&self, shortcut: Shortcut, cb: F) -> std::result::Result<ID, String>
    where
        F: Fn() + Send + Sync + 'static;

    fn add_event_listener<F>(
        &self,
        cb: F,
        event_type: Option<EventType>,
    ) -> std::result::Result<ID, String>
    where
        F: Fn(EventType) + Send + Sync + 'static;

    fn del_event_by_id(&self, id: ID);
    fn del_all_events(&self);

    fn startup(self: &Arc<Self>, work_thread: Option<bool>) -> Option<JoinHandleType>;
    fn shutdown(&self);
}

#[cfg(test)]
mod tests {
    use std::{
        fmt::{Debug, Display},
        iter::Zip,
    };

    use keycode::KeyMapping;
    use rdev::{Event, Key};

    use super::*;

    #[test]
    fn test_event_info() {
        let event_type = EventType::KeyboardEvent(Some(KeyInfo::new(
            KeyId::from(KeyMappingId::UsA),
            KeyState::Pressed,
        )));

        match event_type {
            EventType::KeyboardEvent(info) => {
                if let Some(k) = info {
                    println!("KeyboardEvent {:?}", k)
                }
            }
            _ => {}
        }
    }

    #[test]
    fn enumhashable() {
        use std::collections::HashMap;
        let mut map: HashMap<Option<EventType>, (ID, ID)> = HashMap::new();

        map.insert(Some(EventType::KeyboardEvent(None)), (2, 2));
        map.insert(Some(EventType::MouseEvent(None)), (1, 2));
        map.insert(None, (3, 3));

        // println!("{:?}", map);

        let et = EventType::KeyboardEvent(Some(KeyInfo::new(
            KeyId::from(KeyMappingId::UsA),
            KeyState::Pressed,
        )));
        for (k, v) in map.iter() {
            if let Some(k) = k {
                if std::mem::discriminant(k) == std::mem::discriminant(&et) {
                    println!("{:?}", v);
                }
            }
        }

        let win_vkcode = KeyMappingId::UsA;
        let win_vkcode2 = KeyMappingId::AltLeft;

        if let Some(modifier) = KeyMap::from(KeyMappingId::ShiftLeft).modifier {
            println!("modifier {:?}", modifier);
        }

        if let Some(modifier) = KeyMap::from(KeyMappingId::ShiftLeft).modifier {
            println!("modifier {:?}", modifier);
        }

        println!("{:?}", KeyMap::from(win_vkcode));
        let win_vk = KeyMap::from(win_vkcode).win;
        println!("{:?} {:?} {:?}", win_vk, 'a' as u16, 0x41 as u16);

        // let key_id = KeyMap::from_key_mapping(keycode::KeyMapping::Win(162))
        //     .unwrap()
        //     .id;

        // println!("{:?}", key_id);

        // let key_id = KeyMap::from_key_mapping(keycode::KeyMapping::Win(win_vkcode)).unwrap().id;
        // // KeyId from Win(0x12) is 0x12
        // let usa = KeyId::UsA;
        // println!("{:?}", usa);

        println!("MetaLeft {:?}", KeyMap::from(KeyMappingId::MetaLeft));
        println!("MetaRight {:?}", KeyMap::from(KeyMappingId::MetaRight));
        println!("ControlLeft {:?}", KeyMap::from(KeyMappingId::ControlLeft));
        println!(
            "ControlRight {:?}",
            KeyMap::from(KeyMappingId::ControlRight)
        );
        println!("AltLeft {:?}", KeyMap::from(KeyMappingId::AltLeft));
        println!("AltRight {:?}", KeyMap::from(KeyMappingId::AltRight));
    }
}
