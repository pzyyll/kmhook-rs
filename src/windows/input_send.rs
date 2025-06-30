// ABOUTME: Implements various strategies for sending text to a window.
// ABOUTME: Provides both direct message sending and input simulation methods.

#![allow(dead_code)]

use std::ffi::OsStr;
use std::iter::once;
use std::os::windows::ffi::OsStrExt;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Foundation::{GetLastError, GlobalFree, HWND, LPARAM, WPARAM};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::System::Ole::CF_UNICODETEXT;
use windows::Win32::UI::Controls::{EM_REPLACESEL, EM_SETSEL};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE,
    VIRTUAL_KEY, VK_CONTROL, VK_V,
};
use windows::Win32::UI::WindowsAndMessaging::{
    IsWindow, PostMessageW, SendMessageW, SetForegroundWindow, WM_CHAR, WM_SETTEXT,
};

/// Enum representing the different strategies for sending text.
pub enum SendStrategy {
    /// Uses the WM_SETTEXT message. This is the most direct way but may not work for all controls.
    SetText,
    /// Sends WM_CHAR messages for each character. Slower but more compatible.
    Char,
    /// Uses EM_SETSEL and EM_REPLACESEL to replace the selection. Primarily for edit controls.
    ReplaceSel,
    /// Simulates keyboard input using SendInput. This is the most reliable method.
    SendInput,
    /// Copy text to clipboard and paste it into the target window.
    ClipboardPaste, // Not implemented yet
}

/// Sends text to the specified window using the chosen strategy.
///
/// # Arguments
///
/// * `hwnd` - Handle to the target window.
/// * `text` - The text string to send.
/// * `strategy` - The sending strategy to use.
///
/// # Returns
///
/// * `Ok(())` if the operation was successful.
/// * `Err(String)` if the operation failed.
pub fn send_text(hwnd: HWND, text: &str, strategy: SendStrategy) -> Result<(), String> {
    match strategy {
        SendStrategy::SetText => send_text_by_settext(hwnd, text),
        SendStrategy::Char => send_text_by_char(hwnd, text),
        SendStrategy::ReplaceSel => send_text_by_replace_sel(hwnd, text),
        SendStrategy::SendInput => send_text_by_sendinput(hwnd, text),
        SendStrategy::ClipboardPaste => send_text_by_clipboard_paste(hwnd, text),
    }
}

fn send_text_by_clipboard_paste(hwnd: HWND, text: &str) -> Result<(), String> {
    check_hwnd(hwnd)?;

    unsafe {
        if !SetForegroundWindow(hwnd).as_bool() {
            return Err(format!(
                "Failed to set window to foreground: {:?}",
                GetLastError()
            ));
        }
    }

    // Add a small delay to ensure the window is ready
    std::thread::sleep(std::time::Duration::from_millis(50));

    // 1. Save original clipboard content by copying the data
    let original_clipboard_data = unsafe {
        if OpenClipboard(None).is_err() {
            return Err("Failed to open clipboard".to_string());
        }

        let original_data = match GetClipboardData(CF_UNICODETEXT.0 as u32) {
            Ok(handle) => {
                if !handle.is_invalid() {
                    // Convert HANDLE to the appropriate type for GlobalLock
                    // We need to create a wrapper that matches the expected type
                    let global_handle = std::mem::transmute(handle);
                    let ptr = GlobalLock(global_handle);
                    if !ptr.is_null() {
                        // Calculate the length of the wide string
                        let mut len = 0;
                        let src_ptr = ptr as *const u16;
                        while *src_ptr.offset(len) != 0 {
                            len += 1;
                        }
                        len += 1; // Include null terminator

                        // Copy to our own buffer
                        let mut data = vec![0u16; len as usize];
                        std::ptr::copy_nonoverlapping(src_ptr, data.as_mut_ptr(), len as usize);
                        let _ = GlobalUnlock(global_handle);
                        Some(data)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            Err(_) => None,
        };

        let _ = CloseClipboard();
        original_data
    };

    // 2. Set new text to clipboard
    let wide_text: Vec<u16> = OsStr::new(text).encode_wide().chain(once(0)).collect();

    unsafe {
        if OpenClipboard(None).is_err() {
            return Err("Failed to open clipboard for setting data".to_string());
        }

        let h_global = GlobalAlloc(GMEM_MOVEABLE, wide_text.len() * 2);
        if h_global.is_err() {
            let _ = CloseClipboard();
            return Err("Failed to allocate memory for clipboard".to_string());
        }

        let hglobal = h_global.unwrap();
        let p_global = GlobalLock(hglobal);
        if p_global.is_null() {
            let _ = GlobalFree(Some(hglobal));
            let _ = CloseClipboard();
            return Err("Failed to lock memory for clipboard".to_string());
        }

        // Copy data - the length parameter should be the number of bytes
        std::ptr::copy_nonoverlapping(
            wide_text.as_ptr() as *const u8,
            p_global as *mut u8,
            wide_text.len() * 2,
        );

        let _ = GlobalUnlock(hglobal);
        let _ = EmptyClipboard();

        if SetClipboardData(CF_UNICODETEXT.0 as u32, Some(HANDLE(hglobal.0))).is_err() {
            let _ = GlobalFree(Some(hglobal));
            let _ = CloseClipboard();
            return Err("Failed to set clipboard data".to_string());
        }

        let _ = CloseClipboard();
    }

    // 3. Simulate Ctrl+V
    let inputs = [
        // Press Ctrl
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_CONTROL,
                    wScan: 0,
                    dwFlags: Default::default(),
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
        // Press V
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_V,
                    wScan: 0,
                    dwFlags: Default::default(),
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
        // Release V
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_V,
                    wScan: 0,
                    dwFlags: KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
        // Release Ctrl
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_CONTROL,
                    wScan: 0,
                    dwFlags: KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
    ];

    unsafe {
        let sent = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        if sent != inputs.len() as u32 {
            return Err(format!(
                "SendInput failed: sent {} of {} events",
                sent,
                inputs.len()
            ));
        }
    }

    // Add a small delay to ensure the paste operation completes
    std::thread::sleep(std::time::Duration::from_millis(100));

    // 4. Restore original clipboard content
    if let Some(original_data) = original_clipboard_data {
        unsafe {
            if OpenClipboard(None).is_ok() {
                let _ = EmptyClipboard();

                // Allocate new memory for the original data
                let h_global = GlobalAlloc(GMEM_MOVEABLE, original_data.len() * 2);
                if let Ok(hglobal) = h_global {
                    let p_global = GlobalLock(hglobal);
                    if !p_global.is_null() {
                        std::ptr::copy_nonoverlapping(
                            original_data.as_ptr() as *const u8,
                            p_global as *mut u8,
                            original_data.len() * 2,
                        );
                        let _ = GlobalUnlock(hglobal);
                        let _ = SetClipboardData(CF_UNICODETEXT.0 as u32, Some(HANDLE(hglobal.0)));
                    } else {
                        let _ = GlobalFree(Some(hglobal));
                    }
                }

                let _ = CloseClipboard();
            }
        }
    } else {
        // If there was no original data, just clear the clipboard
        unsafe {
            if OpenClipboard(None).is_ok() {
                let _ = EmptyClipboard();
                let _ = CloseClipboard();
            }
        }
    }

    Ok(())
}

fn check_hwnd(hwnd: HWND) -> Result<(), String> {
    if unsafe { !IsWindow(Some(hwnd)).as_bool() } {
        println!("Invalid window handle: {:?}", hwnd);
        return Err("Invalid window handle".to_string());
    }
    Ok(())
}

/// Strategy 1: Send text using WM_SETTEXT message.
/// This is a very direct method but is not supported by all window controls.
fn send_text_by_settext(hwnd: HWND, text: &str) -> Result<(), String> {
    check_hwnd(hwnd)?;

    let wide_text: Vec<u16> = OsStr::new(text).encode_wide().chain(once(0)).collect();
    let result = unsafe {
        SendMessageW(
            hwnd,
            WM_SETTEXT,
            None,
            Some(LPARAM(wide_text.as_ptr() as isize)),
        )
    };

    if result.0 == 0 {
        // For some controls, a return value of 0 does not necessarily mean failure.
        // However, for many standard controls, non-zero indicates success.
        // We will consider 0 as a potential failure for fallback purposes.
        Err("WM_SETTEXT failed or returned 0.".to_string())
    } else {
        Ok(())
    }
}

/// Strategy 2: Send text by posting WM_CHAR messages for each character.
/// This simulates character input at a message level.
fn send_text_by_char(hwnd: HWND, text: &str) -> Result<(), String> {
    check_hwnd(hwnd)?;

    for ch in text.chars() {
        let result =
            unsafe { PostMessageW(Some(hwnd), WM_CHAR, WPARAM(ch as u32 as usize), LPARAM(0)) };

        if let Err(e) = result {
            return Err(format!("Failed to post WM_CHAR for '{}': {:?}", ch, e));
        }
    }
    Ok(())
}

/// Strategy 3: Send text using EM_SETSEL and EM_REPLACESEL.
/// This is typically used for edit controls to replace the current selection.
fn send_text_by_replace_sel(hwnd: HWND, text: &str) -> Result<(), String> {
    check_hwnd(hwnd)?;

    let wide_text: Vec<u16> = OsStr::new(text).encode_wide().chain(once(0)).collect();

    // Select all text (from start to end)
    unsafe {
        SendMessageW(hwnd, EM_SETSEL, Some(WPARAM(0)), Some(LPARAM(-1)));
    }

    // Replace the selection with the new text
    let result = unsafe {
        SendMessageW(
            hwnd,
            EM_REPLACESEL,
            Some(WPARAM(1)),
            Some(LPARAM(wide_text.as_ptr() as isize)),
        )
    };

    if result.0 == 0 {
        Err("EM_REPLACESEL failed.".to_string())
    } else {
        Ok(())
    }
}

/// Strategy 4: Send text by simulating keyboard events with SendInput.
/// This is the most robust method as it mimics actual user input.
fn send_text_by_sendinput(hwnd: HWND, text: &str) -> Result<(), String> {
    check_hwnd(hwnd)?;

    unsafe {
        let result = SetForegroundWindow(hwnd);
        if !result.as_bool() {
            let err = GetLastError();
            return Err(format!("Failed to set window to foreground: {:?}", err));
        }
    }

    std::thread::sleep(std::time::Duration::from_millis(50));

    let mut inputs: Vec<INPUT> = Vec::with_capacity(text.len() * 2);

    for ch in text.encode_utf16() {
        let key_down = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(0),
                    wScan: ch,
                    dwFlags: KEYEVENTF_UNICODE,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };

        let key_up = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(0),
                    wScan: ch,
                    dwFlags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };

        inputs.push(key_down);
        inputs.push(key_up);
    }

    // Add this check
    if inputs.is_empty() {
        return Err("No valid characters to send.".to_string());
    }

    let result = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };

    if result as usize != inputs.len() {
        Err("SendInput failed to send all key events.".to_string())
    } else {
        Ok(())
    }
}
