use windows::Win32::{
    Foundation::{LPARAM, WPARAM},
    UI::WindowsAndMessaging::{DispatchMessageW, GetMessageW, TranslateMessage, MSG},
};

use libloading::{Library, Symbol};
use std::env;

fn main() {
    let current_dir = env::current_dir().unwrap();
    println!("Current directory: {:?}", current_dir);
    let dll_path = r"F:\codespace\kmhook-rs\winhook_cdylib\out\build\Visual Studio Community 2022 Release - x86_amd64\Debug\winhook.dll";

    unsafe {
        match Library::new(dll_path) {
            // typedef void (*KeyboardCallback)(WPARAM, LPARAM);
            // typedef void (*MouseCallback)(WPARAM, LPARAM);

            // extern "C" {
            // WINHOOK_API int SetKeyboardHook();
            // WINHOOK_API void UnhookKeyboard();
            // WINHOOK_API int RegKeyboardEvent(KeyboardCallback callback);
            // WINHOOK_API void DelKeyboardEvent(int);

            // WINHOOK_API int SetMouseHook();
            // WINHOOK_API void UnhookMouse();
            // WINHOOK_API int RegMouseEvent(MouseCallback callback);
            // WINHOOK_API void DelMouseEvent(int);
            // }
            Ok(hook_dll) => {
                let set_keyboard_hook: Symbol<unsafe extern "C" fn() -> i32> =
                    hook_dll.get(b"SetKeyboardHook\0").unwrap();
                let unhook_keyboard: Symbol<unsafe extern "C" fn() -> ()> =
                    hook_dll.get(b"UnhookKeyboard\0").unwrap();
                let reg_keyboard_event: Symbol<
                    unsafe extern "C" fn(extern "C" fn(WPARAM, LPARAM) -> ()) -> i32,
                > = hook_dll.get(b"RegKeyboardEvent\0").unwrap();
                let del_keyboard_event: Symbol<unsafe extern "C" fn(i32) -> ()> =
                    hook_dll.get(b"DelKeyboardEvent\0").unwrap();
                let set_mouse_hook: Symbol<unsafe extern "C" fn() -> i32> =
                    hook_dll.get(b"SetMouseHook\0").unwrap();
                let unhook_mouse: Symbol<unsafe extern "C" fn() -> ()> =
                    hook_dll.get(b"UnhookMouse\0").unwrap();
                let reg_mouse_event: Symbol<
                    unsafe extern "C" fn(extern "C" fn(WPARAM, LPARAM) -> ()) -> i32,
                > = hook_dll.get(b"RegMouseEvent\0").unwrap();
                let del_mouse_event: Symbol<unsafe extern "C" fn(i32) -> ()> =
                    hook_dll.get(b"DelMouseEvent\0").unwrap();

                set_keyboard_hook();
                set_mouse_hook();

                extern "C" fn keyboard_callback(wparam: WPARAM, lparam: LPARAM) {
                    println!(
                        "keyboard_callback: wparam: {:?}, lparam: {:?}",
                        wparam, lparam
                    );
                }
                extern "C" fn mouse_callback(wparam: WPARAM, lparam: LPARAM) {
                    println!("mouse_callback: wparam: {:?}, lparam: {:?}", wparam, lparam);
                }

                reg_keyboard_event(keyboard_callback);
                reg_mouse_event(mouse_callback);

                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }

                unhook_keyboard();
                unhook_mouse();
            }
            Err(e) => {
                eprintln!("Failed to load library: {}", e);
            }
        }
    }
}
