//! The palette's colours and measurements.
//!
//! The measurements are in device-independent pixels, so the same numbers hold
//! on every screen; the window scales them by the monitor's DPI.

/// A colour, with alpha, in the 0..1 range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
    pub alpha: f32,
}

impl Color {
    /// An opaque colour from three bytes.
    pub const fn rgb(red: u8, green: u8, blue: u8) -> Self {
        Self {
            red: red as f32 / 255.0,
            green: green as f32 / 255.0,
            blue: blue as f32 / 255.0,
            alpha: 1.0,
        }
    }

    /// A colour from three bytes and an alpha byte.
    pub const fn rgba(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red: red as f32 / 255.0,
            green: green as f32 / 255.0,
            blue: blue as f32 / 255.0,
            alpha: alpha as f32 / 255.0,
        }
    }
}

/// Which of the two appearances the palette is wearing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Appearance {
    Light,
    Dark,
}

/// The measurements the palette draws with, in device-independent pixels.
#[derive(Clone, Copy, Debug)]
pub struct Metrics {
    /// The width of the palette window.
    pub width: f32,
    /// The height of the palette window. This is the tallest the palette may
    /// be; a shorter list gets a shorter window.
    pub height: f32,
    /// The distance from the window's edge to its content.
    pub padding: f32,
    /// The height of the search field.
    pub field_height: f32,
    /// The distance from the field's left edge to the text inside it. It
    /// leaves room for the search glyph.
    pub field_inset: f32,
    /// The height of one result row.
    pub row_height: f32,
    /// The height of the tile that leads a row.
    pub icon_size: f32,
    /// The size of a row's name.
    pub name_size: f32,
    /// The size of a row's detail text.
    pub detail_size: f32,
    /// The size of the footer line.
    pub footer_size: f32,
    /// The size of the letter in a tile.
    pub icon_text_size: f32,
    /// The corner radius of the window and of the selected row.
    pub radius: f32,
    /// The space between the search field and the first row.
    pub list_gap: f32,
    /// The height of the footer, hairline and hint together.
    pub footer_height: f32,
    /// The most rows the palette will show at once.
    pub max_rows: usize,
}

impl Default for Metrics {
    fn default() -> Self {
        Self {
            width: 680.0,
            height: 440.0,
            padding: 12.0,
            field_height: 44.0,
            field_inset: 34.0,
            row_height: 40.0,
            icon_size: 22.0,
            name_size: 13.5,
            detail_size: 12.0,
            footer_size: 11.5,
            icon_text_size: 11.0,
            radius: 10.0,
            list_gap: 8.0,
            footer_height: 32.0,
            max_rows: 9,
        }
    }
}

/// Everything the palette draws with, for one appearance.
#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub appearance: Appearance,
    /// The window's background.
    pub background: Color,
    /// A second background, for the panels that sit on the first.
    pub background_secondary: Color,
    /// The background of the search field.
    pub field: Color,
    /// The colour of a row's name.
    pub text: Color,
    /// The colour of secondary text.
    pub text_dim: Color,
    /// The colour of the text on a tile.
    pub on_icon: Color,
    /// The fill of the selected row.
    pub selection: Color,
    /// The fill of the row under the pointer.
    pub hover: Color,
    /// The colour of the accent - the caret, the focus.
    pub accent: Color,
    /// The colour of the busy state.
    pub loader: Color,
    /// The hairline around the window.
    pub border: Color,
    /// The palette: the seven colours the rest of the interface draws from.
    pub palette: [Color; 7],
    pub metrics: Metrics,
}

/// How many colours a palette holds.
pub const PALETTE: usize = 7;

impl Theme {
    /// The dark appearance.
    pub fn dark() -> Self {
        Self {
            appearance: Appearance::Dark,
            background: Color::rgb(0x1d, 0x1d, 0x1f),
            background_secondary: Color::rgb(0x25, 0x25, 0x28),
            field: Color::rgb(0x2b, 0x2b, 0x2e),
            text: Color::rgb(0xf2, 0xf2, 0xf7),
            text_dim: Color::rgb(0x8e, 0x8e, 0x93),
            on_icon: Color::rgb(0xff, 0xff, 0xff),
            selection: Color::rgb(0x3a, 0x3a, 0x3d),
            hover: Color::rgba(0x4a, 0x8c, 0xff, 0x1f),
            accent: Color::rgb(0x4a, 0x8c, 0xff),
            loader: Color::rgb(0xd6, 0xae, 0xff),
            border: Color::rgba(0xff, 0xff, 0xff, 0x1a),
            palette: [
                Color::rgb(0xf8, 0x7e, 0x7e),
                Color::rgb(0xff, 0xb7, 0x65),
                Color::rgb(0xfe, 0xff, 0x7f),
                Color::rgb(0x7e, 0xec, 0xae),
                Color::rgb(0x72, 0xba, 0xfb),
                Color::rgb(0xce, 0x9d, 0xfa),
                Color::rgb(0xff, 0xa2, 0xd4),
            ],
            metrics: Metrics::default(),
        }
    }

    /// The light appearance.
    pub fn light() -> Self {
        Self {
            appearance: Appearance::Light,
            background: Color::rgb(0xf7, 0xf7, 0xf9),
            background_secondary: Color::rgb(0xee, 0xee, 0xf2),
            field: Color::rgb(0xff, 0xff, 0xff),
            text: Color::rgb(0x1c, 0x1c, 0x1e),
            text_dim: Color::rgb(0x7c, 0x7c, 0x86),
            on_icon: Color::rgb(0xff, 0xff, 0xff),
            selection: Color::rgb(0xe6, 0xe6, 0xeb),
            hover: Color::rgba(0x0a, 0x63, 0xd8, 0x14),
            accent: Color::rgb(0x0a, 0x63, 0xd8),
            loader: Color::rgb(0x8a, 0x5c, 0xd6),
            border: Color::rgba(0x00, 0x00, 0x00, 0x16),
            palette: [
                Color::rgb(0xd6, 0x3b, 0x3b),
                Color::rgb(0xc0, 0x6a, 0x14),
                Color::rgb(0x8a, 0x7a, 0x00),
                Color::rgb(0x2f, 0x8f, 0x5b),
                Color::rgb(0x1f, 0x6f, 0xbf),
                Color::rgb(0x7a, 0x4f, 0xd6),
                Color::rgb(0xc0, 0x43, 0x8f),
            ],
            metrics: Metrics::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_colour_holds_its_channels() {
        let color = Color::rgb(0x20, 0x40, 0x80);
        assert!((color.red - 0.12549).abs() < 0.001);
        assert!((color.green - 0.25098).abs() < 0.001);
        assert!((color.blue - 0.50196).abs() < 0.001);
        assert_eq!(color.alpha, 1.0);
    }

    #[test]
    fn an_alpha_colour_keeps_its_opacity() {
        let color = Color::rgba(0xff, 0xff, 0xff, 0x40);
        assert!((color.alpha - 0.25098).abs() < 0.001);
        assert_eq!(color.red, 1.0);
    }

    #[test]
    fn both_appearances_answer_every_colour() {
        for theme in [Theme::dark(), Theme::light()] {
            assert!(theme.background.alpha > 0.0);
            assert!(theme.field.alpha > 0.0);
            assert!(theme.text.alpha > 0.0);
            assert!(theme.text_dim.alpha > 0.0);
            assert!(theme.selection.alpha > 0.0);
            assert!(theme.accent.alpha > 0.0);
        }
    }

    #[test]
    fn the_appearances_do_not_look_alike() {
        assert_ne!(Theme::dark().background, Theme::light().background);
        assert_ne!(Theme::dark().text, Theme::light().text);
    }
}
