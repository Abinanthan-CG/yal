//! Acting on individual windows: focus, close, minimise, maximise, restore,
//! move and resize.
//!
//! The window list is a read-only inventory; these functions are the part
//! that does something to the window it names.

/// A failure while acting on a window.
#[derive(Debug, thiserror::Error)]
pub enum ControlError {
    #[error("the window is gone")]
    Gone,

    #[error("the action was refused: {0}")]
    Failed(String),
}

/// What can be done to a window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowAction {
    /// Close the window.
    Close,
    /// Minimise it.
    Minimize,
    /// Maximise it.
    Maximize,
    /// Put it back where it was.
    Restore,
    /// Bring it to the front and give it the keyboard.
    Focus,
    /// Fill the left half of the screen.
    LeftHalf,
    /// Fill the right half of the screen.
    RightHalf,
    /// Fill the top half of the screen.
    TopHalf,
    /// Fill the bottom half of the screen.
    BottomHalf,
    /// Fill the screen without changing the z-order.
    Fullscreen,
}

/// Carry out a size-and-position action.
#[cfg(windows)]
fn perform_place(handle: u64, action: WindowAction) -> Result<(), ControlError> {
    // The work area is the part of the monitor the user can use, which
    // leaves out the taskbar. The window is measured against the monitor
    // it mostly sits on, not always the primary one.
    let (left, top, width, height) = {
        use windows::Win32::Foundation::{HWND, RECT};
        use windows::Win32::Graphics::Gdi::{
            GetMonitorInfoW, MonitorFromWindow, HMONITOR, MONITORINFO,
            MONITOR_DEFAULTTONEAREST,
        };

        let window = HWND(handle as *mut core::ffi::c_void);
        let monitor = unsafe { MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST) };
        let mut info = MONITORINFO {
            cbSize: core::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
            let work: &RECT = &info.rcWork;
            (
                work.left,
                work.top,
                work.right - work.left,
                work.bottom - work.top,
            )
        } else {
            (0, 0, 1920, 1080)
        }
    };

    match action {
        WindowAction::LeftHalf => place(handle, Some(left), Some(top), Some(width / 2), Some(height)),
        WindowAction::RightHalf => {
            place(handle, Some(left + width / 2), Some(top), Some(width - width / 2), Some(height))
        }
        WindowAction::TopHalf => place(handle, Some(left), Some(top), Some(width), Some(height / 2)),
        WindowAction::BottomHalf => {
            place(handle, Some(left), Some(top + height / 2), Some(width), Some(height - height / 2))
        }
        WindowAction::Fullscreen => place(handle, Some(left), Some(top), Some(width), Some(height)),
        _ => unreachable!("perform_place is only called for size-and-position actions"),
    }
}

/// Carry out an action on a window.
#[cfg(windows)]
pub fn perform(handle: u64, action: WindowAction) -> Result<(), ControlError> {
    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        PostMessageW, SetForegroundWindow, ShowWindow, SW_MAXIMIZE, SW_MINIMIZE, SW_RESTORE,
        SW_SHOW, WM_CLOSE,
    };

    let window = HWND(handle as *mut core::ffi::c_void);
    unsafe {
        match action {
            WindowAction::Close => {
                PostMessageW(Some(window), WM_CLOSE, WPARAM(0), LPARAM(0))
                    .map_err(|err| ControlError::Failed(err.to_string()))?;
            }
            // The window manager's answer is only interesting when it was
            // asked to change visibility; a failure is reported by `Close`
            // and ignored for the others, whose failure is silent.
            WindowAction::Minimize => {
                let _ = ShowWindow(window, SW_MINIMIZE);
            }
            WindowAction::Maximize => {
                let _ = ShowWindow(window, SW_MAXIMIZE);
            }
            WindowAction::Restore => {
                let _ = ShowWindow(window, SW_RESTORE);
            }
            WindowAction::Focus => {
                // Showing alone does not take the keyboard; the foreground
                // system call does. It is refused when neither the caller
                // nor the user set it up, which is reported as an error
                // rather than silently ignored.
                let _ = ShowWindow(window, SW_SHOW);
                SetForegroundWindow(window)
                    .ok()
                    .map_err(|_| ControlError::Failed(
                        "the system refused to give the window the foreground".into(),
                    ))?;
            }
            WindowAction::LeftHalf
            | WindowAction::RightHalf
            | WindowAction::TopHalf
            | WindowAction::BottomHalf
            | WindowAction::Fullscreen => {
                // A window that is minimised cannot be sized; restore first
                // so the geometry applies to a real window.
                let _ = ShowWindow(window, SW_RESTORE);
                return perform_place(handle, action);
            }
        }
        Ok(())
    }
}

/// Act on a window on a host without window handles.
#[cfg(not(windows))]
pub fn perform(_handle: u64, _action: WindowAction) -> Result<(), ControlError> {
    Err(ControlError::Failed(
        "window control is not implemented on this platform".into(),
    ))
}

/// Read a window's position and size, for tests and tools.
#[cfg(windows)]
pub fn rect(handle: u64) -> Result<(i32, i32, i32, i32), ControlError> {
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;

    let window = HWND(handle as *mut core::ffi::c_void);
    let mut r = RECT::default();
    unsafe { GetWindowRect(window, &mut r) }.map_err(|_| ControlError::Gone)?;
    Ok((r.left, r.top, r.right - r.left, r.bottom - r.top))
}

/// Read a window's position and size on a host without window handles.
#[cfg(not(windows))]
pub fn rect(_handle: u64) -> Result<(i32, i32, i32, i32), ControlError> {
    Err(ControlError::Gone)
}

/// Move a window, optionally resizing it.
///
/// `x` and `y` of `None` leave the position alone. Width and height of
/// `None` leave the size alone.
#[cfg(windows)]
pub fn place(
    handle: u64,
    x: Option<i32>,
    y: Option<i32>,
    width: Option<i32>,
    height: Option<i32>,
) -> Result<(), ControlError> {
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowRect, SetWindowPos, SET_WINDOW_POS_FLAGS, SWP_NOZORDER, SWP_NOMOVE, SWP_NOSIZE,
    };

    let window = HWND(handle as *mut core::ffi::c_void);
    unsafe {
        let mut rect = RECT::default();
        GetWindowRect(window, &mut rect).map_err(|_| ControlError::Gone)?;

        let (this_x, this_y) = (rect.left, rect.top);
        let (this_w, this_h) = (rect.right - rect.left, rect.bottom - rect.top);

        let x = x.unwrap_or(this_x);
        let y = y.unwrap_or(this_y);
        let width = width.unwrap_or(this_w);
        let height = height.unwrap_or(this_h);

        if width <= 0 || height <= 0 {
            return Err(ControlError::Failed(
                "the window was asked for a size it cannot hold".into(),
            ));
        }

        // Only the parts that did not change are dropped from the request,
        // so a pure move does not disturb the size and vice versa.
        let mut flags = SET_WINDOW_POS_FLAGS(SWP_NOZORDER.0);
        if x == this_x && y == this_y {
            flags.0 |= SWP_NOMOVE.0;
        }
        if width == this_w && height == this_h {
            flags.0 |= SWP_NOSIZE.0;
        }
        SetWindowPos(window, Some(HWND::default()), x, y, width, height, flags)
            .map_err(|err| ControlError::Failed(err.to_string()))
    }
}

/// Move a window on a host without window handles.
#[cfg(not(windows))]
pub fn place(
    _handle: u64,
    _x: Option<i32>,
    _y: Option<i32>,
    _width: Option<i32>,
    _height: Option<i32>,
) -> Result<(), ControlError> {
    Err(ControlError::Failed(
        "window control is not implemented on this platform".into(),
    ))
}

/// A window action the palette offers, with the words that find it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowMenuItem {
    pub action: WindowAction,
    pub title: &'static str,
    pub keywords: &'static [&'static str],
}

/// The window actions offered as menu items for a selected window.
pub const WINDOW_MENU: &[WindowMenuItem] = &[
    WindowMenuItem {
        action: WindowAction::Focus,
        title: "Bring to front",
        keywords: &["focus", "front", "activate", "switch"],
    },
    WindowMenuItem {
        action: WindowAction::Minimize,
        title: "Minimize",
        keywords: &["minimize", "hide", "taskbar"],
    },
    WindowMenuItem {
        action: WindowAction::Maximize,
        title: "Maximize",
        keywords: &["maximize", "fullscreen", "grow"],
    },
    WindowMenuItem {
        action: WindowAction::LeftHalf,
        title: "Move to left half",
        keywords: &["left", "half", "split", "side by side"],
    },
    WindowMenuItem {
        action: WindowAction::RightHalf,
        title: "Move to right half",
        keywords: &["right", "half", "split", "side by side"],
    },
    WindowMenuItem {
        action: WindowAction::TopHalf,
        title: "Move to top half",
        keywords: &["top", "upper", "half"],
    },
    WindowMenuItem {
        action: WindowAction::BottomHalf,
        title: "Move to bottom half",
        keywords: &["bottom", "lower", "half"],
    },
    WindowMenuItem {
        action: WindowAction::Fullscreen,
        title: "Fill the screen",
        keywords: &["fill", "full"],
    },
    WindowMenuItem {
        action: WindowAction::Restore,
        title: "Restore",
        keywords: &["restore", "back", "original"],
    },
    WindowMenuItem {
        action: WindowAction::Close,
        title: "Close",
        keywords: &["close", "quit", "exit"],
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_menu_covers_the_window_actions() {
        for menu in WINDOW_MENU {
            assert!(!menu.title.is_empty());
            assert!(!menu.keywords.is_empty());
        }
        // Every action the palette can carry appears in the menu.
        for action in [
            WindowAction::Close,
            WindowAction::Minimize,
            WindowAction::Maximize,
            WindowAction::Restore,
            WindowAction::Focus,
            WindowAction::LeftHalf,
            WindowAction::RightHalf,
            WindowAction::TopHalf,
            WindowAction::BottomHalf,
            WindowAction::Fullscreen,
        ] {
            assert!(WINDOW_MENU.iter().any(|menu| menu.action == action));
        }
    }
}
