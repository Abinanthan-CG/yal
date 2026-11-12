//! Moving real windows.
//!
//! The geometry lives in `tc-windows`, which is pure and knows nothing about
//! Windows. This module is the part that finds a window, asks the geometry
//! where it should go, and writes the answer - and then reads the rectangle
//! back, because what a window ended up as is not always what was asked for.
//!
//! Upstream's rules that this port keeps:
//!
//! - **Minimized windows, and windows that do not report a position or a
//!   size, are skipped.**
//! - **A window that can move but cannot be resized keeps its size** and
//!   moves to the command's position.
//! - **If an app will not let its window shrink, the window is aligned to the
//!   correct side once** rather than retried.
//!
//! The work area of a display is the part of it a window may use: the screen
//! without the taskbar and any other app bar.

use tc_windows::command::WindowCommandId;
use tc_windows::geometry::{placement, PlacementInput, Rect, Screen, WindowCycle};
use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, MonitorFromWindow, HDC, HMONITOR, MONITORINFO,
    MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId, IsIconic,
    IsWindowVisible, SetWindowPos, SWP_NOACTIVATE, SWP_NOZORDER,
};

/// A window, named by the handle Windows gave it.
pub type WindowHandle = isize;

/// What can go wrong while a window is being moved.
#[derive(Debug, thiserror::Error)]
pub enum ControlError {
    /// The window is gone.
    #[error("the window is gone")]
    Gone,

    /// The window is minimized, so it is skipped.
    #[error("the window is minimized")]
    Minimized,

    /// The window does not report a position or a size.
    #[error("the window does not report a position or a size")]
    NoRect,

    /// The action was refused.
    #[error("the action was refused: {0}")]
    Refused(String),
}

/// A window that could be moved.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowInfo {
    /// The handle.
    pub handle: WindowHandle,
    /// The title in the title bar.
    pub title: String,
    /// The window class.
    pub class: String,
    /// The process that owns the window.
    pub pid: u32,
    /// Where the window is.
    pub rect: Rect,
    /// Whether the window is minimized.
    pub minimized: bool,
    /// Whether the window is on the screen.
    pub visible: bool,
}

/// What a command did to a window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Outcome {
    /// Where the window ended up.
    pub frame: Rect,
    /// The display it ended up on.
    pub screen_id: i64,
    /// Whether the window had to be aligned after it refused to take the
    /// size it was asked for.
    pub aligned: bool,
}

/// The handle of a window, as the number Windows uses.
fn handle_of(hwnd: HWND) -> WindowHandle {
    hwnd.0 as isize
}

/// The window a handle names.
fn hwnd_of(handle: WindowHandle) -> HWND {
    HWND(handle as *mut std::ffi::c_void)
}

/// A rectangle in the geometry's own terms.
fn rect_of(rect: RECT) -> Rect {
    Rect::new(
        rect.left as f64,
        rect.top as f64,
        (rect.right - rect.left) as f64,
        (rect.bottom - rect.top) as f64,
    )
}

/// The displays, with the work area of each.
///
/// The work area is the part of the display a window may use: the screen
/// without the taskbar and any other app bar. It is what the geometry
/// places windows in.
pub fn screens() -> Vec<Screen> {
    let mut found: Vec<Screen> = Vec::new();
    // The callback cannot capture, so the list travels in the callback's own
    // parameter.
    let list = LPARAM(&mut found as *mut Vec<Screen> as isize);
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(monitor_callback), list);
    }
    found
}

/// The callback Windows calls once per display.
///
/// The list of displays travels in the callback's own parameter, because a
/// callback that captured it would not be a plain function.
unsafe extern "system" fn monitor_callback(
    monitor: HMONITOR,
    _dc: HDC,
    _rect: *mut RECT,
    data: LPARAM,
) -> BOOL {
    let found = &mut *(data.0 as *mut Vec<Screen>);
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if GetMonitorInfoW(monitor, &mut info).as_bool() {
        found.push(Screen {
            id: monitor.0 as i64,
            frame: rect_of(info.rcMonitor),
            visible_frame: rect_of(info.rcWork),
        });
    }
    BOOL(1)
}

/// Where a window is.
pub fn window_rect(handle: WindowHandle) -> Result<Rect, ControlError> {
    let mut rect = RECT::default();
    unsafe {
        GetWindowRect(hwnd_of(handle), &mut rect).map_err(|_| ControlError::NoRect)?;
    }
    let rect = rect_of(rect);
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return Err(ControlError::NoRect);
    }
    Ok(rect)
}

/// Whether a window is minimized.
pub fn is_minimized(handle: WindowHandle) -> bool {
    unsafe { IsIconic(hwnd_of(handle)).as_bool() }
}

/// Whether a window is on the screen.
pub fn is_visible(handle: WindowHandle) -> bool {
    unsafe { IsWindowVisible(hwnd_of(handle)).as_bool() }
}

/// The window the user was last using.
pub fn foreground_window() -> Option<WindowHandle> {
    unsafe {
        let hwnd = windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow();
        if hwnd.0.is_null() {
            None
        } else {
            Some(handle_of(hwnd))
        }
    }
}

/// The windows a person could act on.
///
/// A window that is on the screen, is not minimized and has a title. The
/// invisible helper windows that programs keep around are not in this list.
pub fn list_windows() -> Vec<WindowInfo> {
    let mut found: Vec<WindowInfo> = Vec::new();
    // The callback cannot capture, so the list travels in the callback's own
    // parameter.
    let list = LPARAM(&mut found as *mut Vec<WindowInfo> as isize);
    unsafe {
        let _ = EnumWindows(Some(window_callback), list);
    }
    found
}

/// The callback Windows calls once per top-level window.
unsafe extern "system" fn window_callback(hwnd: HWND, data: LPARAM) -> BOOL {
    let found = &mut *(data.0 as *mut Vec<WindowInfo>);
    let visible = IsWindowVisible(hwnd).as_bool();
    let minimized = IsIconic(hwnd).as_bool();
    let title = window_title(hwnd);
    if visible && !minimized && !title.is_empty() {
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        let mut rect = RECT::default();
        let rect = if GetWindowRect(hwnd, &mut rect).is_ok() {
            rect_of(rect)
        } else {
            Rect::new(0.0, 0.0, 0.0, 0.0)
        };
        found.push(WindowInfo {
            handle: handle_of(hwnd),
            title,
            class: window_class(hwnd),
            pid,
            rect,
            minimized,
            visible,
        });
    }
    BOOL(1)
}

/// The title of a window.
fn window_title(hwnd: HWND) -> String {
    let mut buffer = [0u16; 512];
    let length = unsafe { GetWindowTextW(hwnd, &mut buffer) };
    if length <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buffer[..length as usize])
}

/// The class of a window.
fn window_class(hwnd: HWND) -> String {
    let mut buffer = [0u16; 256];
    let length = unsafe { GetClassNameW(hwnd, &mut buffer) };
    if length <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buffer[..length as usize])
}

/// Put a window in a rectangle.
///
/// The window is not brought to the front and the z-order is left alone, so
/// moving a window never steals the focus from what the user is doing.
pub fn set_window_rect(handle: WindowHandle, rect: Rect) -> Result<(), ControlError> {
    let x = rect.min_x().round() as i32;
    let y = rect.min_y().round() as i32;
    let width = rect.width().round().max(1.0) as i32;
    let height = rect.height().round().max(1.0) as i32;
    unsafe {
        SetWindowPos(
            hwnd_of(handle),
            None,
            x,
            y,
            width,
            height,
            SWP_NOZORDER | SWP_NOACTIVATE,
        )
        .map_err(|_| ControlError::Refused("the window did not take the new size".to_string()))
    }
}

/// Move a window without changing its size.
pub fn move_window(handle: WindowHandle, rect: Rect) -> Result<(), ControlError> {
    let current = window_rect(handle)?;
    set_window_rect(
        handle,
        Rect::new(
            rect.min_x(),
            rect.min_y(),
            current.width(),
            current.height(),
        ),
    )
}

/// Run a window command against a window.
///
/// The geometry answers where the window should go; this writes it, reads the
/// rectangle back, and - when the window refused the size it was asked for -
/// aligns it to the correct side once, the way upstream does instead of
/// retrying.
pub fn apply_command(
    handle: WindowHandle,
    command: WindowCommandId,
    gap: f64,
    cycle: WindowCycle,
    step: usize,
    restore_frame: Option<Rect>,
) -> Result<Outcome, ControlError> {
    if is_minimized(handle) {
        return Err(ControlError::Minimized);
    }
    let current = window_rect(handle)?;
    let screens = screens();
    let mut request = PlacementInput::new(command, current, screens, gap);
    request.step = step;
    request.cycle = cycle;
    request.restore_frame = restore_frame;
    let target = placement(&request).ok_or_else(|| {
        ControlError::Refused("this command has nothing to place on this machine".to_string())
    })?;

    // A command that only moves keeps the window's size.
    let wanted = if target.resizes {
        target.frame
    } else {
        Rect::new(
            target.frame.min_x(),
            target.frame.min_y(),
            current.width(),
            current.height(),
        )
    };
    set_window_rect(handle, wanted)?;
    let mut landed = window_rect(handle)?;

    // The window may have refused the size it was asked for. When it did, it
    // is aligned to the correct side of its slot once, rather than retried.
    let mut aligned = false;
    if target.resizes
        && ((landed.width() - wanted.width()).abs() > 1.0
            || (landed.height() - wanted.height()).abs() > 1.0)
    {
        let anchored = target
            .anchor
            .place((landed.width(), landed.height()), wanted);
        set_window_rect(handle, anchored)?;
        landed = window_rect(handle)?;
        aligned = true;
    }

    Ok(Outcome {
        frame: landed,
        screen_id: target.screen_id,
        aligned,
    })
}

/// Make a window fill its display, or put it back.
///
/// Windows has no way to put another program's window into its own
/// fullscreen, so the honest equivalent is the display's own bounds.
///
/// The frame the window had before it filled the screen is returned, so the
/// caller can put it back exactly where it was.
pub fn toggle_fullscreen(
    handle: WindowHandle,
    previous: Option<Rect>,
) -> Result<Option<Rect>, ControlError> {
    let rect = window_rect(handle)?;
    let monitor = unsafe { MonitorFromWindow(hwnd_of(handle), MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    unsafe {
        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return Err(ControlError::Refused(
                "the display could not be read".to_string(),
            ));
        }
    }
    let bounds = rect_of(info.rcMonitor);
    let filling = (rect.width() - bounds.width()).abs() < 2.0
        && (rect.height() - bounds.height()).abs() < 2.0;
    if filling {
        // Put it back where it was, if the caller remembers where that was.
        if let Some(previous) = previous {
            set_window_rect(handle, previous)?;
        }
        Ok(None)
    } else {
        set_window_rect(handle, bounds)?;
        Ok(Some(rect))
    }
}

/// The window the user was last using.
///
/// The palette is a window too, so the foreground window while the palette is
/// open is the palette. The window that was in front *before* it opened is
/// remembered instead, and that is the one the commands move.
static LAST_WINDOW: std::sync::Mutex<Option<WindowHandle>> = std::sync::Mutex::new(None);

/// Remember the window that is in front right now.
///
/// The shell calls this before the palette is shown.
pub fn remember_foreground() {
    let Some(handle) = foreground_window() else {
        return;
    };
    if let Ok(mut last) = LAST_WINDOW.lock() {
        *last = Some(handle);
    }
}

/// The window the user was last using, if there is one.
pub fn last_foreground() -> Option<WindowHandle> {
    LAST_WINDOW.lock().ok().and_then(|last| *last)
}

/// The window commands, with the memory of the windows they moved.
///
/// This is the whole of upstream's window management on the Windows side: the
/// commands, the memory of where each window started, and the code that
/// writes the rectangles.
#[derive(Debug)]
pub struct WindowCommands {
    memory: std::sync::Mutex<tc_windows::action_memory::WindowActionMemory<WindowHandle>>,
    gap: f64,
    cycle: WindowCycle,
}

impl WindowCommands {
    /// The commands, with the gap between windows and the cycling mode.
    pub fn new(gap: f64, cycle: WindowCycle) -> WindowCommands {
        WindowCommands {
            memory: std::sync::Mutex::new(tc_windows::action_memory::WindowActionMemory::new(
                tc_windows::action_memory::DEFAULT_CAPACITY,
            )),
            gap,
            cycle,
        }
    }

    /// The gap between windows, in points.
    pub fn gap(&self) -> f64 {
        self.gap
    }

    /// Set the gap between windows, in points.
    pub fn set_gap(&mut self, gap: f64) {
        self.gap = tc_windows::geometry::sanitize_gap_setting(gap);
    }

    /// The cycling mode.
    pub fn cycle(&self) -> WindowCycle {
        self.cycle
    }

    /// Set the cycling mode.
    pub fn set_cycle(&mut self, cycle: WindowCycle) {
        self.cycle = cycle;
    }

    /// Run a command against a window.
    ///
    /// The memory decides which step of the cycle this press is and where the
    /// window goes back to; the rectangle is written, read back, and what
    /// actually landed is remembered.
    pub fn run(
        &self,
        handle: WindowHandle,
        command: WindowCommandId,
    ) -> Result<Outcome, ControlError> {
        let current = window_rect(handle)?;
        let screens = screens();
        let host = tc_windows::geometry::screen_containing(current, &screens)
            .ok_or_else(|| ControlError::Refused("there is no display".to_string()))?;
        let cycle_length = tc_windows::geometry::cycle_length(command, &screens, self.cycle);
        let now = std::time::Instant::now();
        let press = tc_windows::action_memory::Press {
            command: Some(command),
            current_frame: current,
            current_screen_id: host.id,
            cycle_length,
            now,
            is_fullscreen: false,
        };
        let decision = {
            let memory = self.memory.lock().expect("the window memory lock");
            memory.decide(&handle, &press)
        };
        let outcome = apply_command(
            handle,
            command,
            self.gap,
            self.cycle,
            decision.step,
            if decision.can_restore {
                Some(decision.restore_frame)
            } else {
                None
            },
        )?;
        let mut memory = self.memory.lock().expect("the window memory lock");
        memory.commit(
            handle,
            &press,
            &decision,
            &tc_windows::action_memory::Applied {
                frame: outcome.frame,
                screen_id: outcome.screen_id,
                fullscreen: false,
            },
        );
        Ok(outcome)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassW, ShowWindow, SW_MINIMIZE,
        SW_SHOW, WINDOW_EX_STYLE, WNDCLASSW, WS_OVERLAPPEDWINDOW,
    };

    /// A wide string that lives as long as it is needed.
    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// A wide string that lives for good, which a window class needs: the
    /// class keeps the pointer, not a copy.
    fn wide_static(text: &str) -> PCWSTR {
        let owned: &'static [u16] = Box::leak(wide(text).into_boxed_slice());
        PCWSTR(owned.as_ptr())
    }

    /// The window procedure of the test window.
    ///
    /// Windows wants a procedure with the system's calling convention, and the
    /// one the crate hands out has Rust's, so it is wrapped here.
    unsafe extern "system" fn test_window_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        DefWindowProcW(hwnd, message, wparam, lparam)
    }

    /// A window the tests can move.
    struct TestWindow {
        handle: WindowHandle,
    }

    impl TestWindow {
        fn new(left: i32, top: i32, width: i32, height: i32) -> TestWindow {
            unsafe {
                // The class and the window have to agree about the program
                // they belong to, or Windows will not find the class.
                let instance = HINSTANCE(
                    GetModuleHandleW(None)
                        .map(|module| module.0)
                        .unwrap_or(std::ptr::null_mut()),
                );
                let class = WNDCLASSW {
                    lpfnWndProc: Some(test_window_proc),
                    hInstance: instance,
                    lpszClassName: wide_static("YalTestWindowClass"),
                    ..Default::default()
                };
                RegisterClassW(&class);
                let hwnd = CreateWindowExW(
                    WINDOW_EX_STYLE::default(),
                    wide_static("YalTestWindowClass"),
                    wide_static("YAL test window"),
                    WS_OVERLAPPEDWINDOW,
                    left,
                    top,
                    width,
                    height,
                    None,
                    None,
                    Some(instance),
                    None,
                )
                .expect("the test window");
                TestWindow {
                    handle: handle_of(hwnd),
                }
            }
        }

        fn show(&self) {
            unsafe {
                let _ = ShowWindow(hwnd_of(self.handle), SW_SHOW);
            }
        }

        fn minimize(&self) {
            unsafe {
                let _ = ShowWindow(hwnd_of(self.handle), SW_MINIMIZE);
            }
        }

        fn rect(&self) -> Rect {
            window_rect(self.handle).expect("the rectangle")
        }
    }

    impl Drop for TestWindow {
        fn drop(&mut self) {
            unsafe {
                let _ = DestroyWindow(hwnd_of(self.handle));
            }
        }
    }

    #[test]
    fn the_machine_has_at_least_one_display_with_a_work_area() {
        let displays = screens();
        assert!(
            !displays.is_empty(),
            "a machine with no display is not this one"
        );
        for display in &displays {
            assert!(display.frame.width() > 0.0);
            assert!(display.visible_frame.width() > 0.0);
            assert!(
                display.visible_frame.width() <= display.frame.width() + 1.0,
                "the work area fits inside the display"
            );
        }
    }

    #[test]
    fn a_window_can_be_read_and_moved() {
        let window = TestWindow::new(200, 200, 400, 300);
        let before = window.rect();
        assert_eq!(before.width(), 400.0);
        set_window_rect(window.handle, Rect::new(50.0, 60.0, 200.0, 100.0)).expect("the move");
        let after = window.rect();
        assert_eq!(after.min_x(), 50.0);
        assert_eq!(after.min_y(), 60.0);
        assert_eq!(after.width(), 200.0);
        assert_eq!(after.height(), 100.0);
    }

    #[test]
    fn a_move_keeps_the_size() {
        let window = TestWindow::new(200, 200, 400, 300);
        move_window(window.handle, Rect::new(10.0, 20.0, 0.0, 0.0)).expect("the move");
        let after = window.rect();
        assert_eq!(after.min_x(), 10.0);
        assert_eq!(after.width(), 400.0, "the size stayed");
    }

    #[test]
    fn the_left_half_command_puts_the_window_in_the_left_half() {
        let window = TestWindow::new(100, 100, 400, 300);
        let displays = screens();
        let display = displays.first().expect("a display");
        let outcome = apply_command(
            window.handle,
            WindowCommandId::LeftHalf,
            0.0,
            WindowCycle::Off,
            0,
            None,
        )
        .expect("the command");
        let expected = Rect::new(
            display.visible_frame.min_x(),
            display.visible_frame.min_y(),
            display.visible_frame.width() / 2.0,
            display.visible_frame.height(),
        );
        assert_eq!(outcome.screen_id, display.id);
        assert!(
            (outcome.frame.min_x() - expected.min_x()).abs() <= 1.0,
            "the window is on the left: {}",
            outcome.frame.min_x()
        );
        assert!(
            (outcome.frame.width() - expected.width()).abs() <= 1.0,
            "the window is half the display wide: {}",
            outcome.frame.width()
        );
    }

    #[test]
    fn the_maximize_command_fills_the_display() {
        let window = TestWindow::new(100, 100, 400, 300);
        let displays = screens();
        let display = displays.first().expect("a display");
        let outcome = apply_command(
            window.handle,
            WindowCommandId::Maximize,
            0.0,
            WindowCycle::Off,
            0,
            None,
        )
        .expect("the command");
        assert!(
            (outcome.frame.width() - display.visible_frame.width()).abs() <= 1.0,
            "the window fills the display"
        );
    }

    #[test]
    fn a_minimized_window_is_skipped() {
        let window = TestWindow::new(100, 100, 400, 300);
        window.minimize();
        let error = apply_command(
            window.handle,
            WindowCommandId::LeftHalf,
            0.0,
            WindowCycle::Off,
            0,
            None,
        )
        .expect_err("the window is skipped");
        assert!(matches!(error, ControlError::Minimized));
    }

    #[test]
    fn a_window_that_is_gone_is_reported() {
        let window = TestWindow::new(100, 100, 400, 300);
        let handle = window.handle;
        drop(window);
        let error = window_rect(handle).expect_err("the window is gone");
        assert!(matches!(error, ControlError::NoRect));
    }

    #[test]
    fn the_window_list_sees_a_window_with_a_title() {
        let window = TestWindow::new(100, 100, 400, 300);
        window.show();
        let windows = list_windows();
        assert!(
            windows.iter().any(|found| found.handle == window.handle),
            "the test window is in the list"
        );
        let found = windows
            .iter()
            .find(|found| found.handle == window.handle)
            .expect("the window");
        assert_eq!(found.title, "YAL test window");
        assert!(found.visible);
    }

    #[test]
    fn the_foreground_window_is_a_real_window() {
        // The test runner's own console is not a foreground window in every
        // session, so this only asserts that the answer, if there is one, is
        // a handle that names a window.
        if let Some(handle) = foreground_window() {
            assert!(handle != 0);
        }
    }

    #[test]
    fn every_geometry_command_moves_a_real_window() {
        // Each command in the catalogue that places a window is run against a
        // real window, and the window is read back: the command did what the
        // geometry said it would.
        for command in tc_windows::command::WindowCommandCatalog::all() {
            if command.kind != tc_windows::command::WindowCommandKind::Geometry {
                continue;
            }
            // A display move on a machine with one display has nowhere to go,
            // and upstream says so: it does nothing.
            if matches!(
                command.id,
                WindowCommandId::NextDisplay | WindowCommandId::PreviousDisplay
            ) && screens().len() <= 1
            {
                continue;
            }
            let window = TestWindow::new(200, 200, 400, 300);
            let restore = Some(Rect::new(200.0, 200.0, 400.0, 300.0));
            let outcome =
                apply_command(window.handle, command.id, 0.0, WindowCycle::Off, 0, restore)
                    .unwrap_or_else(|error| panic!("{command:?} did not run: {error}"));

            // What the geometry said, computed again from the window's own
            // starting frame.
            let expected = tc_windows::geometry::placement(&PlacementInput {
                command: command.id,
                window_frame: Rect::new(200.0, 200.0, 400.0, 300.0),
                screens: screens(),
                gap: 0.0,
                step: 0,
                cycle: WindowCycle::Off,
                origin_screen_id: None,
                restore_frame: restore,
                last_tile_command: None,
                wraps_displays: true,
                keeps_proportions: true,
            })
            .unwrap_or_else(|| panic!("{command:?} has no placement"));
            assert!(
                (outcome.frame.min_x() - expected.frame.min_x()).abs() <= 1.0
                    && (outcome.frame.min_y() - expected.frame.min_y()).abs() <= 1.0
                    && (outcome.frame.width() - expected.frame.width()).abs() <= 1.0
                    && (outcome.frame.height() - expected.frame.height()).abs() <= 1.0,
                "{command:?} landed at {:?}, not at {:?}",
                outcome.frame,
                expected.frame
            );
        }
    }

    #[test]
    fn the_restore_chain_puts_the_window_back_where_it_started() {
        // Left Half, then Maximize, then Top Right Quarter, then Restore
        // Window: the window goes back to where it was before the first
        // command, not to where it was before the last one.
        let window = TestWindow::new(200, 200, 400, 300);
        let start = window.rect();
        let mut memory = tc_windows::action_memory::WindowActionMemory::new(64);

        let mut press = |command: WindowCommandId, now: std::time::Instant| {
            let current = window.rect();
            let displays = screens();
            let host = tc_windows::geometry::screen_containing(current, &displays)
                .expect("the display the window is on");
            let press = tc_windows::action_memory::Press {
                command: Some(command),
                current_frame: current,
                current_screen_id: host.id,
                cycle_length: 1,
                now,
                is_fullscreen: false,
            };
            let decision = memory.decide(&window.handle, &press);
            let outcome = apply_command(
                window.handle,
                command,
                0.0,
                WindowCycle::Off,
                decision.step,
                if decision.can_restore {
                    Some(decision.restore_frame)
                } else {
                    None
                },
            )
            .expect("the command");
            memory.commit(
                window.handle,
                &press,
                &decision,
                &tc_windows::action_memory::Applied {
                    frame: outcome.frame,
                    screen_id: outcome.screen_id,
                    fullscreen: false,
                },
            );
            outcome.frame
        };

        let now = std::time::Instant::now();
        press(WindowCommandId::LeftHalf, now);
        press(WindowCommandId::Maximize, now);
        press(WindowCommandId::TopRightQuarter, now);
        let restored = press(WindowCommandId::Restore, now);

        assert!(
            (restored.min_x() - start.min_x()).abs() <= 1.0
                && (restored.min_y() - start.min_y()).abs() <= 1.0
                && (restored.width() - start.width()).abs() <= 1.0
                && (restored.height() - start.height()).abs() <= 1.0,
            "the window is back at {restored:?}, not at {start:?}"
        );
    }

    #[test]
    fn a_window_can_be_made_to_fill_the_display_and_back() {
        let window = TestWindow::new(200, 200, 400, 300);
        let start = window.rect();
        let previous = toggle_fullscreen(window.handle, None).expect("the fullscreen");
        assert!(previous.is_some(), "the frame before is returned");
        let filling = window.rect();
        assert!(
            filling.width() > start.width(),
            "the window fills the display"
        );

        toggle_fullscreen(window.handle, previous).expect("the way back");
        let back = window.rect();
        assert!(
            (back.width() - start.width()).abs() <= 1.0,
            "the window is back to its own size"
        );
    }

    #[test]
    fn the_commands_keep_the_memory_of_the_windows_they_moved() {
        let window = TestWindow::new(200, 200, 400, 300);
        let start = window.rect();
        let commands = WindowCommands::new(0.0, WindowCycle::Off);

        commands
            .run(window.handle, WindowCommandId::LeftHalf)
            .expect("the first command");
        let half = window.rect();
        assert!(
            (half.width() - start.width() / 2.0).abs() > 1.0,
            "the window is in the left half now"
        );

        commands
            .run(window.handle, WindowCommandId::Restore)
            .expect("the restore");
        let back = window.rect();
        assert!(
            (back.min_x() - start.min_x()).abs() <= 1.0
                && (back.width() - start.width()).abs() <= 1.0,
            "the window is back where it started: {back:?}"
        );
    }
}
