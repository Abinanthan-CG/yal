//! The palette's paint: Direct2D shapes, GPU-cached Windows app icons, and
//! DirectWrite typography in pure 1:1 DIP coordinates.

use std::collections::HashMap;
use std::ffi::c_void;

use windows::core::PCWSTR;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct2D::{
    Common::{
        D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT, D2D_RECT_F, D2D_SIZE_U,
    },
    D2D1CreateFactory, ID2D1Bitmap, ID2D1Factory, ID2D1HwndRenderTarget, ID2D1SolidColorBrush,
    ID2D1StrokeStyle, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
    D2D1_BITMAP_PROPERTIES, D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_ELLIPSE,
    D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_FEATURE_LEVEL_DEFAULT,
    D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_PRESENT_OPTIONS_NONE, D2D1_RENDER_TARGET_PROPERTIES,
    D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_NONE, D2D1_ROUNDED_RECT,
};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, IDWriteTextLayout,
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT, DWRITE_FONT_WEIGHT_BOLD, DWRITE_FONT_WEIGHT_MEDIUM,
    DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_TEXT_ALIGNMENT,
    DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_TEXT_ALIGNMENT_TRAILING,
    DWRITE_TEXT_METRICS, DWRITE_TRIMMING, DWRITE_TRIMMING_GRANULARITY_CHARACTER,
    DWRITE_WORD_WRAPPING_NO_WRAP,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R8G8B8A8_UNORM;
use windows_numerics::Vector2;

use crate::model::{IconBitmap, Model, Row};
use crate::theme::{Appearance, Color, Theme};

/// Primary UI font (DirectWrite cleanly resolves Segoe UI Variable on Windows 11
/// and falls back gracefully on Windows 10).
const FONT_FAMILY: &str = "Segoe UI";

/// Maximum GPU-cached application icon bitmaps to keep RAM strictly bounded.
const MAX_CACHED_BITMAPS: usize = 128;

/// Draws the palette onto a window in crisp 1:1 DIPs.
pub struct Renderer {
    target: ID2D1HwndRenderTarget,
    dwrite: IDWriteFactory,
    name_format: IDWriteTextFormat,
    detail_format: IDWriteTextFormat,
    section_format: IDWriteTextFormat,
    pill_format: IDWriteTextFormat,
    footer_format: IDWriteTextFormat,
    keycap_format: IDWriteTextFormat,
    icon_format: IDWriteTextFormat,
    calc_expr_format: IDWriteTextFormat,
    calc_answer_format: IDWriteTextFormat,
    calc_answer_med_format: IDWriteTextFormat,
    calc_answer_sm_format: IDWriteTextFormat,
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
            DWRITE_FONT_WEIGHT_MEDIUM,
            14.0,
            DWRITE_TEXT_ALIGNMENT_LEADING,
        )?;
        let detail_format = dwrite_format(
            &dwrite,
            FONT_FAMILY,
            DWRITE_FONT_WEIGHT_NORMAL,
            12.0,
            DWRITE_TEXT_ALIGNMENT_TRAILING,
        )?;
        let section_format = dwrite_format(
            &dwrite,
            FONT_FAMILY,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            11.0,
            DWRITE_TEXT_ALIGNMENT_LEADING,
        )?;
        let pill_format = dwrite_format(
            &dwrite,
            FONT_FAMILY,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            11.5,
            DWRITE_TEXT_ALIGNMENT_CENTER,
        )?;
        let footer_format = dwrite_format(
            &dwrite,
            FONT_FAMILY,
            DWRITE_FONT_WEIGHT_NORMAL,
            11.5,
            DWRITE_TEXT_ALIGNMENT_LEADING,
        )?;
        let keycap_format = dwrite_format(
            &dwrite,
            FONT_FAMILY,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            10.5,
            DWRITE_TEXT_ALIGNMENT_CENTER,
        )?;
        let icon_format = dwrite_format(
            &dwrite,
            FONT_FAMILY,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            12.5,
            DWRITE_TEXT_ALIGNMENT_CENTER,
        )?;
        let calc_expr_format = dwrite_format(
            &dwrite,
            FONT_FAMILY,
            DWRITE_FONT_WEIGHT_NORMAL,
            13.5,
            DWRITE_TEXT_ALIGNMENT_LEADING,
        )?;
        let calc_answer_format = dwrite_format(
            &dwrite,
            FONT_FAMILY,
            DWRITE_FONT_WEIGHT_BOLD,
            27.0,
            DWRITE_TEXT_ALIGNMENT_TRAILING,
        )?;
        let calc_answer_med_format = dwrite_format(
            &dwrite,
            FONT_FAMILY,
            DWRITE_FONT_WEIGHT_BOLD,
            21.0,
            DWRITE_TEXT_ALIGNMENT_TRAILING,
        )?;
        let calc_answer_sm_format = dwrite_format(
            &dwrite,
            FONT_FAMILY,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            17.5,
            DWRITE_TEXT_ALIGNMENT_TRAILING,
        )?;

        let renderer = Self {
            target,
            dwrite,
            name_format,
            detail_format,
            section_format,
            pill_format,
            footer_format,
            keycap_format,
            icon_format,
            calc_expr_format,
            calc_answer_format,
            calc_answer_med_format,
            calc_answer_sm_format,
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
            &self.section_format,
            &self.footer_format,
            &self.calc_expr_format,
            &self.calc_answer_format,
            &self.calc_answer_med_format,
            &self.calc_answer_sm_format,
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
    pub fn draw(&mut self, model: &Model, theme: &Theme) {
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

        // 1. Header search icon (clean magnifying glass — handle starts from the rim!).
        self.draw_search_glyph(theme, 16.0, metrics.field_height / 2.0);

        // Optional header mode pill on the right (e.g., "Settings").
        if let Some(badge) = model.header_badge.as_deref() {
            if !badge.is_empty() {
                self.draw_pill_right(
                    badge,
                    width - 14.0,
                    metrics.field_height / 2.0,
                    theme.accent.with_alpha(0.16),
                    theme.accent.with_alpha(0.40),
                    theme.accent,
                );
            }
        }

        // Full-width subtle header divider hairline.
        self.fill_rect(
            0.0,
            metrics.field_height,
            width,
            metrics.field_height + 1.0,
            theme.border,
        );

        // 2. Scrollable list viewport (clipped cleanly between list_top and list_bottom).
        let viewport_clip = D2D_RECT_F {
            left: 0.0,
            top: layout.list_top,
            right: width,
            bottom: layout.list_bottom,
        };
        unsafe {
            self.target
                .PushAxisAlignedClip(&viewport_clip, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
        }

        if model.rows.is_empty() {
            let empty_text = if model.query.trim().is_empty() {
                "Start typing to search apps, commands, or calculate…"
            } else {
                "No matching results"
            };
            if let Some(text_layout) =
                self.layout(empty_text, &self.footer_format, width - pad * 4.0, 20.0)
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

            // Section headers.
            for section in &layout.sections {
                let top = layout.list_top + section.y - scroll;
                if top + section.height < layout.list_top || top > layout.list_bottom {
                    continue;
                }
                if let Some(text_layout) = self.layout(
                    &section.title,
                    &self.section_format,
                    width - pad * 2.0 - 20.0,
                    section.height,
                ) {
                    if let Some(brush) = self.brush(theme.text_dim.with_alpha(0.85)) {
                        unsafe {
                            self.target.DrawTextLayout(
                                Vector2 {
                                    X: pad + 10.0,
                                    Y: top + 6.0,
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
                        width - right_margin,
                        item.height,
                        is_selected,
                    );
                } else {
                    self.draw_standard_row(
                        row,
                        theme,
                        pad,
                        top,
                        width - right_margin,
                        item.height,
                        is_selected,
                        is_hovered,
                    );
                }
            }

            // Slim rounded scrollbar when content overflows.
            if show_scrollbar {
                let track_h = layout.viewport_height() - 8.0;
                if track_h > 20.0 {
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
            }
        }

        unsafe {
            self.target.PopAxisAlignedClip();
        }

        // 3. Recessed Footer Bar with YAL emblem + result count + drawn keycap hints.
        let footer_top = height - metrics.footer_height;
        self.fill_rect(0.0, footer_top, width, height, theme.background_secondary);
        self.fill_rect(0.0, footer_top, width, footer_top + 1.0, theme.border);
        self.draw_footer(model, theme, footer_top, width, metrics.footer_height);

        // 4. Crisp 1px inner-edge highlight frame so the window pops cleanly on any wallpaper.
        let frame_rect = D2D_RECT_F {
            left: 0.5,
            top: 0.5,
            right: (width - 0.5).max(1.0),
            bottom: (height - 0.5).max(1.0),
        };
        if let Some(border) = self.brush(theme.border) {
            unsafe {
                self.target.DrawRectangle(&frame_rect, &border, 1.0, None);
            }
        }

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
    ) {
        let metrics = theme.metrics;
        let row_rect = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left,
                top: top + 1.0,
                right,
                bottom: top + row_height - 1.0,
            },
            radiusX: 8.0,
            radiusY: 8.0,
        };

        if is_selected {
            if let Some(sel) = self.brush(theme.selection) {
                unsafe {
                    self.target.FillRoundedRectangle(&row_rect, &sel);
                }
            }
            let rim_color = match theme.appearance {
                Appearance::Dark => Color::rgba(0xff, 0xff, 0xff, 0x16),
                Appearance::Light => Color::rgba(0x0f, 0x17, 0x2a, 0x14),
            };
            if let Some(rim) = self.brush(rim_color) {
                unsafe {
                    self.target.DrawRoundedRectangle(&row_rect, &rim, 1.0, None);
                }
            }
            // Sleek vertical accent pill on the left edge of the selected row.
            let bar = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: left + 3.0,
                    top: top + 10.0,
                    right: left + 6.0,
                    bottom: top + row_height - 10.0,
                },
                radiusX: 1.5,
                radiusY: 1.5,
            };
            if let Some(accent) = self.brush(theme.accent) {
                unsafe {
                    self.target.FillRoundedRectangle(&bar, &accent);
                }
            }
        } else if is_hovered {
            if let Some(hover) = self.brush(theme.hover) {
                unsafe {
                    self.target.FillRoundedRectangle(&row_rect, &hover);
                }
            }
        }

        // Row Icon: real Windows GPU-cached bitmap when available, else soft-tinted squircle badge.
        let icon_size = metrics.icon_size;
        let icon_left = left + 13.0;
        let icon_top = top + (row_height - icon_size) / 2.0;
        let mut drew_bitmap = false;

        if let (Some(key), Some(bitmap_data)) = (row.icon_key.as_deref(), row.bitmap.as_deref()) {
            if let Some(gpu_bitmap) = self.cached_bitmap(key, bitmap_data) {
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
                drew_bitmap = true;
            }
        }

        if !drew_bitmap {
            let (glyph, tint) = match row.icon.as_ref() {
                Some(icon) => (icon.letter.as_str(), icon.color),
                None => ("·", theme.accent),
            };
            let tile = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: icon_left,
                    top: icon_top,
                    right: icon_left + icon_size,
                    bottom: icon_top + icon_size,
                },
                radiusX: 6.5,
                radiusY: 6.5,
            };
            let fill_alpha = match theme.appearance {
                Appearance::Dark => 0.18,
                Appearance::Light => 0.13,
            };
            if let Some(bg) = self.brush(tint.with_alpha(fill_alpha)) {
                unsafe {
                    self.target.FillRoundedRectangle(&tile, &bg);
                }
            }
            if let Some(ring) = self.brush(tint.with_alpha(0.35)) {
                unsafe {
                    self.target.DrawRoundedRectangle(&tile, &ring, 1.0, None);
                }
            }
            if let Some(layout) = self.layout(glyph, &self.icon_format, icon_size, icon_size) {
                if let Some(fg) = self.brush(tint) {
                    unsafe {
                        self.target.DrawTextLayout(
                            Vector2 {
                                X: icon_left,
                                Y: icon_top + (icon_size - 16.5) / 2.0,
                            },
                            &layout,
                            &fg,
                            D2D1_DRAW_TEXT_OPTIONS_CLIP,
                        );
                    }
                }
            }
        }

        // Right-aligned shortcut / active value pill (e.g. "Alt+Space", "Dark", "Ctrl+,").
        let mut cursor_right = right - 12.0;
        if !row.shortcut.is_empty() {
            let pill_bg = if is_selected {
                theme.accent.with_alpha(0.20)
            } else {
                match theme.appearance {
                    Appearance::Dark => Color::rgba(0xff, 0xff, 0xff, 0x0e),
                    Appearance::Light => Color::rgba(0x0f, 0x17, 0x2a, 0x0b),
                }
            };
            let pill_border = if is_selected {
                theme.accent.with_alpha(0.45)
            } else {
                theme.border
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
                pill_border,
                pill_fg,
            );
            cursor_right -= pill_w + 10.0;
        }

        // Row primary title & right-aligned muted detail.
        let text_left = icon_left + icon_size + 12.0;
        let available = (cursor_right - text_left).max(0.0);

        let detail_w = if !row.detail.is_empty() && available > 120.0 {
            let max_detail = (available * 0.46).min(240.0);
            if let Some(layout) = self.layout(&row.detail, &self.detail_format, max_detail, 20.0) {
                let measured = measure_width(&layout).min(max_detail);
                if let Some(brush) = self.brush(theme.text_dim) {
                    unsafe {
                        self.target.DrawTextLayout(
                            Vector2 {
                                X: cursor_right - max_detail,
                                Y: top + (row_height - 16.5) / 2.0,
                            },
                            &layout,
                            &brush,
                            D2D1_DRAW_TEXT_OPTIONS_CLIP,
                        );
                    }
                }
                measured + 12.0
            } else {
                0.0
            }
        } else {
            0.0
        };

        let title_width = (available - detail_w).max(0.0);
        if let Some(layout) = self.layout(&row.name, &self.name_format, title_width, 22.0) {
            if let Some(brush) = self.brush(theme.text) {
                unsafe {
                    self.target.DrawTextLayout(
                        Vector2 {
                            X: text_left,
                            Y: top + (row_height - 19.0) / 2.0,
                        },
                        &layout,
                        &brush,
                        D2D1_DRAW_TEXT_OPTIONS_CLIP,
                    );
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
                top: top + 2.0,
                right,
                bottom: top + height - 2.0,
            },
            radiusX: 10.0,
            radiusY: 10.0,
        };
        let fill = if is_selected {
            theme.selection
        } else {
            theme.background_secondary
        };
        if let Some(bg) = self.brush(fill) {
            unsafe {
                self.target.FillRoundedRectangle(&card, &bg);
            }
        }
        let border_col = if is_selected {
            theme.accent.with_alpha(0.50)
        } else {
            theme.border
        };
        if let Some(border) = self.brush(border_col) {
            unsafe {
                self.target.DrawRoundedRectangle(&card, &border, 1.0, None);
            }
        }

        // Subtle center divider.
        let mid_x = left + (right - left) * 0.45;
        self.fill_rect(
            mid_x,
            top + 16.0,
            mid_x + 1.0,
            top + height - 16.0,
            theme.border,
        );

        // Left side: optional live color swatch + expression + badge pill.
        let mut expr_left = left + 16.0;
        if let Some([r, g, b, a]) = row.color_swatch {
            let sw = 44.0_f32;
            let sw_top = top + (height - sw) / 2.0;
            let swatch_rect = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: expr_left,
                    top: sw_top,
                    right: expr_left + sw,
                    bottom: sw_top + sw,
                },
                radiusX: 10.0,
                radiusY: 10.0,
            };
            // Subtle checkerboard base when alpha < 255 so transparency is visible.
            if a < 255 {
                if let Some(base) = self.brush(Color::rgb(0x40, 0x42, 0x4c)) {
                    unsafe {
                        self.target.FillRoundedRectangle(&swatch_rect, &base);
                    }
                }
                let cell = sw / 4.0;
                if let Some(light) = self.brush(Color::rgb(0x74, 0x78, 0x86)) {
                    for gy in 0..4 {
                        for gx in 0..4 {
                            if (gx + gy) % 2 == 0 {
                                let tile = D2D_RECT_F {
                                    left: expr_left + gx as f32 * cell + 2.0,
                                    top: sw_top + gy as f32 * cell + 2.0,
                                    right: (expr_left + (gx + 1) as f32 * cell - 1.0)
                                        .min(expr_left + sw - 2.0),
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
            if let Some(ring) = self.brush(Color::rgba(0xff, 0xff, 0xff, 0x35)) {
                unsafe {
                    self.target
                        .DrawRoundedRectangle(&swatch_rect, &ring, 1.2, None);
                }
            }
            expr_left += sw + 14.0;
        }

        let expr_w = (mid_x - expr_left - 12.0).max(0.0);
        if let Some(layout) = self.layout(&row.detail, &self.calc_expr_format, expr_w, 22.0) {
            if let Some(brush) = self.brush(theme.text_dim) {
                unsafe {
                    self.target.DrawTextLayout(
                        Vector2 {
                            X: expr_left,
                            Y: top + 20.0,
                        },
                        &layout,
                        &brush,
                        D2D1_DRAW_TEXT_OPTIONS_CLIP,
                    );
                }
            }
        }
        let badge = if row.badge.is_empty() {
            "Calculator"
        } else {
            row.badge.as_str()
        };
        self.draw_pill_left(
            badge,
            expr_left,
            top + height - 25.0,
            theme.accent.with_alpha(0.18),
            theme.accent.with_alpha(0.40),
            theme.accent,
        );

        // Right side: big crisp bold answer (auto-fitted so long strings like
        // OKLCH colors or ISO timestamps never clip) + subtle Copy hint.
        let ans_left = mid_x + 14.0;
        let ans_w = (right - 18.0 - ans_left).max(0.0);
        let (ans_format, ans_y_offset) = {
            let probe = self.layout(&row.name, &self.calc_answer_format, 2000.0, 40.0);
            let fits_large = probe.as_ref().is_some_and(|l| measure_width(l) <= ans_w);
            if fits_large {
                (&self.calc_answer_format, 16.0_f32)
            } else {
                let probe_med = self.layout(&row.name, &self.calc_answer_med_format, 2000.0, 34.0);
                let fits_med = probe_med
                    .as_ref()
                    .is_some_and(|l| measure_width(l) <= ans_w);
                if fits_med {
                    (&self.calc_answer_med_format, 21.0_f32)
                } else {
                    (&self.calc_answer_sm_format, 24.0_f32)
                }
            }
        };
        if let Some(layout) = self.layout(&row.name, ans_format, ans_w, 40.0) {
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
        if let Some(hint) = self.layout("Enter to copy", &self.detail_format, ans_w, 18.0) {
            if let Some(brush) = self.brush(theme.text_dim) {
                unsafe {
                    self.target.DrawTextLayout(
                        Vector2 {
                            X: ans_left,
                            Y: top + height - 28.0,
                        },
                        &hint,
                        &brush,
                        D2D1_DRAW_TEXT_OPTIONS_CLIP,
                    );
                }
            }
        }
    }

    fn draw_footer(&mut self, model: &Model, theme: &Theme, top: f32, width: f32, height: f32) {
        let mid_y = top + height / 2.0;

        // Left: sleek geometric YAL emblem (rounded square + inner highlight bar).
        let logo_x = 14.0;
        let logo_y = mid_y - 7.5;
        let logo = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: logo_x,
                top: logo_y,
                right: logo_x + 15.0,
                bottom: logo_y + 15.0,
            },
            radiusX: 4.0,
            radiusY: 4.0,
        };
        if let Some(bg) = self.brush(theme.accent.with_alpha(0.22)) {
            unsafe {
                self.target.FillRoundedRectangle(&logo, &bg);
            }
        }
        if let Some(ring) = self.brush(theme.accent.with_alpha(0.60)) {
            unsafe {
                self.target.DrawRoundedRectangle(&logo, &ring, 1.0, None);
            }
        }
        let bar = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: logo_x + 3.5,
                top: logo_y + 6.0,
                right: logo_x + 11.5,
                bottom: logo_y + 9.0,
            },
            radiusX: 1.2,
            radiusY: 1.2,
        };
        if let Some(accent) = self.brush(theme.accent) {
            unsafe {
                self.target.FillRoundedRectangle(&bar, &accent);
            }
        }

        let status = model
            .result_count
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(&model.footer);
        if !status.is_empty() {
            if let Some(layout) = self.layout(status, &self.footer_format, 260.0, 18.0) {
                if let Some(brush) = self.brush(theme.text_dim) {
                    unsafe {
                        self.target.DrawTextLayout(
                            Vector2 {
                                X: logo_x + 23.0,
                                Y: mid_y - 8.0,
                            },
                            &layout,
                            &brush,
                            D2D1_DRAW_TEXT_OPTIONS_CLIP,
                        );
                    }
                }
            }
        }

        // Right: crisp drawn keycap hints.
        let mut cursor_right = width - 14.0;
        let keycap_bg = match theme.appearance {
            Appearance::Dark => Color::rgba(0xff, 0xff, 0xff, 0x0e),
            Appearance::Light => Color::rgba(0x0f, 0x17, 0x2a, 0x0b),
        };
        for (keycap, label) in model.footer_hints.iter().rev() {
            if let Some(label_layout) = self.layout(label, &self.footer_format, 120.0, 18.0) {
                let label_w = measure_width(&label_layout);
                let label_x = cursor_right - label_w;
                if let Some(brush) = self.brush(theme.text_dim) {
                    unsafe {
                        self.target.DrawTextLayout(
                            Vector2 {
                                X: label_x,
                                Y: mid_y - 8.0,
                            },
                            &label_layout,
                            &brush,
                            D2D1_DRAW_TEXT_OPTIONS_CLIP,
                        );
                    }
                }
                cursor_right = label_x - 6.0;
            }

            if let Some(cap_layout) = self.layout(keycap, &self.keycap_format, 80.0, 16.0) {
                let cap_text_w = measure_width(&cap_layout);
                let cap_w = (cap_text_w + 10.0).max(20.0);
                let cap_h = 19.0;
                let cap_left = cursor_right - cap_w;
                let cap_top = mid_y - cap_h / 2.0;
                let cap_rect = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: cap_left,
                        top: cap_top,
                        right: cursor_right,
                        bottom: cap_top + cap_h,
                    },
                    radiusX: 4.5,
                    radiusY: 4.5,
                };
                if let Some(bg) = self.brush(keycap_bg) {
                    unsafe {
                        self.target.FillRoundedRectangle(&cap_rect, &bg);
                    }
                }
                if let Some(border) = self.brush(theme.border) {
                    unsafe {
                        self.target
                            .DrawRoundedRectangle(&cap_rect, &border, 1.0, None);
                    }
                }
                if let Some(centered) = self.layout(keycap, &self.keycap_format, cap_w, 16.0) {
                    if let Some(fg) = self.brush(theme.text) {
                        unsafe {
                            self.target.DrawTextLayout(
                                Vector2 {
                                    X: cap_left,
                                    Y: cap_top + 2.0,
                                },
                                &centered,
                                &fg,
                                D2D1_DRAW_TEXT_OPTIONS_CLIP,
                            );
                        }
                    }
                }
                cursor_right = cap_left - 14.0;
            }
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

    fn draw_pill_left(
        &mut self,
        text: &str,
        left: f32,
        center_y: f32,
        bg: Color,
        border: Color,
        fg: Color,
    ) -> f32 {
        let Some(probe) = self.layout(text, &self.pill_format, 200.0, 18.0) else {
            return 0.0;
        };
        let text_w = measure_width(&probe);
        let pill_w = (text_w + 16.0).max(32.0);
        let pill_h = 21.0;
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
            radiusX: 6.0,
            radiusY: 6.0,
        };
        if let Some(brush) = self.brush(bg) {
            unsafe {
                self.target.FillRoundedRectangle(&rect, &brush);
            }
        }
        if let Some(brush) = self.brush(border) {
            unsafe {
                self.target.DrawRoundedRectangle(&rect, &brush, 1.0, None);
            }
        }
        if let Some(layout) = self.layout(text, &self.pill_format, width, height) {
            if let Some(brush) = self.brush(fg) {
                unsafe {
                    self.target.DrawTextLayout(
                        Vector2 {
                            X: left,
                            Y: top + (height - 16.0) / 2.0,
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

    /// Clean magnifying glass icon with the handle starting from the rim (never inside the circle).
    fn draw_search_glyph(&mut self, theme: &Theme, left: f32, middle: f32) {
        let radius = 6.5_f32;
        let center = Vector2 {
            X: left + radius + 1.0,
            Y: middle - 1.5,
        };
        let ellipse = D2D1_ELLIPSE {
            point: center,
            radiusX: radius,
            radiusY: radius,
        };
        let diag = std::f32::consts::FRAC_1_SQRT_2;
        let start = Vector2 {
            X: center.X + radius * diag,
            Y: center.Y + radius * diag,
        };
        let end = Vector2 {
            X: start.X + 4.8,
            Y: start.Y + 4.8,
        };
        if let Some(brush) = self.brush(theme.text_dim) {
            unsafe {
                self.target
                    .DrawEllipse(&ellipse, &brush, 1.75, None::<&ID2D1StrokeStyle>);
                self.target
                    .DrawLine(start, end, &brush, 1.75, None::<&ID2D1StrokeStyle>);
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
