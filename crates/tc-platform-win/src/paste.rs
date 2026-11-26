//! Copy-and-paste delivery back to the application the user came from.
//!
//! When the user chooses a Clipboard History entry, an Emoji, or a Snippet,
//! YAL hides its palette, writes the chosen text to the clipboard, returns
//! focus to the previously active window, and — only if Windows actually
//! restored focus to that exact window — injects `Ctrl+V` (or `Ctrl+Shift+V`
//! when targeting terminal emulators like Windows Terminal, Alacritty, or
//! WezTerm) via `SendInput`.

use std::ffi::c_void;
use std::time::Duration;

use tc_platform::Clipboard;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_BACK, VK_CONTROL,
    VK_SHIFT, VK_V,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetClassNameW, GetForegroundWindow, GetWindowThreadProcessId, IsIconic, IsWindow,
    IsWindowVisible, SetForegroundWindow,
};

use crate::clipboard::WindowsClipboard;
use crate::window_control;

/// Returns `true` when there is a valid, visible, non-shell target window
/// previously focused before YAL opened that can receive a `Ctrl+V` paste.
///
/// Returns `false` when the user was on the Desktop, Taskbar, Start Menu,
/// System Tray, or when the previous window was closed/minimized — letting
/// Clipboard History fall back to an active clipboard copy instead of
/// blindly firing `Ctrl+V` into nothing.
pub fn has_paste_target() -> bool {
    let Some(handle) = window_control::last_foreground() else {
        return false;
    };
    let hwnd = HWND(handle as *mut c_void);
    if hwnd.0.is_null() {
        return false;
    }
    // SAFETY: `hwnd` is non-null and validated via `IsWindow` inside `is_pasteable_window`.
    unsafe { is_pasteable_window(hwnd) }
}

/// Check whether `hwnd` is a real, visible, non-minimized application window
/// (excluding YAL itself and non-pasteable Windows shell chrome such as
/// Desktop, Taskbar, Start Menu, Search, and System Tray).
unsafe fn is_pasteable_window(hwnd: HWND) -> bool {
    if hwnd.0.is_null()
        || !IsWindow(Some(hwnd)).as_bool()
        || !IsWindowVisible(hwnd).as_bool()
        || IsIconic(hwnd).as_bool()
    {
        return false;
    }
    let mut pid = 0u32;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if pid == 0 || pid == std::process::id() {
        return false;
    }

    let mut buf = [0u16; 128];
    let len = GetClassNameW(hwnd, &mut buf);
    if len > 0 {
        let class = String::from_utf16_lossy(&buf[..len as usize]);
        if matches!(
            class.as_str(),
            "Progman"
                | "WorkerW"
                | "Shell_TrayWnd"
                | "Shell_SecondaryTrayWnd"
                | "NotifyIconOverflowWindow"
                | "TopLevelWindowForOverflowXamlIsland"
                | "Shell_InputSwitchTopLevelWindow"
                | "TaskListThumbnailWnd"
                | "TaskSwitcherWnd"
                | "MultitaskingViewFrame"
                | "TrayNotifyWnd"
        ) {
            return false;
        }
        if class == "Windows.UI.Core.CoreWindow" {
            if let Ok(proc_handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
                let mut pbuf = [0u16; 512];
                let mut plen = pbuf.len() as u32;
                let ok = QueryFullProcessImageNameW(
                    proc_handle,
                    PROCESS_NAME_FORMAT(0),
                    windows::core::PWSTR(pbuf.as_mut_ptr()),
                    &mut plen,
                )
                .is_ok();
                let _ = windows::Win32::Foundation::CloseHandle(proc_handle);
                if ok && plen > 0 {
                    let exe = String::from_utf16_lossy(&pbuf[..plen as usize]).to_ascii_lowercase();
                    if exe.ends_with("startmenuexperiencehost.exe")
                        || exe.ends_with("searchhost.exe")
                        || exe.ends_with("searchapp.exe")
                        || exe.ends_with("shellexperiencehost.exe")
                    {
                        return false;
                    }
                }
            }
        }
    }
    true
}

/// Write `text` to the clipboard immediately, then asynchronously restore the
/// previous foreground window and send `Ctrl+V` (or `Ctrl+Shift+V` for
/// terminals) only if focus genuinely returned to that window.
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
        // SAFETY: `hwnd` is non-null and validated with `is_pasteable_window` before restoring focus.
        unsafe {
            if !is_pasteable_window(hwnd) {
                return;
            }
            let _ = SetForegroundWindow(hwnd);
        }
        std::thread::sleep(Duration::from_millis(35));
        // SAFETY: `hwnd` is re-validated and verified as the active foreground window.
        unsafe {
            if is_pasteable_window(hwnd) && GetForegroundWindow() == hwnd {
                let use_ctrl_shift_v = is_terminal_window(hwnd);
                send_paste(use_ctrl_shift_v);
            }
        }
    });
    Ok(())
}

/// Asynchronously erase `backspaces` characters in the currently focused window
/// and paste `text` via the clipboard (`Ctrl+Shift+V` for terminals, `Ctrl+V`
/// otherwise). Used by global snippet keyword auto-expansion.
pub fn erase_and_paste_async(backspaces: usize, text: String) -> Result<(), String> {
    std::thread::Builder::new()
        .name("yal-snippet-paste".to_string())
        .spawn(move || {
            // Brief settle delay so the trigger Space/Tab finishes processing in the target app.
            std::thread::sleep(Duration::from_millis(10));
            if WindowsClipboard::new().write_text(&text).is_err() {
                return;
            }
            let count = backspaces.min(256);
            if count > 0 {
                // SAFETY: `send_backspaces` constructs a valid `INPUT` slice for `SendInput`.
                unsafe {
                    send_backspaces(count);
                }
            }
            std::thread::sleep(Duration::from_millis(20));
            // SAFETY: `GetForegroundWindow` returns null or a valid window handle; `IsWindow`
            // verifies the handle before inspecting its class name or injecting paste keys.
            unsafe {
                let hwnd = GetForegroundWindow();
                if !hwnd.0.is_null() && IsWindow(Some(hwnd)).as_bool() {
                    let use_ctrl_shift_v = is_terminal_window(hwnd);
                    send_paste(use_ctrl_shift_v);
                }
            }
        })
        .map(|_| ())
        .map_err(|error| format!("the snippet paste thread could not be started: {error}"))
}

/// Write an RGBA image to the Windows clipboard as a DIB immediately, then
/// asynchronously restore focus to the previous foreground window and paste
/// with `Ctrl+V`.
pub fn paste_image_async(width: u32, height: u32, rgba: Vec<u8>) -> Result<(), String> {
    WindowsClipboard::new().write_dib_rgba(width, height, &rgba)?;
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
        // SAFETY: `hwnd` is non-null and validated with `is_pasteable_window` before restoring focus.
        unsafe {
            if !is_pasteable_window(hwnd) {
                return;
            }
            let _ = SetForegroundWindow(hwnd);
        }
        std::thread::sleep(Duration::from_millis(35));
        // SAFETY: `hwnd` is re-validated and verified as the active foreground window.
        unsafe {
            if is_pasteable_window(hwnd) && GetForegroundWindow() == hwnd {
                send_paste(false);
            }
        }
    });
    Ok(())
}

/// Write a list of file paths to the Windows clipboard as `CF_HDROP` immediately,
/// then asynchronously restore focus to the previous foreground window and paste
/// with `Ctrl+V`.
pub fn paste_files_async(paths: Vec<String>) -> Result<(), String> {
    WindowsClipboard::new().write_files(&paths)?;
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
        // SAFETY: `hwnd` is non-null and validated with `is_pasteable_window` before restoring focus.
        unsafe {
            if !is_pasteable_window(hwnd) {
                return;
            }
            let _ = SetForegroundWindow(hwnd);
        }
        std::thread::sleep(Duration::from_millis(35));
        // SAFETY: `hwnd` is re-validated and verified as the active foreground window.
        unsafe {
            if is_pasteable_window(hwnd) && GetForegroundWindow() == hwnd {
                send_paste(false);
            }
        }
    });
    Ok(())
}

/// Detect common Windows terminal emulators that expect `Ctrl+Shift+V`.
unsafe fn is_terminal_window(hwnd: HWND) -> bool {
    let mut buf = [0u16; 128];
    let len = GetClassNameW(hwnd, &mut buf);
    if len <= 0 {
        return false;
    }
    let class = String::from_utf16_lossy(&buf[..len as usize]).to_ascii_lowercase();
    matches!(
        class.as_str(),
        "cascadia_hosting_window_class"
            | "consolewindowclass"
            | "mintty"
            | "org.wezfurlong.wezterm"
            | "alacritty"
            | "ghostty"
    ) || class.contains("terminal")
}

unsafe fn send_backspaces(count: usize) {
    let mut inputs = Vec::with_capacity(count * 2);
    for _ in 0..count {
        inputs.push(key_input(VK_BACK.0, false));
        inputs.push(key_input(VK_BACK.0, true));
    }
    let _ = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
}

unsafe fn send_paste(with_shift: bool) {
    let mut inputs = Vec::with_capacity(6);
    inputs.push(key_input(VK_CONTROL.0, false));
    if with_shift {
        inputs.push(key_input(VK_SHIFT.0, false));
    }
    inputs.push(key_input(VK_V.0, false));
    inputs.push(key_input(VK_V.0, true));
    if with_shift {
        inputs.push(key_input(VK_SHIFT.0, true));
    }
    inputs.push(key_input(VK_CONTROL.0, true));
    let _ = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
}

fn key_input(vk: u16, up: bool) -> INPUT {
    use windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY;
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                dwFlags: if up {
                    KEYEVENTF_KEYUP
                } else {
                    Default::default()
                },
                ..Default::default()
            },
        },
    }
}
