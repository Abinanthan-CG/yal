//! The native Win32 palette window.
//!
//! Borderless, always-on-top, rounded Windows 11 DWM frame painted in 1:1 DIPs
//! by Direct2D, paired with a seamlessly styled native Win32 `EDIT` control
//! using `SetFocus` (instant typing on open) and `EM_SETCUEBANNER` (native
//! placeholder text with zero fake-text hacks).

use std::ffi::c_void;
use std::time::{Duration, Instant};

use windows::core::{w, BOOL, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE,
    DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
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
    InitCommonControlsEx, ICC_STANDARD_CLASSES, INITCOMMONCONTROLSEX, WM_MOUSELEAVE,
};
use windows::Win32::UI::HiDpi::{
    GetDpiForMonitor, GetDpiForSystem, GetDpiForWindow, SetProcessDpiAwarenessContext,
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, MDT_EFFECTIVE_DPI,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, SetFocus, TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT, VK_BACK, VK_CONTROL,
    VK_DELETE, VK_DOWN, VK_ESCAPE, VK_K, VK_LEFT, VK_OEM_COMMA, VK_P, VK_RETURN, VK_RIGHT,
    VK_SHIFT, VK_TAB, VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    GetForegroundWindow, GetMessageW, GetParent, GetWindowLongPtrW, GetWindowTextLengthW,
    GetWindowTextW, GetWindowThreadProcessId, IsWindow, IsWindowVisible, LoadCursorW, PeekMessageW,
    PostMessageW, PostQuitMessage, RegisterClassExW, SendMessageW, SetForegroundWindow,
    SetWindowLongPtrW, SetWindowPos, ShowWindow, TranslateMessage, CREATESTRUCTW, CS_HREDRAW,
    CS_VREDRAW, EN_CHANGE, ES_AUTOHSCROLL, ES_LEFT, GWLP_USERDATA, GWLP_WNDPROC, HTCLIENT,
    IDC_ARROW, MINMAXINFO, MSG, PM_REMOVE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOZORDER, SW_HIDE,
    SW_SHOW, WA_INACTIVE, WINDOW_EX_STYLE, WINDOW_STYLE, WM_ACTIVATE, WM_CHAR, WM_COMMAND,
    WM_CREATE, WM_CTLCOLOREDIT, WM_DESTROY, WM_DPICHANGED, WM_ERASEBKGND, WM_GETMINMAXINFO,
    WM_KEYDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCCALCSIZE, WM_NCCREATE,
    WM_NCHITTEST, WM_PAINT, WM_SETFOCUS, WM_SETFONT, WM_SETTEXT, WM_SIZE, WNDCLASSEXW, WS_CHILD,
    WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP, WS_TABSTOP,
    WS_THICKFRAME, WS_VISIBLE,
};

use crate::event::{ActionKey, Handler, PaletteEvent, PaletteReply};
use crate::model::Model;
use crate::paint::Renderer;
use crate::theme::{Appearance, Color, Metrics, Theme};

/// Native Win32 cue banner message (ComCtl32 v6, enabled via `yal.manifest`).
const EM_SETCUEBANNER: u32 = 0x1501;
const EM_SETSEL: u32 = 0x00B1;

const WM_APP_UP: u32 = 0x8001;
const WM_APP_DOWN: u32 = 0x8002;
const WM_APP_ESCAPE: u32 = 0x8003;
const WM_APP_RETURN: u32 = 0x8004;
const WM_APP_SHOW: u32 = 0x8005;
const WM_APP_CLOSE: u32 = 0x8006;
const WM_APP_QUIT: u32 = 0x8007;
const WM_APP_COMMAND: u32 = 0x8008;
const WM_APP_TOGGLE_TRAY: u32 = 0x8009;
const WM_APP_SETTINGS: u32 = 0x800A;
const WM_APP_REFRESH: u32 = 0x800B;
const WM_APP_LEFT: u32 = 0x800C;
const WM_APP_RIGHT: u32 = 0x800D;
const WM_APP_CTRL_RETURN: u32 = 0x800E;
const WM_APP_ACTION: u32 = 0x800F;

const ACTION_TAB: usize = 1;
const ACTION_CTRL_P: usize = 2;
const ACTION_CTRL_K: usize = 3;
const ACTION_CTRL_DELETE: usize = 4;

/// Debounce window so clicking the tray icon while the palette is open closes
/// the palette rather than blur-dismissing and immediately re-opening it.
const TRAY_BLUR_DEBOUNCE: Duration = Duration::from_millis(250);

/// Default search placeholder when the model does not specify one.
const DEFAULT_PLACEHOLDER: &str = "Search apps, commands, or calculate…";

/// What the palette knows about the display it is on.
#[derive(Clone, Copy, Debug)]
pub struct Display {
    pub system_dpi: u32,
    pub window_dpi: u32,
    pub target_width: f32,
    pub target_height: f32,
}

/// Everything the palette's window procedure needs.
struct State {
    hwnd: HWND,
    renderer: Option<Renderer>,
    model: Model,
    theme: Theme,
    edit: HWND,
    edit_proc: isize,
    font: Option<HFONT>,
    field_brush: Option<HBRUSH>,
    last_blur_dismiss: Option<Instant>,
    tracking_mouse_leave: bool,
    suppress_en_change: bool,
    handler: Handler,
}

/// The palette's window, in a form that can cross a thread boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PaletteHandle(isize);

impl PaletteHandle {
    pub fn of(hwnd: HWND) -> PaletteHandle {
        PaletteHandle(hwnd.0 as isize)
    }

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

/// Asks the palette to toggle in response to a tray click (debounced against blur).
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

/// Silently asks the palette to refresh its current model in-place (for example
/// once background-loaded app icons finish decoding), without showing, focusing,
/// or clearing the search field.
pub fn post_refresh(handle: PaletteHandle) {
    unsafe {
        let _ = PostMessageW(Some(handle.hwnd()), WM_APP_REFRESH, WPARAM(0), LPARAM(0));
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
        let initial_height = model.layout(metrics).window_height;

        let state_ptr = Box::into_raw(Box::new(State {
            hwnd: HWND(std::ptr::null_mut()),
            renderer: None,
            model,
            theme,
            edit: HWND(std::ptr::null_mut()),
            edit_proc: 0,
            font: None,
            field_brush: None,
            last_blur_dismiss: None,
            tracking_mouse_leave: false,
            suppress_en_change: false,
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
                metrics.width as i32,
                initial_height as i32,
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
        place_window_rect(hwnd, metrics, initial_height);

        Ok(Self {
            hwnd,
            state: state_ptr,
        })
    }

    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

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

    pub fn model(&self) -> &Model {
        unsafe { &(*self.state).model }
    }

    pub fn set_model(&mut self, model: Model) {
        let (hwnd, metrics, target_h) = unsafe {
            (*self.state).model = model;
            let m = (*self.state).theme.metrics;
            let h = (*self.state).model.layout(m).window_height;
            ((*self.state).hwnd, m, h)
        };
        resize_window_rect(hwnd, metrics, target_h);
        self.repaint();
    }

    pub fn repaint(&self) {
        unsafe {
            let _ = InvalidateRect(Some(self.hwnd), None, false);
        }
    }

    pub fn show(&self) {
        unsafe {
            show_window(self.state);
        }
    }

    pub fn hide(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    pub fn run(&self) {
        unsafe {
            let mut message = std::mem::zeroed::<MSG>();
            while GetMessageW(&mut message, None, 0, 0).0 > 0 {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }

    pub fn pump(&self) {
        unsafe {
            let mut message = std::mem::zeroed::<MSG>();
            while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }

    pub fn post_show(&self) {
        post_show(self.handle());
    }

    pub fn post_close(&self) {
        post_close(self.handle());
    }

    pub fn handle(&self) -> PaletteHandle {
        PaletteHandle::of(self.hwnd)
    }
}

impl Drop for Palette {
    fn drop(&mut self) {
        unsafe {
            if !self.state.is_null() {
                let edit = (*self.state).edit;
                let edit_proc = (*self.state).edit_proc;
                if edit_proc != 0 && !edit.0.is_null() && IsWindow(Some(edit)).as_bool() {
                    let _ = SetWindowLongPtrW(edit, GWLP_WNDPROC, edit_proc);
                }
                if !self.hwnd.0.is_null() && IsWindow(Some(self.hwnd)).as_bool() {
                    let _ = SetWindowLongPtrW(self.hwnd, GWLP_USERDATA, 0);
                    let _ = DestroyWindow(self.hwnd);
                }
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

unsafe extern "system" fn window_procedure(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_NCCREATE {
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
            let dpi = GetDpiForWindow(hwnd).max(96);
            apply_dwm_chrome(hwnd, &state.theme);

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

            let font = create_edit_font(dpi);
            let _ = SendMessageW(
                edit,
                WM_SETFONT,
                Some(WPARAM(font.0 as usize)),
                Some(LPARAM(1)),
            );

            state.edit = edit;
            state.font = Some(font);

            let placeholder = if state.model.placeholder.is_empty() {
                DEFAULT_PLACEHOLDER
            } else {
                state.model.placeholder.as_str()
            };
            set_cue_banner(edit, placeholder);

            let previous =
                SetWindowLongPtrW(edit, GWLP_WNDPROC, edit_procedure as *const () as isize);
            state.edit_proc = previous;

            if let Ok(renderer) = Renderer::new(hwnd, dpi as f32) {
                state.renderer = Some(renderer);
            }
            LRESULT(0)
        }
        WM_DPICHANGED => {
            let new_dpi = (wparam.0 & 0xffff) as u32;
            let dpi = new_dpi.max(96);
            let (edit, metrics, target_h) = {
                let state = &mut *state_ptr;
                if let Some(renderer) = state.renderer.as_ref() {
                    renderer.set_dpi(dpi as f32);
                }
                let new_font = create_edit_font(dpi);
                let _ = SendMessageW(
                    state.edit,
                    WM_SETFONT,
                    Some(WPARAM(new_font.0 as usize)),
                    Some(LPARAM(1)),
                );
                if let Some(old) = state.font.replace(new_font) {
                    let _ = DeleteObject(old.into());
                }
                let m = state.theme.metrics;
                (state.edit, m, state.model.layout(m).window_height)
            };
            resize_window_rect_for_dpi(hwnd, metrics, target_h, dpi as f32);
            let width_px = ((metrics.width * (dpi as f32 / 96.0)).round()) as i32;
            let has_badge = (*state_ptr).model.header_badge.is_some();
            place_field_rect(hwnd, edit, metrics, width_px, has_badge);
            let _ = InvalidateRect(Some(hwnd), None, false);
            LRESULT(0)
        }
        WM_SIZE => {
            let width = (lparam.0 & 0xffff) as u32;
            let height = ((lparam.0 >> 16) & 0xffff) as u32;
            let dpi = GetDpiForWindow(hwnd).max(96) as f32;
            let (edit, metrics, has_badge) = {
                let state = &mut *state_ptr;
                if let Some(renderer) = state.renderer.as_ref() {
                    renderer.set_dpi(dpi);
                    renderer.resize(width, height);
                }
                (
                    state.edit,
                    state.theme.metrics,
                    state.model.header_badge.is_some(),
                )
            };
            place_field_rect(hwnd, edit, metrics, width as i32, has_badge);
            LRESULT(0)
        }
        WM_SETFOCUS => {
            // Whenever the top-level palette window receives focus, immediately
            // hand keyboard focus to the search input so typing works without clicking.
            let edit = (*state_ptr).edit;
            if !edit.0.is_null() && IsWindow(Some(edit)).as_bool() {
                let _ = SetFocus(Some(edit));
            }
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
            if code as u32 == EN_CHANGE && !(*state_ptr).suppress_en_change {
                let edit = (*state_ptr).edit;
                let text = read_edit_text(edit);
                (*state_ptr).model.query = text.clone();
                dispatch(state_ptr, PaletteEvent::QueryChanged(text));
            }
            LRESULT(0)
        }
        WM_CTLCOLOREDIT => {
            let control = HDC(wparam.0 as *mut c_void);
            let state = &*state_ptr;
            let _ = SetTextColor(control, colorref(state.theme.text));
            let _ = SetBkColor(control, colorref(state.theme.field));
            let brush = state.field_brush.map(|b| b.0).unwrap_or_default();
            LRESULT(brush as isize)
        }
        WM_NCHITTEST => LRESULT(HTCLIENT as isize),
        WM_MOUSEMOVE => {
            let x_phys = (lparam.0 & 0xffff) as i16 as f32;
            let y_phys = ((lparam.0 >> 16) & 0xffff) as i16 as f32;
            let state = &mut *state_ptr;
            if !state.tracking_mouse_leave {
                let mut tme = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                if TrackMouseEvent(&mut tme).is_ok() {
                    state.tracking_mouse_leave = true;
                }
            }
            let hovered = hit_test_point(hwnd, state, x_phys, y_phys);
            let changed = if let Some(grid) = state.model.grid.as_mut() {
                let c = grid.hovered != hovered;
                grid.hovered = hovered;
                c
            } else {
                let c = state.model.hovered != hovered;
                state.model.hovered = hovered;
                c
            };
            if changed {
                let _ = InvalidateRect(Some(hwnd), None, false);
            }
            LRESULT(0)
        }
        WM_MOUSELEAVE => {
            let state = &mut *state_ptr;
            state.tracking_mouse_leave = false;
            let cleared = if let Some(grid) = state.model.grid.as_mut() {
                grid.hovered.take().is_some()
            } else {
                state.model.hovered.take().is_some()
            };
            if cleared {
                let _ = InvalidateRect(Some(hwnd), None, false);
            }
            LRESULT(0)
        }
        WM_MOUSEWHEEL => {
            let wheel_delta = ((wparam.0 >> 16) as i16) as f32;
            let state = &mut *state_ptr;
            let metrics = state.theme.metrics;
            // One wheel notch (120) scrolls ~1.5 rows cleanly.
            let delta_dip = -(wheel_delta / 120.0) * (metrics.row_height * 1.5);
            state.model.scroll_by(delta_dip, metrics);
            let _ = InvalidateRect(Some(hwnd), None, false);
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            let x_phys = (lparam.0 & 0xffff) as i16 as f32;
            let y_phys = ((lparam.0 >> 16) & 0xffff) as i16 as f32;
            let clicked = {
                let state = &mut *state_ptr;
                let hit = hit_test_point(hwnd, state, x_phys, y_phys);
                if let Some(index) = hit {
                    if let Some(grid) = state.model.grid.as_mut() {
                        grid.selected = index;
                    } else {
                        state.model.selected = index;
                    }
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
            info.ptMinTrackSize.y = 140;
            LRESULT(0)
        }
        WM_KEYDOWN => {
            if wparam.0 as u16 == VK_ESCAPE.0 {
                handle_escape(state_ptr);
            }
            LRESULT(0)
        }
        WM_APP_UP => {
            let state = &mut *state_ptr;
            if let Some(grid) = state.model.grid.as_mut() {
                grid.select_up();
            } else {
                state.model.select_previous();
            }
            let metrics = state.theme.metrics;
            state.model.ensure_selected_visible(metrics);
            let _ = InvalidateRect(Some(hwnd), None, false);
            LRESULT(0)
        }
        WM_APP_DOWN => {
            let state = &mut *state_ptr;
            if let Some(grid) = state.model.grid.as_mut() {
                grid.select_down();
            } else {
                state.model.select_next();
            }
            let metrics = state.theme.metrics;
            state.model.ensure_selected_visible(metrics);
            let _ = InvalidateRect(Some(hwnd), None, false);
            LRESULT(0)
        }
        WM_APP_LEFT => {
            let state = &mut *state_ptr;
            if let Some(grid) = state.model.grid.as_mut() {
                grid.select_left();
                let metrics = state.theme.metrics;
                state.model.ensure_selected_visible(metrics);
                let _ = InvalidateRect(Some(hwnd), None, false);
            }
            LRESULT(0)
        }
        WM_APP_RIGHT => {
            let state = &mut *state_ptr;
            if let Some(grid) = state.model.grid.as_mut() {
                grid.select_right();
                let metrics = state.theme.metrics;
                state.model.ensure_selected_visible(metrics);
                let _ = InvalidateRect(Some(hwnd), None, false);
            }
            LRESULT(0)
        }
        WM_APP_ESCAPE => {
            handle_escape(state_ptr);
            LRESULT(0)
        }
        WM_APP_RETURN => {
            let selected = active_selected_index(&(*state_ptr).model);
            dispatch(state_ptr, PaletteEvent::Activated(selected));
            LRESULT(0)
        }
        WM_APP_CTRL_RETURN => {
            let selected = active_selected_index(&(*state_ptr).model);
            dispatch(state_ptr, PaletteEvent::SecondaryActivated(selected));
            LRESULT(0)
        }
        WM_APP_ACTION => {
            let action = match wparam.0 {
                ACTION_TAB => Some(ActionKey::Tab),
                ACTION_CTRL_P => Some(ActionKey::CtrlP),
                ACTION_CTRL_K => Some(ActionKey::CtrlK),
                ACTION_CTRL_DELETE => Some(ActionKey::CtrlDelete),
                _ => None,
            };
            if let Some(key) = action {
                let selected = active_selected_index(&(*state_ptr).model);
                dispatch(state_ptr, PaletteEvent::Action(key, selected));
            }
            LRESULT(0)
        }
        WM_APP_SETTINGS => {
            clear_edit_silently(state_ptr);
            dispatch(state_ptr, PaletteEvent::OpenSettings);
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
        WM_APP_REFRESH => {
            if IsWindowVisible(hwnd).as_bool() {
                dispatch_refresh(state_ptr);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

fn active_selected_index(model: &Model) -> usize {
    if let Some(grid) = model.grid.as_ref() {
        grid.selected
    } else {
        model.selected
    }
}

/// Convert physical mouse coordinates into exact 1:1 DIPs and hit-test against
/// the exact same `Model::hit_test_dip` geometry that `Renderer::draw` paints.
unsafe fn hit_test_point(hwnd: HWND, state: &State, x_phys: f32, y_phys: f32) -> Option<usize> {
    let dpi_scale = GetDpiForWindow(hwnd).max(96) as f32 / 96.0;
    let x_dip = x_phys / dpi_scale;
    let y_dip = y_phys / dpi_scale;
    state.model.hit_test_dip(x_dip, y_dip, state.theme.metrics)
}

/// Subclassed edit control: forwards navigation keys & action chords cleanly to
/// the palette while letting Windows handle all typing, paste, selection, and IME.
unsafe extern "system" fn edit_procedure(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let parent = GetParent(hwnd).unwrap_or_default();
    let pointer = GetWindowLongPtrW(parent, GWLP_USERDATA) as *mut State;

    // Copy needed values up front; never hold &State across SendMessageW.
    let (edit_proc, in_grid) = if !pointer.is_null() {
        ((*pointer).edit_proc, (*pointer).model.grid.is_some())
    } else {
        (0, false)
    };

    if message == WM_CHAR {
        // Suppress system beep / control char box on Enter, Ctrl+Enter, Escape, Tab,
        // Ctrl+P, Ctrl+K, and Ctrl+Shift+Backspace (0x7F DEL).
        let ch = wparam.0 as u32;
        if matches!(ch, 0x09 | 0x0A | 0x0B | 0x0D | 0x10 | 0x1B | 0x7F) {
            return LRESULT(0);
        }
    }

    if message == WM_KEYDOWN {
        let key = wparam.0 as u16;
        let ctrl_down = (GetKeyState(VK_CONTROL.0 as i32) as u16 & 0x8000) != 0;
        let shift_down = (GetKeyState(VK_SHIFT.0 as i32) as u16 & 0x8000) != 0;
        let empty_text = GetWindowTextLengthW(hwnd) == 0;

        let forwarded: Option<(u32, usize)> = if ctrl_down && key == VK_OEM_COMMA.0 {
            Some((WM_APP_SETTINGS, 0))
        } else if ctrl_down && key == VK_RETURN.0 {
            Some((WM_APP_CTRL_RETURN, 0))
        } else if ctrl_down && key == VK_P.0 {
            Some((WM_APP_ACTION, ACTION_CTRL_P))
        } else if ctrl_down && key == VK_K.0 {
            Some((WM_APP_ACTION, ACTION_CTRL_K))
        } else if ctrl_down && (key == VK_DELETE.0 || (shift_down && key == VK_BACK.0)) {
            Some((WM_APP_ACTION, ACTION_CTRL_DELETE))
        } else if key == VK_TAB.0 && !ctrl_down {
            Some((WM_APP_ACTION, ACTION_TAB))
        } else if in_grid && (empty_text || ctrl_down) && key == VK_LEFT.0 {
            Some((WM_APP_LEFT, 0))
        } else if in_grid && (empty_text || ctrl_down) && key == VK_RIGHT.0 {
            Some((WM_APP_RIGHT, 0))
        } else {
            match key {
                value if value == VK_UP.0 => Some((WM_APP_UP, 0)),
                value if value == VK_DOWN.0 => Some((WM_APP_DOWN, 0)),
                value if value == VK_ESCAPE.0 => Some((WM_APP_ESCAPE, 0)),
                value if value == VK_RETURN.0 => Some((WM_APP_RETURN, 0)),
                _ => None,
            }
        };
        if let Some((custom, arg)) = forwarded {
            let _ = SendMessageW(parent, custom, Some(WPARAM(arg)), Some(LPARAM(0)));
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

/// First Escape clears non-empty text; Escape with an empty search box asks the
/// app handler (`PaletteEvent::Escape`) whether to go back a screen or dismiss.
unsafe fn handle_escape(state_ptr: *mut State) {
    let edit = (*state_ptr).edit;
    let has_text = GetWindowTextLengthW(edit) > 0;
    if has_text {
        clear_edit_silently(state_ptr);
        (*state_ptr).model.query = String::new();
        dispatch(state_ptr, PaletteEvent::QueryChanged(String::new()));
    } else {
        dispatch(state_ptr, PaletteEvent::Escape);
    }
}

unsafe fn clear_edit_silently(state_ptr: *mut State) {
    let edit = (*state_ptr).edit;
    (*state_ptr).suppress_en_change = true;
    let empty: [u16; 1] = [0];
    let _ = SendMessageW(
        edit,
        WM_SETTEXT,
        Some(WPARAM(0)),
        Some(LPARAM(empty.as_ptr() as isize)),
    );
    (*state_ptr).suppress_en_change = false;
}

unsafe fn read_edit_text(edit: HWND) -> String {
    let len = GetWindowTextLengthW(edit).max(0) as usize;
    if len == 0 {
        return String::new();
    }
    let mut buffer = vec![0u16; len + 1];
    let read = GetWindowTextW(edit, &mut buffer).max(0) as usize;
    String::from_utf16_lossy(&buffer[..read])
}

unsafe fn set_cue_banner(edit: HWND, text: &str) {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    // WPARAM(1) keeps the cue banner visible while focused until the user types.
    let _ = SendMessageW(
        edit,
        EM_SETCUEBANNER,
        Some(WPARAM(1)),
        Some(LPARAM(wide.as_ptr() as isize)),
    );
}

unsafe fn show_window_without_shown_event(state_ptr: *mut State) {
    clear_edit_silently(state_ptr);
    let (hwnd, edit, metrics, target_h) = {
        let state = &mut *state_ptr;
        state.last_blur_dismiss = None;
        state.model.query = String::new();
        state.model.scroll_offset = 0.0;
        let m = state.theme.metrics;
        (
            state.hwnd,
            state.edit,
            m,
            state.model.layout(m).window_height,
        )
    };

    place_window_rect(hwnd, metrics, target_h);
    let _ = ShowWindow(hwnd, SW_SHOW);
    bring_to_foreground_and_focus(hwnd, edit);
}

/// Reliably steals foreground focus and places the blinking keyboard caret
/// inside `edit` so the user can type immediately without clicking the mouse.
unsafe fn bring_to_foreground_and_focus(hwnd: HWND, edit: HWND) {
    let foreground = GetForegroundWindow();
    let our_thread = GetCurrentThreadId();
    let fg_thread = if !foreground.0.is_null() {
        GetWindowThreadProcessId(foreground, None)
    } else {
        0
    };

    let attached = if fg_thread != 0 && fg_thread != our_thread {
        AttachThreadInput(our_thread, fg_thread, true).as_bool()
    } else {
        false
    };

    let _ = BringWindowToTop(hwnd);
    let _ = SetForegroundWindow(hwnd);
    if !edit.0.is_null() && IsWindow(Some(edit)).as_bool() {
        let _ = SetFocus(Some(edit));
        let _ = SendMessageW(edit, EM_SETSEL, Some(WPARAM(0)), Some(LPARAM(0)));
    }

    if attached {
        let _ = AttachThreadInput(our_thread, fg_thread, false);
    }
}

unsafe fn show_window(state_ptr: *mut State) {
    show_window_without_shown_event(state_ptr);
    dispatch(state_ptr, PaletteEvent::Shown);
}

unsafe fn apply_theme_live(state_ptr: *mut State, new_theme: Theme) {
    let hwnd = (*state_ptr).hwnd;
    let edit = (*state_ptr).edit;
    let new_brush = CreateSolidBrush(colorref(new_theme.field));
    let old_brush = (*state_ptr).field_brush.replace(new_brush);
    if let Some(old) = old_brush {
        let _ = DeleteObject(old.into());
    }
    (*state_ptr).theme = new_theme;
    apply_dwm_chrome(hwnd, &new_theme);
    let _ = InvalidateRect(Some(edit), None, true);
}

unsafe fn apply_dwm_chrome(hwnd: HWND, theme: &Theme) {
    let corner = DWMWCP_ROUND;
    let _ = DwmSetWindowAttribute(
        hwnd,
        DWMWA_WINDOW_CORNER_PREFERENCE,
        &corner as *const _ as *const c_void,
        std::mem::size_of_val(&corner) as u32,
    );

    let dark = BOOL::from(theme.appearance == Appearance::Dark);
    let _ = DwmSetWindowAttribute(
        hwnd,
        DWMWA_USE_IMMERSIVE_DARK_MODE,
        &dark as *const _ as *const c_void,
        std::mem::size_of_val(&dark) as u32,
    );

    let border_rgb = match theme.appearance {
        Appearance::Dark => colorref(Color::rgb(0x32, 0x34, 0x42)),
        Appearance::Light => colorref(Color::rgb(0xd4, 0xd7, 0xe2)),
    };
    let _ = DwmSetWindowAttribute(
        hwnd,
        DWMWA_BORDER_COLOR,
        &border_rgb as *const _ as *const c_void,
        std::mem::size_of_val(&border_rgb) as u32,
    );
}

/// Dispatches an event to the app handler without holding `&mut State` across
/// the callback or any Win32 sizing/visibility calls.
unsafe fn dispatch(state_ptr: *mut State, event: PaletteEvent) {
    let handler = {
        let state = &mut *state_ptr;
        let placeholder: Handler = Box::new(crate::event::ignore);
        std::mem::replace(&mut state.handler, placeholder)
    };
    let reply = handler(event);
    let (hwnd, edit) = {
        let state = &mut *state_ptr;
        state.handler = handler;
        (state.hwnd, state.edit)
    };

    match reply {
        PaletteReply::Keep => {}
        PaletteReply::Update(model) => {
            apply_model_update(state_ptr, hwnd, edit, model);
        }
        PaletteReply::Navigate(model) => {
            clear_edit_silently(state_ptr);
            apply_model_update(state_ptr, hwnd, edit, model);
        }
        PaletteReply::UpdateTheme(new_theme, model) => {
            apply_theme_live(state_ptr, *new_theme);
            apply_model_update(state_ptr, hwnd, edit, model);
        }
        PaletteReply::Close => {
            let _ = ShowWindow(hwnd, SW_HIDE);
        }
        PaletteReply::Quit => {
            let _ = DestroyWindow(hwnd);
        }
    }
}

/// Silent in-place refresh that preserves current selection, hover, and scroll
/// offset so background icon loads never jump the user's cursor or selection.
unsafe fn dispatch_refresh(state_ptr: *mut State) {
    let handler = {
        let state = &mut *state_ptr;
        let placeholder: Handler = Box::new(crate::event::ignore);
        std::mem::replace(&mut state.handler, placeholder)
    };
    let reply = handler(PaletteEvent::Refresh);
    let hwnd = {
        let state = &mut *state_ptr;
        state.handler = handler;
        state.hwnd
    };
    if let PaletteReply::Update(mut model) = reply {
        let (metrics, target_h) = {
            let state = &mut *state_ptr;
            let metrics = state.theme.metrics;
            let max_idx = model.rows.len().saturating_sub(1);
            model.selected = state.model.selected.min(max_idx);
            model.hovered = state.model.hovered.filter(|&i| i < model.rows.len());
            model.scroll_offset = state.model.scroll_offset;
            model.ensure_selected_visible(metrics);
            let target_h = model.layout(metrics).window_height;
            state.model = model;
            (metrics, target_h)
        };
        resize_window_rect(hwnd, metrics, target_h);
        let _ = InvalidateRect(Some(hwnd), None, false);
    }
}

unsafe fn apply_model_update(state_ptr: *mut State, hwnd: HWND, edit: HWND, mut model: Model) {
    let (metrics, placeholder, has_badge, target_h) = {
        let state = &mut *state_ptr;
        let metrics = state.theme.metrics;
        model.ensure_selected_visible(metrics);
        let placeholder = if model.placeholder.is_empty() {
            DEFAULT_PLACEHOLDER.to_string()
        } else {
            model.placeholder.clone()
        };
        let has_badge = model.header_badge.is_some();
        let target_h = model.layout(metrics).window_height;
        state.model = model;
        (metrics, placeholder, has_badge, target_h)
    };
    set_cue_banner(edit, &placeholder);
    resize_window_rect(hwnd, metrics, target_h);
    let dpi = GetDpiForWindow(hwnd).max(96) as f32;
    let width_px = (metrics.width * (dpi / 96.0)).round() as i32;
    place_field_rect(hwnd, edit, metrics, width_px, has_badge);
    let _ = InvalidateRect(Some(hwnd), None, false);
}

unsafe fn dismiss(state_ptr: *mut State) {
    let hwnd = (*state_ptr).hwnd;
    dispatch(state_ptr, PaletteEvent::Dismissed);
    if IsWindow(Some(hwnd)).as_bool() && IsWindowVisible(hwnd).as_bool() {
        let _ = ShowWindow(hwnd, SW_HIDE);
    }
}

fn create_edit_font(dpi: u32) -> HFONT {
    // 13pt (~17.3 DIPs) crisp Segoe UI for the header search input.
    let height = -unsafe { MulDiv(13, dpi as i32, 72) };
    unsafe {
        CreateFontW(
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
        )
    }
}

/// Sizes the palette for the monitor the cursor is currently on and places it
/// centered horizontally and cleanly in the upper third.
fn place_window_rect(hwnd: HWND, metrics: Metrics, window_height_dip: f32) {
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
    let width = (metrics.width * scale).round() as i32;
    let height = (window_height_dip * scale).round() as i32;

    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    let _ = unsafe { GetMonitorInfoW(monitor, &mut info) };

    let x = info.rcWork.left + (info.rcWork.right - info.rcWork.left - width) / 2;
    let y = info.rcWork.top + (info.rcWork.bottom - info.rcWork.top - height) / 4;

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

/// Resizes the window to fit its content in exact monitor DPI physical pixels.
fn resize_window_rect(hwnd: HWND, metrics: Metrics, window_height_dip: f32) {
    let dpi = unsafe { GetDpiForWindow(hwnd).max(96) } as f32;
    resize_window_rect_for_dpi(hwnd, metrics, window_height_dip, dpi);
}

fn resize_window_rect_for_dpi(hwnd: HWND, metrics: Metrics, window_height_dip: f32, dpi: f32) {
    let scale = dpi.max(96.0) / 96.0;
    let width = (metrics.width * scale).round() as i32;
    let height = (window_height_dip * scale).round() as i32;
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

/// Vertically centers the single-line Win32 `EDIT` control cleanly inside the
/// top search header bar.
fn place_field_rect(
    hwnd: HWND,
    edit: HWND,
    metrics: Metrics,
    window_width_px: i32,
    has_badge: bool,
) {
    if edit.0.is_null() {
        return;
    }
    let dpi = unsafe { GetDpiForWindow(hwnd).max(96) } as f32;
    let scale = dpi / 96.0;
    let left = (metrics.field_inset * scale).round() as i32;
    let right_reserve = if has_badge { 112.0 } else { 18.0 };
    let width = (window_width_px - left - (right_reserve * scale).round() as i32).max(60);
    // 13pt Segoe UI line box is ~22 DIPs tall; centering inside 54 DIPs puts the
    // baseline dead-center with the magnifying glass icon.
    let edit_h_dip = 24.0_f32;
    let top_dip = ((metrics.field_height - edit_h_dip) / 2.0).max(4.0);
    let top = (top_dip * scale).round() as i32;
    let height = (edit_h_dip * scale).round() as i32;
    unsafe {
        let _ = SetWindowPos(edit, None, left, top, width, height, SWP_NOZORDER);
    }
}

/// A theme colour in GDI `COLORREF` (`0x00BBGGRR`) layout.
fn colorref(color: Color) -> COLORREF {
    let r = (color.red.clamp(0.0, 1.0) * 255.0).round() as u32;
    let g = (color.green.clamp(0.0, 1.0) * 255.0).round() as u32;
    let b = (color.blue.clamp(0.0, 1.0) * 255.0).round() as u32;
    COLORREF(r | (g << 8) | (b << 16))
}

unsafe fn set_dpi_aware() {
    let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
}

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
