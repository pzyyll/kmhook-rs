use crate::types::KeyId;
use windows::Win32::UI::{
    Input::KeyboardAndMouse::{
        VIRTUAL_KEY, VK_LCONTROL, VK_LMENU, VK_LWIN, VK_RCONTROL, VK_RMENU, VK_RWIN,
    },
    WindowsAndMessaging::KBDLLHOOKSTRUCT,
};

impl KeyId {
    fn from_win(scancode: u32, vkcode: u32) -> std::result::Result<Self, ()> {
        match VIRTUAL_KEY(vkcode as u16) {
            VK_LWIN => Ok(Self(crate::types::KeyCode::MetaLeft)),
            VK_RWIN => Ok(Self(crate::types::KeyCode::MetaRight)),
            VK_LCONTROL => Ok(Self(crate::types::KeyCode::ControlLeft)),
            VK_RCONTROL => Ok(Self(crate::types::KeyCode::ControlRight)),
            VK_LMENU => Ok(Self(crate::types::KeyCode::AltLeft)),
            VK_RMENU => Ok(Self(crate::types::KeyCode::AltRight)),
            _ => {
                let keymap = crate::types::KeyMap::from_key_mapping(keycode::KeyMapping::Win(
                    scancode as u16,
                ))?;
                Ok(Self(crate::types::KeyCode::from(keymap.id)))
            }
        }
    }
}

impl TryFrom<KBDLLHOOKSTRUCT> for KeyId {
    type Error = ();

    fn try_from(value: KBDLLHOOKSTRUCT) -> Result<Self, Self::Error> {
        let scancode = value.scanCode;
        let vkcode = value.vkCode;
        KeyId::from_win(scancode, vkcode)
    }
}
