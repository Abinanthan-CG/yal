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
    /// Fill the top left quarter of the screen.
    TopLeftQuarter,
    /// Fill the top right quarter of the screen.
    TopRightQuarter,
    /// Fill the bottom left quarter of the screen.
    BottomLeftQuarter,
    /// Fill the bottom right quarter of the screen.
    BottomRightQuarter,
    /// Fill the left third of the screen.
    LeftThird,
    /// Fill the middle third of the screen.
    CenterThird,
    /// Fill the right third of the screen.
    RightThird,
    /// Centre the window where it is, unchanged in size.
    Center,
    /// Fill the screen without changing the z-order.
    Fullscreen,
}

/// Carry out a size-and-position action.
/// The rectangle a size-and-position action puts a window in.
///
/// The area is the work rectangle `(left, top, width, height)`; the window's
/// own size matters only to `Center`, which moves the window whole. Pure so
/// the shapes can be tested without a window. Division remainders stay on
/// the far edge, so the inner edge of every slice is exact and no pixel is
/// orphaned between neighbours.
fn placed_rect(
    action: WindowAction,
    left: i32,
    top: i32,
    width: i32,
    height: i32,
    window_width: i32,
    window_height: i32,
) -> (i32, i32, i32, i32) {
    let half_width = width / 2;
    let half_height = height / 2;
    let third_width = width / 3;
    match action {
        WindowAction::LeftHalf => (left, top, half_width, height),
        WindowAction::RightHalf => (left + half_width, top, width - half_width, height),
        WindowAction::TopHalf => (left, top, width, half_height),
        WindowAction::BottomHalf => (left, top + half_height, width, height - half_height),
        WindowAction::TopLeftQuarter => (left, top, half_width, half_height),
        WindowAction::TopRightQuarter => (left + half_width, top, width - half_width, half_height),
        WindowAction::BottomLeftQuarter => (left, top + half_height, half_width, height - half_height),
        WindowAction::BottomRightQuarter => (
            left + half_width,
            top + half_height,
            width - half_width,
            height - half_height,
        ),
        WindowAction::LeftThird => (left, top, third_width, height),
        WindowAction::CenterThird => (left + third_width, top, third_width, height),
        WindowAction::RightThird => {
            (left + 2 * third_width, top, width - 2 * third_width, height)
        }
        WindowAction::Center => {
            // A window the size of the work area, or larger, sits where it
            // is: a negative centering offset would walk it off the screen.
            let offset_x = ((width - window_width) / 2).max(0);
            let offset_y = ((height - window_height) / 2).max(0);
            (left + offset_x, top + offset_y, window_width, window_height)
        }
        _ => unreachable!(
            "placed_rect is only called for size-and-position actions"
        ),
    }
}

#[cfg(windows)]
fn perform_place(handle: u64, action: WindowAction) -> Result<(), ControlError> {
    // The work area is the part of the monitor the user can use, which
    // leaves out the taskbar. The window is measured against the monitor
    // it mostly sits on, not always the primary one.
    let (left, top, width, height) = {
        use windows::Win32::Foundation::{HWND, RECT};
        use windows::Win32::Graphics::Gdi::{
            GetMonitorInfoW, MonitorFromWindow, MONITORINFO,
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

    // Centre needs the window's own size; the others ignore it.
    let (window_width, window_height) = match action {
        WindowAction::Center => {
            let (_, _, w, h) = rect(handle)?;
            (w, h)
        }
        _ => (0, 0),
    };
    let (x, y, w, h) = placed_rect(action, left, top, width, height, window_width, window_height);
    place(handle, Some(x), Some(y), Some(w), Some(h))
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
            | WindowAction::TopLeftQuarter
            | WindowAction::TopRightQuarter
            | WindowAction::BottomLeftQuarter
            | WindowAction::BottomRightQuarter
            | WindowAction::LeftThird
            | WindowAction::CenterThird
            | WindowAction::RightThird
            | WindowAction::Center
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
/// Pin a window above every other window, whatever rank they hold.
///
/// A launcher window that hides and shows again can come back without its
/// topmost rank — the window manager drops the flag when the window is
/// removed from the screen — and then it rises behind a maximised
/// application, exactly where a launcher is useless. The pin is re-issued
/// with every show; it claims no focus, which the caller arranges.
#[cfg(windows)]
pub fn pin_above_everything(handle: u64) -> bool {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SET_WINDOW_POS_FLAGS,
    };

    // The topmost band, expressed as the window the position is relative
    // to; the banner is the documented constant for "above everything".
    let topmost = HWND(-1isize as *mut core::ffi::c_void);
    unsafe {
        SetWindowPos(
            HWND(handle as *mut core::ffi::c_void),
            Some(topmost),
            0,
            0,
            0,
            0,
            SET_WINDOW_POS_FLAGS(SWP_NOMOVE.0 | SWP_NOSIZE.0 | SWP_SHOWWINDOW.0),
        )
        .is_ok()
    }
}

/// Pin a window above every other window.
#[cfg(not(windows))]
pub fn pin_above_everything(_handle: u64) -> bool {
    false
}

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
        action: WindowAction::TopLeftQuarter,
        title: "Move to top left corner",
        keywords: &["quarter", "corner", "top left"],
    },
    WindowMenuItem {
        action: WindowAction::TopRightQuarter,
        title: "Move to top right corner",
        keywords: &["quarter", "corner", "top right"],
    },
    WindowMenuItem {
        action: WindowAction::BottomLeftQuarter,
        title: "Move to bottom left corner",
        keywords: &["quarter", "corner", "bottom left"],
    },
    WindowMenuItem {
        action: WindowAction::BottomRightQuarter,
        title: "Move to bottom right corner",
        keywords: &["quarter", "corner", "bottom right"],
    },
    WindowMenuItem {
        action: WindowAction::LeftThird,
        title: "Move to left third",
        keywords: &["third", "left"],
    },
    WindowMenuItem {
        action: WindowAction::CenterThird,
        title: "Move to middle third",
        keywords: &["third", "middle", "centre", "center"],
    },
    WindowMenuItem {
        action: WindowAction::RightThird,
        title: "Move to right third",
        keywords: &["third", "right"],
    },
    WindowMenuItem {
        action: WindowAction::Center,
        title: "Centre the window",
        keywords: &["center", "centre", "middle", "distribute"],
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
            WindowAction::TopLeftQuarter,
            WindowAction::TopRightQuarter,
            WindowAction::BottomLeftQuarter,
            WindowAction::BottomRightQuarter,
            WindowAction::LeftThird,
            WindowAction::CenterThird,
            WindowAction::RightThird,
            WindowAction::Center,
            WindowAction::Fullscreen,
        ] {
            assert!(WINDOW_MENU.iter().any(|menu| menu.action == action));
        }
    }

    const WORK: (i32, i32, i32, i32) = (0, 0, 1920, 1080);

    #[test]
    fn halves_and_thirds_tile_the_screen_without_gaps() {
        // Neighbours share an inner edge exactly, and the last slice
        // carries the division remainder, so nothing is orphaned.
        let (x, y, w, h) = placed_rect(
            WindowAction::LeftHalf,
            WORK.0,
            WORK.1,
            WORK.2,
            WORK.3,
            0,
            0,
        );
        assert_eq!((x, y, w, h), (0, 0, 960, 1080));
        let (x, _, w, _) = placed_rect(
            WindowAction::RightHalf,
            WORK.0,
            WORK.1,
            WORK.2,
            WORK.3,
            0,
            0,
        );
        assert_eq!((x, w), (960, 960));
        let (x, _, w, _) = placed_rect(
            WindowAction::LeftThird,
            WORK.0,
            WORK.1,
            WORK.2,
            WORK.3,
            0,
            0,
        );
        assert_eq!((x, w), (0, 640));
        let (x, _, w, _) = placed_rect(
            WindowAction::CenterThird,
            WORK.0,
            WORK.1,
            WORK.2,
            WORK.3,
            0,
            0,
        );
        assert_eq!((x, w), (640, 640));
        let (x, _, w, _) = placed_rect(
            WindowAction::RightThird,
            WORK.0,
            WORK.1,
            WORK.2,
            WORK.3,
            0,
            0,
        );
        assert_eq!((x, w), (1280, 640));
    }

    #[test]
    fn quarters_split_both_axes() {
        let (x, y, w, h) = placed_rect(
            WindowAction::TopLeftQuarter,
            WORK.0,
            WORK.1,
            WORK.2,
            WORK.3,
            0,
            0,
        );
        assert_eq!((x, y, w, h), (0, 0, 960, 540));
        let (x, y, w, h) = placed_rect(
            WindowAction::BottomRightQuarter,
            WORK.0,
            WORK.1,
            WORK.2,
            WORK.3,
            0,
            0,
        );
        assert_eq!((x, y, w, h), (960, 540, 960, 540));
    }

    #[test]
    fn centring_keeps_the_window_whole_and_on_screen() {
        // A smaller window sits in the middle.
        let (x, y, w, h) = placed_rect(
            WindowAction::Center,
            WORK.0,
            WORK.1,
            WORK.2,
            WORK.3,
            800,
            600,
        );
        assert_eq!((x, y, w, h), (560, 240, 800, 600));
        // A window larger than the work area does not walk off the edge.
        let (x, y, w, h) = placed_rect(
            WindowAction::Center,
            0,
            0,
            1920,
            1080,
            2400,
            1200,
        );
        assert_eq!((x, y), (0, 0));
        assert_eq!((w, h), (2400, 1200));
    }

    #[test]
    fn odd_widths_leave_the_remainder_on_the_far_edge() {
        // A 1901-pixel screen splits into 950 and 951: the boundary is at
        // 950 either way, and the right slice absorbs the spare pixel.
        let (_, _, lw, _) = placed_rect(WindowAction::LeftHalf, 0, 0, 1901, 1080, 0, 0);
        let (rx, _, rw, _) = placed_rect(WindowAction::RightHalf, 0, 0, 1901, 1080, 0, 0);
        assert_eq!(lw + rw, 1901);
        assert_eq!(lw, rx);
        let (_, _, fw, _) = placed_rect(WindowAction::LeftThird, 0, 0, 1901, 1080, 0, 0);
        let (_, _, mw, _) = placed_rect(WindowAction::CenterThird, 0, 0, 1901, 1080, 0, 0);
        let (r3x, _, r3w, _) = placed_rect(WindowAction::RightThird, 0, 0, 1901, 1080, 0, 0);
        assert_eq!(fw + mw + r3w, 1901);
        assert_eq!(r3x, 2 * (1901 / 3));
    }
}
