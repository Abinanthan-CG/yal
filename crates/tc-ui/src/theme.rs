//! The palette's colours and measurements.
//!
//! All measurements are in device-independent pixels (DIPs). Direct2D sets the
//! render target DPI to match the window's monitor so coordinates stay 1:1 DIPs
//! while rasterizing crisply at native hardware resolution.

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

    /// Same RGB with a custom opacity (`0.0..=1.0`).
    pub const fn with_alpha(self, alpha: f32) -> Self {
        Self {
            red: self.red,
            green: self.green,
            blue: self.blue,
            alpha,
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
    /// The reference height of the palette window.
    pub height: f32,
    /// Horizontal outer inset for list rows and footer content.
    pub padding: f32,
    /// The height of the top search header bar.
    pub field_height: f32,
    /// Left inset from window edge to the search text (leaves room for search icon).
    pub field_inset: f32,
    /// The height of one standard result row.
    pub row_height: f32,
    /// The size of the icon / badge that leads a row.
    pub icon_size: f32,
    /// The font size of a row's primary title.
    pub name_size: f32,
    /// The font size of a row's secondary detail text.
    pub detail_size: f32,
    /// The font size of the footer line.
    pub footer_size: f32,
    /// The font size of the glyph inside a fallback badge.
    pub icon_text_size: f32,
    /// The corner radius of rounded pills and cards.
    pub radius: f32,
    /// Breathing space between the header divider and list, and between list and footer.
    pub list_gap: f32,
    /// The height of the bottom footer bar.
    pub footer_height: f32,
    /// Maximum visible rows before scrolling.
    pub max_rows: usize,
}

impl Default for Metrics {
    fn default() -> Self {
        Self {
            width: 750.0,
            height: 475.0,
            padding: 8.0,
            field_height: 60.0,
            field_inset: 48.0,
            row_height: 38.0,
            icon_size: 22.0,
            name_size: 13.0,
            detail_size: 13.0,
            footer_size: 12.0,
            icon_text_size: 12.0,
            radius: 6.0,
            list_gap: 6.0,
            footer_height: 42.0,
            max_rows: 9,
        }
    }
}

/// Everything the palette draws with, for one appearance.
#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub appearance: Appearance,
    /// The window's main surface background.
    pub background: Color,
    /// Secondary surface for the recessed footer bar and calculator hero card.
    pub background_secondary: Color,
    /// The background of the search input control (matches header surface).
    pub field: Color,
    /// Primary text colour.
    pub text: Color,
    /// Muted secondary text colour.
    pub text_dim: Color,
    /// Text/symbol colour drawn on opaque icon badges.
    pub on_icon: Color,
    /// Fill of the currently selected row pill.
    pub selection: Color,
    /// Fill of the row under the mouse pointer.
    pub hover: Color,
    /// Accent colour for selection indicator, active badges, and highlights.
    pub accent: Color,
    /// Busy/loader accent colour.
    pub loader: Color,
    /// Hairline divider and subtle border colour.
    pub border: Color,
    /// Seven-colour accent palette for categories and badges.
    pub palette: [Color; 7],
    pub metrics: Metrics,
}

/// How many colours a palette holds.
pub const PALETTE: usize = 7;

impl Theme {
    /// Pure neutral charcoal Dark appearance (zero blue tint, calibrated to Raycast Dark).
    pub fn dark() -> Self {
        Self {
            appearance: Appearance::Dark,
            background: Color::rgb(0x1c, 0x1c, 0x1c),
            background_secondary: Color::rgb(0x1c, 0x1c, 0x1c),
            field: Color::rgb(0x1c, 0x1c, 0x1c),
            text: Color::rgb(0xff, 0xff, 0xff),
            text_dim: Color::rgba(0xff, 0xff, 0xff, 0x99),
            on_icon: Color::rgb(0xff, 0xff, 0xff),
            selection: Color::rgba(0xff, 0xff, 0xff, 0x1a),
            hover: Color::rgba(0xff, 0xff, 0xff, 0x0d),
            accent: Color::rgb(0x4f, 0xa3, 0xf8),
            loader: Color::rgb(0xff, 0xff, 0xff),
            border: Color::rgba(0xff, 0xff, 0xff, 0x1a),
            palette: [
                Color::rgb(0xff, 0x63, 0x63),
                Color::rgb(0xff, 0x92, 0x17),
                Color::rgb(0xff, 0xc5, 0x31),
                Color::rgb(0x59, 0xd4, 0x99),
                Color::rgb(0x56, 0xc2, 0xff),
                Color::rgb(0xa4, 0x85, 0xff),
                Color::rgb(0xcf, 0x2f, 0x98),
            ],
            metrics: Metrics::default(),
        }
    }

    /// Pure neutral Light appearance (calibrated to Raycast Light).
    pub fn light() -> Self {
        Self {
            appearance: Appearance::Light,
            background: Color::rgb(0xf6, 0xf6, 0xf6),
            background_secondary: Color::rgb(0xf6, 0xf6, 0xf6),
            field: Color::rgb(0xf6, 0xf6, 0xf6),
            text: Color::rgb(0x00, 0x00, 0x00),
            text_dim: Color::rgba(0x00, 0x00, 0x00, 0x99),
            on_icon: Color::rgb(0xff, 0xff, 0xff),
            selection: Color::rgba(0x00, 0x00, 0x00, 0x17),
            hover: Color::rgba(0x00, 0x00, 0x00, 0x0d),
            accent: Color::rgb(0x13, 0x8a, 0xf2),
            loader: Color::rgb(0x00, 0x00, 0x00),
            border: Color::rgba(0x00, 0x00, 0x00, 0x1a),
            palette: [
                Color::rgb(0xb1, 0x24, 0x24),
                Color::rgb(0xc7, 0x5d, 0x07),
                Color::rgb(0xf8, 0xa3, 0x00),
                Color::rgb(0x00, 0x6b, 0x4f),
                Color::rgb(0x13, 0x8a, 0xf2),
                Color::rgb(0x6a, 0x3d, 0xec),
                Color::rgb(0x9a, 0x1b, 0x6e),
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
