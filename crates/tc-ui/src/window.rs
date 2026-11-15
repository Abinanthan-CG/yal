//! The palette window.
//!
//! A borderless, always-on-top window with rounded corners, drawn by Direct2D
//! and typed into with a real Windows edit control - so the keyboard, the input
//! method, the clipboard and the selection all behave the way Windows expects.

use std::ffi::c_void;

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontW, CreateSolidBrush, DeleteObject, EndPaint, GetMonitorInfoW, HBRUSH,
    HDC, HFONT, InvalidateRect, MONITORINFO, MonitorFromPoint, PAINTSTRUCT, SetBkColor, SetTextColor,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::WindowsProgramming::MulDiv;
use windows::Win32::UI::Controls::{
    InitCommonControlsEx, INITCOMMONCONTROLSEX, ICC_STANDARD_CLASSES,
};
use windows::Win32::UI::HiDpi::{
    GetDpiForSystem, GetDpiForWindow, SetProcessDpiAwarenessContext,
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, GetParent,
    GetWindowLongPtrW, LoadCursorW, PostQuitMessage, RegisterClassExW, SendMessageW,
    SetForegroundWindow, SetWindowLongPtrW, SetWindowPos, ShowWindow, TranslateMessage,
    CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW, EN_CHANGE, ES_AUTOHSCROLL, ES_LEFT, GWLP_USERDATA,
    GWLP_WNDPROC, HTCLIENT, IDC_ARROW, MINMAXINFO, SW_HIDE, SW_SHOW, WM_COMMAND, WM_CREATE,
    WM_CHAR, WM_CTLCOLOREDIT, WM_DESTROY, WM_ERASEBKGND, WM_GETMINMAXINFO, WM_KEYDOWN,
    WM_MOUSEMOVE, WM_NCCALCSIZE, WM_NCHITTEST, WM_NEXTDLGCTL, WM_PAINT, WM_SETFONT, WM_SIZE,
    WNDCLASSEXW, WINDOW_EX_STYLE, WINDOW_STYLE, WS_CHILD, WS_CLIPCHILDREN, WS_CLIPSIBLINGS,
    WS_POPUP, WS_TABSTOP, WS_THICKFRAME, WS_VISIBLE,
};

use crate::model::Model;
use crate::paint::Renderer;
use crate::theme::{Color, Theme};

/// The message the search field sends when the user pressed the up arrow.
const WM_APP_UP: u32 = 0x8001;
/// The message the search field sends when the user pressed the down arrow.
const WM_APP_DOWN: u32 = 0x8002;
/// The message the search field sends when the user pressed escape.
const WM_APP_ESCAPE: u32 = 0x8003;
/// The message the search field sends when the user pressed return.
const WM_APP_RETURN: u32 = 0x8004;

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
}

/// The palette's window.
pub struct Palette {
    hwnd: HWND,
    state: *mut State,
}

impl Palette {
    /// Builds a palette window.
    pub fn new(theme: Theme, model: Model) -> Result<Self, String> {
        unsafe { set_dpi_aware() };
        // The common controls of Windows 11 - the search field's own look, and
        // its placeholder - need the version 6 library, which an application
        // only gets when it asks for it.
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
            return Err("the palette's window class could not be registered".to_string());
        }

        let mut state = Box::new(State {
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
        });

        let metrics = state.theme.metrics;
        let width = metrics.width as i32;
        let height = content_height(metrics, state.model.rows.len()) as i32;

        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
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
                Some(state.as_mut() as *mut State as *mut c_void),
            )
            .map_err(|error| format!("the palette window could not be created: {error}"))?
        };

        state.hwnd = hwnd;
        // The window is placed once it exists: a window cannot be resized while
        // it is still being created, and only now does it know its monitor.
        place_window(&state);
        let state_ptr = Box::into_raw(state);

        Ok(Self { hwnd, state: state_ptr })
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
        unsafe {
            (*self.state).model = model;
        }
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
            let _ = SetForegroundWindow(self.hwnd);
            let _ = ShowWindow(self.hwnd, SW_SHOW);
            let edit = (*self.state).edit;
            if !edit.0.is_null() {
                // WM_NEXTDLGCTL hands the keyboard to the search field.
                let _ = SendMessageW(
                    self.hwnd,
                    WM_NEXTDLGCTL,
                    Some(WPARAM(edit.0 as usize)),
                    Some(LPARAM(1)),
                );
            }
        }
    }

    /// Hides the palette.
    pub fn hide(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    /// Runs the window's message loop until the window is closed.
    pub fn run(&self) {
        unsafe {
            let mut message = std::mem::zeroed::<windows::Win32::UI::WindowsAndMessaging::MSG>();
            while GetMessageW(&mut message, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
}

impl Drop for Palette {
    fn drop(&mut self) {
        unsafe {
            if !self.state.is_null() {
                let mut state = Box::from_raw(self.state);
                if state.edit_proc != 0 {
                    let _ = SetWindowLongPtrW(state.edit, GWLP_WNDPROC, state.edit_proc);
                }
                if let Some(font) = state.font.take() {
                    let _ = DeleteObject(font.into());
                }
                if let Some(brush) = state.field_brush.take() {
                    let _ = DeleteObject(brush.into());
                }
                let _ = DestroyWindow(self.hwnd);
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

    let pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
    if pointer.is_null() {
        return DefWindowProcW(hwnd, message, wparam, lparam);
    }
    let state = &mut *pointer;

    match message {
        WM_CREATE => {
            let dpi = GetDpiForWindow(hwnd).max(96) as f32;
            let metrics = state.theme.metrics;

            // Windows rounds the palette's corners for us.
            let corner = DWMWCP_ROUND;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                &corner as *const _ as *const c_void,
                std::mem::size_of_val(&corner) as u32,
            );

            // The search field is a real edit control, so the keyboard, the input
            // method and the clipboard are Windows' own. The brush it paints
            // with is made first: the control asks for its colour the moment it
            // is created.
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

            // The field shows a hint while it is empty. Windows' own cue banner
            // needs the version 6 common controls, which an application only
            // gets through its manifest; showing the hint as ordinary text
            // works everywhere and is cleared by the first keystroke.
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

            // The field keeps the arrow keys for the palette's list.
            let previous = SetWindowLongPtrW(
                edit,
                GWLP_WNDPROC,
                edit_procedure as *const () as isize,
            );
            state.edit_proc = previous;

            if let Ok(renderer) = Renderer::new(hwnd) {
                state.renderer = Some(renderer);
            }
            LRESULT(0)
        }
        WM_SIZE => {
            let width = (lparam.0 & 0xffff) as u32;
            let height = ((lparam.0 >> 16) & 0xffff) as u32;
            if let Some(renderer) = state.renderer.as_ref() {
                renderer.resize(width, height);
            }
            place_field(state, (lparam.0 & 0xffff) as i32);
            LRESULT(0)
        }
        WM_PAINT => {
            let mut paint = std::mem::zeroed::<PAINTSTRUCT>();
            let _ = BeginPaint(hwnd, &mut paint);
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
                let mut buffer = [0u16; 512];
                let length = unsafe {
                    windows::Win32::UI::WindowsAndMessaging::GetWindowTextW(state.edit, &mut buffer)
                };
                let text = String::from_utf16_lossy(&buffer[..length as usize]);
                state.model.query = if state.showing_placeholder {
                    String::new()
                } else {
                    text
                };
                let _ = InvalidateRect(Some(hwnd), None, false);
            }
            LRESULT(0)
        }
        WM_CTLCOLOREDIT => {
            // The device context comes with the message; it is the field's own.
            let control = HDC(wparam.0 as *mut c_void);
            let showing = state.showing_placeholder;
            unsafe {
                let _ = SetTextColor(
                    control,
                    colorref(if showing {
                        state.theme.text_dim
                    } else {
                        state.theme.text
                    }),
                );
                let _ = SetBkColor(control, colorref(state.theme.field));
            }
            let brush = state.field_brush.map(|b| b.0).unwrap_or_default();
            LRESULT(brush as isize)
        }
        WM_NCHITTEST => LRESULT(HTCLIENT as isize),
        WM_MOUSEMOVE => {
            // The row under the pointer is drawn a little differently.
            let y = ((lparam.0 >> 16) & 0xffff) as f32;
            let hovered = match state.renderer.as_ref() {
                Some(renderer) => {
                    let scale = renderer.scale(&state.theme);
                    if scale > 0.0 {
                        let metrics = state.theme.metrics;
                        let list_top = metrics.padding + metrics.field_height + metrics.list_gap;
                        state
                            .model
                            .row_at(y / scale, list_top, metrics.row_height)
                    } else {
                        None
                    }
                }
                None => None,
            };
            if state.model.hovered != hovered {
                state.model.hovered = hovered;
                let _ = InvalidateRect(Some(hwnd), None, false);
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
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
            LRESULT(0)
        }
        WM_APP_UP => {
            state.model.select_previous();
            let _ = InvalidateRect(Some(hwnd), None, false);
            LRESULT(0)
        }
        WM_APP_DOWN => {
            state.model.select_next();
            let _ = InvalidateRect(Some(hwnd), None, false);
            LRESULT(0)
        }
        WM_APP_ESCAPE => {
            let _ = ShowWindow(hwnd, SW_HIDE);
            LRESULT(0)
        }
        WM_APP_RETURN => LRESULT(0),
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

/// The search field's own window procedure, which keeps the arrow keys.
unsafe extern "system" fn edit_procedure(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // The palette's state, reached through the parent window.
    let parent = unsafe { GetParent(hwnd).unwrap_or_default() };
    let pointer = GetWindowLongPtrW(parent, GWLP_USERDATA);
    let state = if pointer != 0 {
        Some(&*(pointer as *const State))
    } else {
        None
    };

    if message == WM_CHAR {
        if let Some(state) = state {
            if state.showing_placeholder {
                let character = wparam.0 as u16 as u8;
                if character == 0x08 {
                    // Backspace on an empty field does nothing at all.
                    return LRESULT(0);
                }
                // The first keystroke replaces the hint.
                let _ = SendMessageW(
                    hwnd,
                    windows::Win32::UI::WindowsAndMessaging::WM_SETTEXT,
                    Some(WPARAM(0)),
                    Some(LPARAM(0)),
                );
                let state = &mut *(pointer as *mut State);
                state.showing_placeholder = false;
            }
        }
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

    // The original procedure is kept in the palette's state.
    if let Some(state) = state {
        if state.edit_proc != 0 {
            let original: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT =
                std::mem::transmute(state.edit_proc);
            return original(hwnd, message, wparam, lparam);
        }
    }
    DefWindowProcW(hwnd, message, wparam, lparam)
}

/// Sizes the palette for the display it is on and puts it where the user is
/// looking: the middle of the monitor the pointer is on, a third of the way
/// down.
fn place_window(state: &State) {
    let metrics = state.theme.metrics;
    // While the window is being created its own DPI is not known yet, so the
    // system's is the fallback.
    let dpi = unsafe {
        GetDpiForWindow(state.hwnd)
            .max(GetDpiForSystem())
            .max(96)
    } as f32;
    let scale = dpi / 96.0;
    let width = (metrics.width * scale) as i32;
    let height = (content_height(metrics, state.model.rows.len()) * scale) as i32;

    let mut cursor = POINT::default();
    let _ = unsafe { windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut cursor) };
    let monitor = unsafe { MonitorFromPoint(cursor, windows::Win32::Graphics::Gdi::MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    let _ = unsafe { GetMonitorInfoW(monitor, &mut info) };

    let x = info.rcWork.left + (info.rcWork.right - info.rcWork.left - width) / 2;
    let y = info.rcWork.top + (info.rcWork.bottom - info.rcWork.top - height) / 3;

    unsafe {
        let _ = SetWindowPos(
            state.hwnd,
            Some(windows::Win32::UI::WindowsAndMessaging::HWND_TOPMOST),
            x,
            y,
            width,
            height,
            windows::Win32::UI::WindowsAndMessaging::SWP_NOACTIVATE,
        );
    }
}

/// Puts the search field where it belongs.
///
/// The field is inset from the left of the slab so the magnifying glass has
/// room, and by a pixel all round so the hairline around the slab stays
/// unbroken.
fn place_field(state: &State, window_width: i32) {
    let metrics = state.theme.metrics;
    let dpi = unsafe { GetDpiForWindow(state.hwnd).max(96) } as f32;
    let scale = dpi / 96.0;
    let padding = (metrics.padding * scale) as i32;
    let inset = (metrics.field_inset * scale) as i32;
    let height = (metrics.field_height * scale) as i32;
    let width = window_width - padding * 2 - inset - 12;
    unsafe {
        let _ = SetWindowPos(
            state.edit,
            None,
            padding + inset,
            padding + 1,
            width,
            height - 2,
            windows::Win32::UI::WindowsAndMessaging::SWP_NOZORDER,
        );
    }
}

/// How tall the palette needs to be for a list of so many rows.
///
/// The window is the size of its content: a short list gets a short window,
/// and a long one is cut off at the height the measurements allow.
fn content_height(metrics: crate::theme::Metrics, rows: usize) -> f32 {
    let visible = rows.min(metrics.max_rows) as f32;
    metrics.padding * 2.0
        + metrics.field_height
        + metrics.list_gap
        + visible * metrics.row_height
        + metrics.footer_height
}

/// A theme colour in the shape GDI wants.
fn colorref(color: Color) -> COLORREF {
    COLORREF(
        (color.blue * 255.0) as u32
            | ((color.green * 255.0) as u32) << 8
            | ((color.red * 255.0) as u32) << 16,
    )
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
