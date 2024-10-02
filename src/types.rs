#![allow(unused)]
use std::{sync::Arc, thread::JoinHandle};

use bitflags::bitflags;

pub use keycode::{KeyMap, KeyMappingId as KeyCode, KeyState, KeyboardState};

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
pub struct KeyId(pub KeyCode);

impl KeyId {
    pub fn is_modifier(&self) -> bool {
        KeyMap::from(self.0).modifier.is_some()
    }
}

impl From<KeyCode> for KeyId {
    fn from(id: KeyCode) -> Self {
        Self(id)
    }
}

impl Into<KeyCode> for KeyId {
    fn into(self) -> KeyCode {
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
    pub key_id: KeyId,
    pub state: KeyState,

    /// All keys state
    pub keyboard_state: Option<KeyboardState>,
}

impl KeyInfo {
    pub fn new(key_id: KeyId, state: KeyState) -> Self {
        Self {
            key_id,
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
    pub keys: Vec<KeyCode>,
    _keyboard_state_usb_input: Vec<u8>,
}

impl Shortcut {
    pub fn new(keys: Vec<KeyCode>) -> std::result::Result<Self, String> {
        // Check if keys have duplicates
        let mut unique_keys = std::collections::HashSet::new();
        for key in &keys {
            if !unique_keys.insert(key) {
                return Err("Duplicate keys found".to_string());
            }
        }

        let mut _keyboard_state = KeyboardState::new(Some(crate::consts::MAX_KEYS));
        for key in &keys {
            _keyboard_state.update_key(KeyMap::from(*key), KeyState::Pressed);
        }
        Ok(Self {
            keys,
            _keyboard_state_usb_input: _keyboard_state.usb_input_report().to_vec(),
        })
    }

    pub fn is_input_match(&self, usb_input: &Vec<u8>) -> bool {
        self._keyboard_state_usb_input == *usb_input
    }

    pub fn has_modifier(&self) -> bool {
        return self._keyboard_state_usb_input.len() > 0 && self._keyboard_state_usb_input[0] != 0;
    }

    pub fn has_normal_key(&self) -> bool {
        return self._keyboard_state_usb_input.len() > 2 && self._keyboard_state_usb_input[2] != 0;
    }
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
            KeyId::from(KeyCode::UsA),
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
            KeyId::from(KeyCode::UsA),
            KeyState::Pressed,
        )));
        for (k, v) in map.iter() {
            if let Some(k) = k {
                if std::mem::discriminant(k) == std::mem::discriminant(&et) {
                    println!("{:?}", v);
                }
            }
        }

        let win_vkcode = KeyCode::UsA;
        let win_vkcode2 = KeyCode::AltLeft;

        if let Some(modifier) = KeyMap::from(KeyCode::ShiftLeft).modifier {
            println!("modifier {:?}", modifier);
        }

        if let Some(modifier) = KeyMap::from(KeyCode::ShiftLeft).modifier {
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

        println!("MetaLeft {:?}", KeyMap::from(KeyCode::MetaLeft));
        println!("MetaRight {:?}", KeyMap::from(KeyCode::MetaRight));
        println!("ControlLeft {:?}", KeyMap::from(KeyCode::ControlLeft));
        println!("ControlRight {:?}", KeyMap::from(KeyCode::ControlRight));
        println!("AltLeft {:?}", KeyMap::from(KeyCode::AltLeft));
        println!("AltRight {:?}", KeyMap::from(KeyCode::AltRight));
    }
}
