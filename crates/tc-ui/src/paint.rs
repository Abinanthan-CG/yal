//! The palette's paint: Direct2D shapes and DirectWrite text.
//!
//! The renderer owns no window state. It draws a [`Model`] with a [`Theme`]
//! onto a target bound to one window handle.

use windows::core::PCWSTR;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct2D::{
    Common::{
        D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT, D2D_RECT_F, D2D_SIZE_U,
    },
    D2D1CreateFactory, ID2D1Factory, ID2D1HwndRenderTarget, ID2D1SolidColorBrush, ID2D1StrokeStyle,
    D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_ELLIPSE, D2D1_FACTORY_TYPE_SINGLE_THREADED,
    D2D1_FEATURE_LEVEL_DEFAULT, D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_PRESENT_OPTIONS_NONE,
    D2D1_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_NONE,
    D2D1_ROUNDED_RECT,
};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, IDWriteTextLayout,
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_WEIGHT_SEMI_BOLD,
    DWRITE_TEXT_ALIGNMENT, DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING,
    DWRITE_TEXT_ALIGNMENT_TRAILING, DWRITE_TRIMMING, DWRITE_TRIMMING_GRANULARITY_CHARACTER,
    DWRITE_WORD_WRAPPING_NO_WRAP,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R8G8B8A8_UNORM;
use windows_numerics::Vector2;

use crate::model::Model;
use crate::theme::{Color, Metrics, Theme};

/// The font the palette draws its text with.
const FONT_FAMILY: &str = "Segoe UI";

/// Draws the palette onto a window.
pub struct Renderer {
    target: ID2D1HwndRenderTarget,
    dwrite: IDWriteFactory,
    name_format: IDWriteTextFormat,
    detail_format: IDWriteTextFormat,
    footer_format: IDWriteTextFormat,
    icon_format: IDWriteTextFormat,
    brushes: Vec<(Color, ID2D1SolidColorBrush)>,
}

impl Renderer {
    /// Binds a renderer to a window.
    pub fn new(hwnd: HWND) -> Result<Self, String> {
        let factory: ID2D1Factory = unsafe {
            D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)
                .map_err(|error| format!("Direct2D could not start: {error}"))?
        };
        let dwrite: IDWriteFactory = unsafe {
            DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)
                .map_err(|error| format!("DirectWrite could not start: {error}"))?
        };

        let properties = D2D1_RENDER_TARGET_PROPERTIES {
            r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_R8G8B8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 0.0,
            dpiY: 0.0,
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
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            13.5,
            DWRITE_TEXT_ALIGNMENT_LEADING,
        )?;
        let detail_format = dwrite_format(
            &dwrite,
            FONT_FAMILY,
            DWRITE_FONT_WEIGHT_NORMAL,
            12.0,
            DWRITE_TEXT_ALIGNMENT_TRAILING,
        )?;
        let footer_format = dwrite_format(
            &dwrite,
            FONT_FAMILY,
            DWRITE_FONT_WEIGHT_NORMAL,
            11.5,
            DWRITE_TEXT_ALIGNMENT_LEADING,
        )?;
        let icon_format = dwrite_format(
            &dwrite,
            FONT_FAMILY,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            11.0,
            DWRITE_TEXT_ALIGNMENT_CENTER,
        )?;

        let renderer = Self {
            target,
            dwrite,
            name_format,
            detail_format,
            footer_format,
            icon_format,
            brushes: Vec::new(),
        };
        renderer.trim_ellipsis()?;
        Ok(renderer)
    }

    /// Makes every format cut its text with an ellipsis.
    fn trim_ellipsis(&self) -> Result<(), String> {
        let trimming = DWRITE_TRIMMING {
            granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER,
            delimiter: 0,
            delimiterCount: 0,
        };
        for format in [
            &self.name_format,
            &self.detail_format,
            &self.footer_format,
            &self.icon_format,
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

    /// The target's size in device-independent pixels.
    pub fn size(&self) -> (f32, f32) {
        let size = unsafe { self.target.GetSize() };
        (size.width, size.height)
    }

    /// The factor between the design measurements and the drawing surface.
    ///
    /// Everything the palette draws is measured in the design's own units and
    /// multiplied by this, so the palette keeps its proportions on any display.
    pub fn scale(&self, theme: &Theme) -> f32 {
        let size = unsafe { self.target.GetSize() };
        if theme.metrics.width > 0.0 {
            size.width / theme.metrics.width
        } else {
            1.0
        }
    }

    /// Tells the target that the window changed size.
    pub fn resize(&self, width: u32, height: u32) {
        let size = D2D_SIZE_U { width, height };
        unsafe {
            let _ = self.target.Resize(&size);
        }
    }

    /// The brush for a colour, made once and kept.
    fn brush(&mut self, color: Color) -> Option<ID2D1SolidColorBrush> {
        if let Some((_, brush)) = self.brushes.iter().find(|(held, _)| *held == color) {
            return Some(brush.clone());
        }
        let brush = unsafe {
            self.target
                .CreateSolidColorBrush(&to_d2d(color), None)
                .ok()?
        };
        self.brushes.push((color, brush.clone()));
        Some(brush)
    }

    /// Draws the whole palette.
    pub fn draw(&mut self, model: &Model, theme: &Theme) {
        let design = theme.metrics;
        // The target knows how big the window is in device-independent pixels.
        // Everything is measured against the design size, so the palette keeps
        // its proportions on any display.
        let size = unsafe { self.target.GetSize() };
        let scale = self.scale(theme);
        let metrics = scaled(design, scale);
        let width = size.width;
        let height = size.height;
        let padding = metrics.padding;

        unsafe {
            self.target.BeginDraw();
            self.target.Clear(Some(&to_d2d(theme.background)));
        }

        // The search field: a rounded slab with a hairline around it.
        let field = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: padding,
                top: padding,
                right: width - padding,
                bottom: padding + metrics.field_height,
            },
            radiusX: metrics.radius,
            radiusY: metrics.radius,
        };
        if let Some(field_brush) = self.brush(theme.field) {
            unsafe {
                self.target.FillRoundedRectangle(&field, &field_brush);
            }
        }
        if let Some(border) = self.brush(theme.border) {
            unsafe {
                self.target.DrawRoundedRectangle(&field, &border, 1.0, None);
            }
        }

        // The search glyph, at the left of the field, where the text begins
        // after it.
        self.draw_search_glyph(theme, padding, padding + metrics.field_height / 2.0, scale);

        // The rows.
        let list_top = padding + metrics.field_height + metrics.list_gap;
        let room = height - list_top - padding - metrics.footer_height;
        let visible = (room / metrics.row_height).floor().max(0.0) as usize;
        for (index, row) in model.rows.iter().take(visible).enumerate() {
            let top = list_top + index as f32 * metrics.row_height;

            // The row under the pointer.
            if model.hovered == Some(index) && model.selected != index {
                let hovered = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: padding,
                        top,
                        right: width - padding,
                        bottom: top + metrics.row_height,
                    },
                    radiusX: 8.0,
                    radiusY: 8.0,
                };
                if let Some(hover) = self.brush(theme.hover) {
                    unsafe {
                        self.target.FillRoundedRectangle(&hovered, &hover);
                    }
                }
            }

            // The selected row: a slab with an accent bar down its left side.
            if index == model.selected {
                let selected = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: padding,
                        top,
                        right: width - padding,
                        bottom: top + metrics.row_height,
                    },
                    radiusX: 8.0,
                    radiusY: 8.0,
                };
                if let Some(selection) = self.brush(theme.selection) {
                    unsafe {
                        self.target.FillRoundedRectangle(&selected, &selection);
                    }
                }
                let bar_width = 3.0;
                let bar = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: padding,
                        top: top + 8.0,
                        right: padding + bar_width,
                        bottom: top + metrics.row_height - 8.0,
                    },
                    radiusX: bar_width / 2.0,
                    radiusY: bar_width / 2.0,
                };
                if let Some(accent) = self.brush(theme.accent) {
                    unsafe {
                        self.target.FillRoundedRectangle(&bar, &accent);
                    }
                }
            }

            let icon_left = padding + 10.0;
            let icon_top = top + (metrics.row_height - metrics.icon_size) / 2.0;
            match row.icon.as_ref() {
                Some(icon) => {
                    let tile = D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: icon_left,
                            top: icon_top,
                            right: icon_left + metrics.icon_size,
                            bottom: icon_top + metrics.icon_size,
                        },
                        radiusX: 6.0,
                        radiusY: 6.0,
                    };
                    if let Some(tile_brush) = self.brush(icon.color) {
                        unsafe {
                            self.target.FillRoundedRectangle(&tile, &tile_brush);
                        }
                    }
                    if let Some(layout) = self.layout(
                        &icon.letter,
                        &self.icon_format,
                        metrics.icon_size,
                        metrics.icon_text_size,
                    ) {
                        if let Some(brush) = self.brush(theme.on_icon) {
                            unsafe {
                                self.target.DrawTextLayout(
                                    Vector2 {
                                        X: icon_left,
                                        Y: icon_top
                                            + (metrics.icon_size - metrics.icon_text_size) / 2.0
                                            - 1.0,
                                    },
                                    &layout,
                                    &brush,
                                    D2D1_DRAW_TEXT_OPTIONS_CLIP,
                                );
                            }
                        }
                    }
                }
                None => {
                    let tile = D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: icon_left,
                            top: icon_top,
                            right: icon_left + metrics.icon_size,
                            bottom: icon_top + metrics.icon_size,
                        },
                        radiusX: 6.0,
                        radiusY: 6.0,
                    };
                    if let Some(tile_brush) = self.brush(theme.text_dim) {
                        unsafe {
                            self.target.FillRoundedRectangle(&tile, &tile_brush);
                        }
                    }
                }
            }

            let text_left = icon_left + metrics.icon_size + 12.0;
            let text_right = width - padding - 14.0;
            let text_width = (text_right - text_left - 12.0).max(0.0);

            if let Some(layout) =
                self.layout(&row.name, &self.name_format, text_width, metrics.name_size)
            {
                if let Some(brush) = self.brush(theme.text) {
                    unsafe {
                        self.target.DrawTextLayout(
                            Vector2 {
                                X: text_left,
                                Y: top + (metrics.row_height - metrics.name_size) / 2.0 - 1.0,
                            },
                            &layout,
                            &brush,
                            D2D1_DRAW_TEXT_OPTIONS_CLIP,
                        );
                    }
                }
            }

            if !row.detail.is_empty() {
                if let Some(layout) = self.layout(
                    &row.detail,
                    &self.detail_format,
                    text_width,
                    metrics.detail_size,
                ) {
                    if let Some(brush) = self.brush(theme.text_dim) {
                        unsafe {
                            self.target.DrawTextLayout(
                                Vector2 {
                                    X: text_left,
                                    Y: top + (metrics.row_height - metrics.detail_size) / 2.0,
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

        // The footer's hairline and its hint.
        let footer_top = height - padding - metrics.footer_height;
        let line = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: padding,
                top: footer_top,
                right: width - padding,
                bottom: footer_top + 1.0,
            },
            radiusX: 0.5,
            radiusY: 0.5,
        };
        if let Some(border) = self.brush(theme.border) {
            unsafe {
                self.target.FillRoundedRectangle(&line, &border);
            }
        }
        if !model.footer.is_empty() {
            if let Some(layout) = self.layout(
                &model.footer,
                &self.footer_format,
                width - padding * 2.0,
                metrics.footer_size,
            ) {
                if let Some(brush) = self.brush(theme.text_dim) {
                    unsafe {
                        self.target.DrawTextLayout(
                            Vector2 {
                                X: padding,
                                Y: footer_top + 8.0,
                            },
                            &layout,
                            &brush,
                            D2D1_DRAW_TEXT_OPTIONS_CLIP,
                        );
                    }
                }
            }
        }

        unsafe {
            let _ = self.target.EndDraw(None, None);
        }
    }

    /// Draws the magnifying glass at the left of the search field.
    fn draw_search_glyph(&mut self, theme: &Theme, left: f32, middle: f32, scale: f32) {
        let radius = 6.0 * scale;
        let center = Vector2 {
            X: left + 18.0 * scale,
            Y: middle - 1.0 * scale,
        };
        let ellipse = D2D1_ELLIPSE {
            point: center,
            radiusX: radius,
            radiusY: radius,
        };
        // The handle, from the rim of the glass out to the lower right.
        let handle = Vector2 {
            X: center.X + radius * 1.3,
            Y: center.Y + radius * 1.3,
        };
        let width = 1.4 * scale;
        if let Some(brush) = self.brush(theme.text_dim) {
            unsafe {
                self.target
                    .DrawEllipse(&ellipse, &brush, width, None::<&ID2D1StrokeStyle>);
                self.target
                    .DrawLine(center, handle, &brush, width, None::<&ID2D1StrokeStyle>);
            }
        }
    }

    /// A text layout for one string.
    fn layout(
        &self,
        text: &str,
        format: &IDWriteTextFormat,
        width: f32,
        size: f32,
    ) -> Option<IDWriteTextLayout> {
        if text.is_empty() || width <= 0.0 {
            return None;
        }
        let wide: Vec<u16> = text.encode_utf16().collect();
        unsafe {
            self.dwrite
                .CreateTextLayout(&wide, format, width, size * 2.0)
                .ok()
        }
    }
}

/// The measurements of a theme, scaled for the display the palette is on.
fn scaled(metrics: Metrics, scale: f32) -> Metrics {
    Metrics {
        width: metrics.width * scale,
        height: metrics.height * scale,
        padding: metrics.padding * scale,
        field_height: metrics.field_height * scale,
        field_inset: metrics.field_inset * scale,
        row_height: metrics.row_height * scale,
        icon_size: metrics.icon_size * scale,
        name_size: metrics.name_size * scale,
        detail_size: metrics.detail_size * scale,
        footer_size: metrics.footer_size * scale,
        icon_text_size: metrics.icon_text_size * scale,
        radius: metrics.radius * scale,
        list_gap: metrics.list_gap * scale,
        footer_height: metrics.footer_height * scale,
        max_rows: metrics.max_rows,
    }
}

/// A text format that does not wrap.
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

/// A theme colour in the shape Direct2D wants.
fn to_d2d(color: Color) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: color.red,
        g: color.green,
        b: color.blue,
        a: color.alpha,
    }
}
