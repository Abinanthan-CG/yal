//! The native Win32 palette window.
//!
//! Borderless, always-on-top, rounded Windows 11 DWM frame painted in 1:1 DIPs
//! by Direct2D, paired with a seamlessly styled native Win32 `EDIT` control
//! using `SetFocus` (instant typing on open) and `EM_SETCUEBANNER` (native
//! placeholder text with zero fake-text hacks).

use std::cell::Cell;
use std::ffi::c_void;
use std::time::{Duration, Instant};

use windows::core::{w, BOOL, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE,
    DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontW, CreateSolidBrush, DeleteObject, EndPaint, GetMonitorInfoW,
    InvalidateRect, MonitorFromPoint, MonitorFromRect, ScreenToClient, SetBkColor, SetTextColor,
    HBRUSH, HDC, HFONT, MONITORINFO, MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTONULL, PAINTSTRUCT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{
    AttachThreadInput, GetCurrentProcessId, GetCurrentThreadId,
};
use windows::Win32::UI::Controls::{
    InitCommonControlsEx, SetWindowTheme, ICC_STANDARD_CLASSES, INITCOMMONCONTROLSEX, WM_MOUSELEAVE,
};
use windows::Win32::UI::HiDpi::{
    GetDpiForMonitor, GetDpiForSystem, GetDpiForWindow, SetProcessDpiAwarenessContext,
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, MDT_EFFECTIVE_DPI,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyState, ReleaseCapture, SendInput, SetCapture, SetFocus,
    TrackMouseEvent, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY,
    KEYEVENTF_KEYUP, TME_LEAVE, TRACKMOUSEEVENT, VK_A, VK_B, VK_BACK, VK_C, VK_CONTROL, VK_D,
    VK_DOWN, VK_E, VK_ESCAPE, VK_F, VK_H, VK_I, VK_J, VK_K, VK_L, VK_LEFT, VK_MENU, VK_N, VK_OEM_4,
    VK_OEM_COMMA, VK_P, VK_R, VK_RETURN, VK_RIGHT, VK_S, VK_SHIFT, VK_TAB, VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, BringWindowToTop, CallNextHookEx, CreateWindowExW, DefWindowProcW,
    DestroyWindow, DispatchMessageW, GetCursorPos, GetForegroundWindow, GetMessageW, GetParent,
    GetWindowLongPtrW, GetWindowRect, GetWindowTextLengthW, GetWindowTextW,
    GetWindowThreadProcessId, IsIconic, IsWindow, IsWindowVisible, LoadCursorW, PeekMessageW,
    PostMessageW, PostQuitMessage, RegisterClassExW, SendMessageW, SetCursor, SetForegroundWindow,
    SetWindowLongPtrW, SetWindowPos, SetWindowsHookExW, ShowWindow, TranslateMessage,
    UnhookWindowsHookEx, ASFW_ANY, CREATESTRUCTW, CS_DBLCLKS, CS_HREDRAW, CS_VREDRAW, EN_CHANGE,
    ES_AUTOHSCROLL, ES_AUTOVSCROLL, ES_LEFT, ES_MULTILINE, ES_WANTRETURN, GWLP_USERDATA,
    GWLP_WNDPROC, HHOOK, HTCAPTION, HTCLIENT, HWND_TOPMOST, IDC_ARROW, IDC_IBEAM, MINMAXINFO, MSG,
    MSLLHOOKSTRUCT, PM_REMOVE, SWP_HIDEWINDOW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SWP_NOZORDER, SW_HIDE, SW_SHOW, SW_SHOWNOACTIVATE, WA_INACTIVE, WH_MOUSE_LL, WINDOW_EX_STYLE,
    WINDOW_STYLE, WM_ACTIVATE, WM_ACTIVATEAPP, WM_CAPTURECHANGED, WM_CHAR, WM_COMMAND, WM_CREATE,
    WM_CTLCOLOREDIT, WM_DESTROY, WM_DPICHANGED, WM_ENTERSIZEMOVE, WM_ERASEBKGND, WM_EXITSIZEMOVE,
    WM_GETMINMAXINFO, WM_KEYDOWN, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN,
    WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCACTIVATE, WM_NCCALCSIZE, WM_NCCREATE, WM_NCHITTEST,
    WM_NCLBUTTONDBLCLK, WM_NCPAINT, WM_PAINT, WM_RBUTTONDOWN, WM_SETCURSOR, WM_SETFOCUS,
    WM_SETFONT, WM_SETTEXT, WM_SIZE, WM_SYSCHAR, WM_SYSKEYDOWN, WNDCLASSEXW, WS_CHILD,
    WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP, WS_TABSTOP,
    WS_VISIBLE, WS_VSCROLL,
};

use crate::event::{ActionKey, Handler, PaletteEvent, PaletteReply};
use crate::model::{Model, MASTER_SPLIT_RATIO};
use crate::paint::{
    detail_preview_text, detail_text_fingerprint, utf16_slice, word_bounds_utf16, DetailTextState,
    Renderer,
};
use crate::theme::{Appearance, Color, Metrics, Theme};

/// Native Win32 cue banner message (ComCtl32 v6, enabled via `yal.manifest`).
const EM_SETCUEBANNER: u32 = 0x1501;
const EM_GETSEL: u32 = 0x00B0;
const EM_SETSEL: u32 = 0x00B1;
const EM_SETLIMITTEXT: u32 = 0x00C5;

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
const WM_APP_ALT_RETURN: u32 = 0x8010;
const WM_APP_SHIFT_RETURN: u32 = 0x8011;
const WM_APP_QUICK_SELECT: u32 = 0x8012;
const WM_APP_COPY_CHORD: u32 = 0x8013;
const WM_APP_MENU_CHAR: u32 = 0x8014;
const WM_APP_RESET_POSITION: u32 = 0x8015;
const WM_APP_OUTSIDE_CLICK: u32 = 0x8016;
const WM_APP_COPY_SELECTION: u32 = 0x8017;
const WM_APP_EDITOR_SUBMIT: u32 = 0x8018;

thread_local! {
    /// Low-level mouse hook installed ONLY while the palette window is visible
    /// so clicking anywhere outside YAL (including the Desktop, Taskbar, or an
    /// elevated window) always dismisses YAL reliably with 0% idle overhead.
    static OUTSIDE_CLICK_HOOK: Cell<isize> = const { Cell::new(0) };
    static OUTSIDE_CLICK_HWND: Cell<isize> = const { Cell::new(0) };
}

unsafe fn install_outside_click_hook(hwnd: HWND) {
    OUTSIDE_CLICK_HWND.with(|slot| slot.set(hwnd.0 as isize));
    let existing = OUTSIDE_CLICK_HOOK.with(Cell::get);
    if existing != 0 {
        return;
    }
    if let Ok(hook) = SetWindowsHookExW(WH_MOUSE_LL, Some(outside_click_mouse_proc), None, 0) {
        OUTSIDE_CLICK_HOOK.with(|slot| slot.set(hook.0 as isize));
    }
}

unsafe fn uninstall_outside_click_hook() {
    let raw = OUTSIDE_CLICK_HOOK.with(|slot| slot.replace(0));
    OUTSIDE_CLICK_HWND.with(|slot| slot.set(0));
    if raw != 0 {
        let _ = UnhookWindowsHookEx(HHOOK(raw as *mut c_void));
    }
}

unsafe extern "system" fn outside_click_mouse_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code >= 0 {
        let msg = wparam.0 as u32;
        if matches!(msg, WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN) {
            let hwnd_raw = OUTSIDE_CLICK_HWND.with(Cell::get);
            if hwnd_raw != 0 {
                let hwnd = HWND(hwnd_raw as *mut c_void);
                if IsWindow(Some(hwnd)).as_bool() && IsWindowVisible(hwnd).as_bool() {
                    let info = &*(lparam.0 as *const MSLLHOOKSTRUCT);
                    let mut rect = RECT::default();
                    if GetWindowRect(hwnd, &mut rect).is_ok() {
                        let pt = info.pt;
                        let inside = pt.x >= rect.left
                            && pt.x < rect.right
                            && pt.y >= rect.top
                            && pt.y < rect.bottom;
                        if !inside {
                            let _ = PostMessageW(
                                Some(hwnd),
                                WM_APP_OUTSIDE_CLICK,
                                WPARAM(0),
                                LPARAM(0),
                            );
                        }
                    }
                }
            }
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

const ACTION_TAB: usize = 1;
const ACTION_CTRL_P: usize = 2;
const ACTION_CTRL_K: usize = 3;
const ACTION_CTRL_DELETE: usize = 4;
const ACTION_SHIFT_TAB: usize = 5;
const ACTION_CTRL_I: usize = 6;
const ACTION_CTRL_H: usize = 7;
const ACTION_CTRL_R: usize = 8;
const ACTION_BACK: usize = 9;
const ACTION_CTRL_E: usize = 10;

/// Debounce window so clicking the tray icon while the palette is open closes
/// the palette rather than blur-dismissing and immediately re-opening it.
const TRAY_BLUR_DEBOUNCE: Duration = Duration::from_millis(250);

/// Grace period right after showing the palette during which transient Win32
/// focus churn (e.g. closing a system tray popup menu) must not auto-dismiss it.
const SHOW_ACTIVATION_GRACE: Duration = Duration::from_millis(220);

/// Default search placeholder when the model does not specify one.
const DEFAULT_PLACEHOLDER: &str = "Search for apps or commands…";

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
    editor_edit: HWND,
    editor_edit_proc: isize,
    editor_font: Option<HFONT>,
    detail_text: DetailTextState,
    editor_active: bool,
    editor_seed: Option<(String, String)>,
    field_brush: Option<HBRUSH>,
    last_blur_dismiss: Option<Instant>,
    last_shown_at: Option<Instant>,
    previous_foreground: Option<HWND>,
    hiding: bool,
    custom_origin: Option<(i32, i32)>,
    in_size_move: bool,
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

/// Clears any dragged custom position and snaps the palette back to center.
pub fn post_reset_position(handle: PaletteHandle) {
    unsafe {
        let _ = PostMessageW(
            Some(handle.hwnd()),
            WM_APP_RESET_POSITION,
            WPARAM(0),
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
            style: CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS,
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
            editor_edit: HWND(std::ptr::null_mut()),
            editor_edit_proc: 0,
            editor_font: None,
            detail_text: DetailTextState::default(),
            editor_active: false,
            editor_seed: None,
            field_brush: None,
            last_blur_dismiss: None,
            last_shown_at: None,
            previous_foreground: None,
            hiding: false,
            custom_origin: None,
            in_size_move: false,
            tracking_mouse_leave: false,
            suppress_en_change: false,
            handler,
        }));

        let hwnd = match unsafe {
            CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
                w!("YalPaletteWindow"),
                w!("YAL"),
                WS_POPUP | WS_CLIPCHILDREN | WS_CLIPSIBLINGS,
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
        place_window_rect(hwnd, metrics, initial_height, None);

        Ok(Self {
            hwnd,
            state: state_ptr,
        })
    }

    /// Seed a persisted custom window position (loaded from settings on startup).
    pub fn set_saved_position(&self, origin: Option<(i32, i32)>) {
        let (hwnd, metrics, target_h) = unsafe {
            let state = &mut *self.state;
            state.custom_origin = origin;
            let m = state.theme.metrics;
            (state.hwnd, m, state.model.layout(m).window_height)
        };
        place_window_rect(hwnd, metrics, target_h, origin);
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
            let state = &mut *self.state;
            state.model = model;
            let m = state.theme.metrics;
            let h = state.model.layout(m).window_height;
            (state.hwnd, m, h)
        };
        unsafe {
            cancel_detail_drag_if_needed(self.state);
        }
        resize_window_rect(hwnd, metrics, target_h);
        unsafe {
            let hidden = !IsWindowVisible(hwnd).as_bool();
            sync_editor_control(self.state, hwnd, metrics, hidden);
        }
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
            hide_palette_window(self.state, false);
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
                let editor_edit = (*self.state).editor_edit;
                let editor_edit_proc = (*self.state).editor_edit_proc;
                if editor_edit_proc != 0
                    && !editor_edit.0.is_null()
                    && IsWindow(Some(editor_edit)).as_bool()
                {
                    let _ = SetWindowLongPtrW(editor_edit, GWLP_WNDPROC, editor_edit_proc);
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
                if let Some(editor_font) = state.editor_font.take() {
                    let _ = DeleteObject(editor_font.into());
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
            let (theme, placeholder) = {
                let state = &*state_ptr;
                let placeholder = if state.model.placeholder.is_empty() {
                    DEFAULT_PLACEHOLDER.to_string()
                } else {
                    state.model.placeholder.clone()
                };
                (state.theme, placeholder)
            };
            let dpi = GetDpiForWindow(hwnd).max(96);
            apply_dwm_chrome(hwnd, &theme);

            let field = CreateSolidBrush(colorref(theme.field));
            {
                (*state_ptr).field_brush = Some(field);
            }

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
            set_cue_banner(edit, &placeholder);
            let previous =
                SetWindowLongPtrW(edit, GWLP_WNDPROC, edit_procedure as *const () as isize);
            {
                let state = &mut *state_ptr;
                state.edit = edit;
                state.font = Some(font);
                state.edit_proc = previous;
            }

            let editor_edit = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("EDIT"),
                PCWSTR::null(),
                WS_CHILD
                    | WS_TABSTOP
                    | WS_VSCROLL
                    | WINDOW_STYLE(ES_MULTILINE as u32)
                    | WINDOW_STYLE(ES_WANTRETURN as u32)
                    | WINDOW_STYLE(ES_AUTOVSCROLL as u32)
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

            let editor_font = create_editor_font(dpi);
            let mut prev_editor = 0isize;
            if !editor_edit.0.is_null() {
                let _ = SendMessageW(
                    editor_edit,
                    EM_SETLIMITTEXT,
                    Some(WPARAM(32_000)),
                    Some(LPARAM(0)),
                );
                let _ = SendMessageW(
                    editor_edit,
                    WM_SETFONT,
                    Some(WPARAM(editor_font.0 as usize)),
                    Some(LPARAM(1)),
                );
                apply_editor_theme(editor_edit, theme.appearance);
                prev_editor = SetWindowLongPtrW(
                    editor_edit,
                    GWLP_WNDPROC,
                    editor_edit_procedure as *const () as isize,
                );
            }
            {
                let state = &mut *state_ptr;
                state.editor_edit = editor_edit;
                state.editor_font = Some(editor_font);
                state.editor_edit_proc = prev_editor;
            }

            if let Ok(renderer) = Renderer::new(hwnd, dpi as f32) {
                (*state_ptr).renderer = Some(renderer);
            }
            LRESULT(0)
        }
        WM_DPICHANGED => {
            let new_dpi = (wparam.0 & 0xffff) as u32;
            let dpi = new_dpi.max(96);
            let new_font = create_edit_font(dpi);
            let new_editor_font = create_editor_font(dpi);
            let (edit, editor_edit, old_font, old_editor_font, metrics, layout) = {
                let state = &mut *state_ptr;
                if let Some(renderer) = state.renderer.as_ref() {
                    renderer.set_dpi(dpi as f32);
                }
                let old_font = state.font.replace(new_font);
                let old_editor_font = state.editor_font.replace(new_editor_font);
                let m = state.theme.metrics;
                (
                    state.edit,
                    state.editor_edit,
                    old_font,
                    old_editor_font,
                    m,
                    state.model.layout(m),
                )
            };
            let _ = SendMessageW(
                edit,
                WM_SETFONT,
                Some(WPARAM(new_font.0 as usize)),
                Some(LPARAM(1)),
            );
            if !editor_edit.0.is_null() {
                let _ = SendMessageW(
                    editor_edit,
                    WM_SETFONT,
                    Some(WPARAM(new_editor_font.0 as usize)),
                    Some(LPARAM(1)),
                );
            }
            if let Some(old) = old_font {
                let _ = DeleteObject(old.into());
            }
            if let Some(old) = old_editor_font {
                let _ = DeleteObject(old.into());
            }
            resize_window_rect_for_dpi(hwnd, metrics, layout.window_height, dpi as f32);
            let width_px = ((metrics.width * (dpi as f32 / 96.0)).round()) as i32;
            let (has_badge, inline_pill_w) = {
                let state = &*state_ptr;
                (
                    state.model.header_badge.is_some(),
                    inline_arg_pill_width(&state.model),
                )
            };
            place_field_rect(hwnd, edit, metrics, width_px, has_badge, inline_pill_w);
            place_editor_rect(
                hwnd,
                editor_edit,
                metrics,
                width_px,
                layout.list_top,
                layout.list_bottom,
            );
            let _ = InvalidateRect(Some(hwnd), None, false);
            LRESULT(0)
        }
        WM_SIZE => {
            let width = (lparam.0 & 0xffff) as u32;
            let height = ((lparam.0 >> 16) & 0xffff) as u32;
            let dpi = GetDpiForWindow(hwnd).max(96) as f32;
            let (edit, editor_edit, metrics, has_badge, inline_pill_w, list_top, list_bottom) = {
                let state = &mut *state_ptr;
                if let Some(renderer) = state.renderer.as_ref() {
                    renderer.set_dpi(dpi);
                    renderer.resize(width, height);
                }
                let m = state.theme.metrics;
                let layout = state.model.layout(m);
                (
                    state.edit,
                    state.editor_edit,
                    m,
                    state.model.header_badge.is_some(),
                    inline_arg_pill_width(&state.model),
                    layout.list_top,
                    layout.list_bottom,
                )
            };
            place_field_rect(hwnd, edit, metrics, width as i32, has_badge, inline_pill_w);
            place_editor_rect(
                hwnd,
                editor_edit,
                metrics,
                width as i32,
                list_top,
                list_bottom,
            );
            LRESULT(0)
        }
        WM_SETFOCUS => {
            // Whenever the top-level palette window receives focus, immediately
            // hand keyboard focus to the active input (multi-line editor or search box).
            let target = if (*state_ptr).model.editor.is_some() {
                (*state_ptr).editor_edit
            } else {
                (*state_ptr).edit
            };
            if !target.0.is_null() && IsWindow(Some(target)).as_bool() {
                let _ = SetFocus(Some(target));
            }
            LRESULT(0)
        }
        WM_PAINT => {
            let mut paint = std::mem::zeroed::<PAINTSTRUCT>();
            let _ = BeginPaint(hwnd, &mut paint);
            let release_capture = {
                let state = &mut *state_ptr;
                let was_dragging = state.detail_text.dragging;
                if let Some(renderer) = state.renderer.as_mut() {
                    renderer.draw(&state.model, &state.theme, &mut state.detail_text);
                }
                was_dragging && !state.detail_text.dragging
            };
            let _ = EndPaint(hwnd, &paint);
            if release_capture {
                let _ = ReleaseCapture();
            }
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_COMMAND => {
            let code = (wparam.0 >> 16) & 0xffff;
            let source = HWND(lparam.0 as *mut c_void);
            if code as u32 == EN_CHANGE
                && source == (*state_ptr).edit
                && !(*state_ptr).suppress_en_change
            {
                let edit = (*state_ptr).edit;
                let text = read_edit_text(edit);
                (*state_ptr).model.query = text.clone();
                (*state_ptr).detail_text.selection = None;
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
        WM_NCHITTEST => {
            let mut pt = POINT {
                x: (lparam.0 & 0xffff) as i16 as i32,
                y: ((lparam.0 >> 16) & 0xffff) as i16 as i32,
            };
            let _ = ScreenToClient(hwnd, &mut pt);
            let dpi_scale = GetDpiForWindow(hwnd).max(96) as f32 / 96.0;
            let x_dip = pt.x as f32 / dpi_scale;
            let y_dip = pt.y as f32 / dpi_scale;
            let state = &*state_ptr;
            let metrics = state.theme.metrics;
            let win_h = state.model.layout(metrics).window_height;

            if state.model.action_menu.is_some() {
                return LRESULT(HTCLIENT as isize);
            }

            // Keep header back button and footer right-side Actions button clickable.
            if (state.model.back_button || state.model.editor.is_some())
                && x_dip <= 44.0
                && y_dip <= metrics.field_height
            {
                return LRESULT(HTCLIENT as isize);
            }
            if y_dip >= win_h - metrics.footer_height && x_dip >= metrics.width - 135.0 {
                return LRESULT(HTCLIENT as isize);
            }

            // Allow dragging the palette by its header chrome, bottom footer bar,
            // or outer border padding (the child EDIT control owns its own box).
            let in_header = y_dip <= metrics.field_height;
            let in_footer = y_dip >= win_h - metrics.footer_height;
            let in_edge = x_dip <= metrics.padding || x_dip >= metrics.width - metrics.padding;
            if in_header || in_footer || in_edge {
                LRESULT(HTCAPTION as isize)
            } else {
                LRESULT(HTCLIENT as isize)
            }
        }
        WM_NCLBUTTONDBLCLK => {
            // Double-clicking draggable header/footer chrome re-centers the palette
            // instead of letting DefWindowProcW maximize the borderless window.
            if wparam.0 as u32 == HTCAPTION {
                let (metrics, target_h) = {
                    let state = &mut *state_ptr;
                    state.custom_origin = None;
                    let m = state.theme.metrics;
                    (m, state.model.layout(m).window_height)
                };
                place_window_rect(hwnd, metrics, target_h, None);
                dispatch(state_ptr, PaletteEvent::WindowMoved(None));
                return LRESULT(0);
            }
            DefWindowProcW(hwnd, message, wparam, lparam)
        }
        WM_ENTERSIZEMOVE => {
            (*state_ptr).in_size_move = true;
            LRESULT(0)
        }
        WM_EXITSIZEMOVE => {
            (*state_ptr).in_size_move = false;
            let mut rect = RECT::default();
            if GetWindowRect(hwnd, &mut rect).is_ok() {
                let origin = (rect.left, rect.top);
                (*state_ptr).custom_origin = Some(origin);
                dispatch(state_ptr, PaletteEvent::WindowMoved(Some(origin)));
            }
            LRESULT(0)
        }
        WM_SETCURSOR => {
            if (lparam.0 & 0xffff) as u32 == HTCLIENT {
                let mut pt = POINT::default();
                if GetCursorPos(&mut pt).is_ok() {
                    let _ = ScreenToClient(hwnd, &mut pt);
                    let dpi_scale = GetDpiForWindow(hwnd).max(96) as f32 / 96.0;
                    let x_dip = pt.x as f32 / dpi_scale;
                    let y_dip = pt.y as f32 / dpi_scale;
                    let state = &*state_ptr;
                    if state.model.action_menu.is_none()
                        && state.model.editor.is_none()
                        && state.detail_text.contains_point(x_dip, y_dip)
                    {
                        if let Ok(ibeam) = LoadCursorW(None, IDC_IBEAM) {
                            let _ = SetCursor(Some(ibeam));
                            return LRESULT(1);
                        }
                    }
                }
            }
            DefWindowProcW(hwnd, message, wparam, lparam)
        }
        WM_MOUSEMOVE => {
            let x_phys = (lparam.0 & 0xffff) as i16 as f32;
            let y_phys = ((lparam.0 >> 16) & 0xffff) as i16 as f32;
            if !(*state_ptr).tracking_mouse_leave {
                let mut tme = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                if TrackMouseEvent(&mut tme).is_ok() {
                    (*state_ptr).tracking_mouse_leave = true;
                }
            }
            let dpi_scale = GetDpiForWindow(hwnd).max(96) as f32 / 96.0;
            let x_dip = x_phys / dpi_scale;
            let y_dip = y_phys / dpi_scale;

            if (*state_ptr).detail_text.dragging {
                let aborted = {
                    let state = &mut *state_ptr;
                    if let (Some(detail), Some(renderer), Some((_, vp_t, _, vp_b))) = (
                        state.model.detail.as_ref(),
                        state.renderer.as_ref(),
                        state.detail_text.viewport_rect,
                    ) {
                        if y_dip < vp_t {
                            state
                                .detail_text
                                .scroll_by(-((vp_t - y_dip).min(24.0) * 0.45 + 4.0));
                        } else if y_dip > vp_b {
                            state
                                .detail_text
                                .scroll_by((y_dip - vp_b).min(24.0) * 0.45 + 4.0);
                        }
                        let (text_x, text_y) = state.detail_text.text_origin;
                        let text_w = state.detail_text.text_width;
                        let scroll = state.detail_text.scroll_offset;
                        let text = detail_preview_text(detail);
                        if let Some(pos) = renderer.hit_test_detail_text(
                            text,
                            text_w,
                            x_dip - text_x,
                            (y_dip - text_y) + scroll,
                        ) {
                            let anchor = state.detail_text.selection.map(|(a, _)| a).unwrap_or(pos);
                            state.detail_text.selection = Some((anchor, pos));
                        }
                        false
                    } else {
                        state.detail_text.dragging = false;
                        true
                    }
                };
                if aborted {
                    let _ = ReleaseCapture();
                } else {
                    let _ = InvalidateRect(Some(hwnd), None, false);
                }
                return LRESULT(0);
            }

            let needs_repaint = {
                let state = &mut *state_ptr;
                let metrics = state.theme.metrics;
                let win_h = state.model.layout(metrics).window_height;

                if let Some(menu) = state.model.action_menu.as_mut() {
                    let hovered = menu.hit_test_dip(x_dip, y_dip, metrics, win_h);
                    if menu.hovered != hovered {
                        menu.hovered = hovered;
                        true
                    } else {
                        false
                    }
                } else {
                    let hovered = state.model.hit_test_dip(x_dip, y_dip, metrics);
                    if let Some(grid) = state.model.grid.as_mut() {
                        let c = grid.hovered != hovered;
                        grid.hovered = hovered;
                        c
                    } else {
                        let c = state.model.hovered != hovered;
                        state.model.hovered = hovered;
                        c
                    }
                }
            };
            if needs_repaint {
                let _ = InvalidateRect(Some(hwnd), None, false);
            }
            LRESULT(0)
        }
        WM_MOUSELEAVE => {
            let state = &mut *state_ptr;
            state.tracking_mouse_leave = false;
            let cleared = if let Some(menu) = state.model.action_menu.as_mut() {
                menu.hovered.take().is_some()
            } else if let Some(grid) = state.model.grid.as_mut() {
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
            let mut pt = POINT {
                x: (lparam.0 & 0xffff) as i16 as i32,
                y: ((lparam.0 >> 16) & 0xffff) as i16 as i32,
            };
            let _ = ScreenToClient(hwnd, &mut pt);
            let dpi_scale = GetDpiForWindow(hwnd).max(96) as f32 / 96.0;
            let x_dip = pt.x as f32 / dpi_scale;
            let y_dip = pt.y as f32 / dpi_scale;

            let state = &mut *state_ptr;
            let metrics = state.theme.metrics;
            if state.model.action_menu.is_none()
                && state.model.editor.is_none()
                && state.model.detail.is_some()
                && (state.detail_text.contains_point(x_dip, y_dip)
                    || x_dip >= (metrics.width * MASTER_SPLIT_RATIO).round())
            {
                state.detail_text.scroll_by(-(wheel_delta / 120.0) * 44.0);
                let _ = InvalidateRect(Some(hwnd), None, false);
                return LRESULT(0);
            }
            if state.model.editor.is_none() {
                // One wheel notch (120) scrolls ~1.5 rows cleanly.
                let delta_dip = -(wheel_delta / 120.0) * (metrics.row_height * 1.5);
                state.model.scroll_by(delta_dip, metrics);
                let _ = InvalidateRect(Some(hwnd), None, false);
            }
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let x_phys = (lparam.0 & 0xffff) as i16 as f32;
            let y_phys = ((lparam.0 >> 16) & 0xffff) as i16 as f32;
            let dpi_scale = GetDpiForWindow(hwnd).max(96) as f32 / 96.0;
            let x_dip = x_phys / dpi_scale;
            let y_dip = y_phys / dpi_scale;

            let (started_drag, needs_repaint) = {
                let state = &mut *state_ptr;
                if state.model.action_menu.is_none() && state.model.editor.is_none() {
                    if state.detail_text.contains_point(x_dip, y_dip) {
                        if let (Some(detail), Some(renderer)) =
                            (state.model.detail.as_ref(), state.renderer.as_ref())
                        {
                            let (text_x, text_y) = state.detail_text.text_origin;
                            let text_w = state.detail_text.text_width;
                            let scroll = state.detail_text.scroll_offset;
                            let text = detail_preview_text(detail);
                            if let Some(pos) = renderer.hit_test_detail_text(
                                text,
                                text_w,
                                x_dip - text_x,
                                (y_dip - text_y) + scroll,
                            ) {
                                state.detail_text.selection = Some((pos, pos));
                                state.detail_text.dragging = true;
                                (true, true)
                            } else {
                                (false, false)
                            }
                        } else {
                            (false, false)
                        }
                    } else {
                        (false, state.detail_text.selection.take().is_some())
                    }
                } else {
                    (false, false)
                }
            };
            if started_drag {
                let _ = SetCapture(hwnd);
            }
            if needs_repaint {
                let _ = InvalidateRect(Some(hwnd), None, false);
            }
            LRESULT(0)
        }
        WM_LBUTTONDBLCLK => {
            let x_phys = (lparam.0 & 0xffff) as i16 as f32;
            let y_phys = ((lparam.0 >> 16) & 0xffff) as i16 as f32;
            let dpi_scale = GetDpiForWindow(hwnd).max(96) as f32 / 96.0;
            let x_dip = x_phys / dpi_scale;
            let y_dip = y_phys / dpi_scale;

            let state = &mut *state_ptr;
            if state.model.action_menu.is_none()
                && state.model.editor.is_none()
                && state.detail_text.contains_point(x_dip, y_dip)
            {
                if let (Some(detail), Some(renderer)) =
                    (state.model.detail.as_ref(), state.renderer.as_ref())
                {
                    let (text_x, text_y) = state.detail_text.text_origin;
                    let text_w = state.detail_text.text_width;
                    let scroll = state.detail_text.scroll_offset;
                    let text = detail_preview_text(detail);
                    if let Some(pos) = renderer.hit_test_detail_text(
                        text,
                        text_w,
                        x_dip - text_x,
                        (y_dip - text_y) + scroll,
                    ) {
                        let (ws, we) = word_bounds_utf16(text, pos);
                        state.detail_text.selection = Some((ws, we));
                        state.detail_text.dragging = false;
                        let _ = InvalidateRect(Some(hwnd), None, false);
                        return LRESULT(0);
                    }
                }
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            if (*state_ptr).detail_text.dragging {
                (*state_ptr).detail_text.dragging = false;
                let _ = ReleaseCapture();
                let _ = InvalidateRect(Some(hwnd), None, false);
                return LRESULT(0);
            }

            let x_phys = (lparam.0 & 0xffff) as i16 as f32;
            let y_phys = ((lparam.0 >> 16) & 0xffff) as i16 as f32;
            let dpi_scale = GetDpiForWindow(hwnd).max(96) as f32 / 96.0;
            let x_dip = x_phys / dpi_scale;
            let y_dip = y_phys / dpi_scale;

            let (metrics, win_h, has_menu, has_back) = {
                let state = &*state_ptr;
                let m = state.theme.metrics;
                (
                    m,
                    state.model.layout(m).window_height,
                    state.model.action_menu.is_some(),
                    state.model.back_button || state.model.editor.is_some(),
                )
            };

            // 1. Floating Action Panel click handling.
            if has_menu {
                let action_id = {
                    let state = &mut *state_ptr;
                    if let Some(menu) = state.model.action_menu.as_mut() {
                        if let Some(filtered_idx) = menu.hit_test_dip(x_dip, y_dip, metrics, win_h)
                        {
                            menu.selected = filtered_idx;
                            menu.selected_item().map(|i| i.id.clone())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                };
                if let Some(id) = action_id {
                    dispatch(state_ptr, PaletteEvent::ActionMenuActivated(id));
                } else {
                    let target_h = {
                        let state = &mut *state_ptr;
                        let _ = state.model.action_menu.take();
                        state.model.layout(metrics).window_height
                    };
                    resize_window_rect(hwnd, metrics, target_h);
                    let _ = InvalidateRect(Some(hwnd), None, false);
                    dispatch(state_ptr, PaletteEvent::ActionMenuDismissed);
                }
                return LRESULT(0);
            }

            // 2. Header clickable `←` back button.
            if has_back && x_dip <= 42.0 && y_dip <= metrics.field_height {
                dispatch(state_ptr, PaletteEvent::Escape);
                return LRESULT(0);
            }

            // 3. Footer right-side Actions click zone (`Ctrl+K`).
            if y_dip >= win_h - metrics.footer_height && x_dip >= metrics.width - 125.0 {
                let selected = active_selected_index(&(*state_ptr).model);
                dispatch(state_ptr, PaletteEvent::Action(ActionKey::CtrlK, selected));
                return LRESULT(0);
            }

            // 4. List row or Emoji Grid tile click.
            let clicked = {
                let state = &mut *state_ptr;
                let hit = state.model.hit_test_dip(x_dip, y_dip, metrics);
                if let Some(index) = hit {
                    state.detail_text.selection = None;
                    if let Some(grid) = state.model.grid.as_mut() {
                        grid.selected = index;
                    } else {
                        state.model.selected = index;
                    }
                }
                hit
            };
            if let Some(index) = clicked {
                if (*state_ptr).model.click_selects_only {
                    let _ = InvalidateRect(Some(hwnd), None, false);
                    dispatch(state_ptr, PaletteEvent::SelectionChanged(index));
                } else {
                    dispatch(state_ptr, PaletteEvent::Activated(index));
                }
            }
            LRESULT(0)
        }
        WM_CAPTURECHANGED => {
            (*state_ptr).detail_text.dragging = false;
            LRESULT(0)
        }
        WM_RBUTTONDOWN => {
            let x_phys = (lparam.0 & 0xffff) as i16 as f32;
            let y_phys = ((lparam.0 >> 16) & 0xffff) as i16 as f32;
            let dpi_scale = GetDpiForWindow(hwnd).max(96) as f32 / 96.0;
            let x_dip = x_phys / dpi_scale;
            let y_dip = y_phys / dpi_scale;

            let selected = {
                let state = &*state_ptr;
                if state.model.action_menu.is_none()
                    && state.model.editor.is_none()
                    && state.detail_text.contains_point(x_dip, y_dip)
                {
                    if let (Some(detail), Some((a, b))) = (
                        state.model.detail.as_ref(),
                        state.detail_text.selected_range(),
                    ) {
                        let slice = utf16_slice(detail_preview_text(detail), a, b);
                        if slice.is_empty() {
                            None
                        } else {
                            Some(slice)
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            };
            if let Some(text) = selected {
                dispatch(state_ptr, PaletteEvent::CopySelection(text));
                return LRESULT(0);
            }
            LRESULT(0)
        }
        WM_ACTIVATE => {
            if (wparam.0 & 0xffff) == WA_INACTIVE as usize
                && IsWindowVisible(hwnd).as_bool()
                && !(*state_ptr).in_size_move
                && !(*state_ptr).hiding
            {
                let in_grace = (*state_ptr)
                    .last_shown_at
                    .is_some_and(|at| at.elapsed() < SHOW_ACTIVATION_GRACE);
                if in_grace {
                    let active_edit = if (*state_ptr).model.editor.is_some() {
                        (*state_ptr).editor_edit
                    } else {
                        (*state_ptr).edit
                    };
                    bring_to_foreground_and_focus(hwnd, active_edit, false);
                } else {
                    (*state_ptr).last_blur_dismiss = Some(Instant::now());
                    dismiss(state_ptr, false);
                }
            }
            LRESULT(0)
        }
        WM_ACTIVATEAPP => {
            if wparam.0 == 0
                && IsWindowVisible(hwnd).as_bool()
                && !(*state_ptr).in_size_move
                && !(*state_ptr).hiding
            {
                let in_grace = (*state_ptr)
                    .last_shown_at
                    .is_some_and(|at| at.elapsed() < SHOW_ACTIVATION_GRACE);
                if !in_grace {
                    (*state_ptr).last_blur_dismiss = Some(Instant::now());
                    dismiss(state_ptr, false);
                }
            }
            LRESULT(0)
        }
        WM_APP_OUTSIDE_CLICK => {
            if IsWindowVisible(hwnd).as_bool() && !(*state_ptr).in_size_move && !(*state_ptr).hiding
            {
                let in_grace = (*state_ptr)
                    .last_shown_at
                    .is_some_and(|at| at.elapsed() < SHOW_ACTIVATION_GRACE);
                if !in_grace {
                    (*state_ptr).last_blur_dismiss = Some(Instant::now());
                    dismiss(state_ptr, false);
                }
            }
            LRESULT(0)
        }
        WM_NCCALCSIZE => LRESULT(0),
        WM_NCACTIVATE => LRESULT(1),
        WM_NCPAINT => LRESULT(0),
        WM_GETMINMAXINFO => {
            let info = &mut *(lparam.0 as *mut MINMAXINFO);
            info.ptMaxSize.x = i32::MAX;
            info.ptMaxSize.y = i32::MAX;
            info.ptMaxPosition.x = 0;
            info.ptMaxPosition.y = 0;
            info.ptMinTrackSize.x = 320;
            info.ptMinTrackSize.y = 48;
            LRESULT(0)
        }
        WM_KEYDOWN => {
            let key = wparam.0 as u16;
            let ctrl_down = (GetKeyState(VK_CONTROL.0 as i32) as u16 & 0x8000) != 0;
            let shift_down = (GetKeyState(VK_SHIFT.0 as i32) as u16 & 0x8000) != 0;
            let alt_down = (GetKeyState(VK_MENU.0 as i32) as u16 & 0x8000) != 0;
            if key == VK_ESCAPE.0 {
                let _ = SendMessageW(hwnd, WM_APP_ESCAPE, Some(WPARAM(0)), Some(LPARAM(0)));
            } else if ctrl_down
                && !alt_down
                && !shift_down
                && key == VK_C.0
                && (*state_ptr).model.action_menu.is_none()
                && (*state_ptr).model.editor.is_none()
                && (*state_ptr).detail_text.selected_range().is_some()
            {
                let _ = SendMessageW(
                    hwnd,
                    WM_APP_COPY_SELECTION,
                    Some(WPARAM(0)),
                    Some(LPARAM(0)),
                );
            }
            LRESULT(0)
        }
        WM_APP_UP => {
            let (notify_sel, new_sel) = {
                let state = &mut *state_ptr;
                if let Some(menu) = state.model.action_menu.as_mut() {
                    menu.select_previous();
                    (false, 0)
                } else {
                    let prev = active_selected_index(&state.model);
                    if let Some(grid) = state.model.grid.as_mut() {
                        grid.select_up();
                    } else {
                        state.model.select_previous();
                    }
                    let metrics = state.theme.metrics;
                    state.model.ensure_selected_visible(metrics);
                    let next = active_selected_index(&state.model);
                    if next != prev {
                        state.detail_text.selection = None;
                    }
                    (state.model.detail.is_some() && next != prev, next)
                }
            };
            let _ = InvalidateRect(Some(hwnd), None, false);
            if notify_sel {
                dispatch(state_ptr, PaletteEvent::SelectionChanged(new_sel));
            }
            LRESULT(0)
        }
        WM_APP_DOWN => {
            let is_compact = (*state_ptr).model.compact
                && (*state_ptr).model.action_menu.is_none()
                && (*state_ptr).model.grid.is_none();
            if is_compact {
                dispatch(state_ptr, PaletteEvent::Action(ActionKey::Expand, 0));
                return LRESULT(0);
            }
            let (notify_sel, new_sel) = {
                let state = &mut *state_ptr;
                if let Some(menu) = state.model.action_menu.as_mut() {
                    menu.select_next();
                    (false, 0)
                } else {
                    let prev = active_selected_index(&state.model);
                    if let Some(grid) = state.model.grid.as_mut() {
                        grid.select_down();
                    } else {
                        state.model.select_next();
                    }
                    let metrics = state.theme.metrics;
                    state.model.ensure_selected_visible(metrics);
                    let next = active_selected_index(&state.model);
                    if next != prev {
                        state.detail_text.selection = None;
                    }
                    (state.model.detail.is_some() && next != prev, next)
                }
            };
            let _ = InvalidateRect(Some(hwnd), None, false);
            if notify_sel {
                dispatch(state_ptr, PaletteEvent::SelectionChanged(new_sel));
            }
            LRESULT(0)
        }
        WM_APP_LEFT => {
            let state = &mut *state_ptr;
            if state.model.action_menu.is_none() {
                if let Some(grid) = state.model.grid.as_mut() {
                    grid.select_left();
                    let metrics = state.theme.metrics;
                    state.model.ensure_selected_visible(metrics);
                    let _ = InvalidateRect(Some(hwnd), None, false);
                }
            }
            LRESULT(0)
        }
        WM_APP_RIGHT => {
            let state = &mut *state_ptr;
            if state.model.action_menu.is_none() {
                if let Some(grid) = state.model.grid.as_mut() {
                    grid.select_right();
                    let metrics = state.theme.metrics;
                    state.model.ensure_selected_visible(metrics);
                    let _ = InvalidateRect(Some(hwnd), None, false);
                }
            }
            LRESULT(0)
        }
        WM_APP_MENU_CHAR => {
            let ch = wparam.0 as u32;
            let resize_target = {
                let state = &mut *state_ptr;
                if let Some(menu) = state.model.action_menu.as_mut() {
                    if ch == 0x08 {
                        menu.query.pop();
                        menu.selected = 0;
                    } else if let Some(c) = char::from_u32(ch) {
                        if !c.is_control() {
                            menu.query.push(c);
                            menu.selected = 0;
                        }
                    }
                    let m = state.theme.metrics;
                    Some((m, state.model.layout(m).window_height))
                } else {
                    None
                }
            };
            if let Some((metrics, target_h)) = resize_target {
                resize_window_rect(hwnd, metrics, target_h);
                let _ = InvalidateRect(Some(hwnd), None, false);
            }
            LRESULT(0)
        }
        WM_APP_ESCAPE => {
            let dismissed_menu = {
                let state = &mut *state_ptr;
                if state.model.action_menu.take().is_some() {
                    let m = state.theme.metrics;
                    Some((m, state.model.layout(m).window_height))
                } else {
                    None
                }
            };
            if let Some((metrics, target_h)) = dismissed_menu {
                resize_window_rect(hwnd, metrics, target_h);
                let _ = InvalidateRect(Some(hwnd), None, false);
                dispatch(state_ptr, PaletteEvent::ActionMenuDismissed);
            } else if (*state_ptr).model.editor.is_some() {
                dispatch(state_ptr, PaletteEvent::Escape);
            } else {
                handle_escape(state_ptr);
            }
            LRESULT(0)
        }
        WM_APP_RETURN => {
            let menu_choice = (*state_ptr)
                .model
                .action_menu
                .as_ref()
                .and_then(|m| m.selected_item().map(|i| i.id.clone()));
            if let Some(action_id) = menu_choice {
                dispatch(state_ptr, PaletteEvent::ActionMenuActivated(action_id));
            } else if (*state_ptr).model.action_menu.is_none() {
                let selected = active_selected_index(&(*state_ptr).model);
                dispatch(state_ptr, PaletteEvent::Activated(selected));
            }
            LRESULT(0)
        }
        WM_APP_CTRL_RETURN => {
            let selected = active_selected_index(&(*state_ptr).model);
            dispatch(state_ptr, PaletteEvent::SecondaryActivated(selected));
            LRESULT(0)
        }
        WM_APP_ALT_RETURN => {
            let selected = active_selected_index(&(*state_ptr).model);
            dispatch(state_ptr, PaletteEvent::TertiaryActivated(selected));
            LRESULT(0)
        }
        WM_APP_SHIFT_RETURN => {
            let selected = active_selected_index(&(*state_ptr).model);
            dispatch(state_ptr, PaletteEvent::QuaternaryActivated(selected));
            LRESULT(0)
        }
        WM_APP_QUICK_SELECT => {
            let idx = wparam.0;
            dispatch(state_ptr, PaletteEvent::QuickSelect(idx));
            LRESULT(0)
        }
        WM_APP_COPY_CHORD => {
            let selected = active_selected_index(&(*state_ptr).model);
            dispatch(state_ptr, PaletteEvent::CopyChord(selected));
            LRESULT(0)
        }
        WM_APP_ACTION => {
            let action = match wparam.0 {
                ACTION_TAB => Some(ActionKey::Tab),
                ACTION_SHIFT_TAB => Some(ActionKey::ShiftTab),
                ACTION_CTRL_P => Some(ActionKey::CtrlP),
                ACTION_CTRL_K => Some(ActionKey::CtrlK),
                ACTION_CTRL_I => Some(ActionKey::CtrlI),
                ACTION_CTRL_H => Some(ActionKey::CtrlH),
                ACTION_CTRL_R => Some(ActionKey::CtrlR),
                ACTION_BACK => Some(ActionKey::Back),
                ACTION_CTRL_DELETE => Some(ActionKey::CtrlDelete),
                ACTION_CTRL_E => Some(ActionKey::CtrlE),
                _ => None,
            };
            if let Some(key) = action {
                let selected = active_selected_index(&(*state_ptr).model);
                dispatch(state_ptr, PaletteEvent::Action(key, selected));
            }
            LRESULT(0)
        }
        WM_APP_COPY_SELECTION => {
            let selected = {
                let state = &*state_ptr;
                if let (Some(detail), Some((a, b))) = (
                    state.model.detail.as_ref(),
                    state.detail_text.selected_range(),
                ) {
                    let slice = utf16_slice(detail_preview_text(detail), a, b);
                    if slice.is_empty() {
                        None
                    } else {
                        Some(slice)
                    }
                } else {
                    None
                }
            };
            if let Some(text) = selected {
                dispatch(state_ptr, PaletteEvent::CopySelection(text));
            }
            LRESULT(0)
        }
        WM_APP_EDITOR_SUBMIT => {
            let editor_edit = (*state_ptr).editor_edit;
            if !editor_edit.0.is_null() && IsWindow(Some(editor_edit)).as_bool() {
                let text = read_edit_text(editor_edit).replace("\r\n", "\n");
                dispatch(
                    state_ptr,
                    PaletteEvent::EditorSubmit {
                        text,
                        as_new: wparam.0 != 0,
                    },
                );
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
            if IsWindowVisible(hwnd).as_bool() {
                dismiss(state_ptr, true);
            }
            LRESULT(0)
        }
        WM_APP_TOGGLE_TRAY => {
            if IsWindowVisible(hwnd).as_bool() {
                dismiss(state_ptr, true);
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
            dispatch_command(state_ptr, code);
            LRESULT(0)
        }
        WM_APP_RESET_POSITION => {
            let (metrics, target_h) = {
                let state = &mut *state_ptr;
                state.custom_origin = None;
                let m = state.theme.metrics;
                (m, state.model.layout(m).window_height)
            };
            place_window_rect(hwnd, metrics, target_h, None);
            dispatch(state_ptr, PaletteEvent::WindowMoved(None));
            LRESULT(0)
        }
        WM_APP_REFRESH => {
            if IsWindowVisible(hwnd).as_bool() {
                dispatch_refresh(state_ptr);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            uninstall_outside_click_hook();
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

fn inline_arg_pill_width(model: &Model) -> f32 {
    model
        .inline_arg
        .as_ref()
        .map(|arg| {
            ((arg.command_name.chars().count() as f32 + 2.0) * 6.8 + 26.0).clamp(52.0, 218.0)
        })
        .unwrap_or(0.0)
}

unsafe fn edit_has_selection(hwnd: HWND) -> bool {
    let mut sel_start: u32 = 0;
    let mut sel_end: u32 = 0;
    let _ = SendMessageW(
        hwnd,
        EM_GETSEL,
        Some(WPARAM(&mut sel_start as *mut u32 as usize)),
        Some(LPARAM(&mut sel_end as *mut u32 as isize)),
    );
    sel_start != sel_end
}

/// Subclassed edit control: forwards navigation keys, action chords, Vim/Emacs
/// Alt-motions, Quick-Select `Ctrl+1..9`, and Action Panel filter keys cleanly
/// while letting Windows handle all normal typing, paste, selection, and IME.
unsafe extern "system" fn edit_procedure(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let parent = GetParent(hwnd).unwrap_or_default();
    let pointer = GetWindowLongPtrW(parent, GWLP_USERDATA) as *mut State;

    // Copy needed values up front; never hold &State across SendMessageW.
    let (edit_proc, in_grid, menu_open, in_editor, detail_has_selection) = if !pointer.is_null() {
        (
            (*pointer).edit_proc,
            (*pointer).model.grid.is_some(),
            (*pointer).model.action_menu.is_some(),
            (*pointer).model.editor.is_some(),
            (*pointer).detail_text.selected_range().is_some(),
        )
    } else {
        (0, false, false, false, false)
    };

    // Suppress Windows beep on Alt+key shortcuts we handle.
    if message == WM_SYSCHAR {
        return LRESULT(0);
    }

    if message == WM_CHAR {
        let ch = wparam.0 as u32;
        if menu_open {
            if ch == 0x08 || (ch >= 0x20 && ch != 0x7F) {
                let _ = SendMessageW(
                    parent,
                    WM_APP_MENU_CHAR,
                    Some(WPARAM(ch as usize)),
                    Some(LPARAM(0)),
                );
            }
            return LRESULT(0);
        }
        if ch == 0x03 && !in_editor && detail_has_selection && !edit_has_selection(hwnd) {
            return LRESULT(0);
        }
        // Suppress system beep / control char box on Enter, Ctrl+Enter, Escape, Tab,
        // Ctrl+D/E/H/I/K/P/R, and Ctrl+Shift+Backspace (0x7F DEL).
        if matches!(
            ch,
            0x08 if (GetKeyState(VK_CONTROL.0 as i32) as u16 & 0x8000) != 0
        ) || matches!(
            ch,
            0x04 | 0x05 | 0x09 | 0x0A | 0x0B | 0x0D | 0x10 | 0x12 | 0x1B | 0x7F
        ) {
            return LRESULT(0);
        }
    }

    if message == WM_KEYDOWN || message == WM_SYSKEYDOWN {
        let key = wparam.0 as u16;
        let ctrl_down = (GetKeyState(VK_CONTROL.0 as i32) as u16 & 0x8000) != 0;
        let shift_down = (GetKeyState(VK_SHIFT.0 as i32) as u16 & 0x8000) != 0;
        let alt_down = (GetKeyState(VK_MENU.0 as i32) as u16 & 0x8000) != 0;
        let empty_text = GetWindowTextLengthW(hwnd) == 0;

        // While the floating Action Panel is open, keep keystrokes scoped to it.
        if menu_open {
            let menu_fwd: Option<(u32, usize)> =
                if (ctrl_down && key == VK_K.0) || key == VK_ESCAPE.0 {
                    Some((WM_APP_ESCAPE, 0))
                } else if key == VK_RETURN.0 {
                    Some((WM_APP_RETURN, 0))
                } else if key == VK_UP.0 || (alt_down && (key == VK_K.0 || key == VK_P.0)) {
                    Some((WM_APP_UP, 0))
                } else if key == VK_DOWN.0 || (alt_down && (key == VK_J.0 || key == VK_N.0)) {
                    Some((WM_APP_DOWN, 0))
                } else if key == VK_BACK.0 {
                    Some((WM_APP_MENU_CHAR, 0x08))
                } else {
                    None
                };
            if let Some((custom, arg)) = menu_fwd {
                let _ = SendMessageW(parent, custom, Some(WPARAM(arg)), Some(LPARAM(0)));
                return LRESULT(0);
            }
            if !ctrl_down && !alt_down {
                // Let WM_CHAR fire for printable characters without mutating the EDIT text.
                return LRESULT(0);
            }
        }

        // Ctrl+C copies the Detail preview text selection when the search box itself has no selection.
        if ctrl_down
            && !alt_down
            && !shift_down
            && key == VK_C.0
            && !in_editor
            && detail_has_selection
            && !edit_has_selection(hwnd)
        {
            let _ = SendMessageW(
                parent,
                WM_APP_COPY_SELECTION,
                Some(WPARAM(0)),
                Some(LPARAM(0)),
            );
            return LRESULT(0);
        }

        let forwarded: Option<(u32, usize)> = if ctrl_down && !alt_down && key == VK_OEM_COMMA.0 {
            Some((WM_APP_SETTINGS, 0))
        } else if ctrl_down && !alt_down && key == VK_RETURN.0 {
            Some((WM_APP_CTRL_RETURN, 0))
        } else if alt_down && !ctrl_down && key == VK_RETURN.0 {
            Some((WM_APP_ALT_RETURN, 0))
        } else if shift_down && !ctrl_down && !alt_down && key == VK_RETURN.0 {
            Some((WM_APP_SHIFT_RETURN, 0))
        } else if alt_down && !ctrl_down && key == VK_C.0 {
            Some((WM_APP_COPY_CHORD, 0))
        } else if ctrl_down
            && !alt_down
            && !shift_down
            && (b'1' as u16..=b'9' as u16).contains(&key)
        {
            Some((WM_APP_QUICK_SELECT, (key - b'1' as u16) as usize))
        } else if ctrl_down && !alt_down && key == VK_P.0 {
            Some((WM_APP_ACTION, ACTION_CTRL_P))
        } else if ctrl_down && !alt_down && key == VK_K.0 {
            Some((WM_APP_ACTION, ACTION_CTRL_K))
        } else if ctrl_down && !alt_down && key == VK_I.0 {
            Some((WM_APP_ACTION, ACTION_CTRL_I))
        } else if ctrl_down && !alt_down && key == VK_H.0 {
            Some((WM_APP_ACTION, ACTION_CTRL_H))
        } else if ctrl_down && !alt_down && key == VK_R.0 {
            Some((WM_APP_ACTION, ACTION_CTRL_R))
        } else if ctrl_down && !alt_down && !shift_down && key == VK_E.0 {
            Some((WM_APP_ACTION, ACTION_CTRL_E))
        } else if (ctrl_down && !alt_down && key == VK_OEM_4.0)
            || (alt_down && !ctrl_down && key == VK_LEFT.0)
        {
            Some((WM_APP_ACTION, ACTION_BACK))
        } else if (ctrl_down && !alt_down && !shift_down && key == VK_D.0)
            || (ctrl_down && shift_down && !alt_down && key == VK_BACK.0)
        {
            Some((WM_APP_ACTION, ACTION_CTRL_DELETE))
        } else if key == VK_TAB.0 && !ctrl_down && !alt_down {
            if shift_down {
                Some((WM_APP_ACTION, ACTION_SHIFT_TAB))
            } else {
                Some((WM_APP_ACTION, ACTION_TAB))
            }
        } else if alt_down && !ctrl_down && (key == VK_J.0 || key == VK_N.0) {
            Some((WM_APP_DOWN, 0))
        } else if alt_down && !ctrl_down && (key == VK_K.0 || key == VK_P.0) {
            Some((WM_APP_UP, 0))
        } else if alt_down && !ctrl_down && in_grid && (key == VK_H.0 || key == VK_B.0) {
            Some((WM_APP_LEFT, 0))
        } else if alt_down && !ctrl_down && in_grid && (key == VK_L.0 || key == VK_F.0) {
            Some((WM_APP_RIGHT, 0))
        } else if in_grid && (empty_text || ctrl_down) && key == VK_LEFT.0 {
            Some((WM_APP_LEFT, 0))
        } else if in_grid && (empty_text || ctrl_down) && key == VK_RIGHT.0 {
            Some((WM_APP_RIGHT, 0))
        } else if !alt_down {
            match key {
                value if value == VK_UP.0 => Some((WM_APP_UP, 0)),
                value if value == VK_DOWN.0 => Some((WM_APP_DOWN, 0)),
                value if value == VK_ESCAPE.0 => Some((WM_APP_ESCAPE, 0)),
                value if value == VK_RETURN.0 => Some((WM_APP_RETURN, 0)),
                _ => None,
            }
        } else {
            None
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

/// Subclassed multi-line `EDIT` control for the native text editor surface:
/// handles `Ctrl+Enter` / `Ctrl+S` (plus `Shift` for Save As New), `Ctrl+A`, and `Esc`.
unsafe extern "system" fn editor_edit_procedure(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let parent = GetParent(hwnd).unwrap_or_default();
    let pointer = GetWindowLongPtrW(parent, GWLP_USERDATA) as *mut State;
    let editor_proc = if !pointer.is_null() {
        (*pointer).editor_edit_proc
    } else {
        0
    };

    if message == WM_CHAR {
        let ch = wparam.0 as u32;
        if matches!(ch, 0x01 | 0x0A | 0x13 | 0x1B) {
            return LRESULT(0);
        }
    }

    if message == WM_KEYDOWN {
        let key = wparam.0 as u16;
        let ctrl_down = (GetKeyState(VK_CONTROL.0 as i32) as u16 & 0x8000) != 0;
        let shift_down = (GetKeyState(VK_SHIFT.0 as i32) as u16 & 0x8000) != 0;
        let alt_down = (GetKeyState(VK_MENU.0 as i32) as u16 & 0x8000) != 0;

        if ctrl_down && !alt_down && (key == VK_RETURN.0 || key == VK_S.0) {
            let _ = SendMessageW(
                parent,
                WM_APP_EDITOR_SUBMIT,
                Some(WPARAM(usize::from(shift_down))),
                Some(LPARAM(0)),
            );
            return LRESULT(0);
        }
        if ctrl_down && !alt_down && !shift_down && key == VK_A.0 {
            let _ = SendMessageW(hwnd, EM_SETSEL, Some(WPARAM(0)), Some(LPARAM(-1)));
            return LRESULT(0);
        }
        if key == VK_ESCAPE.0 {
            let _ = SendMessageW(parent, WM_APP_ESCAPE, Some(WPARAM(0)), Some(LPARAM(0)));
            return LRESULT(0);
        }
    }

    if editor_proc != 0 {
        let original: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT =
            std::mem::transmute(editor_proc);
        return original(hwnd, message, wparam, lparam);
    }
    DefWindowProcW(hwnd, message, wparam, lparam)
}

/// First Escape clears non-empty text; Escape with an empty search box asks the
/// app handler (`PaletteEvent::Escape`) whether to go back a screen or dismiss.
/// When Escape closes the palette, any dragged custom position resets so the
/// next open spawns centered on the default active monitor.
unsafe fn handle_escape(state_ptr: *mut State) {
    let edit = (*state_ptr).edit;
    let has_text = GetWindowTextLengthW(edit) > 0;
    if has_text {
        clear_edit_silently(state_ptr);
        (*state_ptr).model.query = String::new();
        dispatch(state_ptr, PaletteEvent::QueryChanged(String::new()));
    } else {
        let hwnd = (*state_ptr).hwnd;
        dispatch(state_ptr, PaletteEvent::Escape);
        if IsWindow(Some(hwnd)).as_bool()
            && !IsWindowVisible(hwnd).as_bool()
            && (*state_ptr).custom_origin.take().is_some()
        {
            dispatch(state_ptr, PaletteEvent::WindowMoved(None));
        }
    }
}

unsafe fn set_edit_text_silently(state_ptr: *mut State, text: &str) {
    let edit = (*state_ptr).edit;
    (*state_ptr).suppress_en_change = true;
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let _ = SendMessageW(
        edit,
        WM_SETTEXT,
        Some(WPARAM(0)),
        Some(LPARAM(wide.as_ptr() as isize)),
    );
    let end = text.encode_utf16().count();
    let _ = SendMessageW(
        edit,
        EM_SETSEL,
        Some(WPARAM(end)),
        Some(LPARAM(end as isize)),
    );
    (*state_ptr).suppress_en_change = false;
}

unsafe fn clear_edit_silently(state_ptr: *mut State) {
    set_edit_text_silently(state_ptr, "");
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

/// Record the active foreground window right before YAL becomes visible,
/// ignoring any window belonging to YAL's own process.
unsafe fn capture_previous_foreground(state_ptr: *mut State) {
    let fg = GetForegroundWindow();
    if fg.0.is_null() || !IsWindow(Some(fg)).as_bool() {
        return;
    }
    let mut pid = 0u32;
    GetWindowThreadProcessId(fg, Some(&mut pid));
    if pid != 0 && pid != GetCurrentProcessId() {
        (*state_ptr).previous_foreground = Some(fg);
    }
}

/// Place the palette cleanly on the active monitor, show it without premature
/// activation bounce, bring it to the foreground, and arm the outside-click hook.
unsafe fn present_palette_window(state_ptr: *mut State, reset_caret: bool) {
    let (hwnd, edit, metrics, target_h, custom_origin, has_badge, inline_pill_w) = {
        let state = &mut *state_ptr;
        state.last_blur_dismiss = None;
        state.last_shown_at = Some(Instant::now());
        let m = state.theme.metrics;
        (
            state.hwnd,
            state.edit,
            m,
            state.model.layout(m).window_height,
            state.custom_origin,
            state.model.header_badge.is_some(),
            inline_arg_pill_width(&state.model),
        )
    };

    place_window_rect(hwnd, metrics, target_h, custom_origin);
    let dpi = GetDpiForWindow(hwnd).max(96) as f32;
    let width_px = (metrics.width * (dpi / 96.0)).round() as i32;
    place_field_rect(hwnd, edit, metrics, width_px, has_badge, inline_pill_w);
    sync_editor_control(state_ptr, hwnd, metrics, true);
    let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    let in_editor = (*state_ptr).model.editor.is_some();
    let active_focus_hwnd = if in_editor {
        (*state_ptr).editor_edit
    } else {
        edit
    };
    bring_to_foreground_and_focus(hwnd, active_focus_hwnd, reset_caret && !in_editor);
    install_outside_click_hook(hwnd);
    let _ = InvalidateRect(Some(hwnd), None, false);
}

/// Hide the palette window cleanly without triggering `ShowWindow(SW_HIDE)`'s
/// default Z-order activation walk (which on Windows 11 can activate
/// `Shell_TrayWnd` or an unrelated background window behind the active app).
/// When `restore_previous` is `true` and YAL currently holds foreground focus,
/// hand foreground directly back to the window that was active before YAL opened.
unsafe fn hide_palette_window(state_ptr: *mut State, restore_previous: bool) {
    let hwnd = (*state_ptr).hwnd;
    if (*state_ptr).detail_text.dragging {
        (*state_ptr).detail_text.dragging = false;
        let _ = ReleaseCapture();
    }
    uninstall_outside_click_hook();
    let prev = (*state_ptr).previous_foreground.take();
    (*state_ptr).hiding = true;
    let was_foreground = IsWindow(Some(hwnd)).as_bool() && GetForegroundWindow() == hwnd;
    let valid_target = if restore_previous && was_foreground {
        prev.filter(|&target| {
            !target.0.is_null()
                && target != hwnd
                && IsWindow(Some(target)).as_bool()
                && IsWindowVisible(target).as_bool()
                && !IsIconic(target).as_bool()
        })
    } else {
        None
    };

    if let Some(target) = valid_target {
        let mut target_pid = 0u32;
        let target_thread = GetWindowThreadProcessId(target, Some(&mut target_pid));
        if target_pid != 0 {
            let _ = AllowSetForegroundWindow(target_pid);
        }
        let _ = SetForegroundWindow(target);
        if GetForegroundWindow() != target {
            let our_thread = GetCurrentThreadId();
            let attached = target_thread != 0
                && target_thread != our_thread
                && AttachThreadInput(our_thread, target_thread, true).as_bool();
            let _ = SetForegroundWindow(target);
            if attached {
                let _ = AttachThreadInput(our_thread, target_thread, false);
            }
        }
    }

    if IsWindow(Some(hwnd)).as_bool() && IsWindowVisible(hwnd).as_bool() {
        let still_foreground = GetForegroundWindow() == hwnd;
        if still_foreground {
            // Fallback: never leave a hidden window holding foreground focus.
            let _ = ShowWindow(hwnd, SW_HIDE);
        } else {
            let _ = SetWindowPos(
                hwnd,
                None,
                0,
                0,
                0,
                0,
                SWP_HIDEWINDOW | SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
            );
        }
    }
    (*state_ptr).hiding = false;
    let _ = AllowSetForegroundWindow(ASFW_ANY);
}

/// Reliably gives foreground focus to YAL without corrupting the Z-order of
/// background windows, and places the keyboard caret inside `edit`.
unsafe fn bring_to_foreground_and_focus(hwnd: HWND, edit: HWND, reset_caret: bool) {
    if GetForegroundWindow() != hwnd {
        let _ = AllowSetForegroundWindow(ASFW_ANY);
        let _ = SetForegroundWindow(hwnd);
    }

    if GetForegroundWindow() != hwnd {
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
        let _ = SetForegroundWindow(hwnd);
        if GetForegroundWindow() != hwnd {
            let _ = BringWindowToTop(hwnd);
            let _ = SetForegroundWindow(hwnd);
        }
        if attached {
            let _ = AttachThreadInput(our_thread, fg_thread, false);
        }
    }

    if GetForegroundWindow() != hwnd {
        // Last-resort UIPI foreground-lock bypass (e.g. when an elevated window
        // currently holds focus and blocks `AttachThreadInput`).
        let alt_physically_down = (GetAsyncKeyState(VK_MENU.0 as i32) as u16 & 0x8000) != 0;
        if !alt_physically_down {
            let pulse = [
                INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: VK_MENU,
                            wScan: 0,
                            dwFlags: KEYEVENTF_EXTENDEDKEY,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                },
                INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: VK_MENU,
                            wScan: 0,
                            dwFlags: KEYEVENTF_EXTENDEDKEY | KEYEVENTF_KEYUP,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                },
            ];
            let _ = SendInput(&pulse, std::mem::size_of::<INPUT>() as i32);
        } else {
            let dummy = [INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_MENU,
                        wScan: 0,
                        dwFlags: KEYEVENTF_EXTENDEDKEY | KEYEVENTF_KEYUP,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            }];
            let _ = SendInput(&dummy, std::mem::size_of::<INPUT>() as i32);
        }
        let _ = SetForegroundWindow(hwnd);
    }

    if !edit.0.is_null() && IsWindow(Some(edit)).as_bool() {
        let _ = SetFocus(Some(edit));
        if reset_caret {
            let _ = SendMessageW(edit, EM_SETSEL, Some(WPARAM(0)), Some(LPARAM(0)));
        }
    }
}

unsafe fn show_window(state_ptr: *mut State) {
    let hwnd = (*state_ptr).hwnd;
    let was_visible = IsWindowVisible(hwnd).as_bool();
    if !was_visible {
        capture_previous_foreground(state_ptr);
    }
    clear_edit_silently(state_ptr);
    {
        let state = &mut *state_ptr;
        state.model.query = String::new();
        state.model.scroll_offset = 0.0;
    }

    // Run `PaletteEvent::Shown` BEFORE making the window visible so Pop-to-Root
    // or saved-query restoration updates the model and window height with zero flicker.
    let reply = invoke_handler(state_ptr, PaletteEvent::Shown);
    let edit = (*state_ptr).edit;
    let mut reset_caret = true;
    match reply {
        PaletteReply::Keep => {}
        PaletteReply::Update(model) => {
            apply_model_update(state_ptr, hwnd, edit, model);
        }
        PaletteReply::UpdateWithEdit(text, model) => {
            reset_caret = false;
            set_edit_text_silently(state_ptr, &text);
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
            hide_palette_window(state_ptr, was_visible);
            return;
        }
        PaletteReply::CloseWithHud(message) => {
            let dark = (*state_ptr).theme.appearance == Appearance::Dark;
            hide_palette_window(state_ptr, was_visible);
            crate::hud::show_hud(message, dark);
            return;
        }
        PaletteReply::Quit => {
            uninstall_outside_click_hook();
            (*state_ptr).previous_foreground = None;
            let _ = DestroyWindow(hwnd);
            return;
        }
    }

    present_palette_window(state_ptr, reset_caret);
}

/// Dispatch an application command (`WM_APP_COMMAND`, e.g. action global hotkey
/// or tray Settings) *before* changing window visibility so toggling an already-
/// open screen off never flashes or churns focus, and opening a screen while
/// hidden builds its model and height before showing it.
unsafe fn dispatch_command(state_ptr: *mut State, code: u32) {
    let hwnd = (*state_ptr).hwnd;
    let was_visible = IsWindowVisible(hwnd).as_bool();
    if !was_visible {
        capture_previous_foreground(state_ptr);
    }

    let reply = invoke_handler(state_ptr, PaletteEvent::Command(code));
    let edit = (*state_ptr).edit;
    let mut reset_caret = true;
    match reply {
        PaletteReply::Keep => {
            if !was_visible {
                (*state_ptr).previous_foreground = None;
            }
            return;
        }
        PaletteReply::Close => {
            hide_palette_window(state_ptr, was_visible);
            return;
        }
        PaletteReply::CloseWithHud(message) => {
            let dark = (*state_ptr).theme.appearance == Appearance::Dark;
            hide_palette_window(state_ptr, was_visible);
            crate::hud::show_hud(message, dark);
            return;
        }
        PaletteReply::Quit => {
            uninstall_outside_click_hook();
            (*state_ptr).previous_foreground = None;
            let _ = DestroyWindow(hwnd);
            return;
        }
        PaletteReply::Update(model) => {
            apply_model_update(state_ptr, hwnd, edit, model);
        }
        PaletteReply::UpdateWithEdit(text, model) => {
            reset_caret = false;
            set_edit_text_silently(state_ptr, &text);
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
    }

    if !was_visible {
        present_palette_window(state_ptr, reset_caret);
    } else {
        let metrics = (*state_ptr).theme.metrics;
        sync_editor_control(state_ptr, hwnd, metrics, false);
        (*state_ptr).last_shown_at = Some(Instant::now());
        let in_editor = (*state_ptr).model.editor.is_some();
        let active_focus_hwnd = if in_editor {
            (*state_ptr).editor_edit
        } else {
            edit
        };
        bring_to_foreground_and_focus(hwnd, active_focus_hwnd, reset_caret && !in_editor);
    }
}

/// Calls the app handler without holding `&mut State` across the callback.
unsafe fn invoke_handler(state_ptr: *mut State, event: PaletteEvent) -> PaletteReply {
    let handler = {
        let state = &mut *state_ptr;
        let placeholder: Handler = Box::new(crate::event::ignore);
        std::mem::replace(&mut state.handler, placeholder)
    };
    let reply = handler(event);
    (*state_ptr).handler = handler;
    reply
}

unsafe fn apply_theme_live(state_ptr: *mut State, new_theme: Theme) {
    let hwnd = (*state_ptr).hwnd;
    let edit = (*state_ptr).edit;
    let editor_edit = (*state_ptr).editor_edit;
    let new_brush = CreateSolidBrush(colorref(new_theme.field));
    let old_brush = (*state_ptr).field_brush.replace(new_brush);
    if let Some(old) = old_brush {
        let _ = DeleteObject(old.into());
    }
    (*state_ptr).theme = new_theme;
    apply_dwm_chrome(hwnd, &new_theme);
    apply_editor_theme(editor_edit, new_theme.appearance);
    let _ = InvalidateRect(Some(edit), None, true);
    if !editor_edit.0.is_null() {
        let _ = InvalidateRect(Some(editor_edit), None, true);
    }
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
        Appearance::Dark => colorref(Color::rgb(0x38, 0x38, 0x38)),
        Appearance::Light => colorref(Color::rgb(0xd6, 0xd6, 0xd6)),
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
    let reply = invoke_handler(state_ptr, event);
    let (hwnd, edit) = {
        let state = &*state_ptr;
        (state.hwnd, state.edit)
    };

    match reply {
        PaletteReply::Keep => {}
        PaletteReply::Update(model) => {
            apply_model_update(state_ptr, hwnd, edit, model);
        }
        PaletteReply::UpdateWithEdit(text, model) => {
            set_edit_text_silently(state_ptr, &text);
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
            hide_palette_window(state_ptr, true);
        }
        PaletteReply::CloseWithHud(message) => {
            let dark = (*state_ptr).theme.appearance == Appearance::Dark;
            hide_palette_window(state_ptr, true);
            crate::hud::show_hud(message, dark);
        }
        PaletteReply::Quit => {
            uninstall_outside_click_hook();
            (*state_ptr).previous_foreground = None;
            let _ = DestroyWindow(hwnd);
        }
    }
}

/// Cancels an active detail-pane drag/selection and releases mouse capture
/// if the current model invalidated the detail text surface.
unsafe fn cancel_detail_drag_if_needed(state_ptr: *mut State) {
    let release_capture = {
        let state = &mut *state_ptr;
        let detail_invalidated = state.model.action_menu.is_some()
            || state.model.editor.is_some()
            || state.model.grid.is_some()
            || state.model.detail.as_ref().is_none_or(|d| {
                detail_text_fingerprint(detail_preview_text(d)) != state.detail_text.last_body_hash
            });
        if detail_invalidated {
            state.detail_text.selection = None;
            if state.detail_text.dragging {
                state.detail_text.dragging = false;
                true
            } else {
                false
            }
        } else {
            false
        }
    };
    if release_capture {
        let _ = ReleaseCapture();
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
        cancel_detail_drag_if_needed(state_ptr);
        resize_window_rect(hwnd, metrics, target_h);
        sync_editor_control(state_ptr, hwnd, metrics, false);
        let _ = InvalidateRect(Some(hwnd), None, false);
    }
}

unsafe fn apply_model_update(state_ptr: *mut State, hwnd: HWND, edit: HWND, mut model: Model) {
    let (metrics, placeholder, has_badge, inline_pill_w, target_h) = {
        let state = &mut *state_ptr;
        let metrics = state.theme.metrics;
        if model.scroll_offset == 0.0
            && model.query == state.model.query
            && model.rows.len() == state.model.rows.len()
        {
            model.scroll_offset = state.model.scroll_offset;
        }
        model.ensure_selected_visible(metrics);
        let placeholder = if model.placeholder.is_empty() {
            DEFAULT_PLACEHOLDER.to_string()
        } else {
            model.placeholder.clone()
        };
        let has_badge = model.header_badge.is_some();
        let inline_pill_w = inline_arg_pill_width(&model);
        let target_h = model.layout(metrics).window_height;
        state.model = model;
        (metrics, placeholder, has_badge, inline_pill_w, target_h)
    };
    cancel_detail_drag_if_needed(state_ptr);
    set_cue_banner(edit, &placeholder);
    resize_window_rect(hwnd, metrics, target_h);
    let dpi = GetDpiForWindow(hwnd).max(96) as f32;
    let width_px = (metrics.width * (dpi / 96.0)).round() as i32;
    place_field_rect(hwnd, edit, metrics, width_px, has_badge, inline_pill_w);
    sync_editor_control(state_ptr, hwnd, metrics, false);
    let _ = InvalidateRect(Some(hwnd), None, false);
}

/// Synchronizes visibility, text seed, geometry, and keyboard focus between the
/// header search `EDIT` and the multi-line `editor_edit` control.
unsafe fn sync_editor_control(
    state_ptr: *mut State,
    hwnd: HWND,
    metrics: Metrics,
    reposition_only_if_active: bool,
) {
    let (edit, editor_edit, layout, editor_spec, was_active, seed_changed) = {
        let state = &*state_ptr;
        let layout = state.model.layout(metrics);
        let spec = state
            .model
            .editor
            .as_ref()
            .map(|ed| (ed.title.clone(), ed.text.clone()));
        let seed_changed = spec.is_some() && state.editor_seed != spec;
        (
            state.edit,
            state.editor_edit,
            layout,
            spec,
            state.editor_active,
            seed_changed,
        )
    };

    if let Some((title, text)) = editor_spec {
        let dpi = GetDpiForWindow(hwnd).max(96) as f32;
        let width_px = (metrics.width * (dpi / 96.0)).round() as i32;
        place_editor_rect(
            hwnd,
            editor_edit,
            metrics,
            width_px,
            layout.list_top,
            layout.list_bottom,
        );
        if !edit.0.is_null() && IsWindow(Some(edit)).as_bool() {
            let _ = ShowWindow(edit, SW_HIDE);
        }
        if !editor_edit.0.is_null() && IsWindow(Some(editor_edit)).as_bool() {
            if !was_active || seed_changed {
                let crlf = normalize_to_crlf(&text);
                let wide: Vec<u16> = crlf.encode_utf16().chain(std::iter::once(0)).collect();
                let _ = SendMessageW(
                    editor_edit,
                    WM_SETTEXT,
                    Some(WPARAM(0)),
                    Some(LPARAM(wide.as_ptr() as isize)),
                );
                let end = wide.len().saturating_sub(1);
                let _ = SendMessageW(
                    editor_edit,
                    EM_SETSEL,
                    Some(WPARAM(end)),
                    Some(LPARAM(end as isize)),
                );
                (*state_ptr).editor_seed = Some((title, text));
            }
            let _ = ShowWindow(editor_edit, SW_SHOW);
            if !reposition_only_if_active && (!was_active || seed_changed) {
                let _ = SetFocus(Some(editor_edit));
            }
        }
        (*state_ptr).editor_active = true;
    } else if was_active {
        (*state_ptr).editor_active = false;
        (*state_ptr).editor_seed = None;
        if !editor_edit.0.is_null() && IsWindow(Some(editor_edit)).as_bool() {
            let _ = ShowWindow(editor_edit, SW_HIDE);
        }
        if !edit.0.is_null() && IsWindow(Some(edit)).as_bool() {
            let _ = ShowWindow(edit, SW_SHOW);
            if !reposition_only_if_active {
                let _ = SetFocus(Some(edit));
            }
        }
    }
}

unsafe fn dismiss(state_ptr: *mut State, restore_previous: bool) {
    uninstall_outside_click_hook();
    dispatch(state_ptr, PaletteEvent::Dismissed);
    hide_palette_window(state_ptr, restore_previous);
}

fn create_edit_font(dpi: u32) -> HFONT {
    // 18 DIP (~13.5pt) crisp Segoe UI matching Raycast's 18px header search input.
    let height = -((18.0 * (dpi.max(96) as f32 / 96.0)).round() as i32);
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

fn create_editor_font(dpi: u32) -> HFONT {
    // 14 DIP crisp Segoe UI for the multi-line text editor surface.
    let height = -((14.0 * (dpi.max(96) as f32 / 96.0)).round() as i32);
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

unsafe fn apply_editor_theme(editor_edit: HWND, appearance: Appearance) {
    if editor_edit.0.is_null() {
        return;
    }
    let theme_name = match appearance {
        Appearance::Dark => w!("DarkMode_Explorer"),
        Appearance::Light => w!("Explorer"),
    };
    let _ = SetWindowTheme(editor_edit, theme_name, None);
}

fn normalize_to_crlf(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 16);
    let mut prev_cr = false;
    for ch in text.chars() {
        if ch == '\n' {
            if !prev_cr {
                out.push('\r');
            }
            out.push('\n');
            prev_cr = false;
        } else {
            out.push(ch);
            prev_cr = ch == '\r';
        }
    }
    out
}

/// Sizes the palette for the active monitor and places it dead-center (or
/// restores the user's dragged custom origin if still on a connected monitor).
fn place_window_rect(
    hwnd: HWND,
    metrics: Metrics,
    window_height_dip: f32,
    custom_origin: Option<(i32, i32)>,
) {
    let sys_dpi = unsafe { GetDpiForSystem().max(96) };

    // If the user dragged the palette to a custom position, prefer the monitor
    // containing that saved spot (falling back to the cursor monitor if detached).
    let (monitor, use_custom) = if let Some((cx, cy)) = custom_origin {
        let probe = RECT {
            left: cx,
            top: cy,
            right: cx + 120,
            bottom: cy + 60,
        };
        let mon = unsafe { MonitorFromRect(&probe, MONITOR_DEFAULTTONULL) };
        if !mon.0.is_null() {
            (mon, Some((cx, cy)))
        } else {
            let mut cursor = POINT::default();
            let _ = unsafe { windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut cursor) };
            (
                unsafe { MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST) },
                None,
            )
        }
    } else {
        let mut cursor = POINT::default();
        let _ = unsafe { windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut cursor) };
        (
            unsafe { MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST) },
            None,
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
            sys_dpi
        }
    };
    let scale = monitor_dpi as f32 / 96.0;
    let width = (metrics.width * scale).round() as i32;
    let height = (window_height_dip * scale).round() as i32;
    // Anchor top edge to the expanded palette height so Compact -> Expanded unfolds
    // smoothly downward without the search bar ever jumping vertically.
    let anchor_height = (metrics.height * scale).round() as i32;

    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    let _ = unsafe { GetMonitorInfoW(monitor, &mut info) };
    let work = info.rcWork;

    let (x, y) = if let Some((cx, cy)) = use_custom {
        let max_x = (work.right - width).max(work.left);
        let max_y = (work.bottom - height).max(work.top);
        (cx.clamp(work.left, max_x), cy.clamp(work.top, max_y))
    } else {
        let centered_x = work.left + (work.right - work.left - width) / 2;
        let centered_y = work.top + (work.bottom - work.top - anchor_height) / 2;
        let max_x = (work.right - width).max(work.left);
        let max_y = (work.bottom - height).max(work.top);
        (
            centered_x.clamp(work.left, max_x),
            centered_y.clamp(work.top, max_y),
        )
    };

    unsafe {
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
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
/// top search header bar, reserving space for an inline `Tab` argument pill if active.
fn place_field_rect(
    hwnd: HWND,
    edit: HWND,
    metrics: Metrics,
    window_width_px: i32,
    has_badge: bool,
    inline_pill_w: f32,
) {
    if edit.0.is_null() {
        return;
    }
    let dpi = unsafe { GetDpiForWindow(hwnd).max(96) } as f32;
    let scale = dpi / 96.0;
    let left_dip = metrics.field_inset
        + if inline_pill_w > 0.0 {
            inline_pill_w + 10.0
        } else {
            0.0
        };
    let left = (left_dip * scale).round() as i32;
    let right_reserve = if has_badge { 132.0 } else { 18.0 };
    let width = (window_width_px - left - (right_reserve * scale).round() as i32).max(60);
    // 18 DIP Segoe UI line box is ~24 DIPs tall; centering inside 60 DIPs puts the
    // baseline dead-center with the header icon / back button.
    let edit_h_dip = 25.0_f32;
    let top_dip = ((metrics.field_height - edit_h_dip) / 2.0).max(4.0);
    let top = (top_dip * scale).round() as i32;
    let height = (edit_h_dip * scale).round() as i32;
    unsafe {
        let _ = SetWindowPos(edit, None, left, top, width, height, SWP_NOZORDER);
    }
}

/// Positions the multi-line `editor_edit` control neatly inside its rounded card frame.
fn place_editor_rect(
    hwnd: HWND,
    editor_edit: HWND,
    metrics: Metrics,
    window_width_px: i32,
    list_top: f32,
    list_bottom: f32,
) {
    if editor_edit.0.is_null() {
        return;
    }
    let dpi = unsafe { GetDpiForWindow(hwnd).max(96) } as f32;
    let scale = dpi / 96.0;
    let pad = metrics.padding;
    let left = ((pad + 12.0) * scale).round() as i32;
    let right_pad = ((pad + 12.0) * scale).round() as i32;
    let width = (window_width_px - left - right_pad).max(60);
    let top = ((list_top + 12.0) * scale).round() as i32;
    let bottom = ((list_bottom - 12.0) * scale).round() as i32;
    let height = (bottom - top).max(40);
    unsafe {
        let _ = SetWindowPos(editor_edit, None, left, top, width, height, SWP_NOZORDER);
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

    #[test]
    fn normalize_to_crlf_converts_lf_without_doubling_existing_crlf() {
        assert_eq!(normalize_to_crlf(""), "");
        assert_eq!(normalize_to_crlf("hello"), "hello");
        assert_eq!(normalize_to_crlf("a\nb\nc"), "a\r\nb\r\nc");
        assert_eq!(normalize_to_crlf("a\r\nb\r\nc"), "a\r\nb\r\nc");
        assert_eq!(normalize_to_crlf("a\r\nb\nc\n"), "a\r\nb\r\nc\r\n");
        assert_eq!(normalize_to_crlf("\n\n"), "\r\n\r\n");
    }
}
