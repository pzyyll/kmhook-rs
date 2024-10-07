use crate::types::{KeyId, KeyMap, VirtualKeyId};
use windows::Win32::UI::{
    Input::{
        KeyboardAndMouse::{
            MapVirtualKeyW, MAPVK_VK_TO_VSC_EX, VIRTUAL_KEY, VK_LCONTROL, VK_LMENU, VK_LWIN,
            VK_RCONTROL, VK_RMENU, VK_RWIN,
        },
        RAWKEYBOARD,
    },
    WindowsAndMessaging::{KBDLLHOOKSTRUCT, RI_KEY_E0, RI_KEY_E1},
};

impl KeyId {
    fn from_scan_code(scancode: u32) -> std::result::Result<Self, ()> {
        let keymap = KeyMap::from_key_mapping(keycode::KeyMapping::Win(scancode as u16))?;
        if let Ok(vk) = VirtualKeyId::try_from(keymap.id) {
            Ok(Self(vk))
        } else {
            Err(())
        }
    }
}

impl TryFrom<KBDLLHOOKSTRUCT> for KeyId {
    type Error = ();

    fn try_from(value: KBDLLHOOKSTRUCT) -> Result<Self, Self::Error> {
        let scancode = value.scanCode;
        let vkcode = value.vkCode;
        match VIRTUAL_KEY(vkcode as u16) {
            VK_LWIN => Ok(Self(VirtualKeyId::MetaLeft)),
            VK_RWIN => Ok(Self(VirtualKeyId::MetaRight)),
            VK_LCONTROL => Ok(Self(VirtualKeyId::ControlLeft)),
            VK_RCONTROL => Ok(Self(VirtualKeyId::ControlRight)),
            VK_LMENU => Ok(Self(VirtualKeyId::AltLeft)),
            VK_RMENU => Ok(Self(VirtualKeyId::AltRight)),
            _ => Self::from_scan_code(scancode),
        }
    }
}

impl TryFrom<RAWKEYBOARD> for KeyId {
    type Error = ();

    fn try_from(keyboard: RAWKEYBOARD) -> Result<Self, Self::Error> {
        let scancode = if keyboard.MakeCode != 0 {
            (keyboard.MakeCode as u32 & 0x7f)
                | ((if keyboard.Flags as u32 & RI_KEY_E0 != 0 {
                    0xe0
                } else if keyboard.Flags as u32 & RI_KEY_E1 != 0 {
                    0xe1
                } else {
                    0x00
                }) << 8)
        } else {
            unsafe { MapVirtualKeyW(keyboard.VKey as u32, MAPVK_VK_TO_VSC_EX) & 0xFFFF }
        };
        Self::from_scan_code(scancode)
    }
}
