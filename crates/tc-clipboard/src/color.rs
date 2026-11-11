//! A parsed colour, and the notations it can be rewritten as.
//!
//! Upstream oracle: `Features/Clipboard/Model/ColorValue.swift` (the parsing
//! and the sRGB/HSL conversion) and `Features/Clipboard/Model/ColorFormat.swift`
//! (the notations and the digits they are spelled in).

/// A parsed colour, stored as the sRGB components every format derives from.
///
/// Upstream `ColorValue`: each channel is 0…1 and already clamped; alpha is 1
/// for a notation that carries none.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorValue {
    /// The red channel, 0…1.
    pub red: f64,
    /// The green channel, 0…1.
    pub green: f64,
    /// The blue channel, 0…1.
    pub blue: f64,
    /// The alpha channel, 0…1. 1 for a notation that carries none.
    pub alpha: f64,
}

impl ColorValue {
    /// A colour from its sRGB components.
    ///
    /// Upstream `ColorValue.init(red:green:blue:alpha:)`: every channel is
    /// clamped into 0…1, and a non-finite channel becomes zero, because `NaN`
    /// survives `min`/`max` and every notation ends in an integer conversion.
    pub fn new(red: f64, green: f64, blue: f64, alpha: f64) -> ColorValue {
        ColorValue {
            red: clamp(red),
            green: clamp(green),
            blue: clamp(blue),
            alpha: clamp(alpha),
        }
    }

    /// A colour from its HSL components.
    ///
    /// Upstream `ColorValue.init(hue:saturation:lightness:alpha:)`: HSL in,
    /// sRGB stored, so one set of components means every format derives from
    /// one source.
    pub fn from_hsl(hue: f64, saturation: f64, lightness: f64, alpha: f64) -> ColorValue {
        let saturation = saturation.clamp(0.0, 1.0);
        let lightness = lightness.clamp(0.0, 1.0);
        let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
        let sector = hue / 60.0;
        let second = chroma * (1.0 - (sector % 2.0 - 1.0).abs());
        let base = lightness - chroma / 2.0;
        let (red, green, blue) = match sector {
            s if s < 1.0 => (chroma, second, 0.0),
            s if s < 2.0 => (second, chroma, 0.0),
            s if s < 3.0 => (0.0, chroma, second),
            s if s < 4.0 => (0.0, second, chroma),
            s if s < 5.0 => (second, 0.0, chroma),
            _ => (chroma, 0.0, second),
        };
        ColorValue::new(red + base, green + base, blue + base, alpha)
    }

    /// The colour in HSL.
    ///
    /// Upstream `ColorValue.hsl`.
    pub fn hsl(&self) -> Hsl {
        let maximum = self.red.max(self.green).max(self.blue);
        let minimum = self.red.min(self.green).min(self.blue);
        let chroma = maximum - minimum;
        let lightness = (maximum + minimum) / 2.0;
        // Rounding leaves a chroma of ~1e-16 at a lightness of 1, whose
        // divisor below is 0.
        let span = 1.0 - (2.0 * lightness - 1.0).abs();
        if chroma <= 0.0 || span <= 0.0 {
            return Hsl {
                hue: 0.0,
                saturation: 0.0,
                lightness,
            };
        }
        let hue = if maximum == self.red {
            60.0 * (((self.green - self.blue) / chroma) % 6.0)
        } else if maximum == self.green {
            60.0 * ((self.blue - self.red) / chroma + 2.0)
        } else {
            60.0 * ((self.red - self.green) / chroma + 4.0)
        };
        Hsl {
            hue: if hue < 0.0 { hue + 360.0 } else { hue },
            saturation: (chroma / span).min(1.0),
            lightness,
        }
    }

    /// Whether the colour carries an alpha that is not fully opaque.
    ///
    /// Upstream `ColorValue.hasAlpha`: every notation states 8 bits, so alpha
    /// that rounds back to opaque is opaque.
    pub fn has_alpha(&self) -> bool {
        (self.alpha * 255.0).round() < 255.0
    }

    /// Parse a colour literal.
    ///
    /// Upstream `ColorValue.parse(_:)`: `#rgb`, `#rgba`, `#rrggbb`,
    /// `#rrggbbaa`, and the `rgb()`, `rgba()`, `hsl()`, `hsla()` and `oklch()`
    /// functions in their comma and CSS4 space forms.
    pub fn parse(text: &str) -> Option<ColorValue> {
        // Every visible row runs this per render, so the shape is rejected
        // before allocating.
        if text.len() > SCAN_LIMIT || !could_be_color(text.as_bytes()) {
            return None;
        }
        // Trimmed before the cap, as the classifier is; else a newline files an
        // unreadable row.
        let token = text.trim();
        if token.is_empty() || token.len() > DETECTION_LIMIT {
            return None;
        }
        if let Some(digits) = token.strip_prefix('#') {
            return parse_hex(digits);
        }
        parse_functional(token)
    }
}

/// A colour in HSL.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hsl {
    /// The hue in degrees, 0…360.
    pub hue: f64,
    /// The saturation, 0…1.
    pub saturation: f64,
    /// The lightness, 0…1.
    pub lightness: f64,
}

/// A colour is one bare token, so anything past the longest `hsla()` form is
/// not one. Upstream `ColorValue.detectionLimit`.
const DETECTION_LIMIT: usize = 64;

/// How much untrimmed text is worth trimming first; past this nothing can be
/// a colour anyway. Upstream `ColorValue.scanLimit`.
const SCAN_LIMIT: usize = 2048;

/// Clamp a channel into 0…1, turning a non-finite one into zero.
///
/// Upstream `ColorValue.clamp(_:)`: `NaN` survives `min`/`max` and
/// `Int(NaN)` traps, so a non-finite channel becomes zero.
fn clamp(value: f64) -> f64 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// Prose and links fail on the closing byte alone: a colour ends in a hex
/// digit or a `)`. Upstream `ColorValue.couldBeColor(_:)`.
fn could_be_color(bytes: &[u8]) -> bool {
    let first = match bytes.iter().find(|byte| !is_space(**byte)) {
        Some(byte) => *byte,
        None => return false,
    };
    let last = match bytes.iter().rev().find(|byte| !is_space(**byte)) {
        Some(byte) => *byte,
        None => return false,
    };
    if first == b'#' {
        return is_hex(last);
    }
    is_letter(first) && last == b')'
}

/// Upstream `ColorValue.isSpace(_:)`.
fn is_space(byte: u8) -> bool {
    byte == 0x20 || (0x09..=0x0d).contains(&byte)
}

/// Upstream `ColorValue.isLetter(_:)`.
fn is_letter(byte: u8) -> bool {
    byte.to_ascii_lowercase().is_ascii_lowercase()
}

/// Upstream `ColorValue.isHex(_:)`.
fn is_hex(byte: u8) -> bool {
    byte.is_ascii_digit()
        || byte.to_ascii_lowercase().is_ascii_lowercase() && byte.is_ascii_hexdigit()
}

/// `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa` — the four lengths CSS defines.
///
/// Upstream `ColorValue.parseHex(_:)`.
fn parse_hex(digits: &str) -> Option<ColorValue> {
    // ASCII, since a wider hex digit is no colour anyone writes.
    if !digits
        .chars()
        .all(|c| c.is_ascii() && c.is_ascii_hexdigit())
    {
        return None;
    }
    let bytes = digits.as_bytes();
    let channels: Vec<f64> = match bytes.len() {
        // Shorthand doubles each digit, so `#0f0` is `#00ff00` rather than
        // `#0f0f00`.
        3 | 4 => bytes
            .iter()
            .map(|byte| f64::from(hex_digit(*byte) * 17) / 255.0)
            .collect(),
        6 | 8 => bytes
            .chunks(2)
            .map(|pair| {
                let high = hex_digit(pair[0]);
                let low = hex_digit(pair[1]);
                f64::from(high * 16 + low) / 255.0
            })
            .collect(),
        _ => return None,
    };
    if channels.len() != bytes.len() / if bytes.len() <= 4 { 1 } else { 2 } {
        return None;
    }
    Some(ColorValue::new(
        channels[0],
        channels[1],
        channels[2],
        if channels.len() == 4 {
            channels[3]
        } else {
            1.0
        },
    ))
}

/// The value of one ASCII hex digit.
fn hex_digit(byte: u8) -> u32 {
    match byte {
        b'0'..=b'9' => u32::from(byte - b'0'),
        b'a'..=b'f' => u32::from(byte - b'a') + 10,
        b'A'..=b'F' => u32::from(byte - b'A') + 10,
        _ => 0,
    }
}

/// `rgb()` / `rgba()` / `hsl()` / `hsla()` / `oklch()`, in the comma and CSS4
/// space forms. Upstream `ColorValue.parseFunctional(_:)`.
fn parse_functional(token: &str) -> Option<ColorValue> {
    let open = token.find('(')?;
    if !token.ends_with(')') {
        return None;
    }
    let function = token[..open].to_ascii_lowercase();
    let body = &token[open + 1..token.len() - 1];
    // `rgba` is an alias of `rgb` rather than its own function, so alpha is
    // always optional.
    let parts = arguments(body)?;
    if parts.len() != 3 && parts.len() != 4 {
        return None;
    }
    let alpha = if parts.len() == 4 {
        component(&parts[3], 1.0)?
    } else {
        1.0
    };
    match function.as_str() {
        "rgb" | "rgba" => {
            let red = component(&parts[0], 255.0)?;
            let green = component(&parts[1], 255.0)?;
            let blue = component(&parts[2], 255.0)?;
            Some(ColorValue::new(red, green, blue, alpha))
        }
        "hsl" | "hsla" => {
            // CSS spells both as percentages; a bare `100` would read as 1.0 and
            // answer white.
            let hue = angle(&parts[0])?;
            let saturation = percentage(&parts[1])?;
            let lightness = percentage(&parts[2])?;
            Some(ColorValue::from_hsl(hue, saturation, lightness, alpha))
        }
        "oklch" => {
            // A percentage chroma is a fraction of 0.4, the bound CSS gives the
            // axis.
            let lightness = component(&parts[0], 1.0)?;
            let chroma = component(&parts[1], 1.0)?;
            let hue = angle(&parts[2])?;
            let chroma = if parts[1].ends_with('%') {
                chroma * 0.4
            } else {
                chroma
            };
            Some(crate::color::ColorValue::from_oklch(
                lightness, chroma, hue, alpha,
            ))
        }
        _ => None,
    }
}

/// One channel as a fraction of `scale`; a trailing `%` is always a fraction of
/// the whole. Upstream `ColorValue.component(_:scale:)`.
fn component(text: &str, scale: f64) -> Option<f64> {
    // `f64::from_str` reads `nan`, `inf` and Swift literals CSS never writes,
    // like `0x10` as 16, so the shape is checked first.
    if !text
        .chars()
        .all(|c| c.is_ascii() && (c.is_ascii_digit() || c == '.' || c == '-' || c == '%'))
    {
        return None;
    }
    let value = if let Some(digits) = text.strip_suffix('%') {
        parse_number(digits)? / 100.0
    } else {
        parse_number(text)? / scale
    };
    if value.is_finite() {
        Some(value)
    } else {
        None
    }
}

/// A channel CSS only ever writes as a percentage.
/// Upstream `ColorValue.percentage(_:)`.
fn percentage(text: &str) -> Option<f64> {
    if text.ends_with('%') {
        component(text, 1.0)
    } else {
        None
    }
}

/// Comma-separated or space-with-slash, never mixed — so the comma decides
/// which is read. Upstream `ColorValue.arguments(of:)`.
fn arguments(body: &str) -> Option<Vec<String>> {
    let parts: Vec<String> = if body.contains(',') {
        if body.contains('/') {
            return None;
        }
        body.split(',')
            .map(|part| part.trim().to_string())
            .collect()
    } else {
        // Counted per side: flattened, `rgb(0 255 / 0.5)` reads an alpha as its
        // blue channel.
        let sides: Vec<Vec<String>> = body
            .split('/')
            .map(|side| side.split_whitespace().map(str::to_string).collect())
            .collect();
        match sides.len() {
            // Alpha is spelled behind the slash, so an unseparated fourth
            // argument is malformed.
            1 if sides[0].len() == 3 => sides[0].clone(),
            2 if sides[0].len() == 3 && sides[1].len() == 1 => {
                let mut all = sides[0].clone();
                all.extend(sides[1].clone());
                all
            }
            _ => return None,
        }
    };
    if parts.iter().all(|part| !part.is_empty()) {
        Some(parts)
    } else {
        None
    }
}

/// A hue in degrees, normalised into 0…<360 so `-30deg` and `330` agree.
/// Upstream `ColorValue.angle(_:)`.
fn angle(text: &str) -> Option<f64> {
    let lowered = text.to_ascii_lowercase();
    let digits = lowered.strip_suffix("deg").unwrap_or(text);
    let degrees = parse_number(digits)?;
    if !degrees.is_finite() {
        return None;
    }
    let remainder = degrees % 360.0;
    Some(if remainder < 0.0 {
        remainder + 360.0
    } else {
        remainder
    })
}

/// Parse a plain decimal number, rejecting the shapes CSS never writes.
fn parse_number(text: &str) -> Option<f64> {
    if text.is_empty() {
        return None;
    }
    let mut seen_digit = false;
    let mut seen_dot = false;
    for (index, c) in text.chars().enumerate() {
        match c {
            '-' if index == 0 => {}
            '.' if !seen_dot => seen_dot = true,
            c if c.is_ascii_digit() => seen_digit = true,
            _ => return None,
        }
    }
    if !seen_digit {
        return None;
    }
    text.parse::<f64>().ok()
}

/// The notations a parsed colour can be rewritten as.
///
/// Upstream `ColorFormat`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColorFormat {
    /// `#RRGGBB`.
    Hex,
    /// `#RRGGBBAA`.
    HexWithAlpha,
    /// `rgba(r, g, b, a)`.
    Rgba,
    /// `hsl(h, s%, l%)`.
    Hsl,
    /// `hsl(h, s%, l%, a)`.
    HslWithAlpha,
    /// `oklch(l% c h)`.
    Oklch,
}

impl ColorFormat {
    /// Every notation, in the order upstream's `CaseIterable` gives them.
    pub const ALL: [ColorFormat; 6] = [
        ColorFormat::Hex,
        ColorFormat::HexWithAlpha,
        ColorFormat::Rgba,
        ColorFormat::Hsl,
        ColorFormat::HslWithAlpha,
        ColorFormat::Oklch,
    ];

    /// The colour written in this notation.
    ///
    /// Upstream `ColorFormat.string(for:)`.
    pub fn string(self, color: &ColorValue) -> String {
        match self {
            ColorFormat::Hex => digits::hex(color, false),
            ColorFormat::HexWithAlpha => digits::hex(color, true),
            ColorFormat::Rgba => format!(
                "rgba({}, {}, {}, {})",
                digits::channel(color.red),
                digits::channel(color.green),
                digits::channel(color.blue),
                digits::decimal(color.alpha)
            ),
            ColorFormat::Hsl | ColorFormat::HslWithAlpha => {
                let hsl = color.hsl();
                let alpha = if self.carries_alpha() {
                    format!(", {}", digits::decimal(color.alpha))
                } else {
                    String::new()
                };
                format!(
                    "hsl({}, {}, {}{})",
                    digits::angle(hsl.hue),
                    digits::percent(hsl.saturation),
                    digits::percent(hsl.lightness),
                    alpha
                )
            }
            ColorFormat::Oklch => {
                let oklch = color.oklch();
                // Oklab's axes run to about ±0.4, so a tenth would state most
                // colours as zero.
                format!(
                    "oklch({} {} {}{})",
                    digits::percent(oklch.l),
                    digits::decimal(oklch.c),
                    digits::angle(oklch.h),
                    digits::slash_alpha(color)
                )
            }
        }
    }

    /// The notations offered for a colour.
    ///
    /// Upstream `ColorFormat.offered(for:)`: an alpha-bearing row only where
    /// there is alpha, so no opaque duplicate is ever offered.
    pub fn offered(color: &ColorValue) -> Vec<ColorFormat> {
        ColorFormat::ALL
            .into_iter()
            .filter(|format| color.has_alpha() || !format.carries_alpha())
            .collect()
    }

    /// The notation the card states and ↵ copies: the shortest one that keeps
    /// the colour whole.
    ///
    /// Upstream `ColorFormat.primary(for:)`.
    pub fn primary(color: &ColorValue) -> ColorFormat {
        if color.has_alpha() {
            ColorFormat::HexWithAlpha
        } else {
            ColorFormat::Hex
        }
    }

    /// Whether the notation carries an alpha channel.
    ///
    /// Upstream `ColorFormat.carriesAlpha`.
    pub fn carries_alpha(self) -> bool {
        matches!(self, ColorFormat::HexWithAlpha | ColorFormat::HslWithAlpha)
    }
}

/// The digits a notation is spelled in.
mod digits {
    use super::ColorValue;

    /// Upstream `ColorDigits.hex(_:includingAlpha:)`.
    pub fn hex(color: &ColorValue, including_alpha: bool) -> String {
        let mut channels = vec![color.red, color.green, color.blue];
        if including_alpha {
            channels.push(color.alpha);
        }
        let digits: Vec<String> = channels
            .iter()
            .map(|value| format!("{:02X}", channel(*value)))
            .collect();
        format!("#{}", digits.join(""))
    }

    /// Upstream `ColorDigits.channel(_:)`.
    pub fn channel(value: f64) -> i64 {
        integer(value * 255.0)
    }

    /// Trailing zeros dropped, so an exact channel reads `1` rather than
    /// `1.000`. Upstream `ColorDigits.decimal(_:)`.
    pub fn decimal(value: f64) -> String {
        if !value.is_finite() {
            return "0".to_string();
        }
        let rounded = (value * 1000.0).round() / 1000.0;
        if rounded == rounded.round() {
            integer(rounded).to_string()
        } else {
            format_general(rounded)
        }
    }

    /// One decimal, dropped when exact: whole percents cost up to 5/255 on the
    /// way back. Upstream `ColorDigits.percent(_:)`.
    pub fn percent(value: f64) -> String {
        format!("{}%", number(value * 100.0))
    }

    /// An angle carries its unit, the way CSS Color 4 writes every hue.
    /// Upstream `ColorDigits.angle(_:)`.
    pub fn angle(value: f64) -> String {
        format!("{}deg", number(value))
    }

    /// The CSS4 slash alpha, said only when there is one.
    /// Upstream `ColorDigits.slashAlpha(_:)`.
    pub fn slash_alpha(color: &ColorValue) -> String {
        if color.has_alpha() {
            format!(" / {}", decimal(color.alpha))
        } else {
            String::new()
        }
    }

    /// Upstream `ColorDigits.number(_:)`.
    fn number(value: f64) -> String {
        if !value.is_finite() {
            return "0".to_string();
        }
        let tenths = (value * 10.0).round() / 10.0;
        if tenths == tenths.round() {
            integer(tenths).to_string()
        } else {
            format!("{:.1}", tenths)
        }
    }

    /// One funnel, because an integer conversion traps on a non-finite double
    /// and every notation ends in one. Upstream `ColorDigits.integer(_:)`.
    fn integer(value: f64) -> i64 {
        if value.is_finite() {
            value.round() as i64
        } else {
            0
        }
    }

    /// The shortest form of a number, as C's `%g` prints it.
    ///
    /// `%g` keeps six significant digits, drops the trailing zeros, and uses
    /// the exponent form when the exponent is below -4 or at least the
    /// precision. Upstream `ColorDigits.general(_:)`.
    fn format_general(value: f64) -> String {
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

    /// Drop the trailing zeros a fixed-point number ends with.
    fn trim_trailing_zeros(text: &str) -> String {
        if !text.contains('.') {
            return text.to_string();
        }
        let trimmed = text.trim_end_matches('0');
        trimmed.strip_suffix('.').unwrap_or(trimmed).to_string()
    }
}
