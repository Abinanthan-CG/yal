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
