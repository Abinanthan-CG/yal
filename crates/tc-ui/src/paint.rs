//! The palette's paint: Direct2D shapes, GPU-cached Windows app icons, and
//! DirectWrite typography in pure 1:1 DIP coordinates.

use std::collections::HashMap;
use std::ffi::c_void;

use windows::core::{IUnknown, Interface, BOOL, PCWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct2D::{
    Common::{
        D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_HOLLOW,
        D2D1_FIGURE_END_OPEN, D2D1_PIXEL_FORMAT, D2D_RECT_F, D2D_SIZE_F, D2D_SIZE_U,
    },
    D2D1CreateFactory, ID2D1Bitmap, ID2D1Factory, ID2D1HwndRenderTarget, ID2D1PathGeometry,
    ID2D1SolidColorBrush, ID2D1StrokeStyle, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_ARC_SEGMENT,
    D2D1_ARC_SIZE_LARGE, D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, D2D1_BITMAP_PROPERTIES,
    D2D1_CAP_STYLE_ROUND, D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT,
    D2D1_ELLIPSE, D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_FEATURE_LEVEL_DEFAULT,
    D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_LINE_JOIN_ROUND, D2D1_PRESENT_OPTIONS_NONE,
    D2D1_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_NONE,
    D2D1_ROUNDED_RECT, D2D1_STROKE_STYLE_PROPERTIES, D2D1_SWEEP_DIRECTION_CLOCKWISE,
};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, IDWriteTextLayout,
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT, DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_FONT_WEIGHT_NORMAL,
    DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_HIT_TEST_METRICS, DWRITE_TEXT_ALIGNMENT,
    DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_TEXT_ALIGNMENT_TRAILING,
    DWRITE_TEXT_METRICS, DWRITE_TEXT_RANGE, DWRITE_TRIMMING, DWRITE_TRIMMING_GRANULARITY_CHARACTER,
    DWRITE_WORD_WRAPPING_NO_WRAP, DWRITE_WORD_WRAPPING_WRAP,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R8G8B8A8_UNORM;
use windows_numerics::{Matrix3x2, Vector2};

use crate::model::{
    ActionMenuModel, DetailPane, GridModel, IconBitmap, IconKind, Model, Row, ToastKind,
    ACTION_MENU_HEADER_H, ACTION_MENU_ROW_H, GRID_CELL_HEIGHT, MASTER_SPLIT_RATIO,
};
use crate::theme::{Appearance, Color, Theme};

/// Primary UI font (DirectWrite cleanly resolves Segoe UI Variable on Windows 11
/// and falls back gracefully on Windows 10).
const FONT_FAMILY: &str = "Segoe UI";
/// Windows color emoji font for the 2D Emoji & Symbol grid.
const EMOJI_FONT_FAMILY: &str = "Segoe UI Emoji";

/// Maximum GPU-cached application icon bitmaps to keep RAM strictly bounded.
const MAX_CACHED_BITMAPS: usize = 128;

/// Layout constants for the horizontal tint/shade color ramp strip in `DetailPane`.
const COLOR_RAMP_TILE_HEIGHT: f32 = 28.0;
const COLOR_RAMP_TILE_GAP: f32 = 4.0;
const COLOR_RAMP_TILE_RADIUS: f32 = 5.0;
const COLOR_RAMP_LABEL_GAP: f32 = 3.0;
const COLOR_RAMP_LABEL_HEIGHT: f32 = 13.0;
const COLOR_RAMP_TOTAL_HEIGHT: f32 =
    COLOR_RAMP_TILE_HEIGHT + COLOR_RAMP_LABEL_GAP + COLOR_RAMP_LABEL_HEIGHT;

/// Geometry for one swatch tile and its centered label in a horizontal color ramp.
#[derive(Clone, Copy, Debug, PartialEq)]
struct ColorRampTile {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
    label_top: f32,
}

/// Interactive scroll & text-selection state for the right-hand Master-Detail preview pane.
#[derive(Debug, Clone, Default)]
pub struct DetailTextState {
    pub scroll_offset: f32,
    pub content_height: f32,
    pub viewport_rect: Option<(f32, f32, f32, f32)>,
    pub text_origin: (f32, f32),
    pub text_width: f32,
    pub selection: Option<(u32, u32)>,
    pub dragging: bool,
    pub last_body_hash: u64,
}

impl DetailTextState {
    pub fn sync_body(&mut self, text: &str) {
        let hash = detail_text_fingerprint(text);
        if hash != self.last_body_hash {
            self.last_body_hash = hash;
            self.scroll_offset = 0.0;
            self.selection = None;
            self.dragging = false;
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn viewport_height(&self) -> f32 {
        match self.viewport_rect {
            Some((_, t, _, b)) => (b - t).max(0.0),
            None => 0.0,
        }
    }

    pub fn max_scroll(&self) -> f32 {
        (self.content_height - self.viewport_height()).max(0.0)
    }

    pub fn clamp_scroll(&mut self) {
        self.scroll_offset = self.scroll_offset.clamp(0.0, self.max_scroll());
    }

    pub fn scroll_by(&mut self, delta_dip: f32) {
        self.scroll_offset = (self.scroll_offset + delta_dip).clamp(0.0, self.max_scroll());
    }

    pub fn contains_point(&self, x_dip: f32, y_dip: f32) -> bool {
        match self.viewport_rect {
            Some((l, t, r, b)) => x_dip >= l && x_dip <= r && y_dip >= t && y_dip <= b,
            None => false,
        }
    }

    pub fn selected_range(&self) -> Option<(u32, u32)> {
        let (a, b) = self.selection?;
        if a == b {
            None
        } else {
            Some((a.min(b), a.max(b)))
        }
    }
}

/// Returns the active text string displayed inside a `DetailPane`'s scrollable text area.
pub fn detail_preview_text(detail: &DetailPane) -> &str {
    if detail.color_swatch.is_some() {
        detail.body.as_str()
    } else if detail.image_key.is_some() && detail.image.is_some() {
        ""
    } else if detail.body.trim().is_empty() {
        detail.title.as_str()
    } else {
        detail.body.as_str()
    }
}

/// Fast FNV-1a 64-bit fingerprint mixed with string byte length.
pub fn detail_text_fingerprint(text: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash ^ (text.len() as u64).rotate_left(32)
}

/// Safely extracts a UTF-16 code unit range `[min(a,b)..max(a,b)]` from `text` without
/// ever splitting UTF-16 surrogate pairs or panicking on out-of-range offsets.
pub fn utf16_slice(text: &str, a: u32, b: u32) -> String {
    let start = a.min(b);
    let end = a.max(b);
    if start == end || text.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    let mut pos: u32 = 0;
    for ch in text.chars() {
        let len = ch.len_utf16() as u32;
        let next = pos.saturating_add(len);
        if next > start && pos < end {
            out.push(ch);
        }
        if pos >= end {
            break;
        }
        pos = next;
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CharClass {
    Word,
    Whitespace,
    Punct,
}

fn classify_char(ch: char) -> CharClass {
    if ch.is_alphanumeric() || ch == '_' {
        CharClass::Word
    } else if ch.is_whitespace() {
        CharClass::Whitespace
    } else {
        CharClass::Punct
    }
}

/// Expands a UTF-16 code unit offset to the surrounding contiguous run of matching
/// character class (`Word`, `Whitespace`, or `Punct`) without splitting surrogate pairs.
pub fn word_bounds_utf16(text: &str, utf16_pos: u32) -> (u32, u32) {
    if text.is_empty() {
        return (0, 0);
    }
    let mut spans: Vec<(u32, u32, CharClass)> = Vec::new();
    let mut pos: u32 = 0;
    for ch in text.chars() {
        let len = ch.len_utf16() as u32;
        let next = pos.saturating_add(len);
        spans.push((pos, next, classify_char(ch)));
        pos = next;
    }
    let total_u16 = pos;
    let clamped = utf16_pos.min(total_u16.saturating_sub(1));
    let idx = spans
        .iter()
        .position(|&(s, e, _)| clamped >= s && clamped < e)
        .unwrap_or(spans.len() - 1);
    let target_class = spans[idx].2;

    let mut left_idx = idx;
    while left_idx > 0 && spans[left_idx - 1].2 == target_class {
        left_idx -= 1;
    }
    let mut right_idx = idx;
    while right_idx + 1 < spans.len() && spans[right_idx + 1].2 == target_class {
        right_idx += 1;
    }
    (spans[left_idx].0, spans[right_idx].1)
}

/// Draws the palette onto a window in crisp 1:1 DIPs.
pub struct Renderer {
    target: ID2D1HwndRenderTarget,
    dwrite: IDWriteFactory,
    name_format: IDWriteTextFormat,
    detail_format: IDWriteTextFormat,
    accessory_format: IDWriteTextFormat,
    section_format: IDWriteTextFormat,
    pill_format: IDWriteTextFormat,
    footer_format: IDWriteTextFormat,
    footer_primary_format: IDWriteTextFormat,
    keycap_format: IDWriteTextFormat,
    icon_format: IDWriteTextFormat,
    calc_expr_format: IDWriteTextFormat,
    calc_expr_med_format: IDWriteTextFormat,
    calc_expr_sm_format: IDWriteTextFormat,
    calc_answer_format: IDWriteTextFormat,
    calc_answer_med_format: IDWriteTextFormat,
    calc_answer_sm_format: IDWriteTextFormat,
    calc_arrow_format: IDWriteTextFormat,
    emoji_glyph_format: IDWriteTextFormat,
    emoji_label_format: IDWriteTextFormat,
    detail_body_format: IDWriteTextFormat,
    meta_label_format: IDWriteTextFormat,
    meta_value_format: IDWriteTextFormat,
    ramp_label_format: IDWriteTextFormat,
    action_title_format: IDWriteTextFormat,
    editor_title_format: IDWriteTextFormat,
    round_stroke: Option<ID2D1StrokeStyle>,
    logo_ring: Option<ID2D1PathGeometry>,
    logo_arrow: Option<ID2D1PathGeometry>,
    brushes: Vec<(Color, ID2D1SolidColorBrush)>,
    bitmaps: HashMap<String, ID2D1Bitmap>,
}

impl Renderer {
    /// Binds a renderer to a window at the given monitor DPI.
    pub fn new(hwnd: HWND, dpi: f32) -> Result<Self, String> {
        let factory: ID2D1Factory = unsafe {
            D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)
                .map_err(|error| format!("Direct2D could not start: {error}"))?
        };
        let dwrite: IDWriteFactory = unsafe {
            DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)
                .map_err(|error| format!("DirectWrite could not start: {error}"))?
        };

        let dpi = dpi.max(96.0);
        let properties = D2D1_RENDER_TARGET_PROPERTIES {
            r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_R8G8B8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: dpi,
            dpiY: dpi,
            usage: D2D1_RENDER_TARGET_USAGE_NONE,
            minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
        };
        let hwnd_properties = D2D1_HWND_RENDER_TARGET_PROPERTIES {
            hwnd,
            pixelSize: D2D_SIZE_U {
                width: 1,
                height: 1,
            },
            presentOptions: D2D1_PRESENT_OPTIONS_NONE,
        };
        let target = unsafe {
            factory
                .CreateHwndRenderTarget(&properties, &hwnd_properties)
                .map_err(|error| format!("the palette could not be bound to its window: {error}"))?
        };

        let name_format = dwrite_format(
            &dwrite,
            FONT_FAMILY,
            DWRITE_FONT_WEIGHT_NORMAL,
            13.5,
            DWRITE_TEXT_ALIGNMENT_LEADING,
        )?;
        let detail_format = dwrite_format(
            &dwrite,
            FONT_FAMILY,
            DWRITE_FONT_WEIGHT_NORMAL,
            13.0,
            DWRITE_TEXT_ALIGNMENT_LEADING,
        )?;
        let accessory_format = dwrite_format(
            &dwrite,
            FONT_FAMILY,
            DWRITE_FONT_WEIGHT_NORMAL,
            13.0,
            DWRITE_TEXT_ALIGNMENT_TRAILING,
        )?;
        let section_format = dwrite_format(
            &dwrite,
            FONT_FAMILY,
            DWRITE_FONT_WEIGHT_MEDIUM,
            11.5,
            DWRITE_TEXT_ALIGNMENT_LEADING,
        )?;
        let pill_format = dwrite_format(
            &dwrite,
            FONT_FAMILY,
            DWRITE_FONT_WEIGHT_NORMAL,
            11.0,
            DWRITE_TEXT_ALIGNMENT_CENTER,
        )?;
        let footer_format = dwrite_format(
            &dwrite,
            FONT_FAMILY,
            DWRITE_FONT_WEIGHT_NORMAL,
            12.0,
            DWRITE_TEXT_ALIGNMENT_LEADING,
        )?;
        let footer_primary_format = dwrite_format(
            &dwrite,
            FONT_FAMILY,
            DWRITE_FONT_WEIGHT_MEDIUM,
            12.0,
            DWRITE_TEXT_ALIGNMENT_LEADING,
        )?;
        let keycap_format = dwrite_format(
            &dwrite,
            FONT_FAMILY,
            DWRITE_FONT_WEIGHT_MEDIUM,
            10.0,
            DWRITE_TEXT_ALIGNMENT_CENTER,
        )?;
        let icon_format = dwrite_format(
            &dwrite,
            FONT_FAMILY,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            12.0,
            DWRITE_TEXT_ALIGNMENT_CENTER,
        )?;
        let calc_expr_format = dwrite_format(
            &dwrite,
            FONT_FAMILY,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            24.0,
            DWRITE_TEXT_ALIGNMENT_CENTER,
        )?;
        let calc_expr_med_format = dwrite_format(
            &dwrite,
            FONT_FAMILY,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            19.5,
            DWRITE_TEXT_ALIGNMENT_CENTER,
        )?;
        let calc_expr_sm_format = dwrite_format(
            &dwrite,
            FONT_FAMILY,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            15.5,
            DWRITE_TEXT_ALIGNMENT_CENTER,
        )?;
        let calc_answer_format = dwrite_format(
            &dwrite,
            FONT_FAMILY,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            24.0,
            DWRITE_TEXT_ALIGNMENT_CENTER,
        )?;
        let calc_answer_med_format = dwrite_format(
            &dwrite,
            FONT_FAMILY,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            19.5,
            DWRITE_TEXT_ALIGNMENT_CENTER,
        )?;
        let calc_answer_sm_format = dwrite_format(
            &dwrite,
            FONT_FAMILY,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            16.0,
            DWRITE_TEXT_ALIGNMENT_CENTER,
        )?;
        let calc_arrow_format = dwrite_format(
            &dwrite,
            FONT_FAMILY,
            DWRITE_FONT_WEIGHT_NORMAL,
            16.0,
            DWRITE_TEXT_ALIGNMENT_CENTER,
        )?;
        let emoji_glyph_format = dwrite_format(
            &dwrite,
            EMOJI_FONT_FAMILY,
            DWRITE_FONT_WEIGHT_NORMAL,
            24.0,
            DWRITE_TEXT_ALIGNMENT_CENTER,
        )?;
        let emoji_label_format = dwrite_format(
            &dwrite,
            FONT_FAMILY,
            DWRITE_FONT_WEIGHT_NORMAL,
            10.0,
            DWRITE_TEXT_ALIGNMENT_CENTER,
        )?;
        let detail_body_format = dwrite_format_wrapped(
            &dwrite,
            FONT_FAMILY,
            DWRITE_FONT_WEIGHT_NORMAL,
            13.0,
            DWRITE_TEXT_ALIGNMENT_LEADING,
        )?;
        let meta_label_format = dwrite_format(
            &dwrite,
            FONT_FAMILY,
            DWRITE_FONT_WEIGHT_MEDIUM,
            11.5,
            DWRITE_TEXT_ALIGNMENT_LEADING,
        )?;
        let meta_value_format = dwrite_format(
            &dwrite,
            FONT_FAMILY,
            DWRITE_FONT_WEIGHT_NORMAL,
            11.5,
            DWRITE_TEXT_ALIGNMENT_TRAILING,
        )?;
        let ramp_label_format = dwrite_format(
            &dwrite,
            FONT_FAMILY,
            DWRITE_FONT_WEIGHT_MEDIUM,
            9.5,
            DWRITE_TEXT_ALIGNMENT_CENTER,
        )?;
        let action_title_format = dwrite_format(
            &dwrite,
            FONT_FAMILY,
            DWRITE_FONT_WEIGHT_NORMAL,
            13.0,
            DWRITE_TEXT_ALIGNMENT_LEADING,
        )?;
        let editor_title_format = dwrite_format(
            &dwrite,
            FONT_FAMILY,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            15.5,
            DWRITE_TEXT_ALIGNMENT_LEADING,
        )?;

        let round_stroke = unsafe {
            let props = D2D1_STROKE_STYLE_PROPERTIES {
                startCap: D2D1_CAP_STYLE_ROUND,
                endCap: D2D1_CAP_STYLE_ROUND,
                dashCap: D2D1_CAP_STYLE_ROUND,
                lineJoin: D2D1_LINE_JOIN_ROUND,
                miterLimit: 10.0,
                ..Default::default()
            };
            factory.CreateStrokeStyle(&props, None).ok()
        };
        let (logo_ring, logo_arrow) = build_launch_ring_geometry(&factory);

        let renderer = Self {
            target,
            dwrite,
            name_format,
            detail_format,
            accessory_format,
            section_format,
            pill_format,
            footer_format,
            footer_primary_format,
            keycap_format,
            icon_format,
            calc_expr_format,
            calc_expr_med_format,
            calc_expr_sm_format,
            calc_answer_format,
            calc_answer_med_format,
            calc_answer_sm_format,
            calc_arrow_format,
            emoji_glyph_format,
            emoji_label_format,
            detail_body_format,
            meta_label_format,
            meta_value_format,
            ramp_label_format,
            action_title_format,
            editor_title_format,
            round_stroke,
            logo_ring,
            logo_arrow,
            brushes: Vec::new(),
            bitmaps: HashMap::new(),
        };
        renderer.trim_ellipsis()?;
        Ok(renderer)
    }

    fn trim_ellipsis(&self) -> Result<(), String> {
        let trimming = DWRITE_TRIMMING {
            granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER,
            delimiter: 0,
            delimiterCount: 0,
        };
        for format in [
            &self.name_format,
            &self.detail_format,
            &self.accessory_format,
            &self.section_format,
            &self.footer_format,
            &self.footer_primary_format,
            &self.calc_expr_format,
            &self.calc_expr_med_format,
            &self.calc_expr_sm_format,
            &self.calc_answer_format,
            &self.calc_answer_med_format,
            &self.calc_answer_sm_format,
            &self.emoji_label_format,
            &self.meta_label_format,
            &self.meta_value_format,
            &self.ramp_label_format,
            &self.action_title_format,
            &self.editor_title_format,
        ] {
            unsafe {
                let sign = self
                    .dwrite
                    .CreateEllipsisTrimmingSign(format)
                    .map_err(|error| format!("the ellipsis could not be prepared: {error}"))?;
                format
                    .SetTrimming(&trimming, &sign)
                    .map_err(|error| format!("the text could not be trimmed: {error}"))?;
            }
        }
        Ok(())
    }

    /// Synchronize the render target DPI with the monitor hosting the window.
    pub fn set_dpi(&self, dpi: f32) {
        let dpi = dpi.max(96.0);
        unsafe {
            self.target.SetDpi(dpi, dpi);
        }
    }

    /// The target's size in device-independent pixels.
    pub fn size(&self) -> (f32, f32) {
        let size = unsafe { self.target.GetSize() };
        (size.width, size.height)
    }

    /// Tells the target that the window changed size (in physical pixels).
    pub fn resize(&self, width: u32, height: u32) {
        let size = D2D_SIZE_U {
            width: width.max(1),
            height: height.max(1),
        };
        unsafe {
            let _ = self.target.Resize(&size);
        }
    }

    fn brush(&mut self, color: Color) -> Option<ID2D1SolidColorBrush> {
        if let Some((_, brush)) = self.brushes.iter().find(|(held, _)| *held == color) {
            return Some(brush.clone());
        }
        let brush = unsafe {
            self.target
                .CreateSolidColorBrush(&to_d2d(color), None)
                .ok()?
        };
        if self.brushes.len() > 128 {
            self.brushes.clear();
        }
        self.brushes.push((color, brush.clone()));
        Some(brush)
    }

    fn cached_bitmap(&mut self, key: &str, image: &IconBitmap) -> Option<ID2D1Bitmap> {
        if let Some(existing) = self.bitmaps.get(key) {
            return Some(existing.clone());
        }
        if self.bitmaps.len() >= MAX_CACHED_BITMAPS {
            self.bitmaps.clear();
        }
        let props = D2D1_BITMAP_PROPERTIES {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_R8G8B8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 96.0,
            dpiY: 96.0,
        };
        let size = D2D_SIZE_U {
            width: image.width,
            height: image.height,
        };
        let pitch = image.width.saturating_mul(4);
        let bitmap = unsafe {
            self.target
                .CreateBitmap(
                    size,
                    Some(image.premultiplied_rgba.as_ptr() as *const c_void),
                    pitch,
                    &props,
                )
                .ok()?
        };
        self.bitmaps.insert(key.to_string(), bitmap.clone());
        Some(bitmap)
    }

    /// Draws the complete Raycast/Tinycast-inspired palette in pure 1:1 DIPs.
    pub fn draw(&mut self, model: &Model, theme: &Theme, detail_state: &mut DetailTextState) {
        let metrics = theme.metrics;
        let size = unsafe { self.target.GetSize() };
        let width = size.width;
        let height = size.height;
        let layout = model.layout(metrics);
        let pad = metrics.padding;

        unsafe {
            self.target.BeginDraw();
            self.target.Clear(Some(&to_d2d(theme.background)));
        }

        // Native multi-line text editor surface (e.g. Edit Clipboard Entry).
        if let Some(editor) = model.editor.as_ref() {
            detail_state.reset();
            self.draw_back_button(theme, 12.0, (metrics.field_height - 26.0) / 2.0);
            let title = if editor.title.is_empty() {
                "Edit Text"
            } else {
                editor.title.as_str()
            };
            let title_max_w = (width
                - metrics.field_inset
                - if model.header_badge.is_some() {
                    136.0
                } else {
                    20.0
                })
            .max(40.0);
            if let Some(title_layout) =
                self.layout(title, &self.editor_title_format, title_max_w, 22.0)
            {
                if let Some(brush) = self.brush(theme.text) {
                    unsafe {
                        self.target.DrawTextLayout(
                            Vector2 {
                                X: metrics.field_inset,
                                Y: (metrics.field_height - 22.0) / 2.0,
                            },
                            &title_layout,
                            &brush,
                            D2D1_DRAW_TEXT_OPTIONS_CLIP,
                        );
                    }
                }
            }
            self.draw_header_badge(model, theme, width);

            // Full-width subtle header divider hairline.
            self.fill_rect(
                1.0,
                metrics.field_height,
                (width - 1.0).max(1.0),
                metrics.field_height + 1.0,
                theme.border,
            );

            // Sleek rounded surface card framing the native multi-line `editor_edit` control.
            let card = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: pad + 4.0,
                    top: layout.list_top + 4.0,
                    right: (width - pad - 4.0).max(pad + 12.0),
                    bottom: (layout.list_bottom - 4.0).max(layout.list_top + 12.0),
                },
                radiusX: 8.0,
                radiusY: 8.0,
            };
            if let Some(bg) = self.brush(theme.field) {
                unsafe {
                    self.target.FillRoundedRectangle(&card, &bg);
                }
            }
            if let Some(border) = self.brush(theme.border) {
                unsafe {
                    self.target.DrawRoundedRectangle(&card, &border, 1.0, None);
                }
            }

            // Footer bar.
            let footer_top = height - metrics.footer_height;
            self.fill_rect(0.0, footer_top, width, height, theme.background_secondary);
            self.fill_rect(
                1.0,
                footer_top,
                (width - 1.0).max(1.0),
                footer_top + 1.0,
                theme.border,
            );
            self.draw_footer(model, theme, footer_top, width, metrics.footer_height);

            if let Some(menu) = model.action_menu.as_ref() {
                self.draw_action_menu(menu, theme, width, height);
            }

            unsafe {
                let _ = self.target.EndDraw(None, None);
            }
            return;
        }

        // 1. Header leading control: clickable `←` back button on sub-screens,
        // or magnifying glass icon at root.
        if model.back_button {
            self.draw_back_button(theme, 12.0, (metrics.field_height - 26.0) / 2.0);
        } else {
            self.draw_search_glyph(theme, 16.0, metrics.field_height / 2.0);
        }

        // Optional active inline `Tab` argument pill (e.g. `GitHub Search ▸`) right beside icon.
        if let Some(arg) = model.inline_arg.as_ref() {
            let label = format!("{} ▸", arg.command_name);
            let pill_x = metrics.field_inset;
            let pill_mid_y = metrics.field_height / 2.0;
            self.draw_pill_left(
                &label,
                pill_x,
                pill_mid_y,
                theme.accent.with_alpha(0.18),
                theme.accent.with_alpha(0.42),
                theme.accent,
            );
        }

        // Optional header mode/filter pill on the right (e.g., "Settings", "● Recording…").
        self.draw_header_badge(model, theme, width);

        // Compact single-bar mode: render only the borderless search bar (no divider, list, or footer).
        if model.compact
            && model.action_menu.is_none()
            && model.detail.is_none()
            && model.grid.is_none()
        {
            detail_state.reset();
            unsafe {
                let _ = self.target.EndDraw(None, None);
            }
            return;
        }

        // Full-width subtle header divider hairline (inset 1px from native window border).
        self.fill_rect(
            1.0,
            metrics.field_height,
            (width - 1.0).max(1.0),
            metrics.field_height + 1.0,
            theme.border,
        );

        // Compute left list bounds (full width normally, or 38% master column in split mode).
        let split_x = if model.detail.is_some() && model.grid.is_none() {
            (width * MASTER_SPLIT_RATIO).round()
        } else {
            width
        };

        // 2. Scrollable list viewport (clipped cleanly between list_top and list_bottom).
        let viewport_clip = D2D_RECT_F {
            left: 0.0,
            top: layout.list_top,
            right: split_x,
            bottom: layout.list_bottom,
        };
        unsafe {
            self.target
                .PushAxisAlignedClip(&viewport_clip, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
        }

        if let Some(grid) = model.grid.as_ref() {
            if grid.cells.is_empty() {
                if let Some(text_layout) = self.layout(
                    "No matching emoji or symbols",
                    &self.footer_format,
                    width - pad * 4.0,
                    20.0,
                ) {
                    if let Some(brush) = self.brush(theme.text_dim) {
                        unsafe {
                            self.target.DrawTextLayout(
                                Vector2 {
                                    X: pad + 12.0,
                                    Y: layout.list_top + 18.0,
                                },
                                &text_layout,
                                &brush,
                                D2D1_DRAW_TEXT_OPTIONS_CLIP,
                            );
                        }
                    }
                }
            } else {
                let show_scrollbar = layout.max_scroll() > 1.0;
                let right_margin = if show_scrollbar { pad + 8.0 } else { pad };
                self.draw_emoji_grid(
                    grid,
                    theme,
                    pad,
                    layout.list_top,
                    width - right_margin,
                    layout.list_bottom,
                    model.scroll_offset,
                );
                if show_scrollbar {
                    self.draw_scrollbar(&layout, model.scroll_offset, width, theme);
                }
            }
        } else if model.rows.is_empty() {
            let empty_text = if model.query.trim().is_empty() {
                "Start typing to search apps, commands, or calculate…"
            } else {
                "No matching results"
            };
            if let Some(text_layout) =
                self.layout(empty_text, &self.footer_format, split_x - pad * 3.0, 20.0)
            {
                if let Some(brush) = self.brush(theme.text_dim) {
                    unsafe {
                        self.target.DrawTextLayout(
                            Vector2 {
                                X: pad + 12.0,
                                Y: layout.list_top + 18.0,
                            },
                            &text_layout,
                            &brush,
                            D2D1_DRAW_TEXT_OPTIONS_CLIP,
                        );
                    }
                }
            }
        } else {
            let scroll = model.scroll_offset;

            // Section headers (16px left inset matching list_padding_x + item_padding_x).
            for section in &layout.sections {
                let top = layout.list_top + section.y - scroll;
                if top + section.height < layout.list_top || top > layout.list_bottom {
                    continue;
                }
                if let Some(text_layout) = self.layout(
                    &section.title,
                    &self.section_format,
                    (split_x - pad * 2.0 - 16.0).max(40.0),
                    section.height,
                ) {
                    if let Some(brush) = self.brush(theme.text_dim) {
                        unsafe {
                            self.target.DrawTextLayout(
                                Vector2 {
                                    X: pad + 8.0,
                                    Y: top + 4.0,
                                },
                                &text_layout,
                                &brush,
                                D2D1_DRAW_TEXT_OPTIONS_CLIP,
                            );
                        }
                    }
                }
            }

            // Rows (standard rows and Calculator hero cards).
            let show_scrollbar = layout.max_scroll() > 1.0;
            let right_margin = if show_scrollbar { pad + 8.0 } else { pad };

            for item in &layout.rows {
                let top = layout.list_top + item.y - scroll;
                if top + item.height < layout.list_top || top > layout.list_bottom {
                    continue;
                }
                let Some(row) = model.rows.get(item.index) else {
                    continue;
                };
                let is_selected = item.index == model.selected;
                let is_hovered = model.hovered == Some(item.index) && !is_selected;

                if item.is_calculator {
                    self.draw_calculator_card(
                        row,
                        theme,
                        pad,
                        top,
                        split_x - right_margin,
                        item.height,
                        is_selected,
                    );
                } else {
                    self.draw_standard_row(
                        row,
                        theme,
                        pad,
                        top,
                        split_x - right_margin,
                        item.height,
                        is_selected,
                        is_hovered,
                        model.detail.is_some(),
                    );
                }
            }

            // Slim rounded scrollbar when content overflows.
            if show_scrollbar {
                self.draw_scrollbar(&layout, scroll, split_x, theme);
            }
        }

        unsafe {
            self.target.PopAxisAlignedClip();
        }

        // 2b. Optional Master-Detail Split Preview & Metadata Pane on the right (465 DIPs).
        if let Some(detail) = model.detail.as_ref() {
            if model.grid.is_none() {
                let pane_top = metrics.field_height + 1.0;
                let pane_bottom = height - metrics.footer_height;
                self.fill_rect(split_x, pane_top, split_x + 1.0, pane_bottom, theme.border);
                self.draw_detail_pane(
                    detail,
                    theme,
                    detail_state,
                    split_x + 1.0,
                    pane_top,
                    width,
                    pane_bottom,
                );
            } else {
                detail_state.reset();
            }
        } else {
            detail_state.reset();
        }

        // 3. Seamless Footer Bar with minimal brand mark + Primary Action [↵] | Actions [Ctrl] [K].
        let footer_top = height - metrics.footer_height;
        self.fill_rect(0.0, footer_top, width, height, theme.background_secondary);
        self.fill_rect(
            1.0,
            footer_top,
            (width - 1.0).max(1.0),
            footer_top + 1.0,
            theme.border,
        );
        self.draw_footer(model, theme, footer_top, width, metrics.footer_height);

        // 3b. Floating bottom-right `Ctrl+K` Action Panel Popover overlay.
        if let Some(menu) = model.action_menu.as_ref() {
            self.draw_action_menu(menu, theme, width, height);
        }

        // Note: Windows 11 DWM (`DWMWCP_ROUND` + `DWMWA_BORDER_COLOR`) draws the
        // hardware-antialiased rounded 1px outer frame cleanly; drawing a square
        // Direct2D rectangle here would cause a double-border/corner artifact.
        unsafe {
            let _ = self.target.EndDraw(None, None);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_standard_row(
        &mut self,
        row: &Row,
        theme: &Theme,
        left: f32,
        top: f32,
        right: f32,
        row_height: f32,
        is_selected: bool,
        is_hovered: bool,
        compact_master: bool,
    ) {
        let metrics = theme.metrics;
        let row_rect = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left,
                top: top + 1.0,
                right,
                bottom: top + row_height - 1.0,
            },
            radiusX: metrics.radius,
            radiusY: metrics.radius,
        };

        // Pure borderless neutral fill (Raycast Windows 2.13: no outline, no accent bar).
        if is_selected {
            let fill = if row.recording {
                match theme.appearance {
                    Appearance::Dark => Color::rgba(0xf9, 0x73, 0x16, 0x1e),
                    Appearance::Light => Color::rgba(0xf9, 0x73, 0x16, 0x14),
                }
            } else {
                theme.selection
            };
            if let Some(sel) = self.brush(fill) {
                unsafe {
                    self.target.FillRoundedRectangle(&row_rect, &sel);
                }
            }
            if row.recording {
                let border = match theme.appearance {
                    Appearance::Dark => Color::rgba(0xf9, 0x73, 0x16, 0x99),
                    Appearance::Light => Color::rgba(0xea, 0x58, 0x0c, 0x80),
                };
                if let Some(ring) = self.brush(border) {
                    unsafe {
                        self.target
                            .DrawRoundedRectangle(&row_rect, &ring, 1.2, None);
                    }
                }
            }
        } else if is_hovered {
            if let Some(hover) = self.brush(theme.hover) {
                unsafe {
                    self.target.FillRoundedRectangle(&row_rect, &hover);
                }
            }
        }

        // Row Icon: color swatch (for clipboard colors), real Windows GPU-cached bitmap,
        // or calm rounded-square glyph badge.
        let icon_size = metrics.icon_size;
        let icon_left = left + 8.0;
        let icon_top = top + (row_height - icon_size) / 2.0;
        let mut drew_bitmap = false;

        if let Some([r, g, b, a]) = row.color_swatch {
            let swatch = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: icon_left,
                    top: icon_top,
                    right: icon_left + icon_size,
                    bottom: icon_top + icon_size,
                },
                radiusX: 5.0,
                radiusY: 5.0,
            };
            if let Some(fill) = self.brush(Color::rgba(r, g, b, a)) {
                unsafe {
                    self.target.FillRoundedRectangle(&swatch, &fill);
                }
            }
            if let Some(ring) = self.brush(theme.border) {
                unsafe {
                    self.target.DrawRoundedRectangle(&swatch, &ring, 1.0, None);
                }
            }
            drew_bitmap = true;
        }

        if !drew_bitmap {
            if let (Some(key), Some(bitmap_data)) = (row.icon_key.as_deref(), row.bitmap.as_deref())
            {
                if let Some(gpu_bitmap) = self.cached_bitmap(key, bitmap_data) {
                    let is_thumb = key.starts_with("clip-row:");
                    let dest = D2D_RECT_F {
                        left: icon_left,
                        top: icon_top,
                        right: icon_left + icon_size,
                        bottom: icon_top + icon_size,
                    };
                    unsafe {
                        self.target.DrawBitmap(
                            &gpu_bitmap,
                            Some(&dest),
                            1.0,
                            D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                            None,
                        );
                    }
                    if is_thumb {
                        let frame = D2D1_ROUNDED_RECT {
                            rect: dest,
                            radiusX: 3.0,
                            radiusY: 3.0,
                        };
                        if let Some(ring) = self.brush(theme.border) {
                            unsafe {
                                self.target.DrawRoundedRectangle(&frame, &ring, 1.0, None);
                            }
                        }
                    }
                    drew_bitmap = true;
                }
            }
        }

        if !drew_bitmap {
            let (glyph, tint, vector_kind) = match row.icon.as_ref() {
                Some(icon) => (icon.letter.as_str(), icon.color, icon.kind),
                None => ("·", theme.text_dim, None),
            };
            let tile = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: icon_left,
                    top: icon_top,
                    right: icon_left + icon_size,
                    bottom: icon_top + icon_size,
                },
                radiusX: 5.0,
                radiusY: 5.0,
            };
            let fill_alpha = match theme.appearance {
                Appearance::Dark => 0.16,
                Appearance::Light => 0.12,
            };
            if let Some(bg) = self.brush(tint.with_alpha(fill_alpha)) {
                unsafe {
                    self.target.FillRoundedRectangle(&tile, &bg);
                }
            }
            if let Some(kind) = vector_kind {
                self.draw_vector_icon(kind, icon_left, icon_top, icon_size, tint);
            } else if let Some(layout) = self.layout(glyph, &self.icon_format, icon_size, icon_size)
            {
                if let Some(fg) = self.brush(tint) {
                    unsafe {
                        self.target.DrawTextLayout(
                            Vector2 {
                                X: icon_left,
                                Y: icon_top + (icon_size - 16.0) / 2.0,
                            },
                            &layout,
                            &fg,
                            D2D1_DRAW_TEXT_OPTIONS_CLIP,
                        );
                    }
                }
            }
        }

        // Right-aligned shortcut / setting value badge and/or muted type accessory.
        let mut cursor_right = right - 10.0;
        if !row.shortcut.is_empty() {
            if row.recording {
                let (pill_bg, pill_border, pill_fg) = match theme.appearance {
                    Appearance::Dark => (
                        Color::rgba(0xf9, 0x73, 0x16, 0x30),
                        Color::rgba(0xf9, 0x73, 0x16, 0xb0),
                        Color::rgb(0xfb, 0x92, 0x3c),
                    ),
                    Appearance::Light => (
                        Color::rgba(0xf9, 0x73, 0x16, 0x20),
                        Color::rgba(0xea, 0x58, 0x0c, 0x90),
                        Color::rgb(0xc2, 0x41, 0x0c),
                    ),
                };
                let pill_w = self.draw_pill_right(
                    &row.shortcut,
                    cursor_right,
                    top + row_height / 2.0,
                    pill_bg,
                    pill_border,
                    pill_fg,
                );
                cursor_right -= pill_w + 10.0;
            } else if row.shortcut.contains('+') {
                let kc_w = self.draw_shortcut_keycaps(
                    &row.shortcut,
                    cursor_right,
                    top + row_height / 2.0,
                    theme,
                );
                cursor_right -= kc_w + 10.0;
            } else {
                let pill_bg = match theme.appearance {
                    Appearance::Dark => Color::rgba(0xff, 0xff, 0xff, 0x12),
                    Appearance::Light => Color::rgba(0x00, 0x00, 0x00, 0x0d),
                };
                let pill_fg = if is_selected {
                    theme.text
                } else {
                    theme.text_dim
                };
                let pill_w = self.draw_pill_right(
                    &row.shortcut,
                    cursor_right,
                    top + row_height / 2.0,
                    pill_bg,
                    Color::rgba(0, 0, 0, 0),
                    pill_fg,
                );
                cursor_right -= pill_w + 10.0;
            }
        }

        if !compact_master && !row.accessory.is_empty() {
            let max_acc = 110.0_f32;
            if let Some(acc_layout) =
                self.layout(&row.accessory, &self.accessory_format, max_acc, 20.0)
            {
                let acc_w = measure_width(&acc_layout).min(max_acc);
                if let Some(brush) = self.brush(theme.text_dim) {
                    unsafe {
                        self.target.DrawTextLayout(
                            Vector2 {
                                X: cursor_right - max_acc,
                                Y: top + (row_height - 18.0) / 2.0,
                            },
                            &acc_layout,
                            &brush,
                            D2D1_DRAW_TEXT_OPTIONS_CLIP,
                        );
                    }
                }
                cursor_right -= acc_w + 16.0;
            }
        }

        // Raycast inline row layout: Title followed immediately (+10px) by muted Subtitle.
        let text_left = icon_left + icon_size + 12.0;
        let available = (cursor_right - text_left).max(0.0);
        let has_inline_detail = !compact_master && !row.detail.is_empty() && available > 110.0;

        let max_title_w = if has_inline_detail {
            // Leave room for inline subtitle if the title is very long, while letting
            // short/medium titles measure at their exact natural width.
            available.min((available * 0.68).max(160.0))
        } else {
            available
        };

        let mut title_used = 0.0_f32;
        if let Some(layout) = self.layout(&row.name, &self.name_format, max_title_w, 22.0) {
            title_used = measure_width(&layout).min(max_title_w);
            if let Some(brush) = self.brush(theme.text) {
                unsafe {
                    self.target.DrawTextLayout(
                        Vector2 {
                            X: text_left,
                            Y: top + (row_height - 18.5) / 2.0,
                        },
                        &layout,
                        &brush,
                        D2D1_DRAW_TEXT_OPTIONS_CLIP,
                    );
                }
            }
        }

        if has_inline_detail {
            let detail_left = text_left + title_used + 10.0;
            let detail_w = (cursor_right - detail_left).max(0.0);
            if detail_w >= 28.0 {
                if let Some(layout) = self.layout(&row.detail, &self.detail_format, detail_w, 20.0)
                {
                    let detail_col = if row.recording {
                        match theme.appearance {
                            Appearance::Dark => Color::rgb(0xfd, 0xba, 0x74),
                            Appearance::Light => Color::rgb(0x9a, 0x34, 0x12),
                        }
                    } else {
                        theme.text_dim
                    };
                    if let Some(brush) = self.brush(detail_col) {
                        unsafe {
                            self.target.DrawTextLayout(
                                Vector2 {
                                    X: detail_left,
                                    Y: top + (row_height - 18.0) / 2.0,
                                },
                                &layout,
                                &brush,
                                D2D1_DRAW_TEXT_OPTIONS_CLIP,
                            );
                        }
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_calculator_card(
        &mut self,
        row: &Row,
        theme: &Theme,
        left: f32,
        top: f32,
        right: f32,
        height: f32,
        is_selected: bool,
    ) {
        let card = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left,
                top: top + 1.0,
                right,
                bottom: top + height - 1.0,
            },
            radiusX: theme.metrics.radius,
            radiusY: theme.metrics.radius,
        };
        let fill = if is_selected {
            theme.selection
        } else {
            match theme.appearance {
                Appearance::Dark => Color::rgba(0xff, 0xff, 0xff, 0x08),
                Appearance::Light => Color::rgba(0x00, 0x00, 0x00, 0x08),
            }
        };
        if let Some(bg) = self.brush(fill) {
            unsafe {
                self.target.FillRoundedRectangle(&card, &bg);
            }
        }

        // Full-height subtle 1px vertical divider interrupted cleanly by a centered `→` arrow.
        let mid_x = (left + (right - left) * 0.50).round();
        let mid_y = top + height * 0.50;
        let div_col = if is_selected {
            theme.border
        } else {
            theme.border.with_alpha(0.5)
        };
        self.fill_rect(mid_x, top + 1.0, mid_x + 1.0, mid_y - 14.0, div_col);
        self.fill_rect(
            mid_x,
            mid_y + 14.0,
            mid_x + 1.0,
            top + height - 1.0,
            div_col,
        );
        if let Some(arrow) = self.layout("→", &self.calc_arrow_format, 24.0, 22.0) {
            let arrow_col = if is_selected {
                theme.text
            } else {
                theme.text_dim
            };
            if let Some(brush) = self.brush(arrow_col) {
                unsafe {
                    self.target.DrawTextLayout(
                        Vector2 {
                            X: mid_x - 11.5,
                            Y: mid_y - 11.0,
                        },
                        &arrow,
                        &brush,
                        D2D1_DRAW_TEXT_OPTIONS_CLIP,
                    );
                }
            }
        }

        // Neutral borderless Raycast token pills on BOTH left and right halves.
        let pill_bg = match theme.appearance {
            Appearance::Dark => Color::rgba(0xff, 0xff, 0xff, 0x1a),
            Appearance::Light => Color::rgba(0x00, 0x00, 0x00, 0x14),
        };
        let pill_border = Color::rgba(0, 0, 0, 0);
        let pill_fg = match theme.appearance {
            Appearance::Dark => Color::rgba(0xff, 0xff, 0xff, 0xd9),
            Appearance::Light => Color::rgba(0x00, 0x00, 0x00, 0xd9),
        };
        let dim_token_col = match theme.appearance {
            Appearance::Dark => Color::rgba(0xff, 0xff, 0xff, 0x66),
            Appearance::Light => Color::rgba(0x00, 0x00, 0x00, 0x66),
        };

        // Left half: optional Color Swatch + syntax-dimmed centered expression + bottom pill.
        let mut lhs_left = left + 16.0;
        let lhs_right = mid_x - 16.0;
        if let Some([r, g, b, a]) = row.color_swatch {
            let sw = 44.0_f32;
            let sw_top = top + (height - sw) / 2.0;
            let swatch_rect = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: lhs_left,
                    top: sw_top,
                    right: lhs_left + sw,
                    bottom: sw_top + sw,
                },
                radiusX: 10.0,
                radiusY: 10.0,
            };
            if a < 255 {
                if let Some(base) = self.brush(Color::rgb(0x40, 0x40, 0x40)) {
                    unsafe {
                        self.target.FillRoundedRectangle(&swatch_rect, &base);
                    }
                }
                let cell = sw / 4.0;
                if let Some(light) = self.brush(Color::rgb(0x76, 0x76, 0x76)) {
                    for gy in 0..4 {
                        for gx in 0..4 {
                            if (gx + gy) % 2 == 0 {
                                let tile = D2D_RECT_F {
                                    left: lhs_left + gx as f32 * cell + 2.0,
                                    top: sw_top + gy as f32 * cell + 2.0,
                                    right: (lhs_left + (gx + 1) as f32 * cell - 1.0)
                                        .min(lhs_left + sw - 2.0),
                                    bottom: (sw_top + (gy + 1) as f32 * cell - 1.0)
                                        .min(sw_top + sw - 2.0),
                                };
                                unsafe {
                                    self.target.FillRectangle(&tile, &light);
                                }
                            }
                        }
                    }
                }
            }
            if let Some(fill) = self.brush(Color::rgba(r, g, b, a)) {
                unsafe {
                    self.target.FillRoundedRectangle(&swatch_rect, &fill);
                }
            }
            if let Some(ring) = self.brush(Color::rgba(0xff, 0xff, 0xff, 0x30)) {
                unsafe {
                    self.target
                        .DrawRoundedRectangle(&swatch_rect, &ring, 1.0, None);
                }
            }
            lhs_left += sw + 12.0;
        }

        let expr_w = (lhs_right - lhs_left).max(0.0);
        let lhs_center = lhs_left + expr_w / 2.0;
        let (formatted_expr, dim_ranges) = format_calc_expression_spans(&row.detail);
        let (expr_fmt, expr_y_off) = {
            let probe = self.layout(&formatted_expr, &self.calc_expr_format, 2000.0, 38.0);
            if probe.as_ref().is_some_and(|l| measure_width(l) <= expr_w) {
                (&self.calc_expr_format, 32.0_f32)
            } else {
                let probe_med =
                    self.layout(&formatted_expr, &self.calc_expr_med_format, 2000.0, 34.0);
                if probe_med
                    .as_ref()
                    .is_some_and(|l| measure_width(l) <= expr_w)
                {
                    (&self.calc_expr_med_format, 35.0_f32)
                } else {
                    (&self.calc_expr_sm_format, 38.0_f32)
                }
            }
        };
        if let Some(layout) = self.layout(&formatted_expr, expr_fmt, expr_w, 38.0) {
            if !dim_ranges.is_empty() {
                if let Some(dim_brush) = self.brush(dim_token_col) {
                    if let Ok(effect) = dim_brush.cast::<IUnknown>() {
                        for (start, len) in dim_ranges {
                            let range = DWRITE_TEXT_RANGE {
                                startPosition: start,
                                length: len,
                            };
                            unsafe {
                                let _ = layout.SetDrawingEffect(&effect, range);
                            }
                        }
                    }
                }
            }
            if let Some(brush) = self.brush(theme.text) {
                unsafe {
                    self.target.DrawTextLayout(
                        Vector2 {
                            X: lhs_left,
                            Y: top + expr_y_off,
                        },
                        &layout,
                        &brush,
                        D2D1_DRAW_TEXT_OPTIONS_CLIP,
                    );
                }
            }
        }
        let src_badge = if row.source_badge.is_empty() {
            "Expression"
        } else {
            row.source_badge.as_str()
        };
        self.draw_pill_centered(
            src_badge,
            lhs_center,
            top + height - 28.0,
            pill_bg,
            pill_border,
            pill_fg,
        );

        // Right half: symmetric 24px SemiBold answer + neutral bottom pill.
        let ans_left = mid_x + 16.0;
        let ans_right = right - 16.0;
        let ans_w = (ans_right - ans_left).max(0.0);
        let rhs_center = ans_left + ans_w / 2.0;
        let (ans_format, ans_y_offset) = {
            let probe = self.layout(&row.name, &self.calc_answer_format, 2000.0, 38.0);
            if probe.as_ref().is_some_and(|l| measure_width(l) <= ans_w) {
                (&self.calc_answer_format, 32.0_f32)
            } else {
                let probe_med = self.layout(&row.name, &self.calc_answer_med_format, 2000.0, 34.0);
                if probe_med
                    .as_ref()
                    .is_some_and(|l| measure_width(l) <= ans_w)
                {
                    (&self.calc_answer_med_format, 35.0_f32)
                } else {
                    (&self.calc_answer_sm_format, 38.0_f32)
                }
            }
        };
        if let Some(layout) = self.layout(&row.name, ans_format, ans_w, 38.0) {
            if let Some(brush) = self.brush(theme.text) {
                unsafe {
                    self.target.DrawTextLayout(
                        Vector2 {
                            X: ans_left,
                            Y: top + ans_y_offset,
                        },
                        &layout,
                        &brush,
                        D2D1_DRAW_TEXT_OPTIONS_CLIP,
                    );
                }
            }
        }
        let dst_badge = if row.badge.is_empty() {
            "Result"
        } else {
            row.badge.as_str()
        };
        self.draw_pill_centered(
            dst_badge,
            rhs_center,
            top + height - 28.0,
            pill_bg,
            pill_border,
            pill_fg,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_emoji_grid(
        &mut self,
        grid: &GridModel,
        theme: &Theme,
        left: f32,
        list_top: f32,
        right: f32,
        list_bottom: f32,
        scroll: f32,
    ) {
        let cols = grid.cols();
        let cell_w = ((right - left) / cols as f32).max(28.0);
        let first_row = (scroll / GRID_CELL_HEIGHT).floor().max(0.0) as usize;
        let visible_rows = (((list_bottom - list_top) / GRID_CELL_HEIGHT).ceil() as usize) + 2;
        let total_rows = grid.row_count();
        let end_row = (first_row + visible_rows).min(total_rows);

        for r in first_row..end_row {
            let cell_top = list_top + r as f32 * GRID_CELL_HEIGHT - scroll;
            if cell_top + GRID_CELL_HEIGHT < list_top || cell_top > list_bottom {
                continue;
            }
            for c in 0..cols {
                let idx = r * cols + c;
                let Some(cell) = grid.cells.get(idx) else {
                    break;
                };
                let cell_left = left + c as f32 * cell_w;
                let tile = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: cell_left + 3.0,
                        top: cell_top + 3.0,
                        right: cell_left + cell_w - 3.0,
                        bottom: cell_top + GRID_CELL_HEIGHT - 3.0,
                    },
                    radiusX: 9.0,
                    radiusY: 9.0,
                };
                let is_selected = idx == grid.selected;
                let is_hovered = grid.hovered == Some(idx) && !is_selected;

                if is_selected {
                    if let Some(bg) = self.brush(theme.selection) {
                        unsafe {
                            self.target.FillRoundedRectangle(&tile, &bg);
                        }
                    }
                    if let Some(ring) = self.brush(theme.accent.with_alpha(0.70)) {
                        unsafe {
                            self.target.DrawRoundedRectangle(&tile, &ring, 1.4, None);
                        }
                    }
                } else if is_hovered {
                    if let Some(bg) = self.brush(theme.hover) {
                        unsafe {
                            self.target.FillRoundedRectangle(&tile, &bg);
                        }
                    }
                }

                // True GPU Color Emoji rendering via Direct2D COLOR_FONT flag.
                if let Some(glyph_layout) =
                    self.layout(&cell.glyph, &self.emoji_glyph_format, cell_w - 6.0, 34.0)
                {
                    if let Some(fg) = self.brush(theme.text) {
                        unsafe {
                            self.target.DrawTextLayout(
                                Vector2 {
                                    X: cell_left + 3.0,
                                    Y: cell_top + 6.0,
                                },
                                &glyph_layout,
                                &fg,
                                D2D1_DRAW_TEXT_OPTIONS_CLIP
                                    | D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT,
                            );
                        }
                    }
                }

                // Crisp micro-label under the emoji.
                if let Some(label_layout) =
                    self.layout(&cell.name, &self.emoji_label_format, cell_w - 10.0, 14.0)
                {
                    let fg_col = if is_selected {
                        theme.text
                    } else {
                        theme.text_dim
                    };
                    if let Some(fg) = self.brush(fg_col) {
                        unsafe {
                            self.target.DrawTextLayout(
                                Vector2 {
                                    X: cell_left + 5.0,
                                    Y: cell_top + GRID_CELL_HEIGHT - 19.0,
                                },
                                &label_layout,
                                &fg,
                                D2D1_DRAW_TEXT_OPTIONS_CLIP,
                            );
                        }
                    }
                }

                // Subtle pinned indicator dot in top-right corner.
                if cell.pinned {
                    let dot = D2D1_ELLIPSE {
                        point: Vector2 {
                            X: cell_left + cell_w - 10.0,
                            Y: cell_top + 10.0,
                        },
                        radiusX: 2.8,
                        radiusY: 2.8,
                    };
                    if let Some(accent) = self.brush(theme.accent) {
                        unsafe {
                            self.target.FillEllipse(&dot, &accent);
                        }
                    }
                }
            }
        }
    }

    fn draw_scrollbar(
        &mut self,
        layout: &crate::model::PaletteLayout,
        scroll: f32,
        width: f32,
        theme: &Theme,
    ) {
        let track_h = layout.viewport_height() - 8.0;
        if track_h <= 20.0 {
            return;
        }
        let ratio = (layout.viewport_height() / layout.content_height).clamp(0.12, 1.0);
        let thumb_h = (track_h * ratio).max(24.0);
        let progress = (scroll / layout.max_scroll()).clamp(0.0, 1.0);
        let thumb_top = layout.list_top + 4.0 + progress * (track_h - thumb_h);
        let bar = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: width - 7.0,
                top: thumb_top,
                right: width - 3.0,
                bottom: thumb_top + thumb_h,
            },
            radiusX: 2.0,
            radiusY: 2.0,
        };
        if let Some(brush) = self.brush(theme.text_dim.with_alpha(0.32)) {
            unsafe {
                self.target.FillRoundedRectangle(&bar, &brush);
            }
        }
    }

    fn draw_footer(&mut self, model: &Model, theme: &Theme, top: f32, width: f32, height: f32) {
        let mid_y = top + height / 2.0;
        let left_x = 16.0;

        if let Some(toast) = model.toast.as_ref() {
            let dot_col = match toast.kind {
                ToastKind::Success => Color::rgb(0x59, 0xd4, 0x99),
                ToastKind::Info => theme.accent,
                ToastKind::Error => Color::rgb(0xff, 0x63, 0x63),
            };
            let halo = D2D1_ELLIPSE {
                point: Vector2 {
                    X: left_x + 5.0,
                    Y: mid_y,
                },
                radiusX: 6.0,
                radiusY: 6.0,
            };
            let dot = D2D1_ELLIPSE {
                point: Vector2 {
                    X: left_x + 5.0,
                    Y: mid_y,
                },
                radiusX: 3.2,
                radiusY: 3.2,
            };
            if let Some(b) = self.brush(dot_col.with_alpha(0.24)) {
                unsafe {
                    self.target.FillEllipse(&halo, &b);
                }
            }
            if let Some(b) = self.brush(dot_col) {
                unsafe {
                    self.target.FillEllipse(&dot, &b);
                }
            }
            if let Some(layout) = self.layout(&toast.message, &self.footer_format, 320.0, 18.0) {
                if let Some(brush) = self.brush(theme.text) {
                    unsafe {
                        self.target.DrawTextLayout(
                            Vector2 {
                                X: left_x + 18.0,
                                Y: mid_y - 8.5,
                            },
                            &layout,
                            &brush,
                            D2D1_DRAW_TEXT_OPTIONS_CLIP,
                        );
                    }
                }
            }
        } else if let Some(active_emoji) = model.grid.as_ref().and_then(|g| g.active_cell()) {
            let status = if active_emoji.pinned {
                format!("{}  {}  ·  Pinned", active_emoji.glyph, active_emoji.name)
            } else {
                format!("{}  {}", active_emoji.glyph, active_emoji.name)
            };
            if let Some(layout) = self.layout(&status, &self.footer_format, 300.0, 18.0) {
                if let Some(brush) = self.brush(theme.text_dim) {
                    unsafe {
                        self.target.DrawTextLayout(
                            Vector2 {
                                X: left_x,
                                Y: mid_y - 8.5,
                            },
                            &layout,
                            &brush,
                            D2D1_DRAW_TEXT_OPTIONS_CLIP | D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT,
                        );
                    }
                }
            }
        } else {
            // Subtle minimal dual-bar footer mark matching Raycast's left-side menu icon.
            let bar_col = theme.text_dim.with_alpha(0.65);
            let top_bar = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: left_x,
                    top: mid_y - 3.5,
                    right: left_x + 13.0,
                    bottom: mid_y - 2.0,
                },
                radiusX: 0.75,
                radiusY: 0.75,
            };
            let bot_bar = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: left_x,
                    top: mid_y + 1.5,
                    right: left_x + 9.0,
                    bottom: mid_y + 3.0,
                },
                radiusX: 0.75,
                radiusY: 0.75,
            };
            if let Some(b) = self.brush(bar_col) {
                unsafe {
                    self.target.FillRoundedRectangle(&top_bar, &b);
                    self.target.FillRoundedRectangle(&bot_bar, &b);
                }
            }
        }

        // Right: exact Raycast action bar — `Copy Answer [↵]  |  Actions [Ctrl] [K]`.
        let mut cursor_right = width - 12.0;
        let sep_color = match theme.appearance {
            Appearance::Dark => Color::rgba(0xff, 0xff, 0xff, 0x33),
            Appearance::Light => Color::rgba(0x00, 0x00, 0x00, 0x26),
        };
        let total = model.footer_hints.len();
        for (rev_idx, (idx, (keycap, label))) in
            model.footer_hints.iter().enumerate().rev().enumerate()
        {
            if rev_idx > 0 {
                // Vertical 2×12px separator between action items.
                cursor_right -= 8.0;
                let sep = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: cursor_right - 1.5,
                        top: mid_y - 6.0,
                        right: cursor_right,
                        bottom: mid_y + 6.0,
                    },
                    radiusX: 0.75,
                    radiusY: 0.75,
                };
                if let Some(b) = self.brush(sep_color) {
                    unsafe {
                        self.target.FillRoundedRectangle(&sep, &b);
                    }
                }
                cursor_right -= 11.5;
            }

            // 1. Separated individual framed keycaps (`Ctrl` + `K` or `↵`) on the right.
            let cap_total_w = self.draw_shortcut_keycaps(keycap, cursor_right, mid_y, theme);
            cursor_right -= cap_total_w + 8.0;

            // 2. Action label immediately to the left of its keycaps (Primary in bright white).
            let is_primary = idx == 0 && total >= 1;
            let fmt = if is_primary {
                &self.footer_primary_format
            } else {
                &self.footer_format
            };
            let label_col = if is_primary {
                theme.text
            } else {
                theme.text_dim
            };
            if let Some(label_layout) = self.layout(label, fmt, 150.0, 18.0) {
                let label_w = measure_width(&label_layout);
                let label_x = cursor_right - label_w;
                if let Some(brush) = self.brush(label_col) {
                    unsafe {
                        self.target.DrawTextLayout(
                            Vector2 {
                                X: label_x,
                                Y: mid_y - 8.5,
                            },
                            &label_layout,
                            &brush,
                            D2D1_DRAW_TEXT_OPTIONS_CLIP,
                        );
                    }
                }
                cursor_right = label_x - 2.0;
            }
        }
    }

    /// Draw Raycast-style separated framed keycaps (`"Ctrl+K"` -> `[Ctrl]` `2px` `[K]`),
    /// right-aligned at `right`, returning total width consumed.
    fn draw_shortcut_keycaps(
        &mut self,
        shortcut: &str,
        right: f32,
        center_y: f32,
        theme: &Theme,
    ) -> f32 {
        let keys: Vec<&str> = if shortcut.contains('+') {
            shortcut
                .split('+')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect()
        } else {
            vec![shortcut]
        };
        if keys.is_empty() {
            return 0.0;
        }
        let border_col = match theme.appearance {
            Appearance::Dark => Color::rgba(0xff, 0xff, 0xff, 0x33),
            Appearance::Light => Color::rgba(0x00, 0x00, 0x00, 0x2b),
        };
        let fg_col = theme.text_dim;
        let mut cursor = right;
        let start_right = right;
        for (i, key) in keys.iter().rev().enumerate() {
            if i > 0 {
                cursor -= 3.0;
            }
            let cap_w = if let Some(probe) = self.layout(key, &self.keycap_format, 80.0, 16.0) {
                (measure_width(&probe) + 8.0).max(18.0)
            } else {
                18.0
            };
            let cap_h = 18.0_f32;
            let cap_left = cursor - cap_w;
            let cap_top = center_y - cap_h / 2.0;
            let cap_rect = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: cap_left,
                    top: cap_top,
                    right: cursor,
                    bottom: cap_top + cap_h,
                },
                radiusX: 4.0,
                radiusY: 4.0,
            };
            if let Some(border) = self.brush(border_col) {
                unsafe {
                    self.target
                        .DrawRoundedRectangle(&cap_rect, &border, 1.0, None);
                }
            }
            if let Some(centered) = self.layout(key, &self.keycap_format, cap_w, 16.0) {
                let y_nudge = if *key == "↵" { 1.0 } else { 2.0 };
                if let Some(fg) = self.brush(fg_col) {
                    unsafe {
                        self.target.DrawTextLayout(
                            Vector2 {
                                X: cap_left,
                                Y: cap_top + y_nudge,
                            },
                            &centered,
                            &fg,
                            D2D1_DRAW_TEXT_OPTIONS_CLIP,
                        );
                    }
                }
            }
            cursor = cap_left;
        }
        (start_right - cursor).max(0.0)
    }

    fn draw_header_badge(&mut self, model: &Model, theme: &Theme, width: f32) {
        if let Some(badge) = model.header_badge.as_deref() {
            if !badge.is_empty() {
                let metrics = theme.metrics;
                let is_recording = badge.contains("Recording");
                let (badge_bg, badge_border, badge_fg) = if is_recording {
                    match theme.appearance {
                        Appearance::Dark => (
                            Color::rgba(0xf9, 0x73, 0x16, 0x2e),
                            Color::rgba(0xf9, 0x73, 0x16, 0xa8),
                            Color::rgb(0xfb, 0x92, 0x3c),
                        ),
                        Appearance::Light => (
                            Color::rgba(0xf9, 0x73, 0x16, 0x1e),
                            Color::rgba(0xea, 0x58, 0x0c, 0x88),
                            Color::rgb(0xc2, 0x41, 0x0c),
                        ),
                    }
                } else {
                    let bg = match theme.appearance {
                        Appearance::Dark => Color::rgba(0xff, 0xff, 0xff, 0x16),
                        Appearance::Light => Color::rgba(0x00, 0x00, 0x00, 0x10),
                    };
                    (bg, Color::rgba(0, 0, 0, 0), theme.text)
                };
                self.draw_pill_right(
                    badge,
                    width - 16.0,
                    metrics.field_height / 2.0,
                    badge_bg,
                    badge_border,
                    badge_fg,
                );
            }
        }
    }

    fn draw_back_button(&mut self, theme: &Theme, left: f32, top: f32) {
        let size = 26.0_f32;
        let rect = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left,
                top,
                right: left + size,
                bottom: top + size,
            },
            radiusX: 6.0,
            radiusY: 6.0,
        };
        let fill = match theme.appearance {
            Appearance::Dark => Color::rgba(0xff, 0xff, 0xff, 0x0d),
            Appearance::Light => Color::rgba(0x0f, 0x17, 0x2a, 0x0a),
        };
        if let Some(bg) = self.brush(fill) {
            unsafe {
                self.target.FillRoundedRectangle(&rect, &bg);
            }
        }
        if let Some(border) = self.brush(theme.border) {
            unsafe {
                self.target.DrawRoundedRectangle(&rect, &border, 1.0, None);
            }
        }
        if let Some(layout) = self.layout("←", &self.icon_format, size, size) {
            if let Some(fg) = self.brush(theme.text) {
                unsafe {
                    self.target.DrawTextLayout(
                        Vector2 {
                            X: left,
                            Y: top + 4.5,
                        },
                        &layout,
                        &fg,
                        D2D1_DRAW_TEXT_OPTIONS_CLIP,
                    );
                }
            }
        }
    }

    fn draw_pill_left(
        &mut self,
        text: &str,
        left: f32,
        center_y: f32,
        bg: Color,
        border: Color,
        fg: Color,
    ) -> f32 {
        let Some(probe) = self.layout(text, &self.pill_format, 220.0, 18.0) else {
            return 0.0;
        };
        let text_w = measure_width(&probe);
        let pill_w = (text_w + 18.0).clamp(44.0, 210.0);
        let pill_h = 24.0;
        let top = center_y - pill_h / 2.0;
        self.paint_pill(text, left, top, pill_w, pill_h, bg, border, fg);
        pill_w
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_detail_pane(
        &mut self,
        detail: &DetailPane,
        theme: &Theme,
        detail_state: &mut DetailTextState,
        left: f32,
        top: f32,
        right: f32,
        bottom: f32,
    ) {
        let pad = 16.0_f32;
        let pane_w = (right - left).max(40.0);
        let content_w = (pane_w - pad * 2.0).max(20.0);
        let clip = D2D_RECT_F {
            left,
            top,
            right,
            bottom,
        };
        unsafe {
            self.target
                .PushAxisAlignedClip(&clip, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
        }

        // Reserve bottom space for Raycast-style key-value metadata table.
        let meta_row_h = 26.0_f32;
        let meta_count = detail.metadata.len().min(6);
        let meta_total_h = if meta_count > 0 {
            meta_count as f32 * meta_row_h + 22.0
        } else {
            0.0
        };
        let preview_top = top + 14.0;
        let preview_bottom = (bottom - meta_total_h - 10.0).max(preview_top + 48.0);
        let preview_h = (preview_bottom - preview_top).max(40.0);

        // Top preview surface: Color banner > Preview bitmap > Multi-line wrapped text.
        if let Some([r, g, b, a]) = detail.color_swatch {
            let sw_h = if detail.color_ramp.is_empty() {
                (preview_h * 0.58).clamp(72.0, 140.0)
            } else {
                let ramp_section_h = 10.0 + COLOR_RAMP_TOTAL_HEIGHT;
                let body_reserve = if detail.body.is_empty() { 4.0 } else { 28.0 };
                let max_sw = (preview_h - ramp_section_h - body_reserve).max(48.0);
                (preview_h * 0.44).clamp(48.0, 108.0).min(max_sw)
            };
            let card = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: left + pad,
                    top: preview_top,
                    right: right - pad,
                    bottom: preview_top + sw_h,
                },
                radiusX: 10.0,
                radiusY: 10.0,
            };
            if let Some(fill) = self.brush(Color::rgba(r, g, b, a)) {
                unsafe {
                    self.target.FillRoundedRectangle(&card, &fill);
                }
            }
            if let Some(ring) = self.brush(theme.border) {
                unsafe {
                    self.target.DrawRoundedRectangle(&card, &ring, 1.0, None);
                }
            }
            let body_top = if detail.color_ramp.is_empty() {
                preview_top + sw_h + 12.0
            } else {
                let mut cursor_y = preview_top + sw_h + 10.0;
                let ramp_h = self.draw_color_ramp(
                    &detail.color_ramp,
                    theme,
                    left + pad,
                    cursor_y,
                    content_w,
                );
                cursor_y += ramp_h + 10.0;
                cursor_y
            };
            if !detail.body.is_empty() && preview_bottom > body_top + 8.0 {
                self.draw_scrollable_detail_text(
                    &detail.body,
                    theme,
                    detail_state,
                    (left + pad, body_top, right - pad, preview_bottom),
                    0.0,
                    0.0,
                );
            } else {
                detail_state.reset();
            }
        } else if let (Some(key), Some(img)) =
            (detail.image_key.as_deref(), detail.image.as_deref())
        {
            detail_state.reset();
            if let Some(bmp) = self.cached_bitmap(key, img) {
                let aspect = (img.width.max(1) as f32) / (img.height.max(1) as f32);
                let max_w = content_w;
                let max_h = (preview_h - 12.0).max(40.0);
                let (draw_w, draw_h) = if max_w / max_h > aspect {
                    let h = max_h.min((img.height as f32).max(96.0));
                    (h * aspect, h)
                } else {
                    let w = max_w.min((img.width as f32).max(96.0));
                    (w, w / aspect)
                };
                let dx = left + pad + (content_w - draw_w) / 2.0;
                let dy = preview_top + (preview_h - draw_h) / 2.0;
                let dest = D2D_RECT_F {
                    left: dx,
                    top: dy,
                    right: dx + draw_w,
                    bottom: dy + draw_h,
                };
                unsafe {
                    self.target.DrawBitmap(
                        &bmp,
                        Some(&dest),
                        1.0,
                        D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                        None,
                    );
                }
            }
        } else {
            let mut cursor_y = preview_top;
            if !detail.color_ramp.is_empty() {
                let ramp_h = self.draw_color_ramp(
                    &detail.color_ramp,
                    theme,
                    left + pad,
                    cursor_y,
                    content_w,
                );
                cursor_y += ramp_h + 10.0;
            }
            if preview_bottom > cursor_y + 20.0 {
                // Subtle inner preview card for text/code/notes/clipboard body.
                let card_rect = D2D_RECT_F {
                    left: left + pad,
                    top: cursor_y,
                    right: right - pad,
                    bottom: preview_bottom,
                };
                let card = D2D1_ROUNDED_RECT {
                    rect: card_rect,
                    radiusX: 8.0,
                    radiusY: 8.0,
                };
                if let Some(bg) = self.brush(theme.background_secondary) {
                    unsafe {
                        self.target.FillRoundedRectangle(&card, &bg);
                    }
                }
                if let Some(border) = self.brush(theme.border) {
                    unsafe {
                        self.target.DrawRoundedRectangle(&card, &border, 1.0, None);
                    }
                }
                let text_content = detail_preview_text(detail);
                self.draw_scrollable_detail_text(
                    text_content,
                    theme,
                    detail_state,
                    (
                        card_rect.left + 1.0,
                        card_rect.top + 1.0,
                        card_rect.right - 1.0,
                        card_rect.bottom - 1.0,
                    ),
                    11.0,
                    9.0,
                );
            } else {
                detail_state.reset();
            }
        }

        // Bottom metadata table (`Information` header + crisp hairline rows).
        if meta_count > 0 {
            let mut y = bottom - meta_total_h;
            self.fill_rect(left + pad, y, right - pad, y + 1.0, theme.border);
            y += 6.0;
            if let Some(hdr) = self.layout("Information", &self.section_format, content_w, 16.0) {
                if let Some(dim) = self.brush(theme.text_dim.with_alpha(0.85)) {
                    unsafe {
                        self.target.DrawTextLayout(
                            Vector2 {
                                X: left + pad,
                                Y: y,
                            },
                            &hdr,
                            &dim,
                            D2D1_DRAW_TEXT_OPTIONS_CLIP,
                        );
                    }
                }
            }
            y += 18.0;

            for (i, (label, val)) in detail.metadata.iter().take(6).enumerate() {
                if i > 0 {
                    self.fill_rect(
                        left + pad,
                        y,
                        right - pad,
                        y + 1.0,
                        theme.border.with_alpha(0.5),
                    );
                }
                let label_w = (content_w * 0.34).min(110.0);
                let val_w = (content_w - label_w - 8.0).max(40.0);
                if let Some(l_layout) = self.layout(label, &self.meta_label_format, label_w, 18.0) {
                    if let Some(dim) = self.brush(theme.text_dim) {
                        unsafe {
                            self.target.DrawTextLayout(
                                Vector2 {
                                    X: left + pad,
                                    Y: y + 5.0,
                                },
                                &l_layout,
                                &dim,
                                D2D1_DRAW_TEXT_OPTIONS_CLIP,
                            );
                        }
                    }
                }
                if let Some(v_layout) = self.layout(val, &self.meta_value_format, val_w, 18.0) {
                    if let Some(fg) = self.brush(theme.text) {
                        unsafe {
                            self.target.DrawTextLayout(
                                Vector2 {
                                    X: right - pad - val_w,
                                    Y: y + 5.0,
                                },
                                &v_layout,
                                &fg,
                                D2D1_DRAW_TEXT_OPTIONS_CLIP,
                            );
                        }
                    }
                }
                y += meta_row_h;
            }
        }

        unsafe {
            self.target.PopAxisAlignedClip();
        }
    }

    /// Paints scrollable, selectable multi-line text inside the Master-Detail pane
    /// with selection highlight rectangles and an overlay scrollbar when overflowing.
    #[allow(clippy::too_many_arguments)]
    fn draw_scrollable_detail_text(
        &mut self,
        text: &str,
        theme: &Theme,
        detail_state: &mut DetailTextState,
        clip_rect: (f32, f32, f32, f32),
        pad_x: f32,
        pad_y: f32,
    ) {
        detail_state.sync_body(text);
        let (clip_l, clip_t, clip_r, clip_b) = clip_rect;
        let viewport_w = clip_r - clip_l;
        let viewport_h = clip_b - clip_t;
        let base_text_w = viewport_w - pad_x * 2.0;
        let inner_h = viewport_h - pad_y * 2.0;
        if text.is_empty() || base_text_w <= 4.0 || inner_h <= 4.0 {
            detail_state.viewport_rect = None;
            detail_state.content_height = 0.0;
            detail_state.scroll_offset = 0.0;
            return;
        }

        let mut text_w = base_text_w.max(20.0);
        let Some(mut layout) = self.layout(text, &self.detail_body_format, text_w, 100_000.0)
        else {
            detail_state.viewport_rect = None;
            detail_state.content_height = 0.0;
            detail_state.scroll_offset = 0.0;
            return;
        };
        let mut content_h = measure_height(&layout);
        if content_h > inner_h + 1.0 {
            let narrower_w = (base_text_w - 10.0).max(20.0);
            if let Some(narrower) =
                self.layout(text, &self.detail_body_format, narrower_w, 100_000.0)
            {
                text_w = narrower_w;
                content_h = measure_height(&narrower);
                layout = narrower;
            }
        }

        detail_state.viewport_rect = Some((clip_l, clip_t, clip_r, clip_b));
        detail_state.text_origin = (clip_l + pad_x, clip_t + pad_y);
        detail_state.text_width = text_w;
        detail_state.content_height = content_h + pad_y * 2.0;
        detail_state.clamp_scroll();

        let (text_x, text_y) = detail_state.text_origin;
        let scroll = detail_state.scroll_offset;
        let clip = D2D_RECT_F {
            left: clip_l,
            top: clip_t,
            right: clip_r,
            bottom: clip_b,
        };
        unsafe {
            self.target
                .PushAxisAlignedClip(&clip, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
        }

        // Paint text selection highlight rects underneath the glyphs.
        if let Some((sel_start, sel_end)) = detail_state.selected_range() {
            let sel_len = sel_end.saturating_sub(sel_start);
            if sel_len > 0 {
                let mut actual_count: u32 = 0;
                unsafe {
                    let _ = layout.HitTestTextRange(
                        sel_start,
                        sel_len,
                        text_x,
                        text_y - scroll,
                        None,
                        &mut actual_count,
                    );
                }
                let count = (actual_count as usize).min(4096);
                if count > 0 {
                    let mut buf = vec![DWRITE_HIT_TEST_METRICS::default(); count];
                    let mut written: u32 = 0;
                    let ok = unsafe {
                        layout
                            .HitTestTextRange(
                                sel_start,
                                sel_len,
                                text_x,
                                text_y - scroll,
                                Some(&mut buf),
                                &mut written,
                            )
                            .is_ok()
                    };
                    if ok {
                        let draw_count = (written as usize).min(buf.len());
                        if let Some(sel_brush) = self.brush(theme.accent.with_alpha(0.36)) {
                            for m in &buf[..draw_count] {
                                let rr = D2D1_ROUNDED_RECT {
                                    rect: D2D_RECT_F {
                                        left: m.left,
                                        top: m.top,
                                        right: m.left + m.width.max(4.0),
                                        bottom: m.top + m.height,
                                    },
                                    radiusX: 2.0,
                                    radiusY: 2.0,
                                };
                                unsafe {
                                    self.target.FillRoundedRectangle(&rr, &sel_brush);
                                }
                            }
                        }
                    }
                }
            }
        }

        if let Some(fg) = self.brush(theme.text) {
            unsafe {
                self.target.DrawTextLayout(
                    Vector2 {
                        X: text_x,
                        Y: text_y - scroll,
                    },
                    &layout,
                    &fg,
                    D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT,
                );
            }
        }

        unsafe {
            self.target.PopAxisAlignedClip();
        }

        // Thin overlay vertical scrollbar when text overflows the viewport.
        if detail_state.max_scroll() > 1.0 {
            let track_top = clip_t + 4.0;
            let track_bottom = (clip_b - 4.0).max(track_top);
            let track_h = track_bottom - track_top;
            if track_h > 12.0 {
                let bar_left = (clip_r - 7.0).max(clip_l);
                let bar_right = (clip_r - 3.5).max(bar_left + 1.0);
                let track_rr = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: bar_left,
                        top: track_top,
                        right: bar_right,
                        bottom: track_bottom,
                    },
                    radiusX: 1.75,
                    radiusY: 1.75,
                };
                if let Some(track_brush) = self.brush(theme.text_dim.with_alpha(0.10)) {
                    unsafe {
                        self.target.FillRoundedRectangle(&track_rr, &track_brush);
                    }
                }
                let ratio = (detail_state.viewport_height() / detail_state.content_height.max(1.0))
                    .clamp(0.12, 1.0);
                let thumb_h = (track_h * ratio).clamp(18.0, track_h);
                let progress =
                    (detail_state.scroll_offset / detail_state.max_scroll()).clamp(0.0, 1.0);
                let thumb_top = track_top + progress * (track_h - thumb_h);
                let thumb_rr = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: bar_left,
                        top: thumb_top,
                        right: bar_right,
                        bottom: thumb_top + thumb_h,
                    },
                    radiusX: 1.75,
                    radiusY: 1.75,
                };
                if let Some(thumb_brush) = self.brush(theme.text_dim.with_alpha(0.42)) {
                    unsafe {
                        self.target.FillRoundedRectangle(&thumb_rr, &thumb_brush);
                    }
                }
            }
        }
    }

    /// Maps a local DIP coordinate inside the detail preview text into a UTF-16 code unit offset.
    pub fn hit_test_detail_text(
        &self,
        text: &str,
        text_width: f32,
        local_x: f32,
        local_y: f32,
    ) -> Option<u32> {
        if text.is_empty() || text_width <= 0.0 {
            return None;
        }
        let layout = self.layout(text, &self.detail_body_format, text_width, 100_000.0)?;
        let mut is_trailing = BOOL(0);
        let mut is_inside = BOOL(0);
        let mut metrics = DWRITE_HIT_TEST_METRICS::default();
        unsafe {
            layout
                .HitTestPoint(
                    local_x,
                    local_y,
                    &mut is_trailing,
                    &mut is_inside,
                    &mut metrics,
                )
                .ok()?;
        }
        let pos = if is_trailing.as_bool() {
            metrics.textPosition.saturating_add(metrics.length)
        } else {
            metrics.textPosition
        };
        Some(pos)
    }

    /// Paint a horizontal row of rounded tint/shade swatch tiles with tiny centered labels below.
    /// Returns the total vertical DIP height consumed (`0.0` when `ramp` is empty).
    fn draw_color_ramp(
        &mut self,
        ramp: &[([u8; 4], String)],
        theme: &Theme,
        left: f32,
        top: f32,
        width: f32,
    ) -> f32 {
        if ramp.is_empty() {
            return 0.0;
        }
        let tiles = compute_color_ramp_rects(left, top, width, ramp.len());
        if tiles.is_empty() {
            return 0.0;
        }
        for (tile, (rgba, label)) in tiles.iter().zip(ramp.iter()) {
            let [r, g, b, a] = *rgba;
            let card = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: tile.left,
                    top: tile.top,
                    right: tile.right,
                    bottom: tile.bottom,
                },
                radiusX: COLOR_RAMP_TILE_RADIUS,
                radiusY: COLOR_RAMP_TILE_RADIUS,
            };
            if let Some(fill) = self.brush(Color::rgba(r, g, b, a)) {
                unsafe {
                    self.target.FillRoundedRectangle(&card, &fill);
                }
            }
            let border_color = color_ramp_border(*rgba, theme.appearance);
            if let Some(ring) = self.brush(border_color) {
                unsafe {
                    self.target.DrawRoundedRectangle(&card, &ring, 1.0, None);
                }
            }
            if !label.is_empty() {
                let tile_w = (tile.right - tile.left).max(1.0);
                if let Some(layout) = self.layout(
                    label,
                    &self.ramp_label_format,
                    tile_w,
                    COLOR_RAMP_LABEL_HEIGHT,
                ) {
                    if let Some(dim) = self.brush(theme.text_dim) {
                        unsafe {
                            self.target.DrawTextLayout(
                                Vector2 {
                                    X: tile.left,
                                    Y: tile.label_top,
                                },
                                &layout,
                                &dim,
                                D2D1_DRAW_TEXT_OPTIONS_CLIP,
                            );
                        }
                    }
                }
            }
        }
        COLOR_RAMP_TOTAL_HEIGHT
    }

    /// Paint the floating bottom-right Raycast-style `Ctrl+K` Action Panel Popover.
    fn draw_action_menu(&mut self, menu: &ActionMenuModel, theme: &Theme, width: f32, height: f32) {
        let metrics = theme.metrics;
        // Soft backdrop scrim over the list so the Action Panel pops with depth.
        let scrim = match theme.appearance {
            Appearance::Dark => Color::rgba(0x00, 0x00, 0x00, 0x52),
            Appearance::Light => Color::rgba(0x0f, 0x17, 0x2a, 0x22),
        };
        self.fill_rect(
            0.0,
            metrics.field_height + 1.0,
            width,
            height - metrics.footer_height,
            scrim,
        );

        let (left, top, right, bottom) = menu.rect(metrics, height);
        let panel_w = right - left;
        let card = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left,
                top,
                right,
                bottom,
            },
            radiusX: 10.0,
            radiusY: 10.0,
        };
        let panel_bg = match theme.appearance {
            Appearance::Dark => Color::rgb(0x22, 0x22, 0x22),
            Appearance::Light => Color::rgb(0xff, 0xff, 0xff),
        };
        let panel_border = match theme.appearance {
            Appearance::Dark => Color::rgba(0xff, 0xff, 0xff, 0x24),
            Appearance::Light => Color::rgba(0x00, 0x00, 0x00, 0x1c),
        };
        if let Some(bg) = self.brush(panel_bg) {
            unsafe {
                self.target.FillRoundedRectangle(&card, &bg);
            }
        }
        if let Some(border) = self.brush(panel_border) {
            unsafe {
                self.target.DrawRoundedRectangle(&card, &border, 1.0, None);
            }
        }

        // Popover header bar: title or live filter query.
        let header_text = if menu.query.trim().is_empty() {
            if menu.title.is_empty() {
                "Actions — type to filter…".to_string()
            } else {
                format!("{} — Actions", menu.title)
            }
        } else {
            format!("Filter: {}", menu.query)
        };
        if let Some(hdr) = self.layout(&header_text, &self.section_format, panel_w - 24.0, 18.0) {
            let col = if menu.query.trim().is_empty() {
                theme.text_dim
            } else {
                theme.accent
            };
            if let Some(b) = self.brush(col) {
                unsafe {
                    self.target.DrawTextLayout(
                        Vector2 {
                            X: left + 12.0,
                            Y: top + 9.0,
                        },
                        &hdr,
                        &b,
                        D2D1_DRAW_TEXT_OPTIONS_CLIP,
                    );
                }
            }
        }
        self.fill_rect(
            left + 1.0,
            top + ACTION_MENU_HEADER_H,
            right - 1.0,
            top + ACTION_MENU_HEADER_H + 1.0,
            theme.border,
        );

        let filtered = menu.filtered_indices();
        let rows_top = top + ACTION_MENU_HEADER_H + 4.0;
        if filtered.is_empty() {
            if let Some(empty) = self.layout(
                "No matching actions",
                &self.footer_format,
                panel_w - 24.0,
                18.0,
            ) {
                if let Some(b) = self.brush(theme.text_dim) {
                    unsafe {
                        self.target.DrawTextLayout(
                            Vector2 {
                                X: left + 12.0,
                                Y: rows_top + 6.0,
                            },
                            &empty,
                            &b,
                            D2D1_DRAW_TEXT_OPTIONS_CLIP,
                        );
                    }
                }
            }
            return;
        }

        let slots = menu.visible_slots(metrics, height);
        let start = menu.scroll_start_for_slots(filtered.len(), slots);
        let visible = filtered.iter().enumerate().skip(start).take(slots);
        let mut last_sec: Option<&str> = None;

        let rows_clip = D2D_RECT_F {
            left: left + 1.0,
            top: rows_top,
            right: right - 1.0,
            bottom: (bottom - 4.0).max(rows_top),
        };
        unsafe {
            self.target
                .PushAxisAlignedClip(&rows_clip, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
        }

        for (slot, (filtered_idx, &raw_idx)) in visible.enumerate() {
            let Some(item) = menu.items.get(raw_idx) else {
                continue;
            };
            let ry = rows_top + slot as f32 * ACTION_MENU_ROW_H;
            if slot > 0 && last_sec.is_some() && last_sec != Some(item.section.as_str()) {
                self.fill_rect(
                    left + 10.0,
                    ry,
                    right - 10.0,
                    ry + 1.0,
                    theme.border.with_alpha(0.6),
                );
            }
            last_sec = Some(item.section.as_str());

            let is_sel = filtered_idx == menu.selected;
            let is_hov = menu.hovered == Some(filtered_idx) && !is_sel;
            let row_rr = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: left + 6.0,
                    top: ry + 1.5,
                    right: right - 6.0,
                    bottom: ry + ACTION_MENU_ROW_H - 1.5,
                },
                radiusX: 6.0,
                radiusY: 6.0,
            };
            if is_sel {
                if let Some(b) = self.brush(theme.selection) {
                    unsafe {
                        self.target.FillRoundedRectangle(&row_rr, &b);
                    }
                }
            } else if is_hov {
                if let Some(b) = self.brush(theme.hover) {
                    unsafe {
                        self.target.FillRoundedRectangle(&row_rr, &b);
                    }
                }
            }

            let fg_col = if item.destructive {
                Color::rgb(0xf8, 0x71, 0x71)
            } else {
                theme.text
            };
            if let Some(gl) = self.layout(&item.glyph, &self.icon_format, 18.0, 18.0) {
                let gcol = if item.destructive {
                    Color::rgb(0xf8, 0x71, 0x71)
                } else {
                    theme.text_dim
                };
                if let Some(b) = self.brush(gcol) {
                    unsafe {
                        self.target.DrawTextLayout(
                            Vector2 {
                                X: left + 12.0,
                                Y: ry + 7.5,
                            },
                            &gl,
                            &b,
                            D2D1_DRAW_TEXT_OPTIONS_CLIP,
                        );
                    }
                }
            }

            let mut shortcut_reserve = 14.0_f32;
            if !item.shortcut.is_empty() {
                let sc_w = self.draw_shortcut_keycaps(
                    &item.shortcut,
                    right - 12.0,
                    ry + ACTION_MENU_ROW_H / 2.0,
                    theme,
                );
                shortcut_reserve = sc_w + 20.0;
            }
            let title_w = (panel_w - 36.0 - shortcut_reserve).max(40.0);
            if let Some(tl) = self.layout(&item.title, &self.action_title_format, title_w, 20.0) {
                if let Some(b) = self.brush(fg_col) {
                    unsafe {
                        self.target.DrawTextLayout(
                            Vector2 {
                                X: left + 34.0,
                                Y: ry + 7.0,
                            },
                            &tl,
                            &b,
                            D2D1_DRAW_TEXT_OPTIONS_CLIP,
                        );
                    }
                }
            }
        }

        unsafe {
            self.target.PopAxisAlignedClip();
        }
    }

    fn draw_pill_right(
        &mut self,
        text: &str,
        right: f32,
        center_y: f32,
        bg: Color,
        border: Color,
        fg: Color,
    ) -> f32 {
        let Some(probe) = self.layout(text, &self.pill_format, 200.0, 18.0) else {
            return 0.0;
        };
        let text_w = measure_width(&probe);
        let pill_w = (text_w + 18.0).max(32.0);
        let pill_h = 22.0;
        let left = right - pill_w;
        let top = center_y - pill_h / 2.0;
        self.paint_pill(text, left, top, pill_w, pill_h, bg, border, fg);
        pill_w
    }

    fn draw_pill_centered(
        &mut self,
        text: &str,
        center_x: f32,
        center_y: f32,
        bg: Color,
        border: Color,
        fg: Color,
    ) -> f32 {
        let Some(probe) = self.layout(text, &self.pill_format, 240.0, 18.0) else {
            return 0.0;
        };
        let text_w = measure_width(&probe);
        let pill_w = (text_w + 16.0).max(34.0);
        let pill_h = 20.0;
        let left = center_x - pill_w / 2.0;
        let top = center_y - pill_h / 2.0;
        self.paint_pill(text, left, top, pill_w, pill_h, bg, border, fg);
        pill_w
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_pill(
        &mut self,
        text: &str,
        left: f32,
        top: f32,
        width: f32,
        height: f32,
        bg: Color,
        border: Color,
        fg: Color,
    ) {
        let rect = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left,
                top,
                right: left + width,
                bottom: top + height,
            },
            radiusX: 4.0,
            radiusY: 4.0,
        };
        if bg.alpha > 0.0 {
            if let Some(brush) = self.brush(bg) {
                unsafe {
                    self.target.FillRoundedRectangle(&rect, &brush);
                }
            }
        }
        if border.alpha > 0.0 {
            if let Some(brush) = self.brush(border) {
                unsafe {
                    self.target.DrawRoundedRectangle(&rect, &brush, 1.0, None);
                }
            }
        }
        if let Some(layout) = self.layout(text, &self.pill_format, width, height) {
            if let Some(brush) = self.brush(fg) {
                unsafe {
                    self.target.DrawTextLayout(
                        Vector2 {
                            X: left,
                            Y: top + (height - 15.0) / 2.0,
                        },
                        &layout,
                        &brush,
                        D2D1_DRAW_TEXT_OPTIONS_CLIP,
                    );
                }
            }
        }
    }

    fn fill_rect(&mut self, left: f32, top: f32, right: f32, bottom: f32, color: Color) {
        let rect = D2D_RECT_F {
            left,
            top,
            right,
            bottom,
        };
        if let Some(brush) = self.brush(color) {
            unsafe {
                self.target.FillRectangle(&rect, &brush);
            }
        }
    }

    /// YAL's Launch Ring monochrome mark in the left searchbar slot
    /// (250° open reticle ring + 45° launch vector, optically centered at 1:1 DIPs).
    fn draw_search_glyph(&mut self, theme: &Theme, left: f32, middle: f32) {
        let cx = left + 9.2;
        let cy = middle;
        let ring_col = match theme.appearance {
            Appearance::Dark => Color::rgba(0xff, 0xff, 0xff, 0x6c),
            Appearance::Light => Color::rgba(0x00, 0x00, 0x00, 0x62),
        };
        let arrow_col = match theme.appearance {
            Appearance::Dark => Color::rgba(0xff, 0xff, 0xff, 0x98),
            Appearance::Light => Color::rgba(0x00, 0x00, 0x00, 0x8a),
        };
        let stroke = self.round_stroke.clone();
        let ring_geo = self.logo_ring.clone();
        let arrow_geo = self.logo_arrow.clone();
        let ring_brush = self.brush(ring_col);
        let arrow_brush = self.brush(arrow_col);

        unsafe {
            let translate = Matrix3x2::translation(cx, cy);
            self.target.SetTransform(&translate);
            if let (Some(geo), Some(brush)) = (ring_geo.as_ref(), ring_brush.as_ref()) {
                self.target.DrawGeometry(geo, brush, 1.8, stroke.as_ref());
            }
            if let (Some(geo), Some(brush)) = (arrow_geo.as_ref(), arrow_brush.as_ref()) {
                self.target.DrawGeometry(geo, brush, 1.8, stroke.as_ref());
            }
            self.target.SetTransform(&Matrix3x2::identity());
        }
    }

    /// Crisp 1:1 DIP Direct2D vector icon drawn centered inside a row's leading tile.
    fn draw_vector_icon(&mut self, kind: IconKind, left: f32, top: f32, size: f32, color: Color) {
        let Some(brush) = self.brush(color) else {
            return;
        };
        let stroke = self.round_stroke.clone();
        let cx = left + size * 0.5;
        let cy = top + size * 0.5;
        let sw = 1.45_f32;

        unsafe {
            match kind {
                IconKind::Text => {
                    // Clean 3-line document glyph
                    let x0 = cx - 5.0;
                    let x1 = cx + 5.0;
                    let x_short = cx + 1.5;
                    self.target.DrawLine(
                        Vector2 { X: x0, Y: cy - 3.6 },
                        Vector2 { X: x1, Y: cy - 3.6 },
                        &brush,
                        sw,
                        stroke.as_ref(),
                    );
                    self.target.DrawLine(
                        Vector2 { X: x0, Y: cy },
                        Vector2 { X: x1, Y: cy },
                        &brush,
                        sw,
                        stroke.as_ref(),
                    );
                    self.target.DrawLine(
                        Vector2 { X: x0, Y: cy + 3.6 },
                        Vector2 {
                            X: x_short,
                            Y: cy + 3.6,
                        },
                        &brush,
                        sw,
                        stroke.as_ref(),
                    );
                }
                IconKind::Link => {
                    // Interlocked diagonal link capsules rotated -45° around (cx, cy)
                    let left_link = D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: -5.6,
                            top: -2.3,
                            right: 1.0,
                            bottom: 2.3,
                        },
                        radiusX: 2.3,
                        radiusY: 2.3,
                    };
                    let right_link = D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: -1.0,
                            top: -2.3,
                            right: 5.6,
                            bottom: 2.3,
                        },
                        radiusX: 2.3,
                        radiusY: 2.3,
                    };
                    let s = std::f32::consts::FRAC_1_SQRT_2;
                    let transform = Matrix3x2 {
                        M11: s,
                        M12: -s,
                        M21: s,
                        M22: s,
                        M31: cx,
                        M32: cy,
                    };
                    self.target.SetTransform(&transform);
                    self.target
                        .DrawRoundedRectangle(&left_link, &brush, sw, stroke.as_ref());
                    self.target
                        .DrawRoundedRectangle(&right_link, &brush, sw, stroke.as_ref());
                    self.target.SetTransform(&Matrix3x2::identity());
                }
                IconKind::Email => {
                    // Rounded envelope + V flap
                    let env = D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: cx - 5.6,
                            top: cy - 4.0,
                            right: cx + 5.6,
                            bottom: cy + 4.0,
                        },
                        radiusX: 1.5,
                        radiusY: 1.5,
                    };
                    self.target
                        .DrawRoundedRectangle(&env, &brush, sw, stroke.as_ref());
                    self.target.DrawLine(
                        Vector2 {
                            X: cx - 4.8,
                            Y: cy - 3.0,
                        },
                        Vector2 { X: cx, Y: cy + 0.6 },
                        &brush,
                        sw,
                        stroke.as_ref(),
                    );
                    self.target.DrawLine(
                        Vector2 { X: cx, Y: cy + 0.6 },
                        Vector2 {
                            X: cx + 4.8,
                            Y: cy - 3.0,
                        },
                        &brush,
                        sw,
                        stroke.as_ref(),
                    );
                }
                IconKind::Image => {
                    // Framed landscape photo + sun dot
                    let frame = D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: cx - 5.5,
                            top: cy - 4.8,
                            right: cx + 5.5,
                            bottom: cy + 4.8,
                        },
                        radiusX: 1.6,
                        radiusY: 1.6,
                    };
                    self.target
                        .DrawRoundedRectangle(&frame, &brush, sw, stroke.as_ref());
                    let sun = D2D1_ELLIPSE {
                        point: Vector2 {
                            X: cx - 2.3,
                            Y: cy - 1.8,
                        },
                        radiusX: 1.15,
                        radiusY: 1.15,
                    };
                    self.target.FillEllipse(&sun, &brush);
                    self.target.DrawLine(
                        Vector2 {
                            X: cx - 4.2,
                            Y: cy + 3.4,
                        },
                        Vector2 {
                            X: cx - 0.6,
                            Y: cy - 0.2,
                        },
                        &brush,
                        sw,
                        stroke.as_ref(),
                    );
                    self.target.DrawLine(
                        Vector2 {
                            X: cx - 0.6,
                            Y: cy - 0.2,
                        },
                        Vector2 {
                            X: cx + 1.5,
                            Y: cy + 1.8,
                        },
                        &brush,
                        sw,
                        stroke.as_ref(),
                    );
                    self.target.DrawLine(
                        Vector2 {
                            X: cx + 1.5,
                            Y: cy + 1.8,
                        },
                        Vector2 {
                            X: cx + 4.2,
                            Y: cy - 0.8,
                        },
                        &brush,
                        sw,
                        stroke.as_ref(),
                    );
                }
                IconKind::File => {
                    // Clean page outline with top-right corner fold
                    let page = D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: cx - 4.5,
                            top: cy - 5.5,
                            right: cx + 4.5,
                            bottom: cy + 5.5,
                        },
                        radiusX: 1.3,
                        radiusY: 1.3,
                    };
                    self.target
                        .DrawRoundedRectangle(&page, &brush, sw, stroke.as_ref());
                    self.target.DrawLine(
                        Vector2 {
                            X: cx - 2.2,
                            Y: cy - 1.2,
                        },
                        Vector2 {
                            X: cx + 2.2,
                            Y: cy - 1.2,
                        },
                        &brush,
                        1.2,
                        stroke.as_ref(),
                    );
                    self.target.DrawLine(
                        Vector2 {
                            X: cx - 2.2,
                            Y: cy + 2.0,
                        },
                        Vector2 {
                            X: cx + 1.2,
                            Y: cy + 2.0,
                        },
                        &brush,
                        1.2,
                        stroke.as_ref(),
                    );
                }
                IconKind::Code => {
                    // Crisp < > brackets
                    self.target.DrawLine(
                        Vector2 {
                            X: cx - 2.0,
                            Y: cy - 3.6,
                        },
                        Vector2 { X: cx - 5.2, Y: cy },
                        &brush,
                        sw,
                        stroke.as_ref(),
                    );
                    self.target.DrawLine(
                        Vector2 { X: cx - 5.2, Y: cy },
                        Vector2 {
                            X: cx - 2.0,
                            Y: cy + 3.6,
                        },
                        &brush,
                        sw,
                        stroke.as_ref(),
                    );
                    self.target.DrawLine(
                        Vector2 {
                            X: cx + 2.0,
                            Y: cy - 3.6,
                        },
                        Vector2 { X: cx + 5.2, Y: cy },
                        &brush,
                        sw,
                        stroke.as_ref(),
                    );
                    self.target.DrawLine(
                        Vector2 { X: cx + 5.2, Y: cy },
                        Vector2 {
                            X: cx + 2.0,
                            Y: cy + 3.6,
                        },
                        &brush,
                        sw,
                        stroke.as_ref(),
                    );
                }
            }
        }
    }

    fn layout(
        &self,
        text: &str,
        format: &IDWriteTextFormat,
        width: f32,
        height: f32,
    ) -> Option<IDWriteTextLayout> {
        if text.is_empty() || width <= 0.0 {
            return None;
        }
        let wide: Vec<u16> = text.encode_utf16().collect();
        unsafe {
            self.dwrite
                .CreateTextLayout(&wide, format, width, height)
                .ok()
        }
    }
}

fn measure_width(layout: &IDWriteTextLayout) -> f32 {
    let mut metrics = DWRITE_TEXT_METRICS::default();
    unsafe {
        if layout.GetMetrics(&mut metrics).is_ok() {
            metrics.widthIncludingTrailingWhitespace
        } else {
            0.0
        }
    }
}

fn measure_height(layout: &IDWriteTextLayout) -> f32 {
    let mut metrics = DWRITE_TEXT_METRICS::default();
    unsafe {
        if layout.GetMetrics(&mut metrics).is_ok() {
            metrics.height
        } else {
            0.0
        }
    }
}

fn dwrite_format_wrapped(
    dwrite: &IDWriteFactory,
    family: &str,
    weight: DWRITE_FONT_WEIGHT,
    size: f32,
    alignment: DWRITE_TEXT_ALIGNMENT,
) -> Result<IDWriteTextFormat, String> {
    let family_wide: Vec<u16> = family.encode_utf16().chain(std::iter::once(0)).collect();
    let locale: Vec<u16> = "en-us".encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let format = dwrite
            .CreateTextFormat(
                PCWSTR(family_wide.as_ptr()),
                None,
                weight,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                size,
                PCWSTR(locale.as_ptr()),
            )
            .map_err(|error| format!("the text format could not be created: {error}"))?;
        let _ = format.SetTextAlignment(alignment);
        let _ = format.SetWordWrapping(DWRITE_WORD_WRAPPING_WRAP);
        Ok(format)
    }
}

fn dwrite_format(
    dwrite: &IDWriteFactory,
    family: &str,
    weight: DWRITE_FONT_WEIGHT,
    size: f32,
    alignment: DWRITE_TEXT_ALIGNMENT,
) -> Result<IDWriteTextFormat, String> {
    let wide_family: Vec<u16> = family.encode_utf16().chain(std::iter::once(0)).collect();
    let wide_locale: Vec<u16> = "en-us".encode_utf16().chain(std::iter::once(0)).collect();
    let format = unsafe {
        dwrite
            .CreateTextFormat(
                PCWSTR(wide_family.as_ptr()),
                None,
                weight,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                size,
                PCWSTR(wide_locale.as_ptr()),
            )
            .map_err(|error| format!("the font {family} could not be prepared: {error}"))?
    };
    unsafe {
        format
            .SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)
            .map_err(|error| format!("the text could not be set to not wrap: {error}"))?;
        format
            .SetTextAlignment(alignment)
            .map_err(|error| format!("the text could not be aligned: {error}"))?;
    }
    Ok(format)
}

fn to_d2d(color: Color) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: color.red,
        g: color.green,
        b: color.blue,
        a: color.alpha,
    }
}

/// Pre-builds YAL's Launch Ring vector mark (centered at `(0, 0)` in DIPs, matching
/// `assets/yal-mark.svg`) once per `Renderer` so painting allocates zero COM objects.
fn build_launch_ring_geometry(
    factory: &ID2D1Factory,
) -> (Option<ID2D1PathGeometry>, Option<ID2D1PathGeometry>) {
    let scale = 0.85_f32;
    let r = 7.2 * scale;
    let a_start = 10.0_f32.to_radians();
    let a_end = (-100.0_f32).to_radians();
    let start = Vector2 {
        X: r * a_start.cos(),
        Y: r * a_start.sin(),
    };
    let end = Vector2 {
        X: r * a_end.cos(),
        Y: r * a_end.sin(),
    };

    let ring = unsafe {
        factory.CreatePathGeometry().ok().and_then(|geo| {
            let sink = geo.Open().ok()?;
            sink.BeginFigure(start, D2D1_FIGURE_BEGIN_HOLLOW);
            sink.AddArc(&D2D1_ARC_SEGMENT {
                point: end,
                size: D2D_SIZE_F {
                    width: r,
                    height: r,
                },
                rotationAngle: 0.0,
                sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                arcSize: D2D1_ARC_SIZE_LARGE,
            });
            sink.EndFigure(D2D1_FIGURE_END_OPEN);
            sink.Close().ok()?;
            Some(geo)
        })
    };

    let arrow = unsafe {
        factory.CreatePathGeometry().ok().and_then(|geo| {
            let sink = geo.Open().ok()?;
            // 45° launch shaft from (-1.6, +1.6) to (+6.4, -6.4) scaled by `scale`.
            sink.BeginFigure(
                Vector2 {
                    X: -1.6 * scale,
                    Y: 1.6 * scale,
                },
                D2D1_FIGURE_BEGIN_HOLLOW,
            );
            sink.AddLine(Vector2 {
                X: 6.4 * scale,
                Y: -6.4 * scale,
            });
            sink.EndFigure(D2D1_FIGURE_END_OPEN);

            // Crisp 90° launch arrowhead pointing top-right.
            sink.BeginFigure(
                Vector2 {
                    X: 1.8 * scale,
                    Y: -6.4 * scale,
                },
                D2D1_FIGURE_BEGIN_HOLLOW,
            );
            sink.AddLine(Vector2 {
                X: 6.4 * scale,
                Y: -6.4 * scale,
            });
            sink.AddLine(Vector2 {
                X: 6.4 * scale,
                Y: -1.8 * scale,
            });
            sink.EndFigure(D2D1_FIGURE_END_OPEN);

            sink.Close().ok()?;
            Some(geo)
        })
    };

    (ring, arrow)
}

/// Connector words, prepositions, and prose fillers rendered in Raycast's dimmed
/// tertiary color (`rgba(255,255,255,0.40)`) inside the Calculator expression card.
fn is_dimmed_calc_word(word: &str) -> bool {
    matches!(
        word.to_ascii_lowercase().as_str(),
        "till"
            | "until"
            | "til"
            | "'til"
            | "since"
            | "between"
            | "and"
            | "in"
            | "to"
            | "into"
            | "from"
            | "of"
            | "at"
            | "on"
            | "by"
            | "for"
            | "with"
            | "as"
            | "per"
            | "before"
            | "after"
            | "left"
            | "remaining"
            | "how"
            | "many"
            | "what"
            | "when"
            | "is"
            | "are"
            | "off"
            | "plus"
            | "minus"
            | "times"
            | "divided"
            | "over"
            | "mod"
            | "or"
            | "xor"
    )
}

/// Format a Calculator input expression for the left half of the split hero card
/// (substituting standalone `*` -> `×`, `/` -> `÷`, `-` -> `−`) and return the
/// UTF-16 `(start, length)` ranges that should be rendered dimmed (operators,
/// parentheses, and connector words like `till`, `to`, `in`, `of`, `between`).
fn format_calc_expression_spans(expr: &str) -> (String, Vec<(u32, u32)>) {
    let chars: Vec<char> = expr.chars().collect();
    let mut out = String::with_capacity(expr.len() + 4);
    let mut dimmed: Vec<(u32, u32)> = Vec::new();
    let mut utf16_pos: u32 = 0;
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];
        if ch.is_whitespace() {
            out.push(ch);
            utf16_pos += ch.len_utf16() as u32;
            i += 1;
            continue;
        }

        // Alphabetic / apostrophe word tokens (e.g. `hours`, `till`, `'til`, `nov`).
        if ch.is_alphabetic() || (ch == '\'' && chars.get(i + 1).is_some_and(|c| c.is_alphabetic()))
        {
            let start_i = i;
            i += 1;
            while i < chars.len() && (chars[i].is_alphabetic() || chars[i] == '\'') {
                i += 1;
            }
            let word: String = chars[start_i..i].iter().collect();
            let start_u16 = utf16_pos;
            let len_u16 = word.encode_utf16().count() as u32;
            out.push_str(&word);
            utf16_pos += len_u16;
            if is_dimmed_calc_word(&word) {
                dimmed.push((start_u16, len_u16));
            }
            continue;
        }

        // Numeric / hex / color / date literal runs stay bright (`27`, `3:45pm`, `29/11`, `2026-11-27`, `#ff5500`).
        if ch.is_ascii_digit() || ch == '#' {
            let start_i = i;
            i += 1;
            while i < chars.len() {
                let c = chars[i];
                let prev = chars[i - 1];
                let next_is_digit = chars.get(i + 1).is_some_and(|n| n.is_ascii_digit());
                if c.is_ascii_alphanumeric()
                    || c == '.'
                    || c == ','
                    || c == ':'
                    || c == '%'
                    || ((c == '/' || c == '-') && prev.is_ascii_digit() && next_is_digit)
                {
                    i += 1;
                } else {
                    break;
                }
            }
            let lit: String = chars[start_i..i].iter().collect();
            utf16_pos += lit.encode_utf16().count() as u32;
            out.push_str(&lit);
            continue;
        }

        // Operators and punctuation.
        let prev_ws = i == 0 || chars[i - 1].is_whitespace();
        let next_ws = chars.get(i + 1).is_none_or(|c| c.is_whitespace());
        let (mapped, is_dim) = match ch {
            '*' => ('×', true),
            '/' => ('÷', true),
            '-' if prev_ws && next_ws => ('−', true),
            '+' | '^' | '=' | '(' | ')' | '×' | '÷' | '−' | '&' | '|' | '<' | '>' | ',' => {
                (ch, true)
            }
            _ => (ch, false),
        };
        let start_u16 = utf16_pos;
        let len_u16 = mapped.len_utf16() as u32;
        out.push(mapped);
        utf16_pos += len_u16;
        if is_dim {
            dimmed.push((start_u16, len_u16));
        }
        i += 1;
    }

    (out, dimmed)
}

/// Compute evenly spaced horizontal tile rects and label Y anchors for a color ramp strip.
fn compute_color_ramp_rects(left: f32, top: f32, width: f32, count: usize) -> Vec<ColorRampTile> {
    if count == 0 || width <= 0.0 {
        return Vec::new();
    }
    let gaps = count.saturating_sub(1) as f32;
    let gap = if gaps > 0.0 && gaps * COLOR_RAMP_TILE_GAP >= width {
        0.0
    } else {
        COLOR_RAMP_TILE_GAP
    };
    let tile_w = (width - gaps * gap) / count as f32;
    let bottom = top + COLOR_RAMP_TILE_HEIGHT;
    let label_top = bottom + COLOR_RAMP_LABEL_GAP;
    let max_right = left + width;

    (0..count)
        .map(|i| {
            let tile_left = left + i as f32 * (tile_w + gap);
            let tile_right = if i + 1 == count {
                max_right
            } else {
                (tile_left + tile_w).min(max_right)
            };
            ColorRampTile {
                left: tile_left,
                top,
                right: tile_right,
                bottom,
                label_top,
            }
        })
        .collect()
}

/// Choose a subtle contrasting 1px hairline border for a color ramp tile based on swatch luminance.
fn color_ramp_border(rgba: [u8; 4], appearance: Appearance) -> Color {
    let [r, g, b, _] = rgba;
    let luma = (0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32) / 255.0;
    if luma >= 0.60 {
        match appearance {
            Appearance::Dark => Color::rgba(0x00, 0x00, 0x00, 0x45),
            Appearance::Light => Color::rgba(0x0f, 0x17, 0x2a, 0x33),
        }
    } else if luma <= 0.30 {
        match appearance {
            Appearance::Dark => Color::rgba(0xff, 0xff, 0xff, 0x38),
            Appearance::Light => Color::rgba(0xff, 0xff, 0xff, 0x30),
        }
    } else {
        match appearance {
            Appearance::Dark => Color::rgba(0xff, 0xff, 0xff, 0x24),
            Appearance::Light => Color::rgba(0x0f, 0x17, 0x2a, 0x1f),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calc_expression_dims_connectors_and_formats_operators() {
        let (text, ranges) = format_calc_expression_spans("hours till 27 nov");
        assert_eq!(text, "hours till 27 nov");
        assert_eq!(ranges, vec![(6, 4)]); // "till"

        let (math, math_ranges) = format_calc_expression_spans("25 * 4 - 10 / 2");
        assert_eq!(math, "25 × 4 − 10 ÷ 2");
        assert_eq!(math_ranges.len(), 3);

        // Date slashes and hyphens stay untouched and bright.
        let (date, date_ranges) = format_calc_expression_spans("days till 2026-11-27");
        assert_eq!(date, "days till 2026-11-27");
        assert_eq!(date_ranges, vec![(5, 4)]); // only "till"
    }

    #[test]
    fn color_ramp_rects_handle_empty_single_and_nine_step_ramps() {
        assert!(compute_color_ramp_rects(16.0, 100.0, 400.0, 0).is_empty());
        assert!(compute_color_ramp_rects(16.0, 100.0, 0.0, 9).is_empty());
        assert!(compute_color_ramp_rects(16.0, 100.0, -10.0, 3).is_empty());

        let single = compute_color_ramp_rects(16.0, 100.0, 320.0, 1);
        assert_eq!(single.len(), 1);
        assert!((single[0].left - 16.0).abs() < 1e-4);
        assert!((single[0].right - 336.0).abs() < 1e-4);
        assert!((single[0].bottom - single[0].top - COLOR_RAMP_TILE_HEIGHT).abs() < 1e-4);
        assert!((single[0].label_top - (single[0].bottom + COLOR_RAMP_LABEL_GAP)).abs() < 1e-4);

        let left = 16.0_f32;
        let top = 120.0_f32;
        let width = 392.0_f32;
        let tiles = compute_color_ramp_rects(left, top, width, 9);
        assert_eq!(tiles.len(), 9);
        let expected_w = (width - 8.0 * COLOR_RAMP_TILE_GAP) / 9.0;
        for (i, tile) in tiles.iter().enumerate() {
            assert!(tile.left >= left - 1e-4);
            assert!(tile.right <= left + width + 1e-4);
            assert!((tile.right - tile.left - expected_w).abs() < 1e-3);
            assert!((tile.top - top).abs() < 1e-4);
            assert!((tile.bottom - (top + COLOR_RAMP_TILE_HEIGHT)).abs() < 1e-4);
            assert!((tile.label_top - (tile.bottom + COLOR_RAMP_LABEL_GAP)).abs() < 1e-4);
            if i > 0 {
                let gap = tile.left - tiles[i - 1].right;
                assert!((gap - COLOR_RAMP_TILE_GAP).abs() < 1e-3);
            }
        }
        assert!((tiles[0].left - left).abs() < 1e-4);
        assert!((tiles[8].right - (left + width)).abs() < 1e-4);
    }

    #[test]
    fn color_ramp_border_adapts_contrast_for_light_and_dark_swatches() {
        let light_swatch = [250, 250, 250, 255];
        let dark_swatch = [18, 18, 22, 255];

        for appearance in [Appearance::Dark, Appearance::Light] {
            let on_light = color_ramp_border(light_swatch, appearance);
            let on_dark = color_ramp_border(dark_swatch, appearance);
            let on_light_luma =
                0.2126 * on_light.red + 0.7152 * on_light.green + 0.0722 * on_light.blue;
            let on_dark_luma =
                0.2126 * on_dark.red + 0.7152 * on_dark.green + 0.0722 * on_dark.blue;

            assert!(on_light.alpha > 0.0 && on_light.alpha < 0.5);
            assert!(on_dark.alpha > 0.0 && on_dark.alpha < 0.5);
            assert!(
                on_light_luma < 0.2,
                "bright swatch should use dark hairline border"
            );
            assert!(
                on_dark_luma > 0.8,
                "dark swatch should use light hairline border"
            );
        }
    }

    #[test]
    fn utf16_slice_handles_ascii_reversed_empty_and_out_of_range() {
        assert_eq!(utf16_slice("hello world", 0, 5), "hello");
        assert_eq!(utf16_slice("hello world", 5, 0), "hello");
        assert_eq!(utf16_slice("hello world", 3, 3), "");
        assert_eq!(utf16_slice("", 0, 10), "");
        assert_eq!(utf16_slice("hello", 2, 999), "llo");
        assert_eq!(utf16_slice("hello", 100, 200), "");
    }

    #[test]
    fn utf16_slice_is_safe_on_surrogate_pairs_and_mixed_cjk_emoji() {
        // "a😀b" => UTF-16 offsets: 'a'=0..1, '😀'=1..3, 'b'=3..4
        let s = "a😀b";
        assert_eq!(utf16_slice(s, 0, 1), "a");
        assert_eq!(utf16_slice(s, 1, 3), "😀");
        assert_eq!(utf16_slice(s, 3, 4), "b");
        assert_eq!(utf16_slice(s, 3, 1), "😀");
        // Mid-surrogate bounds include overlapping emoji cleanly without panicking.
        assert_eq!(utf16_slice(s, 1, 2), "😀");
        assert_eq!(utf16_slice(s, 2, 3), "😀");
        assert_eq!(utf16_slice(s, 2, 4), "😀b");

        // "🚀🔥✨" => '🚀'=0..2, '🔥'=2..4, '✨'=4..5
        let emojis = "🚀🔥✨";
        assert_eq!(utf16_slice(emojis, 0, 2), "🚀");
        assert_eq!(utf16_slice(emojis, 2, 4), "🔥");
        assert_eq!(utf16_slice(emojis, 1, 3), "🚀🔥");
        assert_eq!(utf16_slice(emojis, 4, 5), "✨");

        // "日本語😀test" => '日'=0..1, '本'=1..2, '語'=2..3, '😀'=3..5, "test"=5..9
        let mixed = "日本語😀test";
        assert_eq!(utf16_slice(mixed, 0, 3), "日本語");
        assert_eq!(utf16_slice(mixed, 3, 5), "😀");
        assert_eq!(utf16_slice(mixed, 4, 9), "😀test");
        assert_eq!(utf16_slice(mixed, 9, 4), "😀test");
        assert_eq!(utf16_slice(mixed, 0, 100), mixed);
    }

    #[test]
    fn word_bounds_utf16_handles_words_whitespace_punct_cjk_and_surrogates() {
        assert_eq!(word_bounds_utf16("", 0), (0, 0));

        let ascii = "foo_bar   baz!!!";
        // "foo_bar" = 0..7, "   " = 7..10, "baz" = 10..13, "!!!" = 13..16
        assert_eq!(word_bounds_utf16(ascii, 0), (0, 7));
        assert_eq!(word_bounds_utf16(ascii, 4), (0, 7));
        assert_eq!(word_bounds_utf16(ascii, 8), (7, 10));
        assert_eq!(word_bounds_utf16(ascii, 11), (10, 13));
        assert_eq!(word_bounds_utf16(ascii, 14), (13, 16));
        // Out-of-bounds clamps to final run.
        assert_eq!(word_bounds_utf16(ascii, 999), (13, 16));

        // Surrogate pairs / emoji (Punct class): "hi 🚀🔥 ok"
        // "hi"=0..2, " "=2..3, "🚀"=3..5, "🔥"=5..7, " "=7..8, "ok"=8..10
        let with_emoji = "hi 🚀🔥 ok";
        assert_eq!(word_bounds_utf16(with_emoji, 3), (3, 7));
        // Mid-surrogate offset 4 inside '🚀' expands to the full emoji run without splitting.
        assert_eq!(word_bounds_utf16(with_emoji, 4), (3, 7));
        assert_eq!(word_bounds_utf16(with_emoji, 6), (3, 7));
        assert_eq!(utf16_slice(with_emoji, 3, 7), "🚀🔥");

        // CJK words are alphanumeric.
        let cjk = "日本語 テスト";
        assert_eq!(word_bounds_utf16(cjk, 1), (0, 3));
        assert_eq!(word_bounds_utf16(cjk, 3), (3, 4));
        assert_eq!(word_bounds_utf16(cjk, 5), (4, 7));
    }

    #[test]
    fn detail_text_state_scroll_clamping_and_sync_body() {
        let mut state = DetailTextState {
            viewport_rect: Some((10.0, 20.0, 210.0, 120.0)), // height 100
            content_height: 260.0,                           // max_scroll 160
            ..DetailTextState::default()
        };
        assert_eq!(state.viewport_height(), 100.0);
        assert_eq!(state.max_scroll(), 160.0);
        assert!(state.contains_point(10.0, 20.0));
        assert!(state.contains_point(210.0, 120.0));
        assert!(!state.contains_point(9.0, 50.0));

        state.scroll_by(50.0);
        assert_eq!(state.scroll_offset, 50.0);
        state.scroll_by(500.0);
        assert_eq!(state.scroll_offset, 160.0);
        state.scroll_by(-999.0);
        assert_eq!(state.scroll_offset, 0.0);

        state.sync_body("initial body");
        state.scroll_by(42.0);
        state.selection = Some((2, 8));
        state.dragging = true;
        assert_eq!(state.selected_range(), Some((2, 8)));

        // Same body preserves scroll and selection.
        state.sync_body("initial body");
        assert_eq!(state.scroll_offset, 42.0);
        assert_eq!(state.selection, Some((2, 8)));
        assert!(state.dragging);

        // Different body resets scroll, selection, and dragging.
        state.sync_body("updated body");
        assert_eq!(state.scroll_offset, 0.0);
        assert_eq!(state.selection, None);
        assert!(!state.dragging);
        assert_eq!(state.selected_range(), None);

        // Degenerate selection returns None.
        state.selection = Some((5, 5));
        assert_eq!(state.selected_range(), None);
    }
}
