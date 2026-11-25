//! The palette's paint: Direct2D shapes, GPU-cached Windows app icons, and
//! DirectWrite typography in pure 1:1 DIP coordinates.

use std::collections::HashMap;
use std::ffi::c_void;

use windows::core::{IUnknown, Interface, PCWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct2D::{
    Common::{
        D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT, D2D_RECT_F, D2D_SIZE_U,
    },
    D2D1CreateFactory, ID2D1Bitmap, ID2D1Factory, ID2D1HwndRenderTarget, ID2D1SolidColorBrush,
    ID2D1StrokeStyle, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
    D2D1_BITMAP_PROPERTIES, D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT,
    D2D1_ELLIPSE, D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_FEATURE_LEVEL_DEFAULT,
    D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_PRESENT_OPTIONS_NONE, D2D1_RENDER_TARGET_PROPERTIES,
    D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_NONE, D2D1_ROUNDED_RECT,
};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, IDWriteTextLayout,
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT, DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_FONT_WEIGHT_NORMAL,
    DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_TEXT_ALIGNMENT, DWRITE_TEXT_ALIGNMENT_CENTER,
    DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_TEXT_ALIGNMENT_TRAILING, DWRITE_TEXT_METRICS,
    DWRITE_TEXT_RANGE, DWRITE_TRIMMING, DWRITE_TRIMMING_GRANULARITY_CHARACTER,
    DWRITE_WORD_WRAPPING_NO_WRAP, DWRITE_WORD_WRAPPING_WRAP,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R8G8B8A8_UNORM;
use windows_numerics::Vector2;

use crate::model::{
    ActionMenuModel, DetailPane, GridModel, IconBitmap, Model, Row, ToastKind,
    ACTION_MENU_HEADER_H, ACTION_MENU_MAX_VISIBLE, ACTION_MENU_ROW_H, GRID_CELL_HEIGHT,
    MASTER_SPLIT_RATIO,
};
use crate::theme::{Appearance, Color, Theme};

/// Primary UI font (DirectWrite cleanly resolves Segoe UI Variable on Windows 11
/// and falls back gracefully on Windows 10).
const FONT_FAMILY: &str = "Segoe UI";
/// Windows color emoji font for the 2D Emoji & Symbol grid.
const EMOJI_FONT_FAMILY: &str = "Segoe UI Emoji";

/// Maximum GPU-cached application icon bitmaps to keep RAM strictly bounded.
const MAX_CACHED_BITMAPS: usize = 128;

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
    action_title_format: IDWriteTextFormat,
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
        let action_title_format = dwrite_format(
            &dwrite,
            FONT_FAMILY,
            DWRITE_FONT_WEIGHT_NORMAL,
            13.0,
            DWRITE_TEXT_ALIGNMENT_LEADING,
        )?;

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
            action_title_format,
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
            &self.action_title_format,
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

        // Optional header mode/filter pill on the right (e.g., "Settings", "Files · Images").
        if let Some(badge) = model.header_badge.as_deref() {
            if !badge.is_empty() {
                let badge_bg = match theme.appearance {
                    Appearance::Dark => Color::rgba(0xff, 0xff, 0xff, 0x16),
                    Appearance::Light => Color::rgba(0x00, 0x00, 0x00, 0x10),
                };
                self.draw_pill_right(
                    badge,
                    width - 16.0,
                    metrics.field_height / 2.0,
                    badge_bg,
                    Color::rgba(0, 0, 0, 0),
                    theme.text,
                );
            }
        }

        // Compact single-bar mode: render only the borderless search bar (no divider, list, or footer).
        if model.compact
            && model.action_menu.is_none()
            && model.detail.is_none()
            && model.grid.is_none()
        {
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
                self.draw_detail_pane(detail, theme, split_x + 1.0, pane_top, width, pane_bottom);
            }
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
            if let Some(sel) = self.brush(theme.selection) {
                unsafe {
                    self.target.FillRoundedRectangle(&row_rect, &sel);
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
        }

        if !drew_bitmap {
            let (glyph, tint) = match row.icon.as_ref() {
                Some(icon) => (icon.letter.as_str(), icon.color),
                None => ("·", theme.text_dim),
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
            if let Some(layout) = self.layout(glyph, &self.icon_format, icon_size, icon_size) {
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
            if row.shortcut.contains('+') {
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
                    if let Some(brush) = self.brush(theme.text_dim) {
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
            let sw_h = (preview_h * 0.58).clamp(72.0, 140.0);
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
            if !detail.body.is_empty() {
                if let Some(layout) = self.layout(
                    &detail.body,
                    &self.detail_body_format,
                    content_w,
                    (preview_bottom - (preview_top + sw_h + 12.0)).max(24.0),
                ) {
                    if let Some(fg) = self.brush(theme.text) {
                        unsafe {
                            self.target.DrawTextLayout(
                                Vector2 {
                                    X: left + pad,
                                    Y: preview_top + sw_h + 12.0,
                                },
                                &layout,
                                &fg,
                                D2D1_DRAW_TEXT_OPTIONS_CLIP,
                            );
                        }
                    }
                }
            }
        } else if let (Some(key), Some(img)) =
            (detail.image_key.as_deref(), detail.image.as_deref())
        {
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
            // Subtle inner preview card for text/code/notes/clipboard body.
            let card = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: left + pad,
                    top: preview_top,
                    right: right - pad,
                    bottom: preview_bottom,
                },
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
            let text_content = if detail.body.trim().is_empty() {
                detail.title.as_str()
            } else {
                detail.body.as_str()
            };
            if let Some(layout) = self.layout(
                text_content,
                &self.detail_body_format,
                (content_w - 24.0).max(20.0),
                (preview_h - 20.0).max(20.0),
            ) {
                if let Some(fg) = self.brush(theme.text) {
                    unsafe {
                        self.target.DrawTextLayout(
                            Vector2 {
                                X: left + pad + 12.0,
                                Y: preview_top + 10.0,
                            },
                            &layout,
                            &fg,
                            D2D1_DRAW_TEXT_OPTIONS_CLIP | D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT,
                        );
                    }
                }
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

        let start = menu.scroll_start(filtered.len());
        let visible = filtered
            .iter()
            .enumerate()
            .skip(start)
            .take(ACTION_MENU_MAX_VISIBLE);
        let mut last_sec: Option<&str> = None;

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

    /// Clean minimal magnifying glass icon in the left searchbar slot
    /// (neutral tertiary stroke, optically centered with the 18px search input).
    fn draw_search_glyph(&mut self, theme: &Theme, left: f32, middle: f32) {
        let radius = 6.2_f32;
        let center = Vector2 {
            X: left + radius + 1.5,
            Y: middle - 1.2,
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
            X: start.X + 4.4,
            Y: start.Y + 4.4,
        };
        let col = match theme.appearance {
            Appearance::Dark => Color::rgba(0xff, 0xff, 0xff, 0x70),
            Appearance::Light => Color::rgba(0x00, 0x00, 0x00, 0x66),
        };
        if let Some(brush) = self.brush(col) {
            unsafe {
                self.target
                    .DrawEllipse(&ellipse, &brush, 1.6, None::<&ID2D1StrokeStyle>);
                self.target
                    .DrawLine(start, end, &brush, 1.6, None::<&ID2D1StrokeStyle>);
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
}
