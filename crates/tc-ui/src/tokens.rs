//! The palette's design tokens.
//!
//! The palette does not run CSS, but it can wear a design system. A *style
//! pack* is a small JSON file of colours, measurements and fonts: the same
//! tokens a design library already holds, in a form the renderer can read.
//!
//! ```json
//! {
//!   "name": "midnight",
//!   "appearance": "dark",
//!   "colors": {
//!     "background": "#121214",
//!     "field": "#1e1e21",
//!     "text": "#f4f4f8",
//!     "textDim": "#8b8b93",
//!     "onIcon": "#ffffff",
//!     "selection": "#33333a",
//!     "accent": "#5b8dff",
//!     "border": "#ffffff24"
//!   },
//!   "metrics": { "width": 680, "rowHeight": 40, "radius": 10 },
//!   "font": "Segoe UI"
//! }
//! ```

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::theme::{Appearance, Color, Metrics, Theme, PALETTE};

/// A colour in a style pack: `#rgb`, `#rrggbb` or `#rrggbbaa`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TokenColor(pub Color);

impl TokenColor {
    /// Reads a colour written as `#rgb`, `#rrggbb` or `#rrggbbaa`.
    pub fn parse(text: &str) -> Option<Color> {
        let hex = text.trim().trim_start_matches('#');
        let mut bytes = [0u8; 4];
        match hex.len() {
            3 => {
                for (index, digit) in hex.chars().enumerate() {
                    let value = digit.to_digit(16)? as u8;
                    bytes[index] = value * 17;
                }
            }
            6 | 8 => {
                for index in 0..hex.len() / 2 {
                    let pair = &hex[index * 2..index * 2 + 2];
                    bytes[index] = u8::from_str_radix(pair, 16).ok()?;
                }
            }
            _ => return None,
        }
        let alpha = if hex.len() == 3 || hex.len() == 6 {
            0xff
        } else {
            bytes[3]
        };
        Some(Color::rgba(bytes[0], bytes[1], bytes[2], alpha))
    }
}

impl Serialize for TokenColor {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let color = self.0;
        let alpha = (color.alpha * 255.0).round() as u8;
        serializer.serialize_str(&format!(
            "#{:02x}{:02x}{:02x}{:02x}",
            (color.red * 255.0).round() as u8,
            (color.green * 255.0).round() as u8,
            (color.blue * 255.0).round() as u8,
            alpha
        ))
    }
}

impl<'de> Deserialize<'de> for TokenColor {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        TokenColor::parse(&text)
            .map(TokenColor)
            .ok_or_else(|| serde::de::Error::custom(format!("{text} is not a colour")))
    }
}

/// The colours of a style pack.
///
/// Every colour a design library would hand over has a name here: the
/// surfaces the window is built from, the text, the accent, and the palette
/// the rest of the interface draws from.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct TokenColors {
    /// The window's background.
    pub background: TokenColor,
    /// A second background, for the panels that sit on the first.
    #[serde(rename = "backgroundSecondary", default)]
    pub background_secondary: Option<TokenColor>,
    /// The search field's background.
    pub field: TokenColor,
    /// The colour of a row's name.
    pub text: TokenColor,
    /// The colour of secondary text.
    #[serde(rename = "textDim")]
    pub text_dim: TokenColor,
    /// The colour of the text on a tile.
    #[serde(rename = "onIcon")]
    pub on_icon: TokenColor,
    /// The fill of the selected row.
    pub selection: TokenColor,
    /// The fill of the row under the pointer. Without one, the accent at a
    /// low opacity is used.
    #[serde(default)]
    pub hover: Option<TokenColor>,
    /// The accent colour.
    pub accent: TokenColor,
    /// The colour of the busy state.
    #[serde(default)]
    pub loader: Option<TokenColor>,
    /// The hairline around the window.
    pub border: TokenColor,
    /// The palette: the colours the rest of the interface draws from.
    #[serde(default)]
    pub palette: Option<Vec<TokenColor>>,
}

/// The measurements of a style pack. Every field is optional; a style pack
/// only has to say what it wants to change.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct TokenMetrics {
    /// The width of the palette window.
    pub width: Option<f32>,
    /// The height of the palette window.
    pub height: Option<f32>,
    /// The distance from the window's edge to its content.
    pub padding: Option<f32>,
    /// The height of the search field.
    #[serde(rename = "fieldHeight")]
    pub field_height: Option<f32>,
    /// The height of one row.
    #[serde(rename = "rowHeight")]
    pub row_height: Option<f32>,
    /// The size of the tile that leads a row.
    #[serde(rename = "iconSize")]
    pub icon_size: Option<f32>,
    /// The size of a row's name.
    #[serde(rename = "nameSize")]
    pub name_size: Option<f32>,
    /// The size of a row's detail text.
    #[serde(rename = "detailSize")]
    pub detail_size: Option<f32>,
    /// The size of the footer line.
    #[serde(rename = "footerSize")]
    pub footer_size: Option<f32>,
    /// The size of the letter in a tile.
    #[serde(rename = "iconTextSize")]
    pub icon_text_size: Option<f32>,
    /// The corner radius.
    pub radius: Option<f32>,
    /// The distance from the field's left edge to the text inside it.
    #[serde(rename = "fieldInset")]
    pub field_inset: Option<f32>,
    /// The space between the search field and the first row.
    #[serde(rename = "listGap")]
    pub list_gap: Option<f32>,
    /// The height of the footer, hairline and hint together.
    #[serde(rename = "footerHeight")]
    pub footer_height: Option<f32>,
    /// The most rows the palette shows at once.
    #[serde(rename = "maxRows")]
    pub max_rows: Option<usize>,
}

/// Which appearance a style pack is for.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AppearanceName {
    /// The light appearance.
    Light,
    /// The dark appearance.
    Dark,
}

impl AppearanceName {
    /// The appearance it names.
    pub fn appearance(self) -> Appearance {
        match self {
            AppearanceName::Light => Appearance::Light,
            AppearanceName::Dark => Appearance::Dark,
        }
    }
}

/// A style pack: the design system's tokens, in a form the palette can wear.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Tokens {
    /// The style pack's name.
    #[serde(default = "default_name")]
    pub name: String,
    /// Which appearance the pack is for.
    #[serde(default = "default_appearance")]
    pub appearance: AppearanceName,
    /// The colours.
    pub colors: TokenColors,
    /// The measurements.
    #[serde(default)]
    pub metrics: TokenMetrics,
    /// The font family.
    #[serde(default = "default_font")]
    pub font: String,
}

fn default_name() -> String {
    "untitled".to_string()
}

fn default_font() -> String {
    "Segoe UI".to_string()
}

fn default_appearance() -> AppearanceName {
    AppearanceName::Dark
}

impl Tokens {
    /// Reads a style pack from a file.
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("the style pack could not be read: {error}"))?;
        serde_json::from_str(&text).map_err(|error| format!("the style pack is not valid: {error}"))
    }

    /// The theme this style pack describes.
    pub fn theme(&self) -> Theme {
        let base = match self.appearance {
            AppearanceName::Light => Theme::light(),
            AppearanceName::Dark => Theme::dark(),
        };
        let metrics = self.metrics;
        Theme {
            appearance: self.appearance.appearance(),
            background: self.colors.background.0,
            background_secondary: self
                .colors
                .background_secondary
                .map(|color| color.0)
                .unwrap_or(base.background_secondary),
            field: self.colors.field.0,
            text: self.colors.text.0,
            text_dim: self.colors.text_dim.0,
            on_icon: self.colors.on_icon.0,
            selection: self.colors.selection.0,
            hover: self
                .colors
                .hover
                .map(|color| color.0)
                .unwrap_or_else(|| accent_at(self.colors.accent.0, 0.12)),
            accent: self.colors.accent.0,
            loader: self
                .colors
                .loader
                .map(|color| color.0)
                .unwrap_or(base.loader),
            border: self.colors.border.0,
            palette: self.palette(base.palette),
            metrics: Metrics {
                width: metrics.width.unwrap_or(base.metrics.width),
                height: metrics.height.unwrap_or(base.metrics.height),
                padding: metrics.padding.unwrap_or(base.metrics.padding),
                field_height: metrics.field_height.unwrap_or(base.metrics.field_height),
                field_inset: metrics.field_inset.unwrap_or(base.metrics.field_inset),
                row_height: metrics.row_height.unwrap_or(base.metrics.row_height),
                icon_size: metrics.icon_size.unwrap_or(base.metrics.icon_size),
                name_size: metrics.name_size.unwrap_or(base.metrics.name_size),
                detail_size: metrics.detail_size.unwrap_or(base.metrics.detail_size),
                footer_size: metrics.footer_size.unwrap_or(base.metrics.footer_size),
                icon_text_size: metrics.icon_text_size.unwrap_or(base.metrics.icon_text_size),
                radius: metrics.radius.unwrap_or(base.metrics.radius),
                list_gap: metrics.list_gap.unwrap_or(base.metrics.list_gap),
                footer_height: metrics.footer_height.unwrap_or(base.metrics.footer_height),
                max_rows: metrics.max_rows.unwrap_or(base.metrics.max_rows),
            },
        }
    }

    /// The palette this style pack describes, over the one it inherits.
    ///
    /// A pack may name fewer colours than the palette holds - the rest are
    /// inherited - but never more: the extra ones are dropped rather than
    /// silently changing the shape of the palette.
    fn palette(&self, base: [Color; PALETTE]) -> [Color; PALETTE] {
        let mut palette = base;
        if let Some(named) = self.colors.palette.as_ref() {
            for (slot, color) in palette.iter_mut().zip(named.iter()) {
                *slot = color.0;
            }
        }
        palette
    }
}

/// An accent colour at a low opacity, for the row under the pointer.
fn accent_at(accent: Color, opacity: f32) -> Color {
    Color {
        red: accent.red,
        green: accent.green,
        blue: accent.blue,
        alpha: opacity,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn colors() -> TokenColors {
        TokenColors {
            background: TokenColor(Color::rgb(0, 0, 0)),
            background_secondary: None,
            field: TokenColor(Color::rgb(1, 1, 1)),
            text: TokenColor(Color::rgb(2, 2, 2)),
            text_dim: TokenColor(Color::rgb(3, 3, 3)),
            on_icon: TokenColor(Color::rgb(4, 4, 4)),
            selection: TokenColor(Color::rgb(5, 5, 5)),
            hover: None,
            accent: TokenColor(Color::rgb(6, 6, 6)),
            loader: None,
            border: TokenColor(Color::rgb(7, 7, 7)),
            palette: None,
        }
    }

    #[test]
    fn a_colour_is_read_from_hex() {
        let color = TokenColor::parse("#ff8000").expect("a colour");
        assert!((color.red - 1.0).abs() < 0.01);
        assert!((color.green - 0.50196).abs() < 0.01);
        assert!((color.blue - 0.0).abs() < 0.01);
        assert_eq!(color.alpha, 1.0);
    }

    #[test]
    fn a_colour_may_carry_its_opacity() {
        let color = TokenColor::parse("#ff800040").expect("a colour");
        assert!((color.alpha - 0.25098).abs() < 0.01);
        assert!((color.red - 1.0).abs() < 0.01);
    }

    #[test]
    fn a_short_colour_is_read() {
        let color = TokenColor::parse("#f80").expect("a colour");
        assert!((color.red - 1.0).abs() < 0.01);
        assert!((color.green - 0.5333).abs() < 0.01);
        assert_eq!(color.alpha, 1.0);
    }

    #[test]
    fn a_colour_that_is_not_a_colour_is_refused() {
        assert!(TokenColor::parse("rebeccapurple").is_none());
        assert!(TokenColor::parse("#12345").is_none());
        assert!(TokenColor::parse("").is_none());
    }

    #[test]
    fn a_colour_written_back_holds_its_opacity() {
        let color = TokenColor::parse("#ff800040").expect("a colour");
        let text = serde_json::to_string(&TokenColor(color)).expect("json");
        assert_eq!(text, "\"#ff800040\"");
    }

    #[test]
    fn a_style_pack_wears_its_own_colours() {
        let tokens = Tokens {
            name: "midnight".to_string(),
            appearance: AppearanceName::Dark,
            colors: colors(),
            metrics: TokenMetrics::default(),
            font: default_font(),
        };
        let theme = tokens.theme();
        assert_eq!(theme.background, Color::rgb(0, 0, 0));
        assert_eq!(theme.field, Color::rgb(1, 1, 1));
        assert_eq!(theme.appearance, Appearance::Dark);
    }

    #[test]
    fn a_style_pack_only_changes_what_it_names() {
        let tokens = Tokens {
            name: "wide".to_string(),
            appearance: AppearanceName::Light,
            colors: colors(),
            metrics: TokenMetrics {
                width: Some(900.0),
                row_height: Some(48.0),
                ..TokenMetrics::default()
            },
            font: default_font(),
        };
        let theme = tokens.theme();
        assert_eq!(theme.metrics.width, 900.0);
        assert_eq!(theme.metrics.row_height, 48.0);
        // Everything else is the light appearance's own.
        assert_eq!(theme.metrics.radius, Theme::light().metrics.radius);
        assert_eq!(theme.appearance, Appearance::Light);
    }

    #[test]
    fn a_style_pack_is_read_from_json() {
        let json = r##"{
            "name": "midnight",
            "appearance": "dark",
            "colors": {
                "background": "#121214",
                "field": "#1e1e21",
                "text": "#f4f4f8",
                "textDim": "#8b8b93",
                "onIcon": "#ffffff",
                "selection": "#33333a",
                "accent": "#5b8dff",
                "border": "#ffffff24"
            },
            "metrics": { "width": 700, "rowHeight": 44 },
            "font": "Cascadia Code"
        }"##;
        let tokens: Tokens = serde_json::from_str(json).expect("a style pack");
        assert_eq!(tokens.name, "midnight");
        assert_eq!(tokens.font, "Cascadia Code");
        let theme = tokens.theme();
        assert_eq!(theme.metrics.width, 700.0);
        assert_eq!(theme.metrics.row_height, 44.0);
        assert!((theme.border.alpha - 0.14118).abs() < 0.01);
    }

    #[test]
    fn a_style_pack_that_is_not_a_style_pack_is_refused() {
        let json = r##"{
            "name": "broken",
            "appearance": "dark",
            "colors": { "background": "not a colour" }
        }"##;
        let error = serde_json::from_str::<Tokens>(json).expect_err("an error");
        assert!(error.to_string().contains("not a colour"));
    }

    #[test]
    fn the_style_packs_we_ship_are_wearable() {
        for (text, name) in [
            (include_str!("../style-packs/midnight.json"), "midnight"),
            (include_str!("../style-packs/daylight.json"), "daylight"),
            (include_str!("../style-packs/aurora.json"), "aurora"),
        ] {
            let tokens: Tokens = serde_json::from_str(text).expect("a style pack");
            assert_eq!(tokens.name, name);
            let theme = tokens.theme();
            assert!(theme.metrics.width > 0.0);
            assert!(theme.metrics.row_height > 0.0);
            assert!(theme.background.alpha > 0.0);
            assert_eq!(theme.palette.len(), PALETTE);
        }
    }

    #[test]
    fn a_style_pack_without_a_hover_wears_the_accent_faintly() {
        let tokens = Tokens {
            name: "faint".to_string(),
            appearance: AppearanceName::Dark,
            colors: colors(),
            metrics: TokenMetrics::default(),
            font: default_font(),
        };
        let theme = tokens.theme();
        assert_eq!(theme.hover.red, theme.accent.red);
        assert!((theme.hover.alpha - 0.12).abs() < 0.001);
    }

    #[test]
    fn a_style_pack_may_name_its_own_hover() {
        let mut own = colors();
        own.hover = Some(TokenColor(Color::rgb(0x33, 0x33, 0x3a)));
        let tokens = Tokens {
            name: "own hover".to_string(),
            appearance: AppearanceName::Dark,
            colors: own,
            metrics: TokenMetrics::default(),
            font: default_font(),
        };
        assert_eq!(tokens.theme().hover, Color::rgb(0x33, 0x33, 0x3a));
    }

    #[test]
    fn a_style_pack_that_names_no_palette_inherits_one() {
        let tokens = Tokens {
            name: "plain".to_string(),
            appearance: AppearanceName::Dark,
            colors: colors(),
            metrics: TokenMetrics::default(),
            font: default_font(),
        };
        assert_eq!(tokens.theme().palette, Theme::dark().palette);
    }

    #[test]
    fn a_style_pack_may_bring_its_own_palette() {
        let mut own = colors();
        own.palette = Some(vec![
            TokenColor(Color::rgb(1, 1, 1)),
            TokenColor(Color::rgb(2, 2, 2)),
            TokenColor(Color::rgb(3, 3, 3)),
        ]);
        let tokens = Tokens {
            name: "painted".to_string(),
            appearance: AppearanceName::Dark,
            colors: own,
            metrics: TokenMetrics::default(),
            font: default_font(),
        };
        let palette = tokens.theme().palette;
        assert_eq!(palette[0], Color::rgb(1, 1, 1));
        assert_eq!(palette[2], Color::rgb(3, 3, 3));
        // The colours it did not name are inherited.
        assert_eq!(palette[3], Theme::dark().palette[3]);
        assert_eq!(palette.len(), PALETTE);
    }

    #[test]
    fn a_style_pack_may_name_a_second_background_and_a_loader() {
        let mut own = colors();
        own.background_secondary = Some(TokenColor(Color::rgb(0x11, 0x22, 0x33)));
        own.loader = Some(TokenColor(Color::rgb(0x44, 0x55, 0x66)));
        let tokens = Tokens {
            name: "surfaces".to_string(),
            appearance: AppearanceName::Dark,
            colors: own,
            metrics: TokenMetrics::default(),
            font: default_font(),
        };
        let theme = tokens.theme();
        assert_eq!(theme.background_secondary, Color::rgb(0x11, 0x22, 0x33));
        assert_eq!(theme.loader, Color::rgb(0x44, 0x55, 0x66));
    }

    #[test]
    fn a_design_library_can_be_written_as_one_pack() {
        // The shape a design library would hand over: surfaces, text, an
        // accent, and a palette of seven.
        let json = r##"{
            "name": "submarine",
            "appearance": "dark",
            "colors": {
                "background": "#001068",
                "backgroundSecondary": "#00685E",
                "field": "#00184C",
                "text": "#EAFBFF",
                "textDim": "#8FA9C0",
                "onIcon": "#FFFFFF",
                "selection": "#00FFEA",
                "accent": "#4FA3F8",
                "loader": "#D6AEFF",
                "border": "#FFFFFF24",
                "palette": [
                    "#F87E7E", "#FFB765", "#FEFF7F", "#7EECAE",
                    "#72BAFB", "#CE9DFA", "#FFA2D4"
                ]
            },
            "metrics": { "width": 720, "rowHeight": 44 },
            "font": "Segoe UI Variable"
        }"##;
        let tokens: Tokens = serde_json::from_str(json).expect("a style pack");
        let theme = tokens.theme();
        assert_eq!(theme.background, Color::rgb(0x00, 0x10, 0x68));
        assert_eq!(theme.background_secondary, Color::rgb(0x00, 0x68, 0x5E));
        assert_eq!(theme.selection, Color::rgb(0x00, 0xff, 0xea));
        assert_eq!(theme.loader, Color::rgb(0xd6, 0xae, 0xff));
        assert_eq!(theme.palette.len(), PALETTE);
        assert_eq!(theme.palette[4], Color::rgb(0x72, 0xba, 0xfb));
        assert_eq!(theme.palette[6], Color::rgb(0xff, 0xa2, 0xd4));
    }
}
