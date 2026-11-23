//! Copy-and-paste delivery back to the application the user came from.
//!
//! When the user chooses a Clipboard History entry, an Emoji, or a Snippet,
//! YAL hides its palette, writes the chosen text to the clipboard, returns
//! focus to the previously active window, and — only if Windows actually
//! restored focus to that exact window — injects `Ctrl+V` via `SendInput`.
//! If the previous window is gone or refuses foreground focus, the text stays
//! safely on the clipboard without pasting into an unintended application.

use std::ffi::c_void;
use std::time::Duration;

use tc_platform::Clipboard;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_CONTROL, VK_V,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, IsWindow, SetForegroundWindow};

use crate::clipboard::WindowsClipboard;
use crate::window_control;

/// Write `text` to the clipboard immediately, then asynchronously restore the
/// previous foreground window and send `Ctrl+V` only if focus returned to it.
pub fn copy_and_paste_async(text: String) -> Result<(), String> {
    WindowsClipboard::new().write_text(&text)?;
    let Some(handle) = window_control::last_foreground() else {
        return Ok(());
    };

    std::thread::spawn(move || {
        // Give YAL a brief moment to hide before restoring focus to the target app.
        std::thread::sleep(Duration::from_millis(45));
        let hwnd = HWND(handle as *mut c_void);
        if hwnd.0.is_null() {
            return;
        }
        unsafe {
            if !IsWindow(Some(hwnd)).as_bool() {
                return;
            }
            let _ = SetForegroundWindow(hwnd);
        }
        std::thread::sleep(Duration::from_millis(35));
        unsafe {
            // Only send Ctrl+V if Windows genuinely restored focus to the
            // exact target window we came from (never paste into a random app).
            if IsWindow(Some(hwnd)).as_bool() && GetForegroundWindow() == hwnd {
                send_ctrl_v();
            }
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
