#![allow(unused)]
use bitflags::bitflags;
use std::str::FromStr;
use std::{sync::Arc, thread::JoinHandle};

pub use keycode::VirtualKeyId;
pub use keycode::{KeyMap, KeyMappingId, KeyState, KeyboardState};

pub type ID = usize;

pub type ClickState = KeyState;

#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy)]
pub struct KeyId(pub VirtualKeyId);

impl KeyId {
    pub fn is_modifier(&self) -> bool {
        self.0.modifier().is_some()
    }
}

impl From<VirtualKeyId> for KeyId {
    fn from(id: VirtualKeyId) -> Self {
        Self(id)
    }
}

impl Into<VirtualKeyId> for KeyId {
    fn into(self) -> VirtualKeyId {
        self.0
    }
}

// impl From<KeyMap> for KeyId {
//     fn from(key_map: KeyMap) -> Self {
//         Self(key_map.id)
//     }
// }

// impl Into<KeyMap> for KeyId {
//     fn into(self) -> KeyMap {
//         KeyMap::from(self.0)
//     }
// }

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub enum MouseButton {
    Left(ClickState),
    Right(ClickState),
    Middle(ClickState),
    X1(ClickState),
    X2(ClickState),
}

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub struct KeyInfo {
    pub key_id: KeyId,
    pub state: KeyState,

    /// All keys state
    pub keyboard_state: Option<Shortcut>,
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

#[derive(Debug, Hash, Eq, PartialEq, Clone, Default)]
pub struct Pos {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub struct MouseInfo {
    pub button: Option<MouseButton>,
    pub pos: Pos,
    pub relative_pos: Pos,
}

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub enum EventType {
    KeyboardEvent(Option<KeyInfo>),
    MouseEvent(Option<MouseInfo>),
    All,
}

// #[derive(Debug)]
// pub struct Shortcut {
//     pub keys: Vec<KeyMappingId>,
//     _keyboard_state_usb_input: Vec<u8>,
// }

// impl Shortcut {
//     pub fn new(keys: Vec<KeyMappingId>) -> std::result::Result<Self, String> {
//         // Check if keys have duplicates
//         let mut unique_keys = std::collections::HashSet::new();
//         for key in &keys {
//             if !unique_keys.insert(key) {
//                 return Err("Duplicate keys found".to_string());
//             }
//         }

//         let mut _keyboard_state = KeyboardState::new(Some(crate::consts::MAX_KEYS));
//         for key in &keys {
//             _keyboard_state.update_key(KeyMap::from(*key), KeyState::Pressed);
//         }
//         Ok(Self {
//             keys,
//             _keyboard_state_usb_input: _keyboard_state.usb_input_report().to_vec(),
//         })
//     }

//     pub fn usb_input(&self) -> &Vec<u8> {
//         &self._keyboard_state_usb_input
//     }

//     pub fn is_input_match(&self, usb_input: &Vec<u8>) -> bool {
//         self._keyboard_state_usb_input == *usb_input
//     }

//     pub fn has_modifier(&self) -> bool {
//         return self._keyboard_state_usb_input.len() > 0 && self._keyboard_state_usb_input[0] != 0;
//     }

//     pub fn has_normal_key(&self) -> bool {
//         return self._keyboard_state_usb_input.len() > 2 && self._keyboard_state_usb_input[2] != 0;
//     }
// }
#[derive(Debug, Clone, Eq, Hash)]
pub struct Shortcut {
    modifiers: Vec<VirtualKeyId>,
    normal_keys: Vec<VirtualKeyId>,
}

impl PartialEq for Shortcut {
    fn eq(&self, other: &Self) -> bool {
        if self.modifiers.len() != other.modifiers.len() {
            return false;
        }

        for key in self.modifiers.iter() {
            let count = other.modifiers.iter().filter(|&k| k == key).count();
            if count != 1 {
                return false;
            }
        }

        self.normal_keys == other.normal_keys
    }
}

impl std::fmt::Display for Shortcut {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let keys = self
            .modifiers
            .iter()
            .chain(self.normal_keys.iter())
            .map(|key| key.to_string())
            .collect::<Vec<String>>()
            .join("+");
        write!(f, "{}", keys)
    }
}

impl Shortcut {
    pub fn default() -> Self {
        Self {
            modifiers: Vec::new(),
            normal_keys: Vec::new(),
        }
    }

    pub fn new(keys: Vec<VirtualKeyId>) -> Result<Self, String> {
        if keys.is_empty() {
            return Err("Empty keys".to_string());
        }

        let mut s = Self::default();
        for key in keys {
            s.set_key(key);
        }

        Ok(s)
    }

    fn normalize_key(key: &str) -> Result<VirtualKeyId, String> {
        let key = key.to_string();

        if key.len() == 1 {
            if let Ok(key) = VirtualKeyId::from_str(format!("Us{}", key).as_str()) {
                return Ok(key);
            }
            VirtualKeyId::from_str(key.as_str()).map_err(|_| format!("Invalid key: {}", key))
        } else {
            let key = key
                .replace("Ctrl", "Control")
                .replace("Menu", "Alt")
                .replace("Win", "Meta")
                .replace("Option", "Alt")
                .replace("Cmd", "Meta")
                .replace("Command", "Meta");
            VirtualKeyId::from_str(key.as_str()).map_err(|_| format!("Invalid key: {}", key))
        }
    }

    pub fn from_str(keys: &str) -> Result<Self, String> {
        keys.trim()
            .split("+")
            .map(|key| Self::normalize_key(key))
            .collect::<Result<Vec<VirtualKeyId>, String>>()
            .and_then(Self::new)
    }

    pub fn set_key(&mut self, key: VirtualKeyId) {
        if key.modifier().is_some() {
            if !self.modifiers.contains(&key) {
                self.modifiers.push(key);
            }
        } else {
            if !self.normal_keys.contains(&key) {
                self.normal_keys.push(key);
            }
        }
    }

    pub fn remove_key(&mut self, key: VirtualKeyId) {
        if key.modifier().is_some() {
            self.modifiers.retain(|&k| k != key);
        } else {
            self.normal_keys.retain(|&k| k != key);
        }
    }

    pub fn has_modifier(&self) -> bool {
        self.modifiers.len() > 0
    }

    pub fn has_normal_key(&self) -> bool {
        self.normal_keys.len() > 0
    }

    pub fn is_match(&self, other: &Self) -> bool {
        if self.modifiers.len() != other.modifiers.len() {
            return false;
        }

        if self.normal_keys.len() != other.normal_keys.len() {
            return false;
        }

        for (i, key) in self.modifiers.iter().enumerate() {
            // let mut count = 0;
            // for other_key in other.modifiers.iter() {
            //     let other_key_bits = other_key.modifier().unwrap().bits();
            //     let key_bits = key.modifier().unwrap().bits();
            //     if other_key_bits & !key_bits == 0 {
            //         count += 1;
            //     }
            //     if count > 1 {
            //         return false;
            //     }
            // }
            let count = other
                .modifiers
                .iter()
                .filter(|&other_key| {
                    let other_key_bits = other_key.modifier().unwrap().bits();
                    let key_bits = key.modifier().unwrap().bits();
                    other_key_bits & !key_bits == 0
                })
                .count();
            if count != 1 {
                return false;
            }
        }

        for (key, other_key) in self.normal_keys.iter().zip(other.normal_keys.iter()) {
            if key != other_key {
                return false;
            }
        }
        true
    }
}

pub type JoinHandleType = JoinHandle<()>;

pub trait EventListener {
    fn new() -> Arc<Self>;
    fn add_global_shortcut<F>(&self, shortcut: &str, cb: F) -> std::result::Result<ID, String>
    where
        F: Fn() + Send + Sync + 'static;

    fn add_global_shortcut_trigger<F>(
        &self,
        shortcut: &str,
        cb: F,
        trigger: u32,
        internal: Option<u32>,
    ) -> std::result::Result<ID, String>
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

    use super::*;

    #[test]
    fn test_event_info() {
        let event_type = EventType::KeyboardEvent(Some(KeyInfo::new(
            KeyId::from(VirtualKeyId::UsA),
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
            KeyId::from(VirtualKeyId::UsA),
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

    #[test]
    fn test_shortcut() {
        let shortcut = Shortcut::from_str("Ctrl+Alt+T").unwrap();
        assert_eq!(shortcut.modifiers.len(), 2);
        assert_eq!(shortcut.normal_keys.len(), 1);
        assert_eq!(shortcut.modifiers[0], VirtualKeyId::Control);
        assert_eq!(shortcut.modifiers[1], VirtualKeyId::Alt);
        assert_eq!(shortcut.normal_keys[0], VirtualKeyId::UsT);

        let shortcut = Shortcut::from_str("Ctrl+Alt+T+X").unwrap();
        assert_eq!(shortcut.modifiers.len(), 2);
        assert_eq!(shortcut.normal_keys.len(), 2);
        assert_eq!(shortcut.modifiers[0], VirtualKeyId::Control);
        assert_eq!(shortcut.modifiers[1], VirtualKeyId::Alt);
        assert_eq!(shortcut.normal_keys[0], VirtualKeyId::UsT);
        assert_eq!(shortcut.normal_keys[1], VirtualKeyId::UsX);
    }

    #[test]
    fn test_is_match_shortcut() {
        let shortcut1 = Shortcut::from_str("Ctrl+Alt+T").unwrap();
        let shortcut2 = Shortcut::from_str("Ctrl+Alt+T").unwrap();
        assert!(shortcut1.is_match(&shortcut2));

        let shortcut2 = Shortcut::from_str("Ctrl+AltRight+T").unwrap();
        assert!(shortcut1.is_match(&shortcut2));

        let shortcut2 = Shortcut::from_str("CtrlLeft+Alt+T+X").unwrap();
        assert!(!shortcut1.is_match(&shortcut2));

        let shortcut2 = Shortcut::from_str("Alt+CtrlRight+X").unwrap();
        assert!(!shortcut1.is_match(&shortcut2));

        let shortcut2 = Shortcut::from_str("Shift+Alt+T").unwrap();
        assert!(!shortcut1.is_match(&shortcut2));
    }

    #[test]
    fn test_keyboard_state() {
        let mut state = Shortcut::default();
        state.set_key(VirtualKeyId::ControlLeft);
        state.set_key(VirtualKeyId::Alt);
        state.set_key(VirtualKeyId::UsT);

        assert_eq!(state.to_string(), "ControlLeft+Alt+UsT");
        assert_eq!(state, Shortcut::from_str("ControlLeft+Alt+UsT").unwrap());
        assert_eq!(state, Shortcut::from_str("Alt+ControlLeft+T").unwrap());

        assert_ne!(state, Shortcut::from_str("Control+Alt+UsT").unwrap());
        assert_ne!(Shortcut::from_str("Control+Alt+UsT").unwrap(), state);

        assert_eq!(
            Shortcut::from_str("Control+Alt+UsT").unwrap(),
            Shortcut::from_str("Ctrl+Alt+T").unwrap()
        );

        let shortcut = Shortcut::from_str("Ctrl+Alt+T").unwrap();
        println!("{}", state.to_string());
        println!("{}", shortcut.to_string());
        // assert!(!state.is_match(&shortcut));
        assert!(shortcut.is_match(&state));

        assert!(Shortcut::from_str("Ctrl+Shift+C")
            .unwrap()
            .is_match(&Shortcut::from_str("CtrlRight+ShiftLeft+C").unwrap()));

        state.remove_key(VirtualKeyId::ControlLeft);
        assert_eq!(state.to_string(), "Alt+UsT");

        state.remove_key(VirtualKeyId::Alt);
        assert_eq!(state.to_string(), "UsT");

        state.remove_key(VirtualKeyId::UsT);
        assert_eq!(state.to_string(), "");
    }
}
