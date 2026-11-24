//! Floating translucent Raycast-style HUD pill (`"Copied to Clipboard"`,
//! `"Pinned"`, `"Left Half"`, etc.) rendered via pure Win32 + Direct2D.
//!
//! Spawns on a short-lived non-blocking worker thread with `WS_EX_NOACTIVATE |
//! WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW` so it never steals
//! keyboard focus or blocks mouse clicks, then fades away cleanly after 1.25s.

use std::ffi::c_void;
use std::time::Duration;

use windows::core::{w, BOOL, PCWSTR};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Direct2D::{
    Common::{
        D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT, D2D_RECT_F, D2D_SIZE_U,
    },
    D2D1CreateFactory, ID2D1Factory, D2D1_DRAW_TEXT_OPTIONS_CLIP,
    D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT, D2D1_FACTORY_TYPE_SINGLE_THREADED,
    D2D1_FEATURE_LEVEL_DEFAULT, D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_PRESENT_OPTIONS_NONE,
    D2D1_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_NONE,
    D2D1_ROUNDED_RECT,
};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL,
    DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_TEXT_ALIGNMENT_CENTER,
    DWRITE_WORD_WRAPPING_NO_WRAP,
};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE,
    DWMWCP_ROUND,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R8G8B8A8_UNORM;
use windows::Win32::Graphics::Gdi::{
    BeginPaint, EndPaint, GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    PAINTSTRUCT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, GetDpiForSystem, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetCursorPos, GetMessageW,
    GetWindowLongPtrW, KillTimer, PostQuitMessage, RegisterClassExW, SetLayeredWindowAttributes,
    SetTimer, SetWindowLongPtrW, ShowWindow, TranslateMessage, CREATESTRUCTW, CS_HREDRAW,
    CS_VREDRAW, GWLP_USERDATA, HWND_TOPMOST, LWA_ALPHA, MSG, SW_SHOWNOACTIVATE, WM_CREATE,
    WM_DESTROY, WM_ERASEBKGND, WM_NCCREATE, WM_PAINT, WM_TIMER, WNDCLASSEXW, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};
use windows_numerics::Vector2;

const HUD_TIMER_CLOSE: usize = 1;
const HUD_LIFETIME_MS: u32 = 1250;

struct HudState {
    text: String,
    dark: bool,
    dpi: f32,
    width_dip: f32,
    height_dip: f32,
}

/// Show a brief floating HUD pill near the lower center of the active monitor.
///
/// Non-blocking, focus-safe, and click-through.
pub fn show_hud(message: impl Into<String>, dark: bool) {
    let text = message.into();
    if text.trim().is_empty() {
        return;
    }
    let _ = std::thread::Builder::new()
        .name("yal-hud".to_string())
        .spawn(move || {
            let _ = run_hud_window(text, dark);
        });
}

fn run_hud_window(text: String, dark: bool) -> Result<(), String> {
    let instance = unsafe { HINSTANCE(GetModuleHandleW(None).map_err(|e| e.to_string())?.0) };
    let class = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(hud_wnd_proc),
        hInstance: instance,
        lpszClassName: w!("YalHudPill"),
        ..Default::default()
    };
    unsafe {
        let _ = RegisterClassExW(&class);
    }

    let mut cursor = POINT::default();
    let _ = unsafe { GetCursorPos(&mut cursor) };
    let monitor = unsafe { MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST) };
    let mut dpi_x = 0u32;
    let mut dpi_y = 0u32;
    let dpi = unsafe {
        if GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y).is_ok()
            && dpi_x >= 96
        {
            dpi_x
        } else {
            GetDpiForSystem().max(96)
        }
    } as f32;
    let scale = dpi / 96.0;

    let char_count = text.chars().count() as f32;
    let width_dip = (char_count * 7.6 + 52.0).clamp(160.0, 380.0);
    let height_dip = 44.0_f32;
    let width_px = (width_dip * scale).round() as i32;
    let height_px = (height_dip * scale).round() as i32;

    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    let _ = unsafe { GetMonitorInfoW(monitor, &mut info) };
    let x = info.rcWork.left + (info.rcWork.right - info.rcWork.left - width_px) / 2;
    let y = info.rcWork.bottom - height_px - (72.0 * scale).round() as i32;

    let state_ptr = Box::into_raw(Box::new(HudState {
        text,
        dark,
        dpi,
        width_dip,
        height_dip,
    }));

    let hwnd = match unsafe {
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT | WS_EX_LAYERED,
            w!("YalHudPill"),
            PCWSTR::null(),
            WS_POPUP,
            x,
            y,
            width_px,
            height_px,
            None,
            None,
            Some(instance),
            Some(state_ptr as *mut c_void),
        )
    } {
        Ok(hwnd) => hwnd,
        Err(err) => {
            unsafe {
                drop(Box::from_raw(state_ptr));
            }
            return Err(err.to_string());
        }
    };

    unsafe {
        let corner = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &corner as *const _ as *const c_void,
            std::mem::size_of_val(&corner) as u32,
        );
        let dark_flag = BOOL::from(dark);
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &dark_flag as *const _ as *const c_void,
            std::mem::size_of_val(&dark_flag) as u32,
        );
        let _ = SetLayeredWindowAttributes(
            hwnd,
            windows::Win32::Foundation::COLORREF(0),
            238,
            LWA_ALPHA,
        );
        let _ = windows::Win32::UI::WindowsAndMessaging::SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            x,
            y,
            width_px,
            height_px,
            windows::Win32::UI::WindowsAndMessaging::SWP_NOACTIVATE,
        );
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        let _ = SetTimer(Some(hwnd), HUD_TIMER_CLOSE, HUD_LIFETIME_MS, None);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        let _ = Duration::from_millis(0);
        drop(Box::from_raw(state_ptr));
    }
    Ok(())
}

unsafe extern "system" fn hud_wnd_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_NCCREATE {
        let create = &*(lparam.0 as *const CREATESTRUCTW);
        let _ = SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
        return DefWindowProcW(hwnd, message, wparam, lparam);
    }
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut HudState;
    if ptr.is_null() {
        return DefWindowProcW(hwnd, message, wparam, lparam);
    }
    match message {
        WM_CREATE => LRESULT(0),
        WM_ERASEBKGND => LRESULT(1),
        WM_TIMER => {
            if wparam.0 == HUD_TIMER_CLOSE {
                let _ = KillTimer(Some(hwnd), HUD_TIMER_CLOSE);
                let _ = DestroyWindow(hwnd);
            }
            LRESULT(0)
        }
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let _ = BeginPaint(hwnd, &mut ps);
            paint_hud(hwnd, &*ptr);
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_DESTROY => {
            let _ = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

unsafe fn paint_hud(hwnd: HWND, state: &HudState) {
    let Ok(factory): Result<ID2D1Factory, _> =
        D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)
    else {
        return;
    };
    let Ok(dwrite): Result<IDWriteFactory, _> = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)
    else {
        return;
    };
    let scale = state.dpi / 96.0;
    let props = D2D1_RENDER_TARGET_PROPERTIES {
        r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_R8G8B8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
        },
        dpiX: state.dpi,
        dpiY: state.dpi,
        usage: D2D1_RENDER_TARGET_USAGE_NONE,
        minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
    };
    let hwnd_props = D2D1_HWND_RENDER_TARGET_PROPERTIES {
        hwnd,
        pixelSize: D2D_SIZE_U {
            width: (state.width_dip * scale).round().max(1.0) as u32,
            height: (state.height_dip * scale).round().max(1.0) as u32,
        },
        presentOptions: D2D1_PRESENT_OPTIONS_NONE,
    };
    let Ok(target) = factory.CreateHwndRenderTarget(&props, &hwnd_props) else {
        return;
    };

    let bg = if state.dark {
        D2D1_COLOR_F {
            r: 0.11,
            g: 0.11,
            b: 0.14,
            a: 1.0,
        }
    } else {
        D2D1_COLOR_F {
            r: 0.98,
            g: 0.98,
            b: 0.99,
            a: 1.0,
        }
    };
    let border_col = if state.dark {
        D2D1_COLOR_F {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 0.16,
        }
    } else {
        D2D1_COLOR_F {
            r: 0.06,
            g: 0.09,
            b: 0.16,
            a: 0.16,
        }
    };
    let fg_col = if state.dark {
        D2D1_COLOR_F {
            r: 0.96,
            g: 0.96,
            b: 0.98,
            a: 1.0,
        }
    } else {
        D2D1_COLOR_F {
            r: 0.09,
            g: 0.10,
            b: 0.12,
            a: 1.0,
        }
    };

    target.BeginDraw();
    target.Clear(Some(&bg));

    if let Ok(border_brush) = target.CreateSolidColorBrush(&border_col, None) {
        let rr = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: 0.5,
                top: 0.5,
                right: state.width_dip - 0.5,
                bottom: state.height_dip - 0.5,
            },
            radiusX: 10.0,
            radiusY: 10.0,
        };
        target.DrawRoundedRectangle(&rr, &border_brush, 1.0, None);
    }

    if let (Ok(fmt), Ok(fg_brush)) = (
        dwrite.CreateTextFormat(
            w!("Segoe UI"),
            None,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            13.5,
            w!("en-us"),
        ),
        target.CreateSolidColorBrush(&fg_col, None),
    ) {
        let _ = fmt.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER);
        let _ = fmt.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP);
        let wide: Vec<u16> = state.text.encode_utf16().collect();
        if let Ok(layout) =
            dwrite.CreateTextLayout(&wide, &fmt, state.width_dip - 20.0, state.height_dip)
        {
            target.DrawTextLayout(
                Vector2 {
                    X: 10.0,
                    Y: (state.height_dip - 19.0) / 2.0,
                },
                &layout,
                &fg_brush,
                D2D1_DRAW_TEXT_OPTIONS_CLIP | D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT,
            );
        }
    }
    let _ = target.EndDraw(None, None);
}
