//! Copy-and-paste delivery back to the application the user came from.
//!
//! When the user chooses a Clipboard History entry, an Emoji, or a Snippet,
//! YAL hides its palette, writes the chosen text to the clipboard, returns
//! focus to the previously active window, and injects a clean `Ctrl+V` via
//! `SendInput` (flagged `LLKHF_INJECTED` so our hotkey hook ignores it).

use std::ffi::c_void;
use std::time::Duration;

use tc_platform::Clipboard;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_CONTROL, VK_V,
};
use windows::Win32::UI::WindowsAndMessaging::{IsWindow, SetForegroundWindow};

use crate::clipboard::WindowsClipboard;
use crate::window_control;

/// Write `text` to the clipboard immediately, then asynchronously restore the
/// previous foreground window and send `Ctrl+V` after YAL hides.
pub fn copy_and_paste_async(text: String) -> Result<(), String> {
    WindowsClipboard::new().write_text(&text)?;
    let target = window_control::last_foreground();

    std::thread::spawn(move || {
        // Give YAL a brief moment to hide before restoring focus to the target app.
        std::thread::sleep(Duration::from_millis(45));
        if let Some(handle) = target {
            let hwnd = HWND(handle as *mut c_void);
            unsafe {
                if !hwnd.0.is_null() && IsWindow(Some(hwnd)).as_bool() {
                    let _ = SetForegroundWindow(hwnd);
                }
            }
            std::thread::sleep(Duration::from_millis(35));
        }
        unsafe {
            send_ctrl_v();
        }
    });
    Ok(())
}

unsafe fn send_ctrl_v() {
    let inputs = [
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_CONTROL,
                    ..Default::default()
                },
            },
        },
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_V,
                    ..Default::default()
                },
            },
        },
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_V,
                    dwFlags: KEYEVENTF_KEYUP,
                    ..Default::default()
                },
            },
        },
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_CONTROL,
                    dwFlags: KEYEVENTF_KEYUP,
                    ..Default::default()
                },
            },
        },
    ];
    let _ = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
}
