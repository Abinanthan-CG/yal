//! The palette window.
//!
//! A borderless, always-on-top window with rounded corners, drawn by Direct2D
//! and typed into with a real Windows edit control - so the keyboard, the input
//! method, the clipboard and the selection all behave the way Windows expects.

use std::ffi::c_void;
use std::time::{Duration, Instant};

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontW, CreateSolidBrush, DeleteObject, EndPaint, GetMonitorInfoW,
    InvalidateRect, MonitorFromPoint, SetBkColor, SetTextColor, HBRUSH, HDC, HFONT, MONITORINFO,
    PAINTSTRUCT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::System::WindowsProgramming::MulDiv;
use windows::Win32::UI::Controls::{
    InitCommonControlsEx, ICC_STANDARD_CLASSES, INITCOMMONCONTROLSEX,
};
use windows::Win32::UI::HiDpi::{
    GetDpiForMonitor, GetDpiForSystem, GetDpiForWindow, SetProcessDpiAwarenessContext,
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, MDT_EFFECTIVE_DPI,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetForegroundWindow,
    GetMessageW, GetParent, GetWindowLongPtrW, GetWindowThreadProcessId, IsWindow, IsWindowVisible,
    LoadCursorW, PeekMessageW, PostMessageW, PostQuitMessage, RegisterClassExW, SendMessageW,
    SetForegroundWindow, SetWindowLongPtrW, SetWindowPos, ShowWindow, TranslateMessage,
    CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW, EN_CHANGE, ES_AUTOHSCROLL, ES_LEFT, GWLP_USERDATA,
    GWLP_WNDPROC, HTCLIENT, IDC_ARROW, MINMAXINFO, MSG, PM_REMOVE, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOZORDER, SW_HIDE, SW_SHOW, WA_INACTIVE, WINDOW_EX_STYLE, WINDOW_STYLE, WM_ACTIVATE,
    WM_CHAR, WM_COMMAND, WM_CREATE, WM_CTLCOLOREDIT, WM_DESTROY, WM_ERASEBKGND, WM_GETMINMAXINFO,
    WM_KEYDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCCALCSIZE, WM_NCHITTEST, WM_NEXTDLGCTL, WM_PAINT,
    WM_PASTE, WM_SETFONT, WM_SIZE, WNDCLASSEXW, WS_CHILD, WS_CLIPCHILDREN, WS_CLIPSIBLINGS,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP, WS_TABSTOP, WS_THICKFRAME, WS_VISIBLE,
};

use crate::event::{Handler, PaletteEvent, PaletteReply};
use crate::model::Model;
use crate::paint::Renderer;
use crate::theme::{Color, Metrics, Theme};

/// The message the search field sends when the user pressed the up arrow.
const WM_APP_UP: u32 = 0x8001;
/// The message the search field sends when the user pressed the down arrow.
const WM_APP_DOWN: u32 = 0x8002;
/// The message the search field sends when the user pressed escape.
const WM_APP_ESCAPE: u32 = 0x8003;
/// The message the search field sends when the user pressed return.
const WM_APP_RETURN: u32 = 0x8004;
/// The message another thread sends to ask the palette to show itself.
const WM_APP_SHOW: u32 = 0x8005;
/// The message another thread sends to ask the palette to close.
const WM_APP_CLOSE: u32 = 0x8006;
/// The message another thread sends to ask the palette to quit its message loop.
const WM_APP_QUIT: u32 = 0x8007;
/// The message another thread sends to show the palette and run an app-defined command.
const WM_APP_COMMAND: u32 = 0x8008;
/// Toggle requested by a tray click (debounced against an immediately-preceding blur dismiss).
const WM_APP_TOGGLE_TRAY: u32 = 0x8009;

/// Debounce window so clicking the tray icon while the palette is open closes
/// the palette rather than blur-dismissing and immediately re-opening it.
const TRAY_BLUR_DEBOUNCE: Duration = Duration::from_millis(250);

/// What the palette knows about the display it is on.
#[derive(Clone, Copy, Debug)]
pub struct Display {
    /// The DPI of the system.
    pub system_dpi: u32,
    /// The DPI of the window's own monitor.
    pub window_dpi: u32,
    /// The width of the drawing surface, in device-independent pixels.
    pub target_width: f32,
    /// The height of the drawing surface, in device-independent pixels.
    pub target_height: f32,
}

/// Everything the palette's window procedure needs.
struct State {
    hwnd: HWND,
    renderer: Option<Renderer>,
    model: Model,
    theme: Theme,
    edit: HWND,
    /// The window procedure the search field had before it was subclassed.
    edit_proc: isize,
    font: Option<HFONT>,
    field_brush: Option<HBRUSH>,
    /// The hint the field shows while it is empty.
    placeholder: String,
    /// Whether the field is showing that hint rather than a query.
    showing_placeholder: bool,
    /// When the window was last hidden by losing focus (`WA_INACTIVE`).
    last_blur_dismiss: Option<Instant>,
    /// What the app does when the palette says something.
    handler: Handler,
}

/// The palette's window, in a form that can cross a thread boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PaletteHandle(isize);

impl PaletteHandle {
    /// The handle for a window.
    pub fn of(hwnd: HWND) -> PaletteHandle {
        PaletteHandle(hwnd.0 as isize)
    }

    /// The window this handle names.
    pub fn hwnd(&self) -> HWND {
        HWND(self.0 as *mut c_void)
    }
}

/// Whether the palette's window is on screen.
pub fn is_visible(handle: PaletteHandle) -> bool {
    unsafe { IsWindowVisible(handle.hwnd()).as_bool() }
}

/// Asks the palette with this window to show itself, from any thread.
pub fn post_show(handle: PaletteHandle) {
    unsafe {
        let _ = PostMessageW(Some(handle.hwnd()), WM_APP_SHOW, WPARAM(0), LPARAM(0));
    }
}

/// Asks the palette with this window to close (hide), from any thread.
pub fn post_close(handle: PaletteHandle) {
    unsafe {
        let _ = PostMessageW(Some(handle.hwnd()), WM_APP_CLOSE, WPARAM(0), LPARAM(0));
    }
}

/// Asks the palette to toggle in response to a tray click (debouncing an
/// immediate `WA_INACTIVE` blur caused by clicking the taskbar tray icon).
pub fn post_toggle_from_tray(handle: PaletteHandle) {
    unsafe {
        let _ = PostMessageW(
            Some(handle.hwnd()),
            WM_APP_TOGGLE_TRAY,
            WPARAM(0),
            LPARAM(0),
        );
    }
}

/// Asks the palette's window thread to exit its message loop cleanly.
pub fn post_quit(handle: PaletteHandle) {
    unsafe {
        let _ = PostMessageW(Some(handle.hwnd()), WM_APP_QUIT, WPARAM(0), LPARAM(0));
    }
}

/// Shows the palette and delivers an application-defined command code on the
/// palette's own thread.
pub fn post_command(handle: PaletteHandle, code: u32) {
    unsafe {
        let _ = PostMessageW(
            Some(handle.hwnd()),
            WM_APP_COMMAND,
            WPARAM(code as usize),
            LPARAM(0),
        );
    }
}

/// The palette's window.
pub struct Palette {
    hwnd: HWND,
    state: *mut State,
}

impl Palette {
    /// Builds a palette window that answers to nobody.
    pub fn new(theme: Theme, model: Model) -> Result<Self, String> {
        Self::with_handler(theme, model, Box::new(crate::event::ignore))
    }

    /// Builds a palette window that tells the app what happens.
    pub fn with_handler(theme: Theme, model: Model, handler: Handler) -> Result<Self, String> {
        unsafe { set_dpi_aware() };
        unsafe { init_common_controls() };
        let instance: HINSTANCE = unsafe {
            HINSTANCE(
                GetModuleHandleW(None)
                    .map_err(|error| format!("the module handle failed: {error}"))?
                    .0,
            )
        };

        let class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(window_procedure),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: Default::default(),
            hCursor: unsafe { LoadCursorW(None, IDC_ARROW).unwrap_or_default() },
            hbrBackground: Default::default(),
            lpszMenuName: PCWSTR::null(),
            lpszClassName: w!("YalPaletteWindow"),
            hIconSm: Default::default(),
        };
        if unsafe { RegisterClassExW(&class) } == 0 {
            let err = unsafe { windows::Win32::Foundation::GetLastError() };
            if err != windows::Win32::Foundation::ERROR_CLASS_ALREADY_EXISTS {
                return Err("the palette's window class could not be registered".to_string());
            }
        }

        let metrics = theme.metrics;
        let initial_rows = model.rows.len();
        let width = metrics.width as i32;
        let height = content_height(metrics, initial_rows) as i32;

        // Convert to raw pointer BEFORE CreateWindowExW so the pointer stored in
        // GWLP_USERDATA keeps valid provenance across all Win32 callbacks.
        let state_ptr = Box::into_raw(Box::new(State {
            hwnd: HWND(std::ptr::null_mut()),
            renderer: None,
            model,
            theme,
            edit: HWND(std::ptr::null_mut()),
            edit_proc: 0,
            font: None,
            field_brush: None,
            placeholder: "Search YAL".to_string(),
            showing_placeholder: true,
            last_blur_dismiss: None,
            handler,
        }));

        let hwnd = match unsafe {
            CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
                w!("YalPaletteWindow"),
                w!("YAL"),
                WS_POPUP | WS_THICKFRAME | WS_CLIPCHILDREN | WS_CLIPSIBLINGS,
                100,
                100,
                width,
                height,
                None,
                None,
                Some(instance),
                Some(state_ptr as *mut c_void),
            )
        } {
            Ok(hwnd) => hwnd,
            Err(error) => {
                unsafe {
                    drop(Box::from_raw(state_ptr));
                }
                return Err(format!("the palette window could not be created: {error}"));
            }
        };

        unsafe {
            (*state_ptr).hwnd = hwnd;
        }
        // Extract Copy fields before SetWindowPos (which synchronously sends WM_SIZE).
        place_window_rect(hwnd, metrics, initial_rows);

        Ok(Self {
            hwnd,
            state: state_ptr,
        })
    }

    /// The window's handle.
    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    /// What the palette knows about the display it is on.
    pub fn display(&self) -> Display {
        unsafe {
            let state = &*self.state;
            let (width, height) = state
                .renderer
                .as_ref()
                .map(|renderer| renderer.size())
                .unwrap_or((0.0, 0.0));
            Display {
                system_dpi: GetDpiForSystem(),
                window_dpi: GetDpiForWindow(self.hwnd),
                target_width: width,
                target_height: height,
            }
        }
    }

    /// The palette's content.
    pub fn model(&self) -> &Model {
        unsafe { &(*self.state).model }
    }

    /// Replaces the palette's content and repaints.
    pub fn set_model(&mut self, model: Model) {
        let (hwnd, metrics, rows) = unsafe {
            (*self.state).model = model;
            (
                (*self.state).hwnd,
                (*self.state).theme.metrics,
                (*self.state).model.rows.len(),
            )
        };
        resize_window_rect(hwnd, metrics, rows);
        self.repaint();
    }

    /// Repaints the palette.
    pub fn repaint(&self) {
        unsafe {
            let _ = InvalidateRect(Some(self.hwnd), None, false);
        }
    }

    /// Shows the palette and gives it the keyboard.
    pub fn show(&self) {
        unsafe {
            show_window(self.state);
        }
    }

    /// Hides the palette.
    pub fn hide(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    /// Runs the window's blocking message loop until closed/quit.
    pub fn run(&self) {
        unsafe {
            let mut message = std::mem::zeroed::<MSG>();
            while GetMessageW(&mut message, None, 0, 0).0 > 0 {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }

    /// Handles pending messages without blocking.
    pub fn pump(&self) {
        unsafe {
            let mut message = std::mem::zeroed::<MSG>();
            while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }

    /// Asks the palette to show itself.
    pub fn post_show(&self) {
        post_show(self.handle());
    }

    /// Asks the palette to close (hide).
    pub fn post_close(&self) {
        post_close(self.handle());
    }

    /// The palette's window, in a form that can cross a thread boundary.
    pub fn handle(&self) -> PaletteHandle {
        PaletteHandle::of(self.hwnd)
    }
}

impl Drop for Palette {
    fn drop(&mut self) {
        unsafe {
            if !self.state.is_null() {
                // Step 1: Unsubclass the edit control while it still exists.
                let edit = (*self.state).edit;
                let edit_proc = (*self.state).edit_proc;
                if edit_proc != 0 && !edit.0.is_null() && IsWindow(Some(edit)).as_bool() {
                    let _ = SetWindowLongPtrW(edit, GWLP_WNDPROC, edit_proc);
                }
                // Step 2: Detach GWLP_USERDATA BEFORE DestroyWindow so WM_ACTIVATE /
                // WM_DESTROY cannot re-enter State or create aliasing references.
                if !self.hwnd.0.is_null() && IsWindow(Some(self.hwnd)).as_bool() {
                    let _ = SetWindowLongPtrW(self.hwnd, GWLP_USERDATA, 0);
                    let _ = DestroyWindow(self.hwnd);
                }
                // Step 3: Now reclaim State and release GDI objects safely.
                let mut state = Box::from_raw(self.state);
                self.state = std::ptr::null_mut();
                if let Some(font) = state.font.take() {
                    let _ = DeleteObject(font.into());
                }
                if let Some(brush) = state.field_brush.take() {
                    let _ = DeleteObject(brush.into());
                }
                drop(state);
            }
        }
    }
}

/// The palette's window procedure.
unsafe extern "system" fn window_procedure(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == windows::Win32::UI::WindowsAndMessaging::WM_NCCREATE {
        let create = &*(lparam.0 as *const CREATESTRUCTW);
        let state = create.lpCreateParams as *mut State;
        let _ = SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);
        return DefWindowProcW(hwnd, message, wparam, lparam);
    }

    let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
    if state_ptr.is_null() {
        return DefWindowProcW(hwnd, message, wparam, lparam);
    }

    match message {
        WM_CREATE => {
            let state = &mut *state_ptr;
            let dpi = GetDpiForWindow(hwnd).max(96) as f32;
            let metrics = state.theme.metrics;

            let corner = DWMWCP_ROUND;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                &corner as *const _ as *const c_void,
                std::mem::size_of_val(&corner) as u32,
            );

            let field = CreateSolidBrush(colorref(state.theme.field));
            state.field_brush = Some(field);

            let edit = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("EDIT"),
                PCWSTR::null(),
                WS_CHILD
                    | WS_VISIBLE
                    | WS_TABSTOP
                    | WINDOW_STYLE(ES_AUTOHSCROLL as u32)
                    | WINDOW_STYLE(ES_LEFT as u32),
                0,
                0,
                10,
                10,
                Some(hwnd),
                None,
                None,
                None,
            )
            .unwrap_or_default();

            let height = -(MulDiv(metrics.name_size as i32, dpi as i32, 72));
            let font = CreateFontW(
                height,
                0,
                0,
                0,
                400,
                0,
                0,
                0,
                windows::Win32::Graphics::Gdi::DEFAULT_CHARSET,
                windows::Win32::Graphics::Gdi::OUT_TT_PRECIS,
                windows::Win32::Graphics::Gdi::CLIP_DEFAULT_PRECIS,
                windows::Win32::Graphics::Gdi::CLEARTYPE_QUALITY,
                0,
                w!("Segoe UI"),
            );
            let _ = SendMessageW(
                edit,
                WM_SETFONT,
                Some(WPARAM(font.0 as usize)),
                Some(LPARAM(1)),
            );

            state.edit = edit;
            state.font = Some(font);

            let cue: Vec<u16> = state
                .placeholder
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let _ = SendMessageW(
                edit,
                windows::Win32::UI::WindowsAndMessaging::WM_SETTEXT,
                Some(WPARAM(0)),
                Some(LPARAM(cue.as_ptr() as isize)),
            );

            let previous =
                SetWindowLongPtrW(edit, GWLP_WNDPROC, edit_procedure as *const () as isize);
            state.edit_proc = previous;

            if let Ok(renderer) = Renderer::new(hwnd) {
                state.renderer = Some(renderer);
            }
            LRESULT(0)
        }
        WM_SIZE => {
            let width = (lparam.0 & 0xffff) as u32;
            let height = ((lparam.0 >> 16) & 0xffff) as u32;
            let (edit, metrics) = {
                let state = &mut *state_ptr;
                if let Some(renderer) = state.renderer.as_ref() {
                    renderer.resize(width, height);
                }
                (state.edit, state.theme.metrics)
            };
            place_field_rect(hwnd, edit, metrics, width as i32);
            LRESULT(0)
        }
        WM_PAINT => {
            let mut paint = std::mem::zeroed::<PAINTSTRUCT>();
            let _ = BeginPaint(hwnd, &mut paint);
            let state = &mut *state_ptr;
            if let Some(renderer) = state.renderer.as_mut() {
                renderer.draw(&state.model, &state.theme);
            }
            let _ = EndPaint(hwnd, &paint);
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_COMMAND => {
            let code = (wparam.0 >> 16) & 0xffff;
            if code as u32 == EN_CHANGE {
                let edit = (*state_ptr).edit;
                let mut buffer = [0u16; 512];
                let length =
                    windows::Win32::UI::WindowsAndMessaging::GetWindowTextW(edit, &mut buffer);
                let text = String::from_utf16_lossy(&buffer[..length as usize]);
                let query_to_dispatch = {
                    let state = &mut *state_ptr;
                    if state.showing_placeholder {
                        state.model.query = String::new();
                        None
                    } else {
                        state.model.query = text.clone();
                        Some(text)
                    }
                };
                if let Some(query) = query_to_dispatch {
                    dispatch(state_ptr, PaletteEvent::QueryChanged(query));
                }
            }
            LRESULT(0)
        }
        WM_CTLCOLOREDIT => {
            let control = HDC(wparam.0 as *mut c_void);
            let state = &*state_ptr;
            let showing = state.showing_placeholder;
            let _ = SetTextColor(
                control,
                colorref(if showing {
                    state.theme.text_dim
                } else {
                    state.theme.text
                }),
            );
            let _ = SetBkColor(control, colorref(state.theme.field));
            let brush = state.field_brush.map(|b| b.0).unwrap_or_default();
            LRESULT(brush as isize)
        }
        WM_NCHITTEST => LRESULT(HTCLIENT as isize),
        WM_MOUSEMOVE => {
            let y = (lparam.0 >> 16) as i16 as f32;
            let state = &mut *state_ptr;
            let hovered = hit_test_row(state, y);
            if state.model.hovered != hovered {
                state.model.hovered = hovered;
                let _ = InvalidateRect(Some(hwnd), None, false);
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            let y = (lparam.0 >> 16) as i16 as f32;
            let clicked = {
                let state = &mut *state_ptr;
                let hit = hit_test_row(state, y);
                if let Some(index) = hit {
                    state.model.selected = index;
                }
                hit
            };
            if let Some(index) = clicked {
                dispatch(state_ptr, PaletteEvent::Activated(index));
            }
            LRESULT(0)
        }
        WM_ACTIVATE => {
            if (wparam.0 & 0xffff) == WA_INACTIVE as usize && IsWindowVisible(hwnd).as_bool() {
                (*state_ptr).last_blur_dismiss = Some(Instant::now());
                dismiss(state_ptr);
            }
            LRESULT(0)
        }
        WM_NCCALCSIZE => LRESULT(0),
        WM_GETMINMAXINFO => {
            let info = &mut *(lparam.0 as *mut MINMAXINFO);
            info.ptMaxSize.x = i32::MAX;
            info.ptMaxSize.y = i32::MAX;
            info.ptMaxPosition.x = 0;
            info.ptMaxPosition.y = 0;
            info.ptMinTrackSize.x = 320;
            info.ptMinTrackSize.y = 200;
            LRESULT(0)
        }
        WM_KEYDOWN => {
            if wparam.0 as u16 == windows::Win32::UI::Input::KeyboardAndMouse::VK_ESCAPE.0 {
                dismiss(state_ptr);
            }
            LRESULT(0)
        }
        WM_APP_UP => {
            (*state_ptr).model.select_previous();
            let _ = InvalidateRect(Some(hwnd), None, false);
            LRESULT(0)
        }
        WM_APP_DOWN => {
            (*state_ptr).model.select_next();
            let _ = InvalidateRect(Some(hwnd), None, false);
            LRESULT(0)
        }
        WM_APP_ESCAPE => {
            dismiss(state_ptr);
            LRESULT(0)
        }
        WM_APP_RETURN => {
            let selected = (*state_ptr).model.selected;
            dispatch(state_ptr, PaletteEvent::Activated(selected));
            LRESULT(0)
        }
        WM_APP_SHOW => {
            show_window(state_ptr);
            LRESULT(0)
        }
        WM_APP_CLOSE => {
            let _ = ShowWindow(hwnd, SW_HIDE);
            LRESULT(0)
        }
        WM_APP_TOGGLE_TRAY => {
            if IsWindowVisible(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_HIDE);
            } else {
                let recently_blurred = (*state_ptr)
                    .last_blur_dismiss
                    .is_some_and(|at| at.elapsed() < TRAY_BLUR_DEBOUNCE);
                if recently_blurred {
                    (*state_ptr).last_blur_dismiss = None;
                } else {
                    show_window(state_ptr);
                }
            }
            LRESULT(0)
        }
        WM_APP_QUIT => {
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_APP_COMMAND => {
            let code = wparam.0 as u32;
            show_window_without_shown_event(state_ptr);
            dispatch(state_ptr, PaletteEvent::Command(code));
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

fn hit_test_row(state: &State, y: f32) -> Option<usize> {
    let renderer = state.renderer.as_ref()?;
    let scale = renderer.scale(&state.theme);
    if scale > 0.0 {
        let metrics = state.theme.metrics;
        let list_top = metrics.padding + metrics.field_height + metrics.list_gap;
        state.model.row_at(y / scale, list_top, metrics.row_height)
    } else {
        None
    }
}

/// The search field's own window procedure, which keeps the arrow keys and
/// clears the placeholder cleanly without aliasing `State` across `SendMessageW`.
unsafe extern "system" fn edit_procedure(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let parent = GetParent(hwnd).unwrap_or_default();
    let pointer = GetWindowLongPtrW(parent, GWLP_USERDATA) as *mut State;

    // Copy the original wndproc up front; never hold &State across SendMessageW.
    let edit_proc = if !pointer.is_null() {
        (*pointer).edit_proc
    } else {
        0
    };

    if message == WM_PASTE && !pointer.is_null() && (*pointer).showing_placeholder {
        (*pointer).showing_placeholder = false;
        let _ = SendMessageW(
            hwnd,
            windows::Win32::UI::WindowsAndMessaging::WM_SETTEXT,
            Some(WPARAM(0)),
            Some(LPARAM(0)),
        );
    }

    if message == WM_CHAR && !pointer.is_null() && (*pointer).showing_placeholder {
        let ch = wparam.0 as u16;
        // Ignore control characters (Backspace 0x08, Ctrl+C 0x03, Tab 0x09, etc.)
        // as full UTF-16 code units so valid Unicode characters are never truncated.
        if ch < 0x20 {
            return LRESULT(0);
        }
        (*pointer).showing_placeholder = false;
        let _ = SendMessageW(
            hwnd,
            windows::Win32::UI::WindowsAndMessaging::WM_SETTEXT,
            Some(WPARAM(0)),
            Some(LPARAM(0)),
        );
    }

    if message == WM_KEYDOWN {
        let key = wparam.0 as u16;
        use windows::Win32::UI::Input::KeyboardAndMouse::{VK_DOWN, VK_ESCAPE, VK_RETURN, VK_UP};
        let forwarded = match key {
            value if value == VK_UP.0 => Some(WM_APP_UP),
            value if value == VK_DOWN.0 => Some(WM_APP_DOWN),
            value if value == VK_ESCAPE.0 => Some(WM_APP_ESCAPE),
            value if value == VK_RETURN.0 => Some(WM_APP_RETURN),
            _ => None,
        };
        if let Some(custom) = forwarded {
            let _ = SendMessageW(parent, custom, Some(WPARAM(0)), Some(LPARAM(0)));
            return LRESULT(0);
        }
    }

    if edit_proc != 0 {
        let original: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT =
            std::mem::transmute(edit_proc);
        return original(hwnd, message, wparam, lparam);
    }
    DefWindowProcW(hwnd, message, wparam, lparam)
}

/// Shows the palette and gives it the keyboard, without firing `PaletteEvent::Shown`.
unsafe fn show_window_without_shown_event(state_ptr: *mut State) {
    let (hwnd, edit, metrics, rows, cue) = {
        let state = &mut *state_ptr;
        state.showing_placeholder = true;
        state.last_blur_dismiss = None;
        state.model.query = String::new();
        let cue: Vec<u16> = state
            .placeholder
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        (
            state.hwnd,
            state.edit,
            state.theme.metrics,
            state.model.rows.len(),
            cue,
        )
    };

    place_window_rect(hwnd, metrics, rows);
    let _ = SendMessageW(
        edit,
        windows::Win32::UI::WindowsAndMessaging::WM_SETTEXT,
        Some(WPARAM(0)),
        Some(LPARAM(cue.as_ptr() as isize)),
    );
    let _ = ShowWindow(hwnd, SW_SHOW);
    bring_to_foreground(hwnd);
    if !edit.0.is_null() {
        let _ = SendMessageW(
            hwnd,
            WM_NEXTDLGCTL,
            Some(WPARAM(edit.0 as usize)),
            Some(LPARAM(1)),
        );
    }
}

/// Brings the palette window to the foreground reliably even when summoned
/// from a background hotkey or tray thread.
unsafe fn bring_to_foreground(hwnd: HWND) {
    if SetForegroundWindow(hwnd).as_bool() {
        return;
    }
    let foreground = GetForegroundWindow();
    if foreground.0.is_null() {
        return;
    }
    let fg_thread = GetWindowThreadProcessId(foreground, None);
    let our_thread = GetCurrentThreadId();
    if fg_thread != 0 && fg_thread != our_thread {
        let attached = AttachThreadInput(our_thread, fg_thread, true).as_bool();
        let _ = SetForegroundWindow(hwnd);
        if attached {
            let _ = AttachThreadInput(our_thread, fg_thread, false);
        }
    }
}

/// Shows the palette and gives it the keyboard.
unsafe fn show_window(state_ptr: *mut State) {
    show_window_without_shown_event(state_ptr);
    dispatch(state_ptr, PaletteEvent::Shown);
}

/// Tells the app what happened, and applies what it says.
///
/// The handler is moved out of `State` while it runs, and no `&mut State` is
/// held across `SetWindowPos` / `ShowWindow` / `DestroyWindow`.
unsafe fn dispatch(state_ptr: *mut State, event: PaletteEvent) {
    let handler = {
        let state = &mut *state_ptr;
        let placeholder: Handler = Box::new(crate::event::ignore);
        std::mem::replace(&mut state.handler, placeholder)
    };
    let reply = handler(event);
    let hwnd = {
        let state = &mut *state_ptr;
        state.handler = handler;
        state.hwnd
    };

    match reply {
        PaletteReply::Keep => {}
        PaletteReply::Update(model) => {
            let (metrics, rows) = {
                let state = &mut *state_ptr;
                state.model = model;
                (state.theme.metrics, state.model.rows.len())
            };
            resize_window_rect(hwnd, metrics, rows);
            let _ = InvalidateRect(Some(hwnd), None, false);
        }
        PaletteReply::Close => {
            let _ = ShowWindow(hwnd, SW_HIDE);
        }
        PaletteReply::Quit => {
            let _ = DestroyWindow(hwnd);
        }
    }
}

/// The user dismissed the palette.
unsafe fn dismiss(state_ptr: *mut State) {
    let hwnd = (*state_ptr).hwnd;
    dispatch(state_ptr, PaletteEvent::Dismissed);
    if IsWindow(Some(hwnd)).as_bool() && IsWindowVisible(hwnd).as_bool() {
        let _ = ShowWindow(hwnd, SW_HIDE);
    }
}

/// Sizes the palette for the monitor the cursor is currently on and places it
/// centered horizontally and a third of the way down.
fn place_window_rect(hwnd: HWND, metrics: Metrics, rows: usize) {
    let mut cursor = POINT::default();
    let _ = unsafe { windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut cursor) };
    let monitor = unsafe {
        MonitorFromPoint(
            cursor,
            windows::Win32::Graphics::Gdi::MONITOR_DEFAULTTONEAREST,
        )
    };
    let mut dpi_x = 0u32;
    let mut dpi_y = 0u32;
    let monitor_dpi = unsafe {
        if GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y).is_ok()
            && dpi_x >= 96
        {
            dpi_x
        } else {
            GetDpiForSystem().max(96)
        }
    };
    let scale = monitor_dpi as f32 / 96.0;
    let width = (metrics.width * scale) as i32;
    let height = (content_height(metrics, rows) * scale) as i32;

    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    let _ = unsafe { GetMonitorInfoW(monitor, &mut info) };

    let x = info.rcWork.left + (info.rcWork.right - info.rcWork.left - width) / 2;
    let y = info.rcWork.top + (info.rcWork.bottom - info.rcWork.top - height) / 3;

    unsafe {
        let _ = SetWindowPos(
            hwnd,
            Some(windows::Win32::UI::WindowsAndMessaging::HWND_TOPMOST),
            x,
            y,
            width,
            height,
            SWP_NOACTIVATE,
        );
    }
}

/// Makes the window the size of its content without moving it.
fn resize_window_rect(hwnd: HWND, metrics: Metrics, rows: usize) {
    let dpi = unsafe { GetDpiForWindow(hwnd).max(96) } as f32;
    let scale = dpi / 96.0;
    let width = (metrics.width * scale) as i32;
    let height = (content_height(metrics, rows) * scale) as i32;
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            None,
            0,
            0,
            width,
            height,
            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}

/// Puts the search field where it belongs.
fn place_field_rect(hwnd: HWND, edit: HWND, metrics: Metrics, window_width: i32) {
    let dpi = unsafe { GetDpiForWindow(hwnd).max(96) } as f32;
    let scale = dpi / 96.0;
    let padding = (metrics.padding * scale) as i32;
    let inset = (metrics.field_inset * scale) as i32;
    let height = (metrics.field_height * scale) as i32;
    let width = window_width - padding * 2 - inset - 12;
    unsafe {
        let _ = SetWindowPos(
            edit,
            None,
            padding + inset,
            padding + 1,
            width,
            height - 2,
            SWP_NOZORDER,
        );
    }
}

/// How tall the palette needs to be for a list of so many rows.
fn content_height(metrics: Metrics, rows: usize) -> f32 {
    let visible = rows.min(metrics.max_rows) as f32;
    metrics.padding * 2.0
        + metrics.field_height
        + metrics.list_gap
        + visible * metrics.row_height
        + metrics.footer_height
}

/// A theme colour in GDI `COLORREF` (`0x00BBGGRR`) layout.
fn colorref(color: Color) -> COLORREF {
    let r = (color.red.clamp(0.0, 1.0) * 255.0).round() as u32;
    let g = (color.green.clamp(0.0, 1.0) * 255.0).round() as u32;
    let b = (color.blue.clamp(0.0, 1.0) * 255.0).round() as u32;
    COLORREF(r | (g << 8) | (b << 16))
}

/// Tells the process that it is per-monitor aware.
unsafe fn set_dpi_aware() {
    let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
}

/// Asks for the common controls of Windows 11 rather than the ones from 1994.
unsafe fn init_common_controls() {
    let controls = INITCOMMONCONTROLSEX {
        dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
        dwICC: ICC_STANDARD_CLASSES,
    };
    let _ = InitCommonControlsEx(&controls);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colorref_packs_rgb_into_windows_bbggrr_order() {
        let color = Color::rgb(0x12, 0x34, 0x56);
        assert_eq!(colorref(color).0, 0x0056_3412);
    }
}
