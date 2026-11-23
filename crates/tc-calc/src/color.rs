//! Clean-room color parser, perceptual color-space converter, and CSS named
//! color matcher.
//!
//! Recognizes hex (`#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`), CSS functional
//! notations (`rgb`, `rgba`, `hsl`, `hsla`, `hwb`, `hsv`, `hsb`, `lab`, `lch`,
//! `oklab`, `oklch`, `cmyk` in both comma and CSS4 space/slash syntax), and
//! named CSS color expressions (`tomato at 50%`, `rebeccapurple as oklch`).
//! Bare CSS color names alone return `None` so app search is never polluted.

use crate::result::{CalcResult, Payload};

/// Canonical internal color in sRGB (`0.0..=1.0` per channel, linear alpha).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Rgba {
    r: f64,
    g: f64,
    b: f64,
    a: f64,
}

impl Rgba {
    fn from_u8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self {
            r: r as f64 / 255.0,
            g: g as f64 / 255.0,
            b: b as f64 / 255.0,
            a: a as f64 / 255.0,
        }
    }

    fn clamped(self) -> Self {
        Self {
            r: self.r.clamp(0.0, 1.0),
            g: self.g.clamp(0.0, 1.0),
            b: self.b.clamp(0.0, 1.0),
            a: self.a.clamp(0.0, 1.0),
        }
    }

    fn to_u8(self) -> [u8; 4] {
        let c = self.clamped();
        [
            (c.r * 255.0).round() as u8,
            (c.g * 255.0).round() as u8,
            (c.b * 255.0).round() as u8,
            (c.a * 255.0).round() as u8,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ColorFormat {
    Hex,
    HexAlpha,
    Rgb,
    Rgba,
    Hsl,
    Hsla,
    Hwb,
    Hsv,
    Hsb,
    Lab,
    Lch,
    Oklab,
    Oklch,
    Cmyk,
}

impl ColorFormat {
    fn parse(text: &str) -> Option<Self> {
        let norm = text
            .trim()
            .to_ascii_lowercase()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        Some(match norm.as_str() {
            "hex" => Self::Hex,
            "hexa" | "hex8" | "hex with alpha" | "hex alpha" => Self::HexAlpha,
            "rgb" => Self::Rgb,
            "rgba" => Self::Rgba,
            "hsl" => Self::Hsl,
            "hsla" => Self::Hsla,
            "hwb" => Self::Hwb,
            "hsv" => Self::Hsv,
            "hsb" => Self::Hsb,
            "lab" | "cielab" => Self::Lab,
            "lch" | "cielch" => Self::Lch,
            "oklab" => Self::Oklab,
            "oklch" => Self::Oklch,
            "cmyk" => Self::Cmyk,
            _ => return None,
        })
    }

    fn label(self) -> &'static str {
        match self {
            Self::Hex => "Hex",
            Self::HexAlpha => "Hex (Alpha)",
            Self::Rgb => "RGB",
            Self::Rgba => "RGBA",
            Self::Hsl => "HSL",
            Self::Hsla => "HSLA",
            Self::Hwb => "HWB",
            Self::Hsv => "HSV",
            Self::Hsb => "HSB",
            Self::Lab => "Lab",
            Self::Lch => "LCH",
            Self::Oklab => "OKLab",
            Self::Oklch => "OKLCH",
            Self::Cmyk => "CMYK",
        }
    }
}

/// Evaluate a color preview or conversion query.
pub fn evaluate(query: &str) -> Option<CalcResult> {
    let trimmed = query.trim();
    if trimmed.is_empty() || trimmed.len() > 160 {
        return None;
    }

    // 1. Check explicit conversion: `<color_expr> (as|in|to|->) <target_format>`.
    if let Some((lhs, target)) = split_target_format(trimmed) {
        let (color, source_label) = parse_color_expression(lhs, true)?;
        let formatted = format_color(color, target);
        let swatch = color.to_u8();
        let mut card = CalcResult::with_badges(
            lhs.trim().to_string(),
            source_label,
            target.label(),
            Payload::text(&formatted),
        )
        .with_color_swatch(swatch);
        card.can_chain = false;
        return Some(card);
    }

    // 2. Direct preview without explicit conversion target.
    // Bare CSS color names alone (like "red" or "orange") are NOT matched.
    let (color, source_label) = parse_color_expression(trimmed, false)?;
    let swatch = color.to_u8();
    let (display, default_target) = if source_label == "Hex" || source_label == "Hex (Alpha)" {
        let target = if swatch[3] < 255 {
            ColorFormat::Rgba
        } else {
            ColorFormat::Rgb
        };
        (format_color(color, target), target.label())
    } else {
        let target = if swatch[3] < 255 {
            ColorFormat::HexAlpha
        } else {
            ColorFormat::Hex
        };
        (format_color(color, target), target.label())
    };

    let target_badge = nearest_css_name(swatch[0], swatch[1], swatch[2]).unwrap_or(default_target);

    let mut card = CalcResult::with_badges(
        trimmed.to_string(),
        source_label,
        target_badge,
        Payload::text(&display),
    )
    .with_color_swatch(swatch);
    card.can_chain = false;
    Some(card)
}

fn split_target_format(input: &str) -> Option<(&str, ColorFormat)> {
    // Check arrow `->` first.
    if let Some(pos) = input.rfind("->") {
        let lhs = input[..pos].trim();
        let rhs = input[pos + 2..].trim();
        if !lhs.is_empty() {
            if let Some(fmt) = ColorFormat::parse(rhs) {
                return Some((lhs, fmt));
            }
        }
    }

    // Split on the last standalone connector word (`as`, `in`, `to`).
    let lower = input.to_ascii_lowercase();
    for connector in [" as ", " in ", " to "] {
        if let Some(pos) = lower.rfind(connector) {
            let lhs = input[..pos].trim();
            let rhs = input[pos + connector.len()..].trim();
            if !lhs.is_empty() {
                if let Some(fmt) = ColorFormat::parse(rhs) {
                    return Some((lhs, fmt));
                }
            }
        }
    }
    None
}

/// Parse a color literal, functional notation, or `<color> at <pct>%`.
/// `allow_bare_named` is true only when followed by an explicit target conversion.
fn parse_color_expression(input: &str, allow_bare_named: bool) -> Option<(Rgba, &'static str)> {
    let trimmed = input.trim();

    // Check `<color> at <alpha%>` (e.g., `tomato at 50%`, `#ff6363 at 25%`).
    let lower = trimmed.to_ascii_lowercase();
    if let Some(pos) = lower.rfind(" at ") {
        let base_str = trimmed[..pos].trim();
        let alpha_str = trimmed[pos + 4..].trim();
        if let Some(alpha) = parse_alpha(alpha_str) {
            let (mut color, label) = parse_single_color(base_str, true)?;
            color.a = alpha;
            return Some((color.clamped(), label));
        }
    }

    parse_single_color(trimmed, allow_bare_named)
}

fn parse_single_color(input: &str, allow_named: bool) -> Option<(Rgba, &'static str)> {
    let s = input.trim();
    if let Some(hex) = s.strip_prefix('#') {
        return parse_hex(hex);
    }

    if let Some(open) = s.find('(') {
        if s.ends_with(')') {
            let func = s[..open].trim().to_ascii_lowercase();
            let inner = &s[open + 1..s.len() - 1];
            return parse_function(&func, inner);
        }
    }

    if allow_named {
        let key: String = s
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '-' && *c != '_')
            .map(|c| c.to_ascii_lowercase())
            .collect();
        if let Some(&(r, g, b)) = lookup_css_named(&key) {
            return Some((Rgba::from_u8(r, g, b, 255), "CSS Color"));
        }
    }

    None
}

fn parse_hex(hex: &str) -> Option<(Rgba, &'static str)> {
    let bytes = hex.as_bytes();
    if !bytes.iter().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let hex_byte = |hi: u8, lo: u8| -> Option<u8> {
        let s = [hi, lo];
        u8::from_str_radix(std::str::from_utf8(&s).ok()?, 16).ok()
    };
    let nibble = |ch: u8| -> Option<u8> {
        let v = (ch as char).to_digit(16)? as u8;
        Some(v * 17)
    };
    match bytes.len() {
        3 => Some((
            Rgba::from_u8(nibble(bytes[0])?, nibble(bytes[1])?, nibble(bytes[2])?, 255),
            "Hex",
        )),
        4 => Some((
            Rgba::from_u8(
                nibble(bytes[0])?,
                nibble(bytes[1])?,
                nibble(bytes[2])?,
                nibble(bytes[3])?,
            ),
            "Hex (Alpha)",
        )),
        6 => Some((
            Rgba::from_u8(
                hex_byte(bytes[0], bytes[1])?,
                hex_byte(bytes[2], bytes[3])?,
                hex_byte(bytes[4], bytes[5])?,
                255,
            ),
            "Hex",
        )),
        8 => Some((
            Rgba::from_u8(
                hex_byte(bytes[0], bytes[1])?,
                hex_byte(bytes[2], bytes[3])?,
                hex_byte(bytes[4], bytes[5])?,
                hex_byte(bytes[6], bytes[7])?,
            ),
            "Hex (Alpha)",
        )),
        _ => None,
    }
}

/// Split functional color arguments supporting both comma syntax
/// `rgb(255, 99, 99, 0.5)` and CSS4 space/slash syntax `rgb(255 99 99 / 50%)`.
fn split_func_args(inner: &str) -> Option<(Vec<&str>, Option<&str>)> {
    if let Some((lhs, rhs)) = inner.split_once('/') {
        let parts: Vec<&str> = lhs
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|t| !t.is_empty())
            .collect();
        let alpha = rhs.trim();
        if alpha.is_empty() {
            return None;
        }
        return Some((parts, Some(alpha)));
    }
    let parts: Vec<&str> = inner
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|t| !t.is_empty())
        .collect();
    Some((parts, None))
}

fn parse_function(func: &str, inner: &str) -> Option<(Rgba, &'static str)> {
    let (args, slash_alpha) = split_func_args(inner)?;
    match func {
        "rgb" | "rgba" => {
            if args.len() != 3 && !(args.len() == 4 && slash_alpha.is_none()) {
                return None;
            }
            let r = parse_rgb_channel(args[0])?;
            let g = parse_rgb_channel(args[1])?;
            let b = parse_rgb_channel(args[2])?;
            let a = match slash_alpha.or_else(|| args.get(3).copied()) {
                Some(t) => parse_alpha(t)?,
                None => 1.0,
            };
            let label = if func == "rgba" || a < 1.0 {
                "RGBA"
            } else {
                "RGB"
            };
            Some((Rgba { r, g, b, a }.clamped(), label))
        }
        "hsl" | "hsla" => {
            if args.len() != 3 && !(args.len() == 4 && slash_alpha.is_none()) {
                return None;
            }
            let h = parse_angle_deg(args[0])?;
            let s = parse_percent_or_unit(args[1])?;
            let l = parse_percent_or_unit(args[2])?;
            let a = match slash_alpha.or_else(|| args.get(3).copied()) {
                Some(t) => parse_alpha(t)?,
                None => 1.0,
            };
            let (r, g, b) = hsl_to_srgb(h, s, l);
            let label = if func == "hsla" || a < 1.0 {
                "HSLA"
            } else {
                "HSL"
            };
            Some((Rgba { r, g, b, a }.clamped(), label))
        }
        "hwb" => {
            if args.len() != 3 && !(args.len() == 4 && slash_alpha.is_none()) {
                return None;
            }
            let h = parse_angle_deg(args[0])?;
            let w = parse_percent_or_unit(args[1])?;
            let bl = parse_percent_or_unit(args[2])?;
            let a = match slash_alpha.or_else(|| args.get(3).copied()) {
                Some(t) => parse_alpha(t)?,
                None => 1.0,
            };
            let (r, g, b) = hwb_to_srgb(h, w, bl);
            Some((Rgba { r, g, b, a }.clamped(), "HWB"))
        }
        "hsv" | "hsb" => {
            if args.len() != 3 && !(args.len() == 4 && slash_alpha.is_none()) {
                return None;
            }
            let h = parse_angle_deg(args[0])?;
            let s = parse_percent_or_unit(args[1])?;
            let v = parse_percent_or_unit(args[2])?;
            let a = match slash_alpha.or_else(|| args.get(3).copied()) {
                Some(t) => parse_alpha(t)?,
                None => 1.0,
            };
            let (r, g, b) = hsv_to_srgb(h, s, v);
            let label = if func == "hsb" { "HSB" } else { "HSV" };
            Some((Rgba { r, g, b, a }.clamped(), label))
        }
        "lab" => {
            if args.len() != 3 && !(args.len() == 4 && slash_alpha.is_none()) {
                return None;
            }
            let l = parse_lab_l(args[0])?;
            let a_ch = parse_number(args[1])?;
            let b_ch = parse_number(args[2])?;
            let alpha = match slash_alpha.or_else(|| args.get(3).copied()) {
                Some(t) => parse_alpha(t)?,
                None => 1.0,
            };
            let (r, g, b) = lab_to_srgb(l, a_ch, b_ch);
            Some((Rgba { r, g, b, a: alpha }.clamped(), "Lab"))
        }
        "lch" => {
            if args.len() != 3 && !(args.len() == 4 && slash_alpha.is_none()) {
                return None;
            }
            let l = parse_lab_l(args[0])?;
            let c = parse_number(args[1])?.max(0.0);
            let h = parse_angle_deg(args[2])?;
            let alpha = match slash_alpha.or_else(|| args.get(3).copied()) {
                Some(t) => parse_alpha(t)?,
                None => 1.0,
            };
            let rad = h.to_radians();
            let (r, g, b) = lab_to_srgb(l, c * rad.cos(), c * rad.sin());
            Some((Rgba { r, g, b, a: alpha }.clamped(), "LCH"))
        }
        "oklab" => {
            if args.len() != 3 && !(args.len() == 4 && slash_alpha.is_none()) {
                return None;
            }
            let l = parse_percent_or_unit(args[0])?;
            let a_ch = parse_number(args[1])?;
            let b_ch = parse_number(args[2])?;
            let alpha = match slash_alpha.or_else(|| args.get(3).copied()) {
                Some(t) => parse_alpha(t)?,
                None => 1.0,
            };
            let (r, g, b) = oklab_to_srgb(l, a_ch, b_ch);
            Some((Rgba { r, g, b, a: alpha }.clamped(), "OKLab"))
        }
        "oklch" => {
            if args.len() != 3 && !(args.len() == 4 && slash_alpha.is_none()) {
                return None;
            }
            let l = parse_percent_or_unit(args[0])?;
            let c = parse_number(args[1])?.max(0.0);
            let h = parse_angle_deg(args[2])?;
            let alpha = match slash_alpha.or_else(|| args.get(3).copied()) {
                Some(t) => parse_alpha(t)?,
                None => 1.0,
            };
            let rad = h.to_radians();
            let (r, g, b) = oklab_to_srgb(l, c * rad.cos(), c * rad.sin());
            Some((Rgba { r, g, b, a: alpha }.clamped(), "OKLCH"))
        }
        "cmyk" | "device-cmyk" => {
            if args.len() != 4 && !(args.len() == 5 && slash_alpha.is_none()) {
                return None;
            }
            let c = parse_percent_or_unit(args[0])?;
            let m = parse_percent_or_unit(args[1])?;
            let y = parse_percent_or_unit(args[2])?;
            let k = parse_percent_or_unit(args[3])?;
            let alpha = match slash_alpha.or_else(|| args.get(4).copied()) {
                Some(t) => parse_alpha(t)?,
                None => 1.0,
            };
            let r = (1.0 - c) * (1.0 - k);
            let g = (1.0 - m) * (1.0 - k);
            let b = (1.0 - y) * (1.0 - k);
            Some((Rgba { r, g, b, a: alpha }.clamped(), "CMYK"))
        }
        _ => None,
    }
}

fn parse_number(s: &str) -> Option<f64> {
    let v = s.trim().parse::<f64>().ok()?;
    if v.is_finite() {
        Some(v)
    } else {
        None
    }
}

fn parse_rgb_channel(s: &str) -> Option<f64> {
    let t = s.trim();
    if let Some(p) = t.strip_suffix('%') {
        Some((parse_number(p)? / 100.0).clamp(0.0, 1.0))
    } else {
        let v = parse_number(t)?;
        if !(0.0..=255.0).contains(&v) {
            return None;
        }
        Some(v / 255.0)
    }
}

fn parse_percent_or_unit(s: &str) -> Option<f64> {
    let t = s.trim();
    if let Some(p) = t.strip_suffix('%') {
        Some((parse_number(p)? / 100.0).clamp(0.0, 1.0))
    } else {
        let v = parse_number(t)?;
        if v > 1.0 && v <= 100.0 {
            Some(v / 100.0)
        } else if (0.0..=1.0).contains(&v) {
            Some(v)
        } else {
            None
        }
    }
}

fn parse_lab_l(s: &str) -> Option<f64> {
    let t = s.trim();
    let v = if let Some(p) = t.strip_suffix('%') {
        parse_number(p)?
    } else {
        parse_number(t)?
    };
    Some(v.clamp(0.0, 100.0))
}

fn parse_alpha(s: &str) -> Option<f64> {
    let t = s.trim();
    if let Some(p) = t.strip_suffix('%') {
        let v = parse_number(p)?;
        if (0.0..=100.0).contains(&v) {
            Some(v / 100.0)
        } else {
            None
        }
    } else {
        let v = parse_number(t)?;
        if (0.0..=1.0).contains(&v) {
            Some(v)
        } else {
            None
        }
    }
}

fn parse_angle_deg(s: &str) -> Option<f64> {
    let t = s.trim().to_ascii_lowercase();
    let deg = if let Some(v) = t.strip_suffix("deg") {
        parse_number(v)?
    } else if let Some(v) = t.strip_suffix("rad") {
        parse_number(v)?.to_degrees()
    } else if let Some(v) = t.strip_suffix("grad") {
        parse_number(v)? * 0.9
    } else if let Some(v) = t.strip_suffix("turn") {
        parse_number(v)? * 360.0
    } else {
        parse_number(&t)?
    };
    Some(deg.rem_euclid(360.0))
}

fn fmt_f(v: f64, decimals: usize) -> String {
    let scale = 10_f64.powi(decimals as i32);
    let rounded = (v * scale).round() / scale;
    let clean = if rounded.abs() < 1e-9 { 0.0 } else { rounded };
    let s = format!("{clean:.decimals$}");
    if decimals == 0 {
        s
    } else {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn format_color(color: Rgba, target: ColorFormat) -> String {
    let c = color.clamped();
    let [r8, g8, b8, a8] = c.to_u8();
    let has_alpha = a8 < 255;

    match target {
        ColorFormat::Hex => {
            if has_alpha {
                format!("#{r8:02X}{g8:02X}{b8:02X}{a8:02X}")
            } else {
                format!("#{r8:02X}{g8:02X}{b8:02X}")
            }
        }
        ColorFormat::HexAlpha => format!("#{r8:02X}{g8:02X}{b8:02X}{a8:02X}"),
        ColorFormat::Rgb => {
            if has_alpha {
                format!("rgba({r8}, {g8}, {b8}, {})", fmt_f(c.a, 2))
            } else {
                format!("rgb({r8}, {g8}, {b8})")
            }
        }
        ColorFormat::Rgba => format!("rgba({r8}, {g8}, {b8}, {})", fmt_f(c.a, 2)),
        ColorFormat::Hsl => {
            let (h, s, l) = srgb_to_hsl(c.r, c.g, c.b);
            if has_alpha {
                format!(
                    "hsla({}, {}%, {}%, {})",
                    fmt_f(h, 0),
                    fmt_f(s * 100.0, 0),
                    fmt_f(l * 100.0, 0),
                    fmt_f(c.a, 2)
                )
            } else {
                format!(
                    "hsl({}, {}%, {}%)",
                    fmt_f(h, 0),
                    fmt_f(s * 100.0, 0),
                    fmt_f(l * 100.0, 0)
                )
            }
        }
        ColorFormat::Hsla => {
            let (h, s, l) = srgb_to_hsl(c.r, c.g, c.b);
            format!(
                "hsla({}, {}%, {}%, {})",
                fmt_f(h, 0),
                fmt_f(s * 100.0, 0),
                fmt_f(l * 100.0, 0),
                fmt_f(c.a, 2)
            )
        }
        ColorFormat::Hwb => {
            let (h, w, bl) = srgb_to_hwb(c.r, c.g, c.b);
            if has_alpha {
                format!(
                    "hwb({} {}% {}% / {}%)",
                    fmt_f(h, 0),
                    fmt_f(w * 100.0, 0),
                    fmt_f(bl * 100.0, 0),
                    fmt_f(c.a * 100.0, 0)
                )
            } else {
                format!(
                    "hwb({} {}% {}%)",
                    fmt_f(h, 0),
                    fmt_f(w * 100.0, 0),
                    fmt_f(bl * 100.0, 0)
                )
            }
        }
        ColorFormat::Hsv | ColorFormat::Hsb => {
            let (h, s, v) = srgb_to_hsv(c.r, c.g, c.b);
            let name = if target == ColorFormat::Hsb {
                "hsb"
            } else {
                "hsv"
            };
            format!(
                "{name}({}, {}%, {}%)",
                fmt_f(h, 0),
                fmt_f(s * 100.0, 0),
                fmt_f(v * 100.0, 0)
            )
        }
        ColorFormat::Lab => {
            let (l, a_ch, b_ch) = srgb_to_lab(c.r, c.g, c.b);
            if has_alpha {
                format!(
                    "lab({}% {} {} / {})",
                    fmt_f(l, 2),
                    fmt_f(a_ch, 2),
                    fmt_f(b_ch, 2),
                    fmt_f(c.a, 2)
                )
            } else {
                format!(
                    "lab({}% {} {})",
                    fmt_f(l, 2),
                    fmt_f(a_ch, 2),
                    fmt_f(b_ch, 2)
                )
            }
        }
        ColorFormat::Lch => {
            let (l, a_ch, b_ch) = srgb_to_lab(c.r, c.g, c.b);
            let ch = (a_ch * a_ch + b_ch * b_ch).sqrt();
            let h = if ch < 1e-4 {
                0.0
            } else {
                b_ch.atan2(a_ch).to_degrees().rem_euclid(360.0)
            };
            if has_alpha {
                format!(
                    "lch({}% {} {} / {})",
                    fmt_f(l, 2),
                    fmt_f(ch, 2),
                    fmt_f(h, 2),
                    fmt_f(c.a, 2)
                )
            } else {
                format!("lch({}% {} {})", fmt_f(l, 2), fmt_f(ch, 2), fmt_f(h, 2))
            }
        }
        ColorFormat::Oklab => {
            let (l, a_ch, b_ch) = srgb_to_oklab(c.r, c.g, c.b);
            if has_alpha {
                format!(
                    "oklab({} {} {} / {})",
                    fmt_f(l, 4),
                    fmt_f(a_ch, 4),
                    fmt_f(b_ch, 4),
                    fmt_f(c.a, 2)
                )
            } else {
                format!(
                    "oklab({} {} {})",
                    fmt_f(l, 4),
                    fmt_f(a_ch, 4),
                    fmt_f(b_ch, 4)
                )
            }
        }
        ColorFormat::Oklch => {
            let (l, a_ch, b_ch) = srgb_to_oklab(c.r, c.g, c.b);
            let ch = (a_ch * a_ch + b_ch * b_ch).sqrt();
            let h = if ch < 1e-5 {
                0.0
            } else {
                b_ch.atan2(a_ch).to_degrees().rem_euclid(360.0)
            };
            if has_alpha {
                format!(
                    "oklch({} {} {} / {})",
                    fmt_f(l, 4),
                    fmt_f(ch, 4),
                    fmt_f(h, 2),
                    fmt_f(c.a, 2)
                )
            } else {
                format!("oklch({} {} {})", fmt_f(l, 4), fmt_f(ch, 4), fmt_f(h, 2))
            }
        }
        ColorFormat::Cmyk => {
            let (cy, m, y, k) = srgb_to_cmyk(c.r, c.g, c.b);
            format!(
                "cmyk({}%, {}%, {}%, {}%)",
                fmt_f(cy * 100.0, 0),
                fmt_f(m * 100.0, 0),
                fmt_f(y * 100.0, 0),
                fmt_f(k * 100.0, 0)
            )
        }
    }
}

// ---------------------------------------------------------------------------
// Color-space math (sRGB, Linear, HSL, HSV, HWB, CMYK, CIE Lab, OKLab)
// ---------------------------------------------------------------------------

fn srgb_to_linear(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(c: f64) -> f64 {
    let clamped = c.clamp(0.0, 1.0);
    if clamped <= 0.0031308 {
        clamped * 12.92
    } else {
        1.055 * clamped.powf(1.0 / 2.4) - 0.055
    }
}

fn srgb_to_hsl(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    if d < 1e-9 {
        return (0.0, 0.0, l);
    }
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let h = if (max - r).abs() < 1e-9 {
        60.0 * (((g - b) / d).rem_euclid(6.0))
    } else if (max - g).abs() < 1e-9 {
        60.0 * (((b - r) / d) + 2.0)
    } else {
        60.0 * (((r - g) / d) + 4.0)
    };
    (h.rem_euclid(360.0), s.clamp(0.0, 1.0), l.clamp(0.0, 1.0))
}

fn hsl_to_srgb(h: f64, s: f64, l: f64) -> (f64, f64, f64) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r1, g1, b1) = match (h.rem_euclid(360.0) / 60.0).floor() as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (r1 + m, g1 + m, b1 + m)
}

fn srgb_to_hsv(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let s = if max < 1e-9 { 0.0 } else { d / max };
    let h = if d < 1e-9 {
        0.0
    } else if (max - r).abs() < 1e-9 {
        60.0 * (((g - b) / d).rem_euclid(6.0))
    } else if (max - g).abs() < 1e-9 {
        60.0 * (((b - r) / d) + 2.0)
    } else {
        60.0 * (((r - g) / d) + 4.0)
    };
    (h.rem_euclid(360.0), s.clamp(0.0, 1.0), max.clamp(0.0, 1.0))
}

fn hsv_to_srgb(h: f64, s: f64, v: f64) -> (f64, f64, f64) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r1, g1, b1) = match (h.rem_euclid(360.0) / 60.0).floor() as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (r1 + m, g1 + m, b1 + m)
}

fn srgb_to_hwb(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let (h, _, _) = srgb_to_hsl(r, g, b);
    let w = r.min(g).min(b);
    let bl = 1.0 - r.max(g).max(b);
    (h, w, bl)
}

fn hwb_to_srgb(h: f64, w: f64, bl: f64) -> (f64, f64, f64) {
    if w + bl >= 1.0 {
        let gray = w / (w + bl);
        return (gray, gray, gray);
    }
    let (r, g, b) = hsl_to_srgb(h, 1.0, 0.5);
    let factor = 1.0 - w - bl;
    (r * factor + w, g * factor + w, b * factor + w)
}

fn srgb_to_cmyk(r: f64, g: f64, b: f64) -> (f64, f64, f64, f64) {
    let k = 1.0 - r.max(g).max(b);
    if (1.0 - k) < 1e-9 {
        return (0.0, 0.0, 0.0, 1.0);
    }
    let c = (1.0 - r - k) / (1.0 - k);
    let m = (1.0 - g - k) / (1.0 - k);
    let y = (1.0 - b - k) / (1.0 - k);
    (c, m, y, k)
}

/// sRGB -> CIE Lab (D65 reference white).
fn srgb_to_lab(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let rl = srgb_to_linear(r);
    let gl = srgb_to_linear(g);
    let bl = srgb_to_linear(b);

    let x = (0.4124564 * rl + 0.3575761 * gl + 0.1804375 * bl) / 0.95047;
    let y = 0.2126729 * rl + 0.7151522 * gl + 0.0721750 * bl;
    let z = (0.0193339 * rl + 0.1191920 * gl + 0.9503041 * bl) / 1.08883;

    let f = |t: f64| {
        if t > 0.008856 {
            t.cbrt()
        } else {
            7.787 * t + 16.0 / 116.0
        }
    };
    let fx = f(x);
    let fy = f(y);
    let fz = f(z);
    (116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz))
}

fn lab_to_srgb(l: f64, a: f64, b: f64) -> (f64, f64, f64) {
    let fy = (l + 16.0) / 116.0;
    let fx = a / 500.0 + fy;
    let fz = fy - b / 200.0;
    let finv = |t: f64| {
        let t3 = t * t * t;
        if t3 > 0.008856 {
            t3
        } else {
            (t - 16.0 / 116.0) / 7.787
        }
    };
    let x = finv(fx) * 0.95047;
    let y = finv(fy);
    let z = finv(fz) * 1.08883;

    let rl = 3.2404542 * x - 1.5371385 * y - 0.4985314 * z;
    let gl = -0.9692660 * x + 1.8760108 * y + 0.0415560 * z;
    let bl = 0.0556434 * x - 0.2040259 * y + 1.0572252 * z;
    (linear_to_srgb(rl), linear_to_srgb(gl), linear_to_srgb(bl))
}

/// sRGB -> Bjorn Ottosson OKLab.
fn srgb_to_oklab(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let rl = srgb_to_linear(r);
    let gl = srgb_to_linear(g);
    let bl = srgb_to_linear(b);

    let l = (0.4122214708 * rl + 0.5363325363 * gl + 0.0514459929 * bl).cbrt();
    let m = (0.2119034982 * rl + 0.6806995451 * gl + 0.1073969566 * bl).cbrt();
    let s = (0.0883024619 * rl + 0.2817188376 * gl + 0.6299787005 * bl).cbrt();

    (
        0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
        1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
        0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s,
    )
}

fn oklab_to_srgb(l: f64, a: f64, b: f64) -> (f64, f64, f64) {
    let l_ = l + 0.3963377774 * a + 0.2158037573 * b;
    let m_ = l - 0.1055613458 * a - 0.0638541728 * b;
    let s_ = l - 0.0894841775 * a - 1.2914855480 * b;

    let lc = l_ * l_ * l_;
    let mc = m_ * m_ * m_;
    let sc = s_ * s_ * s_;

    let rl = 4.0767416621 * lc - 3.3077115913 * mc + 0.2309699292 * sc;
    let gl = -1.2684380046 * lc + 2.6097574011 * mc - 0.3413193965 * sc;
    let bl = -0.0041960863 * lc - 0.7034186147 * mc + 1.7076147010 * sc;
    (linear_to_srgb(rl), linear_to_srgb(gl), linear_to_srgb(bl))
}

// ---------------------------------------------------------------------------
// Standard CSS Color Module Level 4 Named Colors (148 W3C standard names)
// ---------------------------------------------------------------------------

const CSS_COLORS: &[(&str, &str, (u8, u8, u8))] = &[
    ("aliceblue", "Alice Blue", (240, 248, 255)),
    ("antiquewhite", "Antique White", (250, 235, 215)),
    ("aqua", "Aqua", (0, 255, 255)),
    ("aquamarine", "Aquamarine", (127, 255, 212)),
    ("azure", "Azure", (240, 255, 255)),
    ("beige", "Beige", (245, 245, 220)),
    ("bisque", "Bisque", (255, 228, 196)),
    ("black", "Black", (0, 0, 0)),
    ("blanchedalmond", "Blanched Almond", (255, 235, 205)),
    ("blue", "Blue", (0, 0, 255)),
    ("blueviolet", "Blue Violet", (138, 43, 226)),
    ("brown", "Brown", (165, 42, 42)),
    ("burlywood", "Burlywood", (222, 184, 135)),
    ("cadetblue", "Cadet Blue", (95, 158, 160)),
    ("chartreuse", "Chartreuse", (127, 255, 0)),
    ("chocolate", "Chocolate", (210, 105, 30)),
    ("coral", "Coral", (255, 127, 80)),
    ("cornflowerblue", "Cornflower Blue", (100, 149, 237)),
    ("cornsilk", "Cornsilk", (255, 248, 220)),
    ("crimson", "Crimson", (220, 20, 60)),
    ("cyan", "Cyan", (0, 255, 255)),
    ("darkblue", "Dark Blue", (0, 0, 139)),
    ("darkcyan", "Dark Cyan", (0, 139, 139)),
    ("darkgoldenrod", "Dark Goldenrod", (184, 134, 11)),
    ("darkgray", "Dark Gray", (169, 169, 169)),
    ("darkgreen", "Dark Green", (0, 100, 0)),
    ("darkgrey", "Dark Grey", (169, 169, 169)),
    ("darkkhaki", "Dark Khaki", (189, 183, 107)),
    ("darkmagenta", "Dark Magenta", (139, 0, 139)),
    ("darkolivegreen", "Dark Olive Green", (85, 107, 47)),
    ("darkorange", "Dark Orange", (255, 140, 0)),
    ("darkorchid", "Dark Orchid", (153, 50, 204)),
    ("darkred", "Dark Red", (139, 0, 0)),
    ("darksalmon", "Dark Salmon", (233, 150, 122)),
    ("darkseagreen", "Dark Sea Green", (143, 188, 143)),
    ("darkslateblue", "Dark Slate Blue", (72, 61, 139)),
    ("darkslategray", "Dark Slate Gray", (47, 79, 79)),
    ("darkslategrey", "Dark Slate Grey", (47, 79, 79)),
    ("darkturquoise", "Dark Turquoise", (0, 206, 209)),
    ("darkviolet", "Dark Violet", (148, 0, 211)),
    ("deeppink", "Deep Pink", (255, 20, 147)),
    ("deepskyblue", "Deep Sky Blue", (0, 191, 255)),
    ("dimgray", "Dim Gray", (105, 105, 105)),
    ("dimgrey", "Dim Grey", (105, 105, 105)),
    ("dodgerblue", "Dodger Blue", (30, 144, 255)),
    ("firebrick", "Firebrick", (178, 34, 34)),
    ("floralwhite", "Floral White", (255, 250, 240)),
    ("forestgreen", "Forest Green", (34, 139, 34)),
    ("fuchsia", "Fuchsia", (255, 0, 255)),
    ("gainsboro", "Gainsboro", (220, 220, 220)),
    ("ghostwhite", "Ghost White", (248, 248, 255)),
    ("gold", "Gold", (255, 215, 0)),
    ("goldenrod", "Goldenrod", (218, 165, 32)),
    ("gray", "Gray", (128, 128, 128)),
    ("green", "Green", (0, 128, 0)),
    ("greenyellow", "Green Yellow", (173, 255, 47)),
    ("grey", "Grey", (128, 128, 128)),
    ("honeydew", "Honeydew", (240, 255, 240)),
    ("hotpink", "Hot Pink", (255, 105, 180)),
    ("indianred", "Indian Red", (205, 92, 92)),
    ("indigo", "Indigo", (75, 0, 130)),
    ("ivory", "Ivory", (255, 255, 240)),
    ("khaki", "Khaki", (240, 230, 140)),
    ("lavender", "Lavender", (230, 230, 250)),
    ("lavenderblush", "Lavender Blush", (255, 240, 245)),
    ("lawngreen", "Lawn Green", (124, 252, 0)),
    ("lemonchiffon", "Lemon Chiffon", (255, 250, 205)),
    ("lightblue", "Light Blue", (173, 216, 230)),
    ("lightcoral", "Light Coral", (240, 128, 128)),
    ("lightcyan", "Light Cyan", (224, 255, 255)),
    (
        "lightgoldenrodyellow",
        "Light Goldenrod Yellow",
        (250, 250, 210),
    ),
    ("lightgray", "Light Gray", (211, 211, 211)),
    ("lightgreen", "Light Green", (144, 238, 144)),
    ("lightgrey", "Light Grey", (211, 211, 211)),
    ("lightpink", "Light Pink", (255, 182, 193)),
    ("lightsalmon", "Light Salmon", (255, 160, 122)),
    ("lightseagreen", "Light Sea Green", (32, 178, 170)),
    ("lightskyblue", "Light Sky Blue", (135, 206, 250)),
    ("lightslategray", "Light Slate Gray", (119, 136, 153)),
    ("lightslategrey", "Light Slate Grey", (119, 136, 153)),
    ("lightsteelblue", "Light Steel Blue", (176, 196, 222)),
    ("lightyellow", "Light Yellow", (255, 255, 224)),
    ("lime", "Lime", (0, 255, 0)),
    ("limegreen", "Lime Green", (50, 205, 50)),
    ("linen", "Linen", (250, 240, 230)),
    ("magenta", "Magenta", (255, 0, 255)),
    ("maroon", "Maroon", (128, 0, 0)),
    ("mediumaquamarine", "Medium Aquamarine", (102, 205, 170)),
    ("mediumblue", "Medium Blue", (0, 0, 205)),
    ("mediumorchid", "Medium Orchid", (186, 85, 211)),
    ("mediumpurple", "Medium Purple", (147, 112, 219)),
    ("mediumseagreen", "Medium Sea Green", (60, 179, 113)),
    ("mediumslateblue", "Medium Slate Blue", (123, 104, 238)),
    ("mediumspringgreen", "Medium Spring Green", (0, 250, 154)),
    ("mediumturquoise", "Medium Turquoise", (72, 209, 204)),
    ("mediumvioletred", "Medium Violet Red", (199, 21, 133)),
    ("midnightblue", "Midnight Blue", (25, 25, 112)),
    ("mintcream", "Mint Cream", (245, 255, 250)),
    ("mistyrose", "Misty Rose", (255, 228, 225)),
    ("moccasin", "Moccasin", (255, 228, 181)),
    ("navajowhite", "Navajo White", (255, 222, 173)),
    ("navy", "Navy", (0, 0, 128)),
    ("oldlace", "Old Lace", (253, 245, 230)),
    ("olive", "Olive", (128, 128, 0)),
    ("olivedrab", "Olive Drab", (107, 142, 35)),
    ("orange", "Orange", (255, 165, 0)),
    ("orangered", "Orange Red", (255, 69, 0)),
    ("orchid", "Orchid", (218, 112, 214)),
    ("palegoldenrod", "Pale Goldenrod", (238, 232, 170)),
    ("palegreen", "Pale Green", (152, 251, 152)),
    ("paleturquoise", "Pale Turquoise", (175, 238, 238)),
    ("palevioletred", "Pale Violet Red", (219, 112, 147)),
    ("papayawhip", "Papaya Whip", (255, 239, 213)),
    ("peachpuff", "Peach Puff", (255, 218, 185)),
    ("peru", "Peru", (205, 133, 63)),
    ("pink", "Pink", (255, 192, 203)),
    ("plum", "Plum", (221, 160, 221)),
    ("powderblue", "Powder Blue", (176, 224, 230)),
    ("purple", "Purple", (128, 0, 128)),
    ("rebeccapurple", "Rebecca Purple", (102, 51, 153)),
    ("red", "Red", (255, 0, 0)),
    ("rosybrown", "Rosy Brown", (188, 143, 143)),
    ("royalblue", "Royal Blue", (65, 105, 225)),
    ("saddlebrown", "Saddle Brown", (139, 69, 19)),
    ("salmon", "Salmon", (250, 128, 114)),
    ("sandybrown", "Sandy Brown", (244, 164, 96)),
    ("seagreen", "Sea Green", (46, 139, 87)),
    ("seashell", "Seashell", (255, 245, 238)),
    ("sienna", "Sienna", (160, 82, 45)),
    ("silver", "Silver", (192, 192, 192)),
    ("skyblue", "Sky Blue", (135, 206, 235)),
    ("slateblue", "Slate Blue", (106, 90, 205)),
    ("slategray", "Slate Gray", (112, 128, 144)),
    ("slategrey", "Slate Grey", (112, 128, 144)),
    ("snow", "Snow", (255, 250, 250)),
    ("springgreen", "Spring Green", (0, 255, 127)),
    ("steelblue", "Steel Blue", (70, 130, 180)),
    ("tan", "Tan", (210, 180, 140)),
    ("teal", "Teal", (0, 128, 128)),
    ("thistle", "Thistle", (216, 191, 216)),
    ("tomato", "Tomato", (255, 99, 71)),
    ("turquoise", "Turquoise", (64, 224, 208)),
    ("violet", "Violet", (238, 130, 238)),
    ("wheat", "Wheat", (245, 222, 179)),
    ("white", "White", (255, 255, 255)),
    ("whitesmoke", "White Smoke", (245, 245, 245)),
    ("yellow", "Yellow", (255, 255, 0)),
    ("yellowgreen", "Yellow Green", (154, 205, 50)),
];

fn lookup_css_named(key: &str) -> Option<&'static (u8, u8, u8)> {
    CSS_COLORS
        .binary_search_by_key(&key, |&(name, _, _)| name)
        .ok()
        .map(|i| &CSS_COLORS[i].2)
}

fn nearest_css_name(r: u8, g: u8, b: u8) -> Option<&'static str> {
    let mut best: Option<(&'static str, i32)> = None;
    for &(_, display, (cr, cg, cb)) in CSS_COLORS {
        let dr = r as i32 - cr as i32;
        let dg = g as i32 - cg as i32;
        let db = b as i32 - cb as i32;
        let dist_sq = dr * dr + dg * dg + db * db;
        if best.is_none_or(|(_, d)| dist_sq < d) {
            best = Some((display, dist_sq));
        }
    }
    // Only badge with a CSS color name when reasonably close (Euclidean <= 40).
    best.and_then(|(name, d)| if d <= 1600 { Some(name) } else { None })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn display(q: &str) -> Option<String> {
        let card = evaluate(q)?;
        match card.payload {
            Payload::Value { display, .. } => Some(display),
            Payload::Error { .. } => None,
        }
    }

    #[test]
    fn bare_color_names_never_hijack_app_search() {
        for word in ["red", "blue", "orange", "teal", "tomato", "black", "white"] {
            assert!(evaluate(word).is_none(), "{word} must not preview alone");
        }
    }

    #[test]
    fn parses_hex_3_4_6_8_and_sets_swatch() {
        let c6 = evaluate("#ff6347").expect("tomato hex");
        assert_eq!(c6.color_swatch, Some([255, 99, 71, 255]));
        assert_eq!(c6.target_badge.as_deref(), Some("Tomato"));
        assert_eq!(display("#ff6347").as_deref(), Some("rgb(255, 99, 71)"));

        let c3 = evaluate("#f80").expect("short hex");
        assert_eq!(c3.color_swatch, Some([255, 136, 0, 255]));

        let c4 = evaluate("#f808").expect("short alpha hex");
        assert_eq!(c4.color_swatch, Some([255, 136, 0, 136]));

        let c8 = evaluate("#ff636380").expect("8-digit hex");
        assert_eq!(c8.color_swatch, Some([255, 99, 99, 128]));
    }

    #[test]
    fn converts_hex_to_rgb_hsl_hwb_hsv_lab_lch_oklab_oklch_cmyk() {
        assert_eq!(
            display("#12F999 as rgb").as_deref(),
            Some("rgb(18, 249, 153)")
        );
        assert_eq!(
            display("#12F999 as rgba").as_deref(),
            Some("rgba(18, 249, 153, 1)")
        );
        assert_eq!(
            display("#12F999 as hsl").as_deref(),
            Some("hsl(155, 95%, 52%)")
        );
        assert!(display("#12F999 as oklch")
            .unwrap()
            .starts_with("oklch(0.86"));
        assert!(display("#12F999 as oklab")
            .unwrap()
            .starts_with("oklab(0.86"));
        assert!(display("#12F999 as lab").unwrap().starts_with("lab("));
        assert!(display("#12F999 as lch").unwrap().starts_with("lch("));
        assert!(display("#12F999 as hwb").unwrap().starts_with("hwb(155 "));
        assert!(display("#12F999 as hsv").unwrap().starts_with("hsv(155, "));
        assert!(display("#12F999 as hsb").unwrap().starts_with("hsb(155, "));
        assert_eq!(
            display("#12F999 as cmyk").as_deref(),
            Some("cmyk(93%, 0%, 39%, 2%)")
        );
        assert_eq!(
            display("#12F999 as hex with alpha").as_deref(),
            Some("#12F999FF")
        );
    }

    #[test]
    fn supports_css4_slash_alpha_and_named_expressions() {
        let css4 = evaluate("rgb(255 99 99 / 50%)").expect("css4 rgb");
        assert_eq!(css4.color_swatch, Some([255, 99, 99, 128]));
        assert_eq!(
            display("rgb(255 99 99 / 50%)").as_deref(),
            Some("#FF636380")
        );

        let hsl4 = evaluate("hsl(0 100% 69%)").expect("css4 hsl");
        assert_eq!(hsl4.color_swatch.unwrap()[0], 255);

        let named_alpha = evaluate("tomato at 50%").expect("tomato at 50%");
        assert_eq!(named_alpha.color_swatch, Some([255, 99, 71, 128]));
        assert_eq!(display("tomato at 50%").as_deref(), Some("#FF634780"));

        assert_eq!(display("rebeccapurple as hex").as_deref(), Some("#663399"));
        assert_eq!(
            display("cornflowerblue to rgb").as_deref(),
            Some("rgb(100, 149, 237)")
        );
    }

    #[test]
    fn round_trips_oklch_lab_hwb_cmyk_back_to_hex() {
        let oklch = display("#FF6347 as oklch").unwrap();
        let back_hex = display(&format!("{oklch} as hex")).unwrap();
        let orig = evaluate("#FF6347").unwrap().color_swatch.unwrap();
        let rt = evaluate(&back_hex).unwrap().color_swatch.unwrap();
        for i in 0..3 {
            assert!((orig[i] as i32 - rt[i] as i32).abs() <= 2);
        }

        let lab = display("#12F999 as lab").unwrap();
        let rt_lab = evaluate(&format!("{lab} as hex"))
            .unwrap()
            .color_swatch
            .unwrap();
        let orig2 = evaluate("#12F999").unwrap().color_swatch.unwrap();
        for i in 0..3 {
            assert!((orig2[i] as i32 - rt_lab[i] as i32).abs() <= 2);
        }

        assert_eq!(
            display("cmyk(0%, 100%, 100%, 0%) as hex").as_deref(),
            Some("#FF0000")
        );
        assert_eq!(display("hwb(120 0% 0%) as hex").as_deref(), Some("#00FF00"));
    }

    #[test]
    fn rejects_invalid_hex_and_bad_functions() {
        assert!(evaluate("#12345").is_none());
        assert!(evaluate("#zzzzzz").is_none());
        assert!(evaluate("rgb(300, 0, 0)").is_none());
        assert!(evaluate("notacolor(1, 2, 3)").is_none());
    }
}
