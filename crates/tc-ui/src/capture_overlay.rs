//! Full-desktop frozen screenshot capture overlay for Screen OCR and Color Picker.
//!
//! Renders a 1:1 physical-pixel borderless topmost Direct2D surface across the
//! virtual desktop so per-monitor DPI scaling never blurs or shifts sampled
//! coordinates.

use std::ffi::c_void;
use std::mem::ManuallyDrop;

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{
    GetLastError, ERROR_CLASS_ALREADY_EXISTS, HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM,
};
use windows::Win32::Graphics::Direct2D::{
    Common::{D2D1_ALPHA_MODE_IGNORE, D2D1_COLOR_F, D2D1_PIXEL_FORMAT, D2D_RECT_F, D2D_SIZE_U},
    D2D1CreateFactory, ID2D1Bitmap, ID2D1Factory, ID2D1Geometry, ID2D1HwndRenderTarget, ID2D1Layer,
    ID2D1SolidColorBrush, D2D1_ANTIALIAS_MODE_ALIASED, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
    D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR, D2D1_BITMAP_PROPERTIES,
    D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_ELLIPSE, D2D1_FACTORY_TYPE_SINGLE_THREADED,
    D2D1_FEATURE_LEVEL_DEFAULT, D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_LAYER_OPTIONS_NONE,
    D2D1_LAYER_PARAMETERS, D2D1_PRESENT_OPTIONS_NONE, D2D1_RENDER_TARGET_PROPERTIES,
    D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_NONE, D2D1_ROUNDED_RECT,
};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, IDWriteTextLayout,
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_WEIGHT_SEMI_BOLD,
    DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT, DWRITE_TEXT_ALIGNMENT_LEADING,
    DWRITE_TEXT_METRICS, DWRITE_WORD_WRAPPING_NO_WRAP,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::{
    BeginPaint, EndPaint, GetMonitorInfoW, InvalidateRect, MonitorFromPoint, MONITORINFO,
    MONITOR_DEFAULTTONEAREST, PAINTSTRUCT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, ReleaseCapture, SetCapture, SetFocus, VIRTUAL_KEY, VK_DOWN, VK_ESCAPE, VK_LEFT,
    VK_RETURN, VK_RIGHT, VK_SHIFT, VK_TAB, VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, BringWindowToTop, CreateWindowExW, DefWindowProcW, DestroyWindow,
    DispatchMessageW, GetCursorPos, GetForegroundWindow, GetMessageW, GetWindowLongPtrW,
    GetWindowThreadProcessId, IsIconic, IsWindow, IsWindowVisible, LoadCursorW, PostQuitMessage,
    RegisterClassExW, SetCursorPos, SetForegroundWindow, SetWindowLongPtrW, SetWindowPos,
    TranslateMessage, ASFW_ANY, CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW, GWLP_USERDATA, HWND_TOPMOST,
    IDC_CROSS, MSG, SWP_HIDEWINDOW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
    SWP_SHOWWINDOW, WM_CAPTURECHANGED, WM_CLOSE, WM_DESTROY, WM_ERASEBKGND, WM_KEYDOWN,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCCREATE, WM_PAINT,
    WM_RBUTTONDOWN, WM_SYSKEYDOWN, WNDCLASSEXW, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
    WS_VISIBLE,
};
use windows_numerics::{Matrix3x2, Vector2};

#[derive(Debug, Clone)]
pub struct OverlaySnapshot {
    pub origin_x: i32,
    pub origin_y: i32,
    pub width: u32,
    pub height: u32,
    /// Top-down 32-bit BGRA physical pixels covering the virtual desktop.
    pub bgra: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayOcrMode {
    Smart,
    Raw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayColorFormat {
    Hex,
    Rgb,
    Hsl,
    Oklch,
}

#[derive(Debug, Clone)]
pub enum CaptureMode {
    Ocr {
        initial_mode: OverlayOcrMode,
        language_label: String,
    },
    ColorPicker {
        initial_format: OverlayColorFormat,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureResult {
    OcrRegion {
        /// Screen-space physical coordinates.
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        mode: OverlayOcrMode,
        open_preview: bool,
    },
    ColorPicked {
        rgb: [u8; 3],
        format: OverlayColorFormat,
        open_inspector: bool,
    },
    Cancelled,
}

const MIN_SELECTION_PX: u32 = 6;
const LOUPE_RADIUS: f32 = 74.0;
const LOUPE_OFFSET: f32 = 24.0;
const LOUPE_PILL_GAP: f32 = 10.0;
const MONITOR_EDGE_MARGIN: f32 = 12.0;
const ZOOM_STEPS: [i32; 4] = [7, 11, 15, 21];
const DEFAULT_ZOOM_CELLS: i32 = 11;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SelectionRect {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MonitorBounds {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct RectF {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
}

impl RectF {
    fn to_d2d(self) -> D2D_RECT_F {
        D2D_RECT_F {
            left: self.left,
            top: self.top,
            right: self.right,
            bottom: self.bottom,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct LoupePlacement {
    loupe_center: (f32, f32),
    loupe_radius: f32,
    pill_rect: RectF,
}

fn validate_snapshot(snapshot: &OverlaySnapshot) -> Result<(), String> {
    if snapshot.width == 0 || snapshot.height == 0 {
        return Err("capture overlay snapshot has zero dimensions".to_string());
    }
    if i32::try_from(snapshot.width).is_err() || i32::try_from(snapshot.height).is_err() {
        return Err("capture overlay snapshot dimensions exceed Win32 bounds".to_string());
    }
    let expected = (snapshot.width as usize)
        .checked_mul(snapshot.height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| "capture overlay snapshot byte size overflowed".to_string())?;
    if snapshot.bgra.len() != expected {
        return Err(format!(
            "capture overlay snapshot buffer length {} did not match expected {}",
            snapshot.bgra.len(),
            expected
        ));
    }
    Ok(())
}

fn sample_snapshot_rgb(snapshot: &OverlaySnapshot, x: i32, y: i32) -> Option<[u8; 3]> {
    if x < 0 || y < 0 {
        return None;
    }
    let ux = x as u32;
    let uy = y as u32;
    if ux >= snapshot.width || uy >= snapshot.height {
        return None;
    }
    let offset = (uy as usize)
        .checked_mul(snapshot.width as usize)?
        .checked_add(ux as usize)?
        .checked_mul(4)?;
    let px = snapshot.bgra.get(offset..offset + 4)?;
    Some([px[2], px[1], px[0]])
}

fn next_color_format(format: OverlayColorFormat) -> OverlayColorFormat {
    match format {
        OverlayColorFormat::Hex => OverlayColorFormat::Rgb,
        OverlayColorFormat::Rgb => OverlayColorFormat::Hsl,
        OverlayColorFormat::Hsl => OverlayColorFormat::Oklch,
        OverlayColorFormat::Oklch => OverlayColorFormat::Hex,
    }
}

fn format_overlay_color(rgb: [u8; 3], format: OverlayColorFormat) -> String {
    match format {
        OverlayColorFormat::Hex => format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]),
        OverlayColorFormat::Rgb => format!("rgb({}, {}, {})", rgb[0], rgb[1], rgb[2]),
        OverlayColorFormat::Hsl => {
            let (hue, sat, light) = rgb_to_hsl(rgb);
            format!(
                "hsl({}, {}, {})",
                fmt_angle(hue),
                fmt_percent(sat),
                fmt_percent(light)
            )
        }
        OverlayColorFormat::Oklch => {
            let (l, c, h) = rgb_to_oklch(rgb);
            format!(
                "oklch({} {} {})",
                fmt_percent(l),
                fmt_decimal(c),
                fmt_angle(h)
            )
        }
    }
}

fn rgb_to_hsl(rgb: [u8; 3]) -> (f64, f64, f64) {
    let red = f64::from(rgb[0]) / 255.0;
    let green = f64::from(rgb[1]) / 255.0;
    let blue = f64::from(rgb[2]) / 255.0;
    let maximum = red.max(green).max(blue);
    let minimum = red.min(green).min(blue);
    let chroma = maximum - minimum;
    let lightness = (maximum + minimum) / 2.0;
    let span = 1.0 - (2.0 * lightness - 1.0).abs();
    if chroma <= 0.0 || span <= 0.0 {
        return (0.0, 0.0, lightness);
    }
    let hue = if maximum == red {
        60.0 * (((green - blue) / chroma) % 6.0)
    } else if maximum == green {
        60.0 * ((blue - red) / chroma + 2.0)
    } else {
        60.0 * ((red - green) / chroma + 4.0)
    };
    (
        if hue < 0.0 { hue + 360.0 } else { hue },
        (chroma / span).min(1.0),
        lightness,
    )
}

fn rgb_to_oklch(rgb: [u8; 3]) -> (f64, f64, f64) {
    let red = srgb_to_linear(f64::from(rgb[0]) / 255.0);
    let green = srgb_to_linear(f64::from(rgb[1]) / 255.0);
    let blue = srgb_to_linear(f64::from(rgb[2]) / 255.0);
    let long = cube_root(0.4122215 * red + 0.5363325 * green + 0.0514460 * blue);
    let medium = cube_root(0.2119035 * red + 0.6806995 * green + 0.1073970 * blue);
    let short = cube_root(0.0883025 * red + 0.2817188 * green + 0.6299787 * blue);
    let l = 0.2104543 * long + 0.7936178 * medium - 0.0040720 * short;
    let a = 1.9779985 * long - 2.4285922 * medium + 0.4505937 * short;
    let b = 0.0259040 * long + 0.7827718 * medium - 0.8086758 * short;
    let c = (a * a + b * b).sqrt();
    let h = oklab_hue_degrees(a, b);
    (l, c, h)
}

fn srgb_to_linear(channel: f64) -> f64 {
    if channel <= 0.04045 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

fn cube_root(value: f64) -> f64 {
    if value < 0.0 {
        -(-value).powf(1.0 / 3.0)
    } else {
        value.powf(1.0 / 3.0)
    }
}

fn oklab_hue_degrees(a: f64, b: f64) -> f64 {
    if a.abs() <= 1e-6 && b.abs() <= 1e-6 {
        return 0.0;
    }
    let degrees = b.atan2(a) * 180.0 / std::f64::consts::PI;
    if degrees < 0.0 {
        degrees + 360.0
    } else {
        degrees
    }
}

fn fmt_integer(value: f64) -> i64 {
    if value.is_finite() {
        value.round() as i64
    } else {
        0
    }
}

fn fmt_number(value: f64) -> String {
    if !value.is_finite() {
        return "0".to_string();
    }
    let tenths = (value * 10.0).round() / 10.0;
    if tenths == tenths.round() {
        fmt_integer(tenths).to_string()
    } else {
        format!("{tenths:.1}")
    }
}

fn fmt_percent(value: f64) -> String {
    format!("{}%", fmt_number(value * 100.0))
}

fn fmt_angle(value: f64) -> String {
    format!("{}deg", fmt_number(value))
}

fn fmt_decimal(value: f64) -> String {
    if !value.is_finite() {
        return "0".to_string();
    }
    let rounded = (value * 1000.0).round() / 1000.0;
    if rounded == rounded.round() {
        fmt_integer(rounded).to_string()
    } else {
        fmt_general(rounded)
    }
}

fn fmt_general(value: f64) -> String {
    if !value.is_finite() {
        return format!("{value}");
    }
    let exponent = if value == 0.0 {
        0i32
    } else {
        value.abs().log10().floor() as i32
    };
    if !(-4..6).contains(&exponent) {
        let scientific = format!("{value:.5e}");
        let (mantissa, exponent_text) = scientific
            .split_once('e')
            .unwrap_or((scientific.as_str(), "0"));
        let exponent: i32 = exponent_text.parse().unwrap_or(0);
        return format!(
            "{}e{}{:02}",
            trim_trailing_zeros(mantissa),
            if exponent < 0 { '-' } else { '+' },
            exponent.abs()
        );
    }
    let decimals = (6 - 1 - exponent).max(0) as usize;
    trim_trailing_zeros(&format!("{value:.decimals$}"))
}

fn trim_trailing_zeros(text: &str) -> String {
    if !text.contains('.') {
        return text.to_string();
    }
    let trimmed = text.trim_end_matches('0');
    trimmed.strip_suffix('.').unwrap_or(trimmed).to_string()
}

fn clamp_range(value: f32, lo: f32, hi: f32) -> f32 {
    if hi <= lo {
        lo
    } else {
        value.clamp(lo, hi)
    }
}

fn normalize_selection_rect(
    start: (i32, i32),
    end: (i32, i32),
    max_w: u32,
    max_h: u32,
) -> SelectionRect {
    let max_x_bound = i32::try_from(max_w).unwrap_or(i32::MAX);
    let max_y_bound = i32::try_from(max_h).unwrap_or(i32::MAX);
    let sx = start.0.clamp(0, max_x_bound);
    let sy = start.1.clamp(0, max_y_bound);
    let ex = end.0.clamp(0, max_x_bound);
    let ey = end.1.clamp(0, max_y_bound);
    let x0 = sx.min(ex);
    let y0 = sy.min(ey);
    let x1 = sx.max(ex);
    let y1 = sy.max(ey);
    SelectionRect {
        x: x0,
        y: y0,
        width: x1.saturating_sub(x0) as u32,
        height: y1.saturating_sub(y0) as u32,
    }
}

fn is_valid_ocr_selection(rect: SelectionRect) -> bool {
    rect.width >= MIN_SELECTION_PX && rect.height >= MIN_SELECTION_PX
}

fn selection_to_screen_region(
    origin_x: i32,
    origin_y: i32,
    rect: SelectionRect,
) -> (i32, i32, u32, u32) {
    (
        origin_x.saturating_add(rect.x),
        origin_y.saturating_add(rect.y),
        rect.width,
        rect.height,
    )
}

fn nudge_pixel(pos: (i32, i32), dx: i32, dy: i32, width: u32, height: u32) -> (i32, i32) {
    let max_x = i32::try_from(width.saturating_sub(1)).unwrap_or(i32::MAX);
    let max_y = i32::try_from(height.saturating_sub(1)).unwrap_or(i32::MAX);
    (
        pos.0.saturating_add(dx).clamp(0, max_x),
        pos.1.saturating_add(dy).clamp(0, max_y),
    )
}

fn step_zoom_cells(current: i32, wheel_delta: i16) -> i32 {
    let idx = ZOOM_STEPS
        .iter()
        .position(|&step| step == current)
        .unwrap_or(1);
    if wheel_delta > 0 {
        ZOOM_STEPS[idx.saturating_sub(1)]
    } else if wheel_delta < 0 {
        ZOOM_STEPS[(idx + 1).min(ZOOM_STEPS.len() - 1)]
    } else {
        ZOOM_STEPS[idx]
    }
}

fn place_loupe_and_pill(
    cursor: (i32, i32),
    monitor: MonitorBounds,
    pill_size: (f32, f32),
) -> LoupePlacement {
    let r = LOUPE_RADIUS;
    let diam = r * 2.0;
    let pill_w = pill_size.0.max(1.0);
    let pill_h = pill_size.1.max(1.0);
    let total_h = diam + LOUPE_PILL_GAP + pill_h;

    let ml = monitor.left as f32;
    let mt = monitor.top as f32;
    let mr = monitor.right as f32;
    let mb = monitor.bottom as f32;
    let m = MONITOR_EDGE_MARGIN;

    let cur_x = cursor.0 as f32;
    let cur_y = cursor.1 as f32;

    let right_cx = cur_x + LOUPE_OFFSET + r;
    let left_cx = cur_x - LOUPE_OFFSET - r;
    let right_extent = (right_cx + r).max(right_cx + pill_w * 0.5);
    let raw_cx = if right_extent <= mr - m {
        right_cx
    } else {
        left_cx
    };
    let cx = clamp_range(raw_cx, ml + m + r, mr - m - r);

    let down_top = cur_y + LOUPE_OFFSET;
    let up_top = cur_y - LOUPE_OFFSET - total_h;
    let raw_top = if down_top + total_h <= mb - m {
        down_top
    } else {
        up_top
    };
    let group_top = clamp_range(raw_top, mt + m, mb - m - total_h);
    let cy = group_top + r;

    let pill_left = clamp_range(cx - pill_w * 0.5, ml + m, mr - m - pill_w);
    let pill_top = group_top + diam + LOUPE_PILL_GAP;

    LoupePlacement {
        loupe_center: (cx, cy),
        loupe_radius: r,
        pill_rect: RectF {
            left: pill_left,
            top: pill_top,
            right: pill_left + pill_w,
            bottom: pill_top + pill_h,
        },
    }
}

fn place_selection_badge(
    sel: SelectionRect,
    monitor: MonitorBounds,
    badge_size: (f32, f32),
) -> RectF {
    let bw = badge_size.0.max(1.0);
    let bh = badge_size.1.max(1.0);
    let gap = 8.0_f32;
    let margin = 8.0_f32;

    let ml = monitor.left as f32;
    let mt = monitor.top as f32;
    let mr = monitor.right as f32;
    let mb = monitor.bottom as f32;

    let above_top = sel.y as f32 - bh - gap;
    let below_top = (sel.y as f32 + sel.height as f32) + gap;
    let raw_top = if above_top >= mt + margin {
        above_top
    } else {
        below_top
    };

    let left = clamp_range(sel.x as f32, ml + margin, mr - margin - bw);
    let top = clamp_range(raw_top, mt + margin, mb - margin - bh);
    RectF {
        left,
        top,
        right: left + bw,
        bottom: top + bh,
    }
}

struct OverlayGraphics {
    factory: ID2D1Factory,
    dwrite: IDWriteFactory,
    target: ID2D1HwndRenderTarget,
    bitmap: ID2D1Bitmap,
    layer: ID2D1Layer,
    brush: ID2D1SolidColorBrush,
    ui_format: IDWriteTextFormat,
    caption_format: IDWriteTextFormat,
    mono_format: IDWriteTextFormat,
}

impl OverlayGraphics {
    fn new(hwnd: HWND, snapshot: &OverlaySnapshot) -> Result<Self, String> {
        // SAFETY: Direct2D and DirectWrite factory creation takes valid constants and no pointers.
        let factory: ID2D1Factory = unsafe {
            D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)
                .map_err(|e| format!("failed to create Direct2D factory: {e}"))?
        };
        // SAFETY: DirectWrite shared factory creation is thread-safe and self-contained.
        let dwrite: IDWriteFactory = unsafe {
            DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)
                .map_err(|e| format!("failed to create DirectWrite factory: {e}"))?
        };

        let rt_props = D2D1_RENDER_TARGET_PROPERTIES {
            r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_IGNORE,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            usage: D2D1_RENDER_TARGET_USAGE_NONE,
            minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
        };
        let hwnd_props = D2D1_HWND_RENDER_TARGET_PROPERTIES {
            hwnd,
            pixelSize: D2D_SIZE_U {
                width: snapshot.width,
                height: snapshot.height,
            },
            presentOptions: D2D1_PRESENT_OPTIONS_NONE,
        };
        // SAFETY: `hwnd` is a valid live window handle and descriptor structs are initialized.
        let target = unsafe {
            factory
                .CreateHwndRenderTarget(&rt_props, &hwnd_props)
                .map_err(|e| format!("failed to create overlay render target: {e}"))?
        };
        // SAFETY: Force 96 DPI so 1 DIP equals 1 physical pixel across all monitors.
        unsafe {
            target.SetDpi(96.0, 96.0);
        }

        let pitch = snapshot
            .width
            .checked_mul(4)
            .ok_or_else(|| "snapshot stride overflowed".to_string())?;
        let bmp_props = D2D1_BITMAP_PROPERTIES {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_IGNORE,
            },
            dpiX: 96.0,
            dpiY: 96.0,
        };
        let bmp_size = D2D_SIZE_U {
            width: snapshot.width,
            height: snapshot.height,
        };
        // SAFETY: `snapshot.bgra` was validated to contain `width * height * 4` bytes.
        let bitmap = unsafe {
            target
                .CreateBitmap(
                    bmp_size,
                    Some(snapshot.bgra.as_ptr() as *const c_void),
                    pitch,
                    &bmp_props,
                )
                .map_err(|e| format!("failed to upload overlay bitmap: {e}"))?
        };
        // SAFETY: Creating a reusable Direct2D layer with `None` size is valid on `target`.
        let layer = unsafe {
            target
                .CreateLayer(None)
                .map_err(|e| format!("failed to create overlay clip layer: {e}"))?
        };
        let initial_color = rgba_f(255, 255, 255, 1.0);
        // SAFETY: Creating a reusable solid-color brush on a live render target.
        let brush = unsafe {
            target
                .CreateSolidColorBrush(&initial_color, None)
                .map_err(|e| format!("failed to create overlay brush: {e}"))?
        };

        let ui_format = create_text_format(
            &dwrite,
            w!("Segoe UI"),
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            13.0,
            DWRITE_TEXT_ALIGNMENT_LEADING,
        )?;
        let caption_format = create_text_format(
            &dwrite,
            w!("Segoe UI"),
            DWRITE_FONT_WEIGHT_NORMAL,
            10.5,
            DWRITE_TEXT_ALIGNMENT_LEADING,
        )?;
        let mono_format = create_text_format(
            &dwrite,
            w!("Consolas"),
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            13.5,
            DWRITE_TEXT_ALIGNMENT_LEADING,
        )?;

        Ok(Self {
            factory,
            dwrite,
            target,
            bitmap,
            layer,
            brush,
            ui_format,
            caption_format,
            mono_format,
        })
    }

    fn set_brush_color(&self, color: D2D1_COLOR_F) -> &ID2D1SolidColorBrush {
        // SAFETY: `self.brush` is a valid COM solid-color brush bound to `self.target`.
        unsafe {
            self.brush.SetColor(&color);
        }
        &self.brush
    }
}

fn create_text_format(
    dwrite: &IDWriteFactory,
    family: PCWSTR,
    weight: DWRITE_FONT_WEIGHT,
    size: f32,
    alignment: DWRITE_TEXT_ALIGNMENT,
) -> Result<IDWriteTextFormat, String> {
    // SAFETY: `family` and `w!("en-us")` are static null-terminated UTF-16 strings.
    unsafe {
        let fmt = dwrite
            .CreateTextFormat(
                family,
                None,
                weight,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                size,
                w!("en-us"),
            )
            .map_err(|e| format!("failed to create DirectWrite text format: {e}"))?;
        let _ = fmt.SetTextAlignment(alignment);
        let _ = fmt.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);
        let _ = fmt.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP);
        Ok(fmt)
    }
}

fn rgba_f(r: u8, g: u8, b: u8, a: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: f32::from(r) / 255.0,
        g: f32::from(g) / 255.0,
        b: f32::from(b) / 255.0,
        a,
    }
}

enum OverlayModeState {
    Ocr {
        mode: OverlayOcrMode,
        language_label: String,
        drag_start: Option<(i32, i32)>,
        drag_current: (i32, i32),
    },
    ColorPicker {
        format: OverlayColorFormat,
        zoom_cells: i32,
    },
}

struct OverlayState {
    /// Valid for the entire synchronous duration of `run_capture_overlay`.
    snapshot: *const OverlaySnapshot,
    hwnd: HWND,
    previous_foreground: Option<isize>,
    cursor: (i32, i32),
    mode: OverlayModeState,
    graphics: Option<OverlayGraphics>,
    result: Option<CaptureResult>,
    finished: bool,
    tearing_down: bool,
}

impl OverlayState {
    /// # Safety
    /// Caller guarantees `self.snapshot` points to the live `OverlaySnapshot`
    /// borrowed by `run_capture_overlay`.
    unsafe fn snapshot(&self) -> &OverlaySnapshot {
        &*self.snapshot
    }
}

pub fn run_capture_overlay(
    snapshot: &OverlaySnapshot,
    mode: CaptureMode,
    previous_foreground: Option<isize>,
) -> Result<CaptureResult, String> {
    validate_snapshot(snapshot)?;

    // SAFETY: Attempting per-monitor v2 DPI awareness is idempotent and safe to call.
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }

    // SAFETY: Querying the current process module handle for window class registration.
    let instance = unsafe {
        HINSTANCE(
            GetModuleHandleW(None)
                .map_err(|e| format!("failed to resolve module handle: {e}"))?
                .0,
        )
    };

    // SAFETY: Loading the standard crosshair cursor from system resources.
    let cursor_handle = unsafe { LoadCursorW(None, IDC_CROSS).unwrap_or_default() };
    let class = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(overlay_wnd_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: instance,
        hIcon: Default::default(),
        hCursor: cursor_handle,
        hbrBackground: Default::default(),
        lpszMenuName: PCWSTR::null(),
        lpszClassName: w!("YalCaptureOverlay"),
        hIconSm: Default::default(),
    };
    // SAFETY: `class` is fully initialized with a static class name and valid `wnd_proc`.
    if unsafe { RegisterClassExW(&class) } == 0 {
        // SAFETY: `GetLastError` immediately inspects the failed `RegisterClassExW` call.
        let err = unsafe { GetLastError() };
        if err != ERROR_CLASS_ALREADY_EXISTS {
            return Err(format!("failed to register capture overlay class: {err:?}"));
        }
    }

    let mut screen_cursor = POINT::default();
    // SAFETY: `screen_cursor` is a valid mutable stack `POINT`.
    let _ = unsafe { GetCursorPos(&mut screen_cursor) };
    let initial_cursor = nudge_pixel(
        (
            screen_cursor.x.saturating_sub(snapshot.origin_x),
            screen_cursor.y.saturating_sub(snapshot.origin_y),
        ),
        0,
        0,
        snapshot.width,
        snapshot.height,
    );

    let mode_state = match mode {
        CaptureMode::Ocr {
            initial_mode,
            language_label,
        } => OverlayModeState::Ocr {
            mode: initial_mode,
            language_label,
            drag_start: None,
            drag_current: initial_cursor,
        },
        CaptureMode::ColorPicker { initial_format } => OverlayModeState::ColorPicker {
            format: initial_format,
            zoom_cells: DEFAULT_ZOOM_CELLS,
        },
    };

    let state_ptr = Box::into_raw(Box::new(OverlayState {
        snapshot: snapshot as *const OverlaySnapshot,
        hwnd: HWND(std::ptr::null_mut()),
        previous_foreground,
        cursor: initial_cursor,
        mode: mode_state,
        graphics: None,
        result: None,
        finished: false,
        tearing_down: false,
    }));

    let width_i32 = snapshot.width as i32;
    let height_i32 = snapshot.height as i32;
    // SAFETY: Creates the borderless topmost overlay window with `state_ptr` passed via `WM_NCCREATE`.
    let hwnd = match unsafe {
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            w!("YalCaptureOverlay"),
            PCWSTR::null(),
            WS_POPUP | WS_VISIBLE,
            snapshot.origin_x,
            snapshot.origin_y,
            width_i32,
            height_i32,
            None,
            None,
            Some(instance),
            Some(state_ptr as *mut c_void),
        )
    } {
        Ok(hwnd) => hwnd,
        Err(err) => {
            // SAFETY: Window creation failed before `WM_DESTROY`, so we still own `state_ptr`.
            unsafe {
                drop(Box::from_raw(state_ptr));
            }
            return Err(format!("failed to create capture overlay window: {err}"));
        }
    };

    // SAFETY: `state_ptr` is owned on this thread until reclaimed below.
    unsafe {
        (*state_ptr).hwnd = hwnd;
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            snapshot.origin_x,
            snapshot.origin_y,
            width_i32,
            height_i32,
            SWP_SHOWWINDOW,
        );
    }

    match OverlayGraphics::new(hwnd, snapshot) {
        Ok(graphics) => {
            // SAFETY: Single-threaded ownership of `state_ptr` on the overlay UI thread.
            unsafe {
                (*state_ptr).graphics = Some(graphics);
            }
        }
        Err(err) => {
            // SAFETY: Tear down the window and detach `GWLP_USERDATA` before reclaiming `state_ptr`.
            unsafe {
                finish_overlay(state_ptr, CaptureResult::Cancelled);
                if IsWindow(Some(hwnd)).as_bool() {
                    let _ = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                    let _ = DestroyWindow(hwnd);
                }
                drop(Box::from_raw(state_ptr));
            }
            return Err(err);
        }
    }

    // SAFETY: Focus the overlay window, trigger initial render, and run modal message loop.
    unsafe {
        focus_overlay_window(hwnd);
        let _ = InvalidateRect(Some(hwnd), None, false);

        let mut msg = MSG::default();
        loop {
            if (*state_ptr).finished {
                break;
            }
            let ret = GetMessageW(&mut msg, None, 0, 0).0;
            if ret <= 0 {
                if ret == 0 {
                    PostQuitMessage(msg.wParam.0 as i32);
                }
                break;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        if !(*state_ptr).finished {
            finish_overlay(state_ptr, CaptureResult::Cancelled);
        }
        if IsWindow(Some(hwnd)).as_bool() {
            let _ = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            let _ = DestroyWindow(hwnd);
        }
        let state = Box::from_raw(state_ptr);
        Ok(state.result.unwrap_or(CaptureResult::Cancelled))
    }
}

unsafe fn focus_overlay_window(hwnd: HWND) {
    let _ = AllowSetForegroundWindow(ASFW_ANY);
    let _ = SetForegroundWindow(hwnd);
    if GetForegroundWindow() != hwnd {
        let foreground = GetForegroundWindow();
        let our_thread = GetCurrentThreadId();
        let fg_thread = if !foreground.0.is_null() {
            GetWindowThreadProcessId(foreground, None)
        } else {
            0
        };
        let attached = fg_thread != 0
            && fg_thread != our_thread
            && AttachThreadInput(our_thread, fg_thread, true).as_bool();
        let _ = BringWindowToTop(hwnd);
        let _ = SetForegroundWindow(hwnd);
        if attached {
            let _ = AttachThreadInput(our_thread, fg_thread, false);
        }
    }
    let _ = SetFocus(Some(hwnd));
}

unsafe fn restore_previous_foreground(hwnd: HWND, previous_foreground: Option<isize>) {
    let Some(raw) = previous_foreground.filter(|&raw| raw != 0) else {
        return;
    };
    let target = HWND(raw as *mut c_void);
    if target == hwnd
        || !IsWindow(Some(target)).as_bool()
        || !IsWindowVisible(target).as_bool()
        || IsIconic(target).as_bool()
    {
        return;
    }
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

unsafe fn finish_overlay(state_ptr: *mut OverlayState, result: CaptureResult) {
    if state_ptr.is_null() {
        return;
    }
    let (hwnd, prev, graphics) = {
        let state = &mut *state_ptr;
        if state.tearing_down {
            return;
        }
        state.tearing_down = true;
        if state.result.is_none() {
            state.result = Some(result);
        }
        (
            state.hwnd,
            state.previous_foreground.take(),
            state.graphics.take(),
        )
    };
    let _ = ReleaseCapture();
    restore_previous_foreground(hwnd, prev);
    if !hwnd.0.is_null() && IsWindow(Some(hwnd)).as_bool() {
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
    let _ = AllowSetForegroundWindow(ASFW_ANY);
    drop(graphics);
    if !hwnd.0.is_null() && IsWindow(Some(hwnd)).as_bool() {
        let _ = DestroyWindow(hwnd);
    } else {
        (*state_ptr).finished = true;
    }
}

fn shift_is_down() -> bool {
    // SAFETY: `GetKeyState` queries the current thread's virtual key state with no pointers.
    unsafe { (GetKeyState(VK_SHIFT.0 as i32) as u16 & 0x8000) != 0 }
}

fn active_monitor_bounds(snapshot: &OverlaySnapshot, cursor: (i32, i32)) -> MonitorBounds {
    let fallback = MonitorBounds {
        left: 0,
        top: 0,
        right: snapshot.width as i32,
        bottom: snapshot.height as i32,
    };
    let pt = POINT {
        x: snapshot.origin_x.saturating_add(cursor.0),
        y: snapshot.origin_y.saturating_add(cursor.1),
    };
    // SAFETY: `MonitorFromPoint` and `GetMonitorInfoW` take valid stack structs.
    let monitor = unsafe { MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: `info.cbSize` is initialized and `monitor` is returned by `MonitorFromPoint`.
    if !unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        return fallback;
    }
    let left = info
        .rcMonitor
        .left
        .saturating_sub(snapshot.origin_x)
        .clamp(0, fallback.right);
    let top = info
        .rcMonitor
        .top
        .saturating_sub(snapshot.origin_y)
        .clamp(0, fallback.bottom);
    let right = info
        .rcMonitor
        .right
        .saturating_sub(snapshot.origin_x)
        .clamp(0, fallback.right);
    let bottom = info
        .rcMonitor
        .bottom
        .saturating_sub(snapshot.origin_y)
        .clamp(0, fallback.bottom);
    if right > left && bottom > top {
        MonitorBounds {
            left,
            top,
            right,
            bottom,
        }
    } else {
        fallback
    }
}

fn unpack_lparam_xy(lparam: LPARAM) -> (i32, i32) {
    let x = (lparam.0 & 0xFFFF) as i16 as i32;
    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
    (x, y)
}

unsafe extern "system" fn overlay_wnd_proc(
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
    let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut OverlayState;
    if state_ptr.is_null() {
        return DefWindowProcW(hwnd, message, wparam, lparam);
    }

    match message {
        WM_ERASEBKGND => LRESULT(1),
        WM_CLOSE => {
            finish_overlay(state_ptr, CaptureResult::Cancelled);
            LRESULT(0)
        }
        WM_CAPTURECHANGED => {
            let state = &mut *state_ptr;
            if let OverlayModeState::Ocr { drag_start, .. } = &mut state.mode {
                if drag_start.take().is_some() {
                    let _ = InvalidateRect(Some(hwnd), None, false);
                }
            }
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            let raw = unpack_lparam_xy(lparam);
            let state = &mut *state_ptr;
            let snap = state.snapshot();
            let clamped = nudge_pixel(raw, 0, 0, snap.width, snap.height);
            state.cursor = clamped;
            if let OverlayModeState::Ocr {
                drag_start: Some(_),
                drag_current,
                ..
            } = &mut state.mode
            {
                *drag_current = raw;
            }
            let _ = InvalidateRect(Some(hwnd), None, false);
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let raw = unpack_lparam_xy(lparam);
            let (start_capture, picked) = {
                let state = &mut *state_ptr;
                // SAFETY: `state.snapshot` points to the live `OverlaySnapshot` borrowed by
                // `run_capture_overlay` for the entire lifetime of the overlay window.
                let snap = &*state.snapshot;
                let clamped = nudge_pixel(raw, 0, 0, snap.width, snap.height);
                state.cursor = clamped;
                match &mut state.mode {
                    OverlayModeState::Ocr {
                        drag_start,
                        drag_current,
                        ..
                    } => {
                        *drag_start = Some(clamped);
                        *drag_current = clamped;
                        (true, None)
                    }
                    OverlayModeState::ColorPicker { format, .. } => {
                        let fmt = *format;
                        let rgb =
                            sample_snapshot_rgb(snap, clamped.0, clamped.1).unwrap_or([0, 0, 0]);
                        (
                            false,
                            Some(CaptureResult::ColorPicked {
                                rgb,
                                format: fmt,
                                open_inspector: shift_is_down(),
                            }),
                        )
                    }
                }
            };
            if start_capture {
                let _ = SetCapture(hwnd);
                let _ = InvalidateRect(Some(hwnd), None, false);
            }
            if let Some(result) = picked {
                finish_overlay(state_ptr, result);
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            let raw = unpack_lparam_xy(lparam);
            let (had_drag, completed) = {
                let state = &mut *state_ptr;
                let snap = state.snapshot();
                let (width, height, ox, oy) =
                    (snap.width, snap.height, snap.origin_x, snap.origin_y);
                if let OverlayModeState::Ocr {
                    mode,
                    drag_start,
                    drag_current,
                    ..
                } = &mut state.mode
                {
                    if let Some(start) = drag_start.take() {
                        *drag_current = raw;
                        let sel = normalize_selection_rect(start, raw, width, height);
                        let res = if is_valid_ocr_selection(sel) {
                            let (rx, ry, rw, rh) = selection_to_screen_region(ox, oy, sel);
                            Some(CaptureResult::OcrRegion {
                                x: rx,
                                y: ry,
                                width: rw,
                                height: rh,
                                mode: *mode,
                                open_preview: shift_is_down(),
                            })
                        } else {
                            None
                        };
                        (true, res)
                    } else {
                        (false, None)
                    }
                } else {
                    (false, None)
                }
            };
            if had_drag {
                let _ = ReleaseCapture();
            }
            if let Some(result) = completed {
                finish_overlay(state_ptr, result);
            } else if had_drag {
                let _ = InvalidateRect(Some(hwnd), None, false);
            }
            LRESULT(0)
        }
        WM_RBUTTONDOWN => {
            finish_overlay(state_ptr, CaptureResult::Cancelled);
            LRESULT(0)
        }
        WM_MOUSEWHEEL => {
            let wheel_delta = (wparam.0 >> 16) as i16;
            let state = &mut *state_ptr;
            if let OverlayModeState::ColorPicker { zoom_cells, .. } = &mut state.mode {
                *zoom_cells = step_zoom_cells(*zoom_cells, wheel_delta);
                let _ = InvalidateRect(Some(hwnd), None, false);
            }
            LRESULT(0)
        }
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            let vk = VIRTUAL_KEY(wparam.0 as u16);
            handle_keydown(state_ptr, hwnd, vk);
            LRESULT(0)
        }
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let _ = BeginPaint(hwnd, &mut ps);
            paint_overlay(&*state_ptr);
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_DESTROY => {
            let _ = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            (*state_ptr).finished = true;
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

/// # Safety
/// `state_ptr` must be a valid non-null `OverlayState` pointer owned on the current UI thread.
unsafe fn handle_keydown(state_ptr: *mut OverlayState, hwnd: HWND, vk: VIRTUAL_KEY) {
    if vk == VK_ESCAPE {
        finish_overlay(state_ptr, CaptureResult::Cancelled);
        return;
    }
    if vk == VK_TAB {
        {
            let state = &mut *state_ptr;
            match &mut state.mode {
                OverlayModeState::Ocr { mode, .. } => {
                    *mode = match *mode {
                        OverlayOcrMode::Smart => OverlayOcrMode::Raw,
                        OverlayOcrMode::Raw => OverlayOcrMode::Smart,
                    };
                }
                OverlayModeState::ColorPicker { format, .. } => {
                    *format = next_color_format(*format);
                }
            }
        }
        let _ = InvalidateRect(Some(hwnd), None, false);
        return;
    }

    let (nudged_screen, picked) = {
        let state = &mut *state_ptr;
        // SAFETY: `state.snapshot` points to the live `OverlaySnapshot` borrowed by
        // `run_capture_overlay` for the entire lifetime of the overlay window.
        let snap = &*state.snapshot;
        match &mut state.mode {
            OverlayModeState::ColorPicker { format, .. } => {
                if vk == VK_RETURN {
                    let rgb = sample_snapshot_rgb(snap, state.cursor.0, state.cursor.1)
                        .unwrap_or([0, 0, 0]);
                    (
                        None,
                        Some(CaptureResult::ColorPicked {
                            rgb,
                            format: *format,
                            open_inspector: shift_is_down(),
                        }),
                    )
                } else {
                    let delta = match vk {
                        k if k == VK_LEFT => Some((-1, 0)),
                        k if k == VK_RIGHT => Some((1, 0)),
                        k if k == VK_UP => Some((0, -1)),
                        k if k == VK_DOWN => Some((0, 1)),
                        _ => None,
                    };
                    if let Some((dx, dy)) = delta {
                        let next = nudge_pixel(state.cursor, dx, dy, snap.width, snap.height);
                        state.cursor = next;
                        (
                            Some((
                                snap.origin_x.saturating_add(next.0),
                                snap.origin_y.saturating_add(next.1),
                            )),
                            None,
                        )
                    } else {
                        (None, None)
                    }
                }
            }
            OverlayModeState::Ocr { .. } => (None, None),
        }
    };
    if let Some((sx, sy)) = nudged_screen {
        let _ = SetCursorPos(sx, sy);
        let _ = InvalidateRect(Some(hwnd), None, false);
    }
    if let Some(result) = picked {
        finish_overlay(state_ptr, result);
    }
}

fn create_text_layout(
    dwrite: &IDWriteFactory,
    format: &IDWriteTextFormat,
    text: &str,
    max_w: f32,
    max_h: f32,
) -> Option<IDWriteTextLayout> {
    if text.is_empty() || max_w <= 0.0 || max_h <= 0.0 {
        return None;
    }
    let wide: Vec<u16> = text.encode_utf16().collect();
    // SAFETY: `wide` is valid UTF-16 and `format` is a live DirectWrite text format.
    unsafe { dwrite.CreateTextLayout(&wide, format, max_w, max_h).ok() }
}

fn measure_text_width(layout: &IDWriteTextLayout) -> f32 {
    let mut metrics = DWRITE_TEXT_METRICS::default();
    // SAFETY: `metrics` is a valid mutable stack struct.
    unsafe {
        if layout.GetMetrics(&mut metrics).is_ok() {
            metrics.widthIncludingTrailingWhitespace
        } else {
            0.0
        }
    }
}

fn paint_overlay(state: &OverlayState) {
    let Some(gfx) = state.graphics.as_ref() else {
        return;
    };
    // SAFETY: `state.snapshot` remains live for the full synchronous overlay duration.
    let snap = unsafe { state.snapshot() };
    let full_rect = D2D_RECT_F {
        left: 0.0,
        top: 0.0,
        right: snap.width as f32,
        bottom: snap.height as f32,
    };
    let monitor = active_monitor_bounds(snap, state.cursor);

    // SAFETY: `BeginDraw` / `EndDraw` wrap all rendering calls on the live render target.
    unsafe {
        gfx.target.BeginDraw();
        gfx.target.DrawBitmap(
            &gfx.bitmap,
            Some(&full_rect),
            1.0,
            D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
            None,
        );
    }

    match &state.mode {
        OverlayModeState::Ocr {
            mode,
            language_label,
            drag_start,
            drag_current,
        } => {
            paint_ocr_overlay(
                gfx,
                snap,
                monitor,
                state.cursor,
                *mode,
                language_label,
                *drag_start,
                *drag_current,
            );
        }
        OverlayModeState::ColorPicker { format, zoom_cells } => {
            paint_color_picker_overlay(gfx, snap, monitor, state.cursor, *format, *zoom_cells);
        }
    }

    // SAFETY: Matches `BeginDraw` above.
    unsafe {
        let _ = gfx.target.EndDraw(None, None);
    }
}

fn ocr_mode_accent(mode: OverlayOcrMode) -> D2D1_COLOR_F {
    match mode {
        OverlayOcrMode::Smart => rgba_f(0x5E, 0x9E, 0xFF, 1.0),
        OverlayOcrMode::Raw => rgba_f(0xF5, 0x9E, 0x0B, 1.0),
    }
}

fn ocr_mode_label(mode: OverlayOcrMode) -> &'static str {
    match mode {
        OverlayOcrMode::Smart => "Smart",
        OverlayOcrMode::Raw => "Raw",
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_ocr_overlay(
    gfx: &OverlayGraphics,
    snap: &OverlaySnapshot,
    monitor: MonitorBounds,
    cursor: (i32, i32),
    mode: OverlayOcrMode,
    language_label: &str,
    drag_start: Option<(i32, i32)>,
    drag_current: (i32, i32),
) {
    let full_w = snap.width as f32;
    let full_h = snap.height as f32;
    let scrim = rgba_f(10, 12, 16, 0.44);
    let accent = ocr_mode_accent(mode);

    let active_sel = drag_start
        .map(|start| normalize_selection_rect(start, drag_current, snap.width, snap.height))
        .filter(|sel| sel.width >= 1 && sel.height >= 1);

    if let Some(sel) = active_sel {
        let x0 = sel.x as f32;
        let y0 = sel.y as f32;
        let x1 = (sel.x as f32) + (sel.width as f32);
        let y1 = (sel.y as f32) + (sel.height as f32);

        let scrim_brush = gfx.set_brush_color(scrim);
        let bands = [
            D2D_RECT_F {
                left: 0.0,
                top: 0.0,
                right: full_w,
                bottom: y0,
            },
            D2D_RECT_F {
                left: 0.0,
                top: y1,
                right: full_w,
                bottom: full_h,
            },
            D2D_RECT_F {
                left: 0.0,
                top: y0,
                right: x0,
                bottom: y1,
            },
            D2D_RECT_F {
                left: x1,
                top: y0,
                right: full_w,
                bottom: y1,
            },
        ];
        // SAFETY: `gfx.target` is inside `BeginDraw` and `scrim_brush` is bound to it.
        unsafe {
            for band in &bands {
                if band.right > band.left && band.bottom > band.top {
                    gfx.target.FillRectangle(band, scrim_brush);
                }
            }
        }

        // Crisp 2px accent border + L-shaped corner brackets.
        let border_rect = D2D_RECT_F {
            left: x0,
            top: y0,
            right: x1,
            bottom: y1,
        };
        let accent_brush = gfx.set_brush_color(accent);
        // SAFETY: Drawing selection frame on active render target.
        unsafe {
            gfx.target
                .DrawRectangle(&border_rect, accent_brush, 2.0, None);
        }

        let tick = 10.0_f32
            .min((sel.width as f32) * 0.45)
            .min((sel.height as f32) * 0.45);
        if tick >= 3.0 {
            let bracket_brush = gfx.set_brush_color(rgba_f(255, 255, 255, 0.92));
            let sw = 2.0;
            let corners = [
                (
                    Vector2 { X: x0, Y: y0 },
                    Vector2 {
                        X: x0 + tick,
                        Y: y0,
                    },
                    Vector2 {
                        X: x0,
                        Y: y0 + tick,
                    },
                ),
                (
                    Vector2 { X: x1, Y: y0 },
                    Vector2 {
                        X: x1 - tick,
                        Y: y0,
                    },
                    Vector2 {
                        X: x1,
                        Y: y0 + tick,
                    },
                ),
                (
                    Vector2 { X: x0, Y: y1 },
                    Vector2 {
                        X: x0 + tick,
                        Y: y1,
                    },
                    Vector2 {
                        X: x0,
                        Y: y1 - tick,
                    },
                ),
                (
                    Vector2 { X: x1, Y: y1 },
                    Vector2 {
                        X: x1 - tick,
                        Y: y1,
                    },
                    Vector2 {
                        X: x1,
                        Y: y1 - tick,
                    },
                ),
            ];
            // SAFETY: Drawing corner bracket lines on active render target.
            unsafe {
                for (corner, horiz, vert) in corners {
                    gfx.target.DrawLine(corner, horiz, bracket_brush, sw, None);
                    gfx.target.DrawLine(corner, vert, bracket_brush, sw, None);
                }
            }
        }

        // Sleek dark rounded dimension + mode badge above (or below) selection.
        let badge_text = format!(
            "{} × {} px  ·  {}",
            sel.width,
            sel.height,
            ocr_mode_label(mode)
        );
        let text_w = create_text_layout(&gfx.dwrite, &gfx.ui_format, &badge_text, 640.0, 28.0)
            .map(|l| measure_text_width(&l))
            .unwrap_or(140.0);
        let badge_w = (text_w + 24.0).clamp(110.0, 420.0);
        let badge_h = 28.0_f32;
        let badge_rect = place_selection_badge(sel, monitor, (badge_w, badge_h));
        draw_dark_pill(
            gfx,
            badge_rect,
            7.0,
            rgba_f(20, 22, 26, 0.90),
            rgba_f(255, 255, 255, 0.16),
        );
        if let Some(layout) = create_text_layout(
            &gfx.dwrite,
            &gfx.ui_format,
            &badge_text,
            badge_w - 20.0,
            badge_h,
        ) {
            let fg = gfx.set_brush_color(rgba_f(245, 247, 250, 0.96));
            // SAFETY: Drawing DirectWrite layout on active render target.
            unsafe {
                gfx.target.DrawTextLayout(
                    Vector2 {
                        X: badge_rect.left + 12.0,
                        Y: badge_rect.top,
                    },
                    &layout,
                    fg,
                    D2D1_DRAW_TEXT_OPTIONS_CLIP,
                );
            }
        }
    } else {
        let full = D2D_RECT_F {
            left: 0.0,
            top: 0.0,
            right: full_w,
            bottom: full_h,
        };
        let scrim_brush = gfx.set_brush_color(scrim);
        // SAFETY: Filling full-screen dark scrim on active render target.
        unsafe {
            gfx.target.FillRectangle(&full, scrim_brush);
        }

        // Subtle dual-contrast 1px crosshair hairlines through the cursor.
        let cx = cursor.0 as f32 + 0.5;
        let cy = cursor.1 as f32 + 0.5;
        let dark_hair = gfx.set_brush_color(rgba_f(0, 0, 0, 0.34));
        // SAFETY: Drawing dark crosshair backing lines on active render target.
        unsafe {
            gfx.target.DrawLine(
                Vector2 {
                    X: cx + 1.0,
                    Y: 0.0,
                },
                Vector2 {
                    X: cx + 1.0,
                    Y: full_h,
                },
                dark_hair,
                1.0,
                None,
            );
            gfx.target.DrawLine(
                Vector2 {
                    X: 0.0,
                    Y: cy + 1.0,
                },
                Vector2 {
                    X: full_w,
                    Y: cy + 1.0,
                },
                dark_hair,
                1.0,
                None,
            );
        }
        let light_hair = gfx.set_brush_color(rgba_f(255, 255, 255, 0.42));
        // SAFETY: Drawing light crosshair lines on active render target.
        unsafe {
            gfx.target.DrawLine(
                Vector2 { X: cx, Y: 0.0 },
                Vector2 { X: cx, Y: full_h },
                light_hair,
                1.0,
                None,
            );
            gfx.target.DrawLine(
                Vector2 { X: 0.0, Y: cy },
                Vector2 { X: full_w, Y: cy },
                light_hair,
                1.0,
                None,
            );
        }
    }

    paint_ocr_instruction_pill(gfx, monitor, mode, language_label);
}

fn draw_dark_pill(
    gfx: &OverlayGraphics,
    rect: RectF,
    radius: f32,
    fill: D2D1_COLOR_F,
    border: D2D1_COLOR_F,
) {
    let rr = D2D1_ROUNDED_RECT {
        rect: rect.to_d2d(),
        radiusX: radius,
        radiusY: radius,
    };
    let fill_brush = gfx.set_brush_color(fill);
    // SAFETY: Filling rounded pill background on active render target.
    unsafe {
        gfx.target.FillRoundedRectangle(&rr, fill_brush);
    }
    let border_brush = gfx.set_brush_color(border);
    // SAFETY: Stroking rounded pill border on active render target.
    unsafe {
        gfx.target
            .DrawRoundedRectangle(&rr, border_brush, 1.0, None);
    }
}

fn paint_ocr_instruction_pill(
    gfx: &OverlayGraphics,
    monitor: MonitorBounds,
    mode: OverlayOcrMode,
    language_label: &str,
) {
    let text = format!(
        "Screen OCR & QR ({language_label})  ·  Mode: {}  ·  Tab: Toggle  ·  Shift: Preview  ·  Esc: Cancel",
        ocr_mode_label(mode)
    );
    let mon_w = (monitor.right - monitor.left).max(1) as f32;
    let max_pill_w = (mon_w - 32.0).max(200.0);
    let text_w = create_text_layout(&gfx.dwrite, &gfx.ui_format, &text, max_pill_w, 34.0)
        .map(|l| measure_text_width(&l))
        .unwrap_or(460.0);
    let pill_w = (text_w + 46.0).clamp(200.0, max_pill_w);
    let pill_h = 34.0_f32;
    let left = clamp_range(
        monitor.left as f32 + (mon_w - pill_w) * 0.5,
        monitor.left as f32 + 12.0,
        monitor.right as f32 - 12.0 - pill_w,
    );
    let top = (monitor.top as f32) + 18.0;
    let pill_rect = RectF {
        left,
        top,
        right: left + pill_w,
        bottom: top + pill_h,
    };

    draw_dark_pill(
        gfx,
        pill_rect,
        10.0,
        rgba_f(20, 22, 26, 0.90),
        rgba_f(255, 255, 255, 0.16),
    );

    let dot = D2D1_ELLIPSE {
        point: Vector2 {
            X: pill_rect.left + 16.0,
            Y: pill_rect.top + pill_h * 0.5,
        },
        radiusX: 4.5,
        radiusY: 4.5,
    };
    let dot_brush = gfx.set_brush_color(ocr_mode_accent(mode));
    // SAFETY: Drawing mode indicator dot on active render target.
    unsafe {
        gfx.target.FillEllipse(&dot, dot_brush);
    }

    if let Some(layout) =
        create_text_layout(&gfx.dwrite, &gfx.ui_format, &text, pill_w - 38.0, pill_h)
    {
        let fg = gfx.set_brush_color(rgba_f(244, 246, 250, 0.96));
        // SAFETY: Drawing instruction pill label on active render target.
        unsafe {
            gfx.target.DrawTextLayout(
                Vector2 {
                    X: pill_rect.left + 28.0,
                    Y: pill_rect.top,
                },
                &layout,
                fg,
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
            );
        }
    }
}

fn paint_color_picker_overlay(
    gfx: &OverlayGraphics,
    snap: &OverlaySnapshot,
    monitor: MonitorBounds,
    cursor: (i32, i32),
    format: OverlayColorFormat,
    zoom_cells: i32,
) {
    let sampled = sample_snapshot_rgb(snap, cursor.0, cursor.1).unwrap_or([0, 0, 0]);
    let color_text = format_overlay_color(sampled, format);
    let caption_text = "Tab: Format · Scroll: Zoom · Arrows: 1px · Shift+Click: Inspect";

    let color_w = create_text_layout(&gfx.dwrite, &gfx.mono_format, &color_text, 480.0, 22.0)
        .map(|l| measure_text_width(&l))
        .unwrap_or(140.0);
    let caption_w = create_text_layout(&gfx.dwrite, &gfx.caption_format, caption_text, 480.0, 16.0)
        .map(|l| measure_text_width(&l))
        .unwrap_or(280.0);
    let pill_w = (color_w + 54.0).max(caption_w + 26.0).clamp(240.0, 440.0);
    let pill_h = 52.0_f32;

    let placement = place_loupe_and_pill(cursor, monitor, (pill_w, pill_h));
    let (lcx, lcy) = placement.loupe_center;
    let radius = placement.loupe_radius;
    let cells = zoom_cells.max(3);
    let half_cells = cells / 2;
    let cell_size = (radius * 2.0) / (cells as f32);
    let grid_origin_x = lcx - (cells as f32) * 0.5 * cell_size;
    let grid_origin_y = lcy - (cells as f32) * 0.5 * cell_size;

    let loupe_ellipse = D2D1_ELLIPSE {
        point: Vector2 { X: lcx, Y: lcy },
        radiusX: radius,
        radiusY: radius,
    };
    let clip_bounds = D2D_RECT_F {
        left: lcx - radius - 1.0,
        top: lcy - radius - 1.0,
        right: lcx + radius + 1.0,
        bottom: lcy + radius + 1.0,
    };

    // SAFETY: Creating a transient ellipse geometry for circular loupe clipping.
    if let Ok(ellipse_geom) = unsafe { gfx.factory.CreateEllipseGeometry(&loupe_ellipse) } {
        let geom: ID2D1Geometry = ellipse_geom.into();
        let mut params = D2D1_LAYER_PARAMETERS {
            contentBounds: clip_bounds,
            geometricMask: ManuallyDrop::new(Some(geom)),
            maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
            maskTransform: Matrix3x2::identity(),
            opacity: 1.0,
            opacityBrush: ManuallyDrop::new(None),
            layerOptions: D2D1_LAYER_OPTIONS_NONE,
        };
        // SAFETY: `PushLayer` borrows `params`; dropping `ManuallyDrop` fields immediately
        // prevents COM leaks and `PopLayer` is unconditionally invoked below.
        unsafe {
            gfx.target.PushLayer(&params, &gfx.layer);
            ManuallyDrop::drop(&mut params.geometricMask);
            ManuallyDrop::drop(&mut params.opacityBrush);
            gfx.target.SetAntialiasMode(D2D1_ANTIALIAS_MODE_ALIASED);
        }

        // Paint zoomed pixel grid directly from snapshot BGRA with zero interpolation blur.
        let oob_color = rgba_f(0x14, 0x16, 0x1A, 1.0);
        for dy in -half_cells..=half_cells {
            let py = cursor.1.saturating_add(dy);
            let top = grid_origin_y + ((dy + half_cells) as f32) * cell_size;
            let bottom = grid_origin_y + ((dy + half_cells + 1) as f32) * cell_size;
            for dx in -half_cells..=half_cells {
                let px = cursor.0.saturating_add(dx);
                let left = grid_origin_x + ((dx + half_cells) as f32) * cell_size;
                let right = grid_origin_x + ((dx + half_cells + 1) as f32) * cell_size;
                let col = match sample_snapshot_rgb(snap, px, py) {
                    Some([r, g, b]) => rgba_f(r, g, b, 1.0),
                    None => oob_color,
                };
                let cell_rect = D2D_RECT_F {
                    left,
                    top,
                    right,
                    bottom,
                };
                let cell_brush = gfx.set_brush_color(col);
                // SAFETY: Drawing magnified pixel cell inside active clipped layer.
                unsafe {
                    gfx.target.FillRectangle(&cell_rect, cell_brush);
                }
            }
        }

        // Subtle dual-tone hairline pixel grid lines.
        let grid_dark = gfx.set_brush_color(rgba_f(0, 0, 0, 0.18));
        for i in 1..cells {
            let x = grid_origin_x + (i as f32) * cell_size;
            let y = grid_origin_y + (i as f32) * cell_size;
            // SAFETY: Drawing pixel grid hairlines inside active clipped layer.
            unsafe {
                gfx.target.DrawLine(
                    Vector2 {
                        X: x + 0.5,
                        Y: lcy - radius,
                    },
                    Vector2 {
                        X: x + 0.5,
                        Y: lcy + radius,
                    },
                    grid_dark,
                    1.0,
                    None,
                );
                gfx.target.DrawLine(
                    Vector2 {
                        X: lcx - radius,
                        Y: y + 0.5,
                    },
                    Vector2 {
                        X: lcx + radius,
                        Y: y + 0.5,
                    },
                    grid_dark,
                    1.0,
                    None,
                );
            }
        }
        let grid_light = gfx.set_brush_color(rgba_f(255, 255, 255, 0.14));
        for i in 1..cells {
            let x = grid_origin_x + (i as f32) * cell_size;
            let y = grid_origin_y + (i as f32) * cell_size;
            // SAFETY: Drawing light pixel grid hairlines inside active clipped layer.
            unsafe {
                gfx.target.DrawLine(
                    Vector2 {
                        X: x,
                        Y: lcy - radius,
                    },
                    Vector2 {
                        X: x,
                        Y: lcy + radius,
                    },
                    grid_light,
                    1.0,
                    None,
                );
                gfx.target.DrawLine(
                    Vector2 {
                        X: lcx - radius,
                        Y: y,
                    },
                    Vector2 {
                        X: lcx + radius,
                        Y: y,
                    },
                    grid_light,
                    1.0,
                    None,
                );
            }
        }

        // Center pixel reticle (black outer + white inner contrast strokes).
        let center_left = grid_origin_x + (half_cells as f32) * cell_size;
        let center_top = grid_origin_y + (half_cells as f32) * cell_size;
        let outer_reticle = D2D_RECT_F {
            left: center_left - 0.5,
            top: center_top - 0.5,
            right: center_left + cell_size + 0.5,
            bottom: center_top + cell_size + 0.5,
        };
        let inner_reticle = D2D_RECT_F {
            left: center_left + 1.0,
            top: center_top + 1.0,
            right: center_left + cell_size - 1.0,
            bottom: center_top + cell_size - 1.0,
        };
        let black_brush = gfx.set_brush_color(rgba_f(0, 0, 0, 0.92));
        // SAFETY: Drawing center pixel outer reticle.
        unsafe {
            gfx.target
                .DrawRectangle(&outer_reticle, black_brush, 1.5, None);
        }
        let white_brush = gfx.set_brush_color(rgba_f(255, 255, 255, 0.96));
        // SAFETY: Drawing center pixel inner reticle.
        unsafe {
            gfx.target
                .DrawRectangle(&inner_reticle, white_brush, 1.5, None);
            gfx.target
                .SetAntialiasMode(D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
            gfx.target.PopLayer();
        }
    }

    // Outer circular rim: dark outer ring + sampled-color ring + subtle light inner ring.
    let outer_ring = D2D1_ELLIPSE {
        point: Vector2 { X: lcx, Y: lcy },
        radiusX: radius + 2.5,
        radiusY: radius + 2.5,
    };
    let accent_ring = D2D1_ELLIPSE {
        point: Vector2 { X: lcx, Y: lcy },
        radiusX: radius + 0.5,
        radiusY: radius + 0.5,
    };
    let inner_ring = D2D1_ELLIPSE {
        point: Vector2 { X: lcx, Y: lcy },
        radiusX: radius - 1.5,
        radiusY: radius - 1.5,
    };
    let dark_rim = gfx.set_brush_color(rgba_f(14, 16, 20, 0.92));
    // SAFETY: Drawing loupe outer border rings on active render target.
    unsafe {
        gfx.target.DrawEllipse(&outer_ring, dark_rim, 3.0, None);
    }
    let sampled_rim = gfx.set_brush_color(rgba_f(sampled[0], sampled[1], sampled[2], 1.0));
    // SAFETY: Drawing sampled-color accent ring on active render target.
    unsafe {
        gfx.target.DrawEllipse(&accent_ring, sampled_rim, 3.0, None);
    }
    let inner_rim = gfx.set_brush_color(rgba_f(255, 255, 255, 0.45));
    // SAFETY: Drawing subtle inner highlight ring on active render target.
    unsafe {
        gfx.target.DrawEllipse(&inner_ring, inner_rim, 1.0, None);
    }

    // Compact dark rounded readout pill beneath the loupe.
    let pr = placement.pill_rect;
    draw_dark_pill(
        gfx,
        pr,
        10.0,
        rgba_f(20, 22, 26, 0.92),
        rgba_f(255, 255, 255, 0.16),
    );

    let swatch = D2D1_ROUNDED_RECT {
        rect: D2D_RECT_F {
            left: pr.left + 12.0,
            top: pr.top + 8.0,
            right: pr.left + 30.0,
            bottom: pr.top + 26.0,
        },
        radiusX: 4.0,
        radiusY: 4.0,
    };
    let swatch_fill = gfx.set_brush_color(rgba_f(sampled[0], sampled[1], sampled[2], 1.0));
    // SAFETY: Filling 18x18 sampled color swatch inside readout pill.
    unsafe {
        gfx.target.FillRoundedRectangle(&swatch, swatch_fill);
    }
    let swatch_border = gfx.set_brush_color(rgba_f(255, 255, 255, 0.36));
    // SAFETY: Stroking swatch border inside readout pill.
    unsafe {
        gfx.target
            .DrawRoundedRectangle(&swatch, swatch_border, 1.0, None);
    }

    if let Some(layout) = create_text_layout(
        &gfx.dwrite,
        &gfx.mono_format,
        &color_text,
        pill_w - 48.0,
        22.0,
    ) {
        let fg = gfx.set_brush_color(rgba_f(248, 250, 252, 0.98));
        // SAFETY: Drawing monospace color readout inside pill.
        unsafe {
            gfx.target.DrawTextLayout(
                Vector2 {
                    X: pr.left + 38.0,
                    Y: pr.top + 6.0,
                },
                &layout,
                fg,
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
            );
        }
    }

    if let Some(layout) = create_text_layout(
        &gfx.dwrite,
        &gfx.caption_format,
        caption_text,
        pill_w - 24.0,
        16.0,
    ) {
        let dim = gfx.set_brush_color(rgba_f(176, 184, 198, 0.82));
        // SAFETY: Drawing control hint caption inside pill.
        unsafe {
            gfx.target.DrawTextLayout(
                Vector2 {
                    X: pr.left + 12.0,
                    Y: pr.top + 30.0,
                },
                &layout,
                dim,
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_placement_in_monitor(placement: LoupePlacement, monitor: MonitorBounds) {
        let (cx, cy) = placement.loupe_center;
        let r = placement.loupe_radius;
        let ml = monitor.left as f32 + MONITOR_EDGE_MARGIN;
        let mt = monitor.top as f32 + MONITOR_EDGE_MARGIN;
        let mr = monitor.right as f32 - MONITOR_EDGE_MARGIN;
        let mb = monitor.bottom as f32 - MONITOR_EDGE_MARGIN;

        assert!(cx - r >= ml - 0.01, "loupe left {} < {}", cx - r, ml);
        assert!(cx + r <= mr + 0.01, "loupe right {} > {}", cx + r, mr);
        assert!(cy - r >= mt - 0.01, "loupe top {} < {}", cy - r, mt);
        assert!(cy + r <= mb + 0.01, "loupe bottom {} > {}", cy + r, mb);
        assert!(
            placement.pill_rect.left >= ml - 0.01,
            "pill left {} < {}",
            placement.pill_rect.left,
            ml
        );
        assert!(
            placement.pill_rect.right <= mr + 0.01,
            "pill right {} > {}",
            placement.pill_rect.right,
            mr
        );
        assert!(
            placement.pill_rect.top >= mt - 0.01,
            "pill top {} < {}",
            placement.pill_rect.top,
            mt
        );
        assert!(
            placement.pill_rect.bottom <= mb + 0.01,
            "pill bottom {} > {}",
            placement.pill_rect.bottom,
            mb
        );
    }

    #[test]
    fn format_overlay_color_matches_clipboard_oracle() {
        let coral = [0xFF, 0x57, 0x33];
        assert_eq!(
            format_overlay_color(coral, OverlayColorFormat::Hex),
            "#FF5733"
        );
        assert_eq!(
            format_overlay_color(coral, OverlayColorFormat::Rgb),
            "rgb(255, 87, 51)"
        );
        assert_eq!(
            format_overlay_color(coral, OverlayColorFormat::Hsl),
            "hsl(10.6deg, 100%, 60%)"
        );
        assert_eq!(
            format_overlay_color(coral, OverlayColorFormat::Oklch),
            "oklch(68% 0.21 33.7deg)"
        );

        let black = [0, 0, 0];
        assert_eq!(
            format_overlay_color(black, OverlayColorFormat::Hex),
            "#000000"
        );
        assert_eq!(
            format_overlay_color(black, OverlayColorFormat::Rgb),
            "rgb(0, 0, 0)"
        );
        assert_eq!(
            format_overlay_color(black, OverlayColorFormat::Hsl),
            "hsl(0deg, 0%, 0%)"
        );
        assert_eq!(
            format_overlay_color(black, OverlayColorFormat::Oklch),
            "oklch(0% 0 0deg)"
        );

        let white = [255, 255, 255];
        assert_eq!(
            format_overlay_color(white, OverlayColorFormat::Hex),
            "#FFFFFF"
        );
        assert_eq!(
            format_overlay_color(white, OverlayColorFormat::Rgb),
            "rgb(255, 255, 255)"
        );
        assert_eq!(
            format_overlay_color(white, OverlayColorFormat::Hsl),
            "hsl(0deg, 0%, 100%)"
        );
        assert_eq!(
            format_overlay_color(white, OverlayColorFormat::Oklch),
            "oklch(100% 0 0deg)"
        );

        let green = [0, 255, 0];
        assert_eq!(
            format_overlay_color(green, OverlayColorFormat::Hex),
            "#00FF00"
        );
        assert_eq!(
            format_overlay_color(green, OverlayColorFormat::Rgb),
            "rgb(0, 255, 0)"
        );
        assert_eq!(
            format_overlay_color(green, OverlayColorFormat::Hsl),
            "hsl(120deg, 100%, 50%)"
        );
        assert_eq!(
            format_overlay_color(green, OverlayColorFormat::Oklch),
            "oklch(86.6% 0.295 142.5deg)"
        );
    }

    #[test]
    fn color_format_cycles_in_order() {
        assert_eq!(
            next_color_format(OverlayColorFormat::Hex),
            OverlayColorFormat::Rgb
        );
        assert_eq!(
            next_color_format(OverlayColorFormat::Rgb),
            OverlayColorFormat::Hsl
        );
        assert_eq!(
            next_color_format(OverlayColorFormat::Hsl),
            OverlayColorFormat::Oklch
        );
        assert_eq!(
            next_color_format(OverlayColorFormat::Oklch),
            OverlayColorFormat::Hex
        );
    }

    #[test]
    fn loupe_placement_flips_quadrants_near_all_edges_and_negative_monitor() {
        let pill = (280.0, 52.0);
        let primary = MonitorBounds {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1080,
        };

        // Center defaults to bottom-right of cursor.
        let center = place_loupe_and_pill((500, 400), primary, pill);
        assert!(center.loupe_center.0 > 500.0);
        assert!(center.loupe_center.1 > 400.0);
        assert_placement_in_monitor(center, primary);

        // Near right edge flips left.
        let near_right = place_loupe_and_pill((1890, 400), primary, pill);
        assert!(near_right.loupe_center.0 < 1890.0);
        assert!(near_right.loupe_center.1 > 400.0);
        assert_placement_in_monitor(near_right, primary);

        // Near bottom edge flips up.
        let near_bottom = place_loupe_and_pill((500, 1050), primary, pill);
        assert!(near_bottom.loupe_center.0 > 500.0);
        assert!(near_bottom.loupe_center.1 < 1050.0);
        assert_placement_in_monitor(near_bottom, primary);

        // Near bottom-right corner flips both left and up.
        let near_br = place_loupe_and_pill((1900, 1060), primary, pill);
        assert!(near_br.loupe_center.0 < 1900.0);
        assert!(near_br.loupe_center.1 < 1060.0);
        assert_placement_in_monitor(near_br, primary);

        // Near top-left corner stays inside monitor bounds.
        let near_tl = place_loupe_and_pill((5, 5), primary, pill);
        assert!(near_tl.loupe_center.0 > 5.0);
        assert!(near_tl.loupe_center.1 > 5.0);
        assert_placement_in_monitor(near_tl, primary);

        // Negative-origin monitor (left/above primary).
        let neg = MonitorBounds {
            left: -1920,
            top: -1080,
            right: 0,
            bottom: 0,
        };
        for cursor in [
            (-1000, -500),
            (-20, -20),
            (-1900, -1060),
            (-20, -1060),
            (-1900, -20),
        ] {
            let placed = place_loupe_and_pill(cursor, neg, pill);
            assert_placement_in_monitor(placed, neg);
        }
        let neg_br = place_loupe_and_pill((-20, -20), neg, pill);
        assert!(neg_br.loupe_center.0 < -20.0);
        assert!(neg_br.loupe_center.1 < -20.0);
    }

    #[test]
    fn selection_rect_normalization_and_validation() {
        // Forward drag.
        let fwd = normalize_selection_rect((100, 200), (260, 320), 1920, 1080);
        assert_eq!(
            fwd,
            SelectionRect {
                x: 100,
                y: 200,
                width: 160,
                height: 120,
            }
        );
        assert!(is_valid_ocr_selection(fwd));

        // Reverse right-to-left and bottom-to-top drag.
        let rev = normalize_selection_rect((260, 320), (100, 200), 1920, 1080);
        assert_eq!(rev, fwd);

        // Clamping beyond snapshot bounds.
        let clamped = normalize_selection_rect((-50, -10), (2000, 1200), 1920, 1080);
        assert_eq!(
            clamped,
            SelectionRect {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            }
        );

        // Minimum 6x6 threshold.
        let too_narrow = normalize_selection_rect((10, 10), (15, 30), 100, 100);
        assert!(!is_valid_ocr_selection(too_narrow));
        let too_short = normalize_selection_rect((10, 10), (30, 15), 100, 100);
        assert!(!is_valid_ocr_selection(too_short));
        let exact_min = normalize_selection_rect((10, 10), (16, 16), 100, 100);
        assert!(is_valid_ocr_selection(exact_min));

        // Negative virtual-desktop origin mapping.
        let mapped = selection_to_screen_region(-1920, -1080, fwd);
        assert_eq!(mapped, (-1820, -880, 160, 120));
    }

    #[test]
    fn selection_badge_flips_below_near_top_edge() {
        let monitor = MonitorBounds {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1080,
        };
        let near_top = SelectionRect {
            x: 100,
            y: 10,
            width: 200,
            height: 80,
        };
        let badge = place_selection_badge(near_top, monitor, (160.0, 28.0));
        assert!(badge.top >= (near_top.y as f32 + near_top.height as f32));

        let mid = SelectionRect {
            x: 100,
            y: 300,
            width: 200,
            height: 80,
        };
        let badge_above = place_selection_badge(mid, monitor, (160.0, 28.0));
        assert!(badge_above.bottom <= mid.y as f32);
    }

    #[test]
    fn nudge_pixel_clamps_to_snapshot_bounds() {
        assert_eq!(nudge_pixel((0, 0), -1, -1, 100, 50), (0, 0));
        assert_eq!(nudge_pixel((99, 49), 1, 1, 100, 50), (99, 49));
        assert_eq!(nudge_pixel((10, 20), 1, 0, 100, 50), (11, 20));
        assert_eq!(nudge_pixel((10, 20), 0, -1, 100, 50), (10, 19));
    }

    #[test]
    fn step_zoom_cells_cycles_and_clamps() {
        assert_eq!(step_zoom_cells(11, 120), 7);
        assert_eq!(step_zoom_cells(7, 120), 7);
        assert_eq!(step_zoom_cells(7, -120), 11);
        assert_eq!(step_zoom_cells(11, -120), 15);
        assert_eq!(step_zoom_cells(15, -120), 21);
        assert_eq!(step_zoom_cells(21, -120), 21);
        assert_eq!(step_zoom_cells(21, 120), 15);
        assert_eq!(step_zoom_cells(11, 0), 11);
    }

    #[test]
    fn snapshot_validation_and_bgra_sampling() {
        // 2x2 BGRA image.
        let snap = OverlaySnapshot {
            origin_x: -100,
            origin_y: -50,
            width: 2,
            height: 2,
            bgra: vec![
                0x33, 0x57, 0xFF, 0xFF, // (0,0) -> #FF5733
                0x00, 0xFF, 0x00, 0xFF, // (1,0) -> #00FF00
                0xFF, 0x00, 0x00, 0xFF, // (0,1) -> #0000FF
                0x10, 0x20, 0x30, 0xFF, // (1,1) -> #302010
            ],
        };
        assert!(validate_snapshot(&snap).is_ok());
        assert_eq!(sample_snapshot_rgb(&snap, 0, 0), Some([0xFF, 0x57, 0x33]));
        assert_eq!(sample_snapshot_rgb(&snap, 1, 0), Some([0x00, 0xFF, 0x00]));
        assert_eq!(sample_snapshot_rgb(&snap, 0, 1), Some([0x00, 0x00, 0xFF]));
        assert_eq!(sample_snapshot_rgb(&snap, 1, 1), Some([0x30, 0x20, 0x10]));
        assert_eq!(sample_snapshot_rgb(&snap, -1, 0), None);
        assert_eq!(sample_snapshot_rgb(&snap, 2, 0), None);
        assert_eq!(sample_snapshot_rgb(&snap, 0, 2), None);

        let zero_dim = OverlaySnapshot {
            origin_x: 0,
            origin_y: 0,
            width: 0,
            height: 10,
            bgra: vec![],
        };
        assert!(validate_snapshot(&zero_dim).is_err());

        let bad_len = OverlaySnapshot {
            origin_x: 0,
            origin_y: 0,
            width: 2,
            height: 2,
            bgra: vec![0; 12],
        };
        assert!(validate_snapshot(&bad_len).is_err());
    }
}
