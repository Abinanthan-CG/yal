//! Colour values and conversions.
//!
//! Colours are stored as 8-bit red, green and blue components. Conversions to
//! and from hexadecimal, RGB and HSL are exact within floating point; the
//! hexadecimal form is rounded to the nearest representable value.

/// An 8-bit RGB colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

/// A colour in the HSL colour space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hsl {
    /// Hue in degrees, 0 to 360.
    pub hue: f64,
    /// Saturation, 0 to 1.
    pub saturation: f64,
    /// Lightness, 0 to 1.
    pub lightness: f64,
}

/// A failure while parsing a colour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorError(pub String);

impl std::fmt::Display for ColorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ColorError {}

impl Color {
    /// Build a colour from components.
    pub const fn new(red: u8, green: u8, blue: u8) -> Self {
        Self {
            red,
            green,
            blue,
        }
    }

    /// Parse a hexadecimal colour.
    ///
    /// Accepts `#rgb`, `#rrggbb` and the same forms without the leading `#`.
    pub fn from_hex(text: &str) -> Result<Self, ColorError> {
        let hex = text.trim().trim_start_matches('#');
        if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(ColorError(format!("`{text}` is not a hexadecimal colour")));
        }
        let expanded = match hex.len() {
            3 => hex.chars().flat_map(|c| [c, c]).collect::<String>(),
            6 => hex.to_string(),
            _ => {
                return Err(ColorError(format!(
                    "`{text}` must have three or six hexadecimal digits"
                )));
            }
        };
        let value = u32::from_str_radix(&expanded, 16)
            .map_err(|_| ColorError(format!("`{text}` is not a hexadecimal colour")))?;
        Ok(Self {
            red: ((value >> 16) & 0xff) as u8,
            green: ((value >> 8) & 0xff) as u8,
            blue: (value & 0xff) as u8,
        })
    }

    /// The hexadecimal form, without a leading `#`.
    pub fn to_hex(self) -> String {
        format!("{:02x}{:02x}{:02x}", self.red, self.green, self.blue)
    }

    /// The `rgb(r, g, b)` form.
    pub fn to_rgb_string(self) -> String {
        format!("rgb({}, {}, {})", self.red, self.green, self.blue)
    }

    /// Convert to HSL.
    pub fn to_hsl(self) -> Hsl {
        let r = f64::from(self.red) / 255.0;
        let g = f64::from(self.green) / 255.0;
        let b = f64::from(self.blue) / 255.0;

        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let lightness = (max + min) / 2.0;
        let delta = max - min;

        if delta.abs() < f64::EPSILON {
            return Hsl {
                hue: 0.0,
                saturation: 0.0,
                lightness,
            };
        }

        let saturation = delta / (1.0 - (2.0 * lightness - 1.0)).abs();
        let hue = if (max - r).abs() < f64::EPSILON {
            60.0 * (((g - b) / delta) % 6.0)
        } else if (max - g).abs() < f64::EPSILON {
            60.0 * (((b - r) / delta) + 2.0)
        } else {
            60.0 * (((r - g) / delta) + 4.0)
        };

        Hsl {
            hue: if hue < 0.0 { hue + 360.0 } else { hue },
            saturation,
            lightness,
        }
    }

    /// Build a colour from HSL.
    pub fn from_hsl(hsl: Hsl) -> Self {
        let chroma = (1.0 - (2.0 * hsl.lightness - 1.0).abs()) * hsl.saturation;
        let hue = hsl.hue.rem_euclid(360.0) / 60.0;
        let x = chroma * (1.0 - (hue % 2.0 - 1.0).abs());

        let (r, g, b) = match hue as u32 {
            0 => (chroma, x, 0.0),
            1 => (x, chroma, 0.0),
            2 => (0.0, chroma, x),
            3 => (0.0, x, chroma),
            4 => (x, 0.0, chroma),
            _ => (chroma, 0.0, x),
        };

        let m = hsl.lightness - chroma / 2.0;
        Self {
            red: (((r + m) * 255.0).round() as i32).clamp(0, 255) as u8,
            green: (((g + m) * 255.0).round() as i32).clamp(0, 255) as u8,
            blue: (((b + m) * 255.0).round() as i32).clamp(0, 255) as u8,
        }
    }
}

/// A named colour.
#[derive(Debug, Clone, Copy)]
pub struct NamedColor {
    pub name: &'static str,
    pub color: Color,
}

/// The named colours this build knows.
pub const NAMED_COLORS: &[NamedColor] = &[
    NamedColor { name: "black", color: Color::new(0, 0, 0) },
    NamedColor { name: "white", color: Color::new(255, 255, 255) },
    NamedColor { name: "red", color: Color::new(255, 0, 0) },
    NamedColor { name: "green", color: Color::new(0, 128, 0) },
    NamedColor { name: "lime", color: Color::new(0, 255, 0) },
    NamedColor { name: "blue", color: Color::new(0, 0, 255) },
    NamedColor { name: "yellow", color: Color::new(255, 255, 0) },
    NamedColor { name: "cyan", color: Color::new(0, 255, 255) },
    NamedColor { name: "aqua", color: Color::new(0, 255, 255) },
    NamedColor { name: "magenta", color: Color::new(255, 0, 255) },
    NamedColor { name: "fuchsia", color: Color::new(255, 0, 255) },
    NamedColor { name: "gray", color: Color::new(128, 128, 128) },
    NamedColor { name: "grey", color: Color::new(128, 128, 128) },
    NamedColor { name: "silver", color: Color::new(192, 192, 192) },
    NamedColor { name: "maroon", color: Color::new(128, 0, 0) },
    NamedColor { name: "olive", color: Color::new(128, 128, 0) },
    NamedColor { name: "navy", color: Color::new(0, 0, 128) },
    NamedColor { name: "purple", color: Color::new(128, 0, 128) },
    NamedColor { name: "teal", color: Color::new(0, 128, 128) },
    NamedColor { name: "orange", color: Color::new(255, 165, 0) },
    NamedColor { name: "pink", color: Color::new(255, 192, 203) },
    NamedColor { name: "brown", color: Color::new(165, 42, 42) },
    NamedColor { name: "gold", color: Color::new(255, 215, 0) },
    NamedColor { name: "indigo", color: Color::new(75, 0, 130) },
    NamedColor { name: "violet", color: Color::new(238, 130, 238) },
    NamedColor { name: "turquoise", color: Color::new(64, 224, 208) },
    NamedColor { name: "salmon", color: Color::new(250, 128, 114) },
    NamedColor { name: "crimson", color: Color::new(220, 20, 60) },
    NamedColor { name: "coral", color: Color::new(255, 127, 80) },
    NamedColor { name: "khaki", color: Color::new(240, 230, 140) },
    NamedColor { name: "lavender", color: Color::new(230, 230, 250) },
    NamedColor { name: "beige", color: Color::new(245, 245, 220) },
    NamedColor { name: "ivory", color: Color::new(255, 255, 240) },
    NamedColor { name: "beige", color: Color::new(245, 245, 220) },
];

/// Look up a colour by name.
pub fn by_name(name: &str) -> Option<Color> {
    let needle = name.trim().to_lowercase();
    NAMED_COLORS
        .iter()
        .find(|named| named.name == needle)
        .map(|named| named.color)
}

/// Parse a colour from a name or a hexadecimal value.
pub fn parse(text: &str) -> Result<Color, ColorError> {
    let trimmed = text.trim();
    if let Some(color) = by_name(trimmed) {
        return Ok(color);
    }
    Color::from_hex(trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn parses_hexadecimal_forms() {
        assert_eq!(Color::from_hex("#ff8800").unwrap(), Color::new(255, 136, 0));
        assert_eq!(Color::from_hex("ff8800").unwrap(), Color::new(255, 136, 0));
        assert_eq!(Color::from_hex("#f80").unwrap(), Color::new(255, 136, 0));
    }

    #[test]
    fn rejects_malformed_hexadecimal() {
        assert!(Color::from_hex("#gg8800").is_err());
        assert!(Color::from_hex("#ff88").is_err());
    }

    #[test]
    fn hex_round_trips() {
        let color = Color::new(18, 52, 86);
        assert_eq!(Color::from_hex(&color.to_hex()).unwrap(), color);
    }

    #[test]
    fn converts_to_hsl_and_back() {
        let color = Color::new(255, 136, 0);
        let hsl = color.to_hsl();
        // #ff8800 is hsl(32, 100%, 50%).
        assert_relative_eq!(hsl.hue, 32.0, epsilon = 0.5);
        assert_relative_eq!(hsl.saturation, 1.0, epsilon = 0.01);
        assert_relative_eq!(hsl.lightness, 0.5, epsilon = 0.01);
        let back = Color::from_hsl(hsl);
        assert!(back.red.abs_diff(color.red) <= 1);
        assert!(back.green.abs_diff(color.green) <= 1);
        assert!(back.blue.abs_diff(color.blue) <= 1);
    }

    #[test]
    fn greyscale_has_no_hue() {
        let hsl = Color::new(128, 128, 128).to_hsl();
        assert_relative_eq!(hsl.saturation, 0.0, epsilon = 1e-9);
    }

    #[test]
    fn resolves_named_colours() {
        assert_eq!(by_name("Red"), Some(Color::new(255, 0, 0)));
        assert_eq!(by_name("nonsense"), None);
    }

    #[test]
    fn parse_accepts_names_and_hex() {
        assert_eq!(parse("blue").unwrap(), Color::new(0, 0, 255));
        assert_eq!(parse("#0000ff").unwrap(), Color::new(0, 0, 255));
    }
}
