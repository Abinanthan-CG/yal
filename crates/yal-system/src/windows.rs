//! Listing the windows that are open on the desktop.
//!
//! Only top-level windows that are visible and have a non-empty title are
//! reported. A window without a title has nothing to show the user.

use std::ffi::OsString;

/// A failure while enumerating windows.
#[derive(Debug, thiserror::Error)]
pub enum WindowError {
    #[error("the window list could not be read: {0}")]
    Failed(String),
}

/// One open window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    /// The window handle, as a number.
    pub handle: u64,
    /// The window title.
    pub title: String,
    /// The owning process identifier.
    pub process_id: u32,
    /// The owning executable name.
    pub process_name: Option<String>,
}

/// List the visible top-level windows.
#[cfg(windows)]
pub fn list_windows() -> Result<Vec<Window>, WindowError> {
    use std::os::windows::ffi::OsStringExt;
    use windows::core::BOOL;
    use windows::Win32::Foundation::{FALSE, HWND, LPARAM, TRUE};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
        IsWindowVisible,
    };

    // The collector is handed to the callback through the `LPARAM`, because a
    // callback cannot capture its environment.
    let mut collected: Vec<Window> = Vec::new();

    unsafe extern "system" fn callback(handle: HWND, parameter: LPARAM) -> BOOL {
        let pointer = parameter.0 as *mut Vec<Window>;
        if pointer.is_null() {
            return FALSE;
        }
        let collector = unsafe { &mut *pointer };

        if !unsafe { IsWindowVisible(handle) }.as_bool() {
            return TRUE;
        }

        let length = unsafe { GetWindowTextLengthW(handle) };
        if length <= 0 {
            return TRUE;
        }

        let mut buffer = vec![0u16; (length + 1) as usize];
        let written = unsafe { GetWindowTextW(handle, &mut buffer) };
        if written <= 0 {
            return TRUE;
        }
        let title = OsString::from_wide(&buffer[..written as usize])
            .to_string_lossy()
            .into_owned();
        if title.trim().is_empty() {
            return TRUE;
        }

        let mut process_id = 0u32;
        unsafe { GetWindowThreadProcessId(handle, Some(&mut process_id)) };

        collector.push(Window {
            handle: handle.0 as u64,
            title,
            process_id,
            process_name: None,
        });
        TRUE
    }

    unsafe {
        EnumWindows(
            Some(callback),
            LPARAM(&mut collected as *mut Vec<Window> as isize),
        )
        .map_err(|err| WindowError::Failed(err.to_string()))?;
    }

    // Resolve the owning executable for each window.
    for window in &mut collected {
        window.process_name = process_name(window.process_id);
    }
    Ok(collected)
}

/// The executable name that owns a process.
#[cfg(windows)]
fn process_name(process_id: u32) -> Option<String> {
    use std::os::windows::ffi::OsStringExt;
    use windows::core::PWSTR;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
        QueryFullProcessImageNameW,
    };

    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id).ok()?;
        let mut buffer = vec![0u16; 1024];
        let mut size = buffer.len() as u32;
        let ok = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_FORMAT(0),
            PWSTR(buffer.as_mut_ptr()),
            &mut size,
        )
        .is_ok();
        let _ = CloseHandle(handle);
        if !ok {
            return None;
        }
        let path = std::path::PathBuf::from(OsString::from_wide(&buffer[..size as usize]));
        path.file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
    }
}

/// List the visible top-level windows on a non-Windows host.
#[cfg(not(windows))]
pub fn list_windows() -> Result<Vec<Window>, WindowError> {
    Ok(Vec::new())
}

/// The handle of this process's own main window.
///
/// A graphical toolkit can create several windows for one process — helper
/// windows, IME windows, and so on. The main window is the largest visible
/// one, which distinguishes it from the rest without depending on a title
/// or a class name.
#[cfg(windows)]
pub fn own_window_handle() -> Option<u64> {
    use windows::core::BOOL;
    use windows::Win32::Foundation::{FALSE, HWND, LPARAM, RECT, TRUE};
    use windows::Win32::System::Threading::GetCurrentProcessId;
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowRect, GetWindowThreadProcessId, IsWindowVisible,
    };

    let mut best: Option<(u64, i64)> = None;

    unsafe extern "system" fn callback(handle: HWND, parameter: LPARAM) -> BOOL {
        let pointer = parameter.0 as *mut Option<(u64, i64)>;
        if pointer.is_null() {
            return FALSE;
        }
        let slot = unsafe { &mut *pointer };

        let mut process_id = 0u32;
        unsafe { GetWindowThreadProcessId(handle, Some(&mut process_id)) };
        if process_id != unsafe { GetCurrentProcessId() } {
            return TRUE;
        }
        if !unsafe { IsWindowVisible(handle) }.as_bool() {
            return TRUE;
        }
        let mut rect = RECT::default();
        if unsafe { GetWindowRect(handle, &mut rect) }.is_err() {
            return TRUE;
        }
        let area = i64::from(rect.right - rect.left) * i64::from(rect.bottom - rect.top);
        if area <= 0 {
            return TRUE;
        }
        let handle_value = handle.0 as u64;
        match slot {
            Some((_, best_area)) if *best_area >= area => {}
            _ => *slot = Some((handle_value, area)),
        }
        TRUE
    }

    unsafe {
        EnumWindows(
            Some(callback),
            LPARAM(&mut best as *mut Option<(u64, i64)> as isize),
        )
        .ok()?;
    }
    best.map(|(handle, _)| handle)
}

/// The handle of this process's own top-level window.
#[cfg(not(windows))]
pub fn own_window_handle() -> Option<u64> {
    None
}

/// Bring a window to the front and give it the keyboard focus.
///
/// Windows refuses to let a process take the focus away from whatever is
/// currently in front, unless that process is the one that last received
/// input. A launcher is neither, so the call is made from inside the
/// foreground window's own input queue: the two threads are temporarily
/// joined, which makes the focus transfer legal.
///
/// The return value reports whether the request was accepted.
#[cfg(windows)]
pub fn focus_window(handle: u64) -> bool {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows::Win32::UI::Input::KeyboardAndMouse::SetActiveWindow;
    use windows::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, SetForegroundWindow,
        ShowWindow, SW_RESTORE,
    };

    unsafe {
        let window = HWND(handle as *mut core::ffi::c_void);
        let _ = ShowWindow(window, SW_RESTORE);

        let foreground = GetForegroundWindow();
        let our_thread = GetCurrentThreadId();
        let foreground_thread = GetWindowThreadProcessId(foreground, None);

        // Joining the foreground window's input queue makes the focus
        // transfer legal. The two threads must be different.
        let joined = foreground_thread != 0
            && foreground_thread != our_thread
            && AttachThreadInput(our_thread, foreground_thread, true).as_bool();

        let _ = BringWindowToTop(window);
        let _ = SetActiveWindow(window);
        let accepted = SetForegroundWindow(window).as_bool();

        if joined {
            let _ = AttachThreadInput(our_thread, foreground_thread, false);
        }

        accepted
    }
}

/// Take the keyboard focus, using the shortcut Windows allows.
///
/// A process that has just received an ALT key press is permitted to bring
/// its own window to the front, whatever else is in front. The key is
/// pressed and released without the user seeing anything.
#[cfg(windows)]
pub fn focus_window_with_permission(handle: u64) -> bool {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Input::KeyboardAndMouse::{keybd_event, KEYEVENTF_KEYUP, VK_MENU};
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, SetForegroundWindow};

    unsafe {
        // The key press grants the foreground permission; the key release
        // undoes it.
        keybd_event(VK_MENU.0 as u8, 0, Default::default(), 0);
        let accepted = SetForegroundWindow(HWND(handle as *mut core::ffi::c_void)).as_bool();
        keybd_event(VK_MENU.0 as u8, 0, KEYEVENTF_KEYUP, 0);        accepted || GetForegroundWindow() == HWND(handle as *mut core::ffi::c_void)
    }
}

/// The handle of the window that holds the keyboard focus right now.
///
/// The launcher records this when it opens, so an action that types on the
/// user's behalf knows which application to give the focus back to.
#[cfg(windows)]
pub fn foreground_window() -> Option<u64> {
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

    let handle = unsafe { GetForegroundWindow() };
    if handle.0.is_null() {
        None
    } else {
        Some(handle.0 as u64)
    }
}

/// The handle of the window that holds the keyboard focus right now.
#[cfg(not(windows))]
pub fn foreground_window() -> Option<u64> {
    None
}

/// Bring a window to the front and give it the keyboard focus.
#[cfg(not(windows))]
pub fn focus_window(_handle: u64) -> bool {
    false
}

/// Take the keyboard focus, using the shortcut Windows allows.
#[cfg(not(windows))]
pub fn focus_window_with_permission(_handle: u64) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enumerating_windows_does_not_fail() {
        // The list may legitimately be empty in a headless session; the call
        // itself must not fail.
        let result = list_windows();
        assert!(result.is_ok());
        for window in result.unwrap() {
            assert!(!window.title.trim().is_empty());
        }
    }
}
