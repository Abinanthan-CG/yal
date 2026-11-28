//! Color picker formatting, WCAG 2.1 contrast analysis, and tint/shade ramp helpers.

use crate::color::{ColorFormat, ColorValue};

/// Construct an opaque [`ColorValue`] from 8-bit sRGB components.
pub fn color_from_rgb(r: u8, g: u8, b: u8) -> ColorValue {
    ColorValue::new(
        f64::from(r) / 255.0,
        f64::from(g) / 255.0,
        f64::from(b) / 255.0,
        1.0,
    )
}

/// Convert a [`ColorValue`] to 8-bit RGBA bytes, clamping finite channels to
/// `0.0..=1.0` and mapping non-finite channels to `0`.
pub fn color_to_rgba_u8(color: ColorValue) -> [u8; 4] {
    [
        channel_to_u8(color.red),
        channel_to_u8(color.green),
        channel_to_u8(color.blue),
        channel_to_u8(color.alpha),
    ]
}

fn channel_to_u8(value: f64) -> u8 {
    let clamped = if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    };
    (clamped * 255.0).round() as u8
}

/// Format a [`ColorValue`] as `"rgb(r, g, b)"` when opaque or `"rgba(r, g, b, a)"`
/// when translucent.
pub fn format_color_rgb(color: ColorValue) -> String {
    if color.has_alpha() {
        ColorFormat::Rgba.string(&color)
    } else {
        let [r, g, b, _] = color_to_rgba_u8(color);
        format!("rgb({r}, {g}, {b})")
    }
}

/// Output notation choices exposed by the interactive color inspector / picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum PickerColorFormat {
    #[default]
    Hex,
    Rgb,
    Hsl,
    Oklch,
}

impl PickerColorFormat {
    /// All picker formats in cycle order.
    pub const ALL: [PickerColorFormat; 4] = [Self::Hex, Self::Rgb, Self::Hsl, Self::Oklch];

    /// Advance to the next format in [`PickerColorFormat::ALL`].
    pub fn next(self) -> Self {
        match self {
            Self::Hex => Self::Rgb,
            Self::Rgb => Self::Hsl,
            Self::Hsl => Self::Oklch,
            Self::Oklch => Self::Hex,
        }
    }

    /// Step backward to the previous format in [`PickerColorFormat::ALL`].
    pub fn prev(self) -> Self {
        match self {
            Self::Hex => Self::Oklch,
            Self::Rgb => Self::Hex,
            Self::Hsl => Self::Rgb,
            Self::Oklch => Self::Hsl,
        }
    }

    /// Stable lowercase identifier for persistence and IPC.
    pub fn key(self) -> &'static str {
        match self {
            Self::Hex => "hex",
            Self::Rgb => "rgb",
            Self::Hsl => "hsl",
            Self::Oklch => "oklch",
        }
    }

    /// Human-readable uppercase badge label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Hex => "HEX",
            Self::Rgb => "RGB",
            Self::Hsl => "HSL",
            Self::Oklch => "OKLCH",
        }
    }

    /// Parse a format key (case-insensitive, trimmed).
    pub fn parse(raw: &str) -> Option<Self> {
        let trimmed = raw.trim();
        if trimmed.eq_ignore_ascii_case("hex") {
            Some(Self::Hex)
        } else if trimmed.eq_ignore_ascii_case("rgb") {
            Some(Self::Rgb)
        } else if trimmed.eq_ignore_ascii_case("hsl") {
            Some(Self::Hsl)
        } else if trimmed.eq_ignore_ascii_case("oklch") {
            Some(Self::Oklch)
        } else {
            None
        }
    }

    /// Format `color` using this picker notation.
    pub fn format(self, color: ColorValue) -> String {
        match self {
            Self::Hex => ColorFormat::Hex.string(&color),
            Self::Rgb => format_color_rgb(color),
            Self::Hsl => ColorFormat::Hsl.string(&color),
            Self::Oklch => ColorFormat::Oklch.string(&color),
        }
    }
}

/// WCAG 2.1 relative luminance (`0.0..=1.0`) using the sRGB `0.04045` linearization threshold.
pub fn relative_luminance(color: ColorValue) -> f64 {
    let r = srgb_to_linear(color.red);
    let g = srgb_to_linear(color.green);
    let b = srgb_to_linear(color.blue);
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

fn srgb_to_linear(channel: f64) -> f64 {
    let c = sanitize_unit(channel);
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn sanitize_unit(value: f64) -> f64 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// WCAG 2.1 contrast ratio (`1.0..=21.0`) between two colours.
pub fn contrast_ratio(a: ColorValue, b: ColorValue) -> f64 {
    let l1 = relative_luminance(a);
    let l2 = relative_luminance(b);
    let (lighter, darker) = if l1 >= l2 { (l1, l2) } else { (l2, l1) };
    (lighter + 0.05) / (darker + 0.05)
}

/// WCAG 2.1 conformance tier for a contrast ratio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WcagLevel {
    Fail,
    AaLarge,
    Aa,
    Aaa,
}

impl WcagLevel {
    /// Classify a contrast ratio into its highest passing WCAG 2.1 tier.
    pub fn from_ratio(ratio: f64) -> Self {
        if !ratio.is_finite() {
            Self::Fail
        } else if ratio >= 7.0 {
            Self::Aaa
        } else if ratio >= 4.5 {
            Self::Aa
        } else if ratio >= 3.0 {
            Self::AaLarge
        } else {
            Self::Fail
        }
    }

    /// Human-readable WCAG badge label.
    pub fn badge(self) -> &'static str {
        match self {
            Self::Aaa => "AAA",
            Self::Aa => "AA",
            Self::AaLarge => "AA Large",
            Self::Fail => "Fail",
        }
    }
}

/// Contrast ratios and WCAG grades of a colour against pure white and pure black.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContrastReport {
    pub vs_white: f64,
    pub vs_white_level: WcagLevel,
    pub vs_black: f64,
    pub vs_black_level: WcagLevel,
}

/// Compute a [`ContrastReport`] comparing `color` against pure white and pure black.
pub fn contrast_report(color: ColorValue) -> ContrastReport {
    let white = color_from_rgb(255, 255, 255);
    let black = color_from_rgb(0, 0, 0);
    let vs_white = contrast_ratio(color, white);
    let vs_black = contrast_ratio(color, black);
    ContrastReport {
        vs_white,
        vs_white_level: WcagLevel::from_ratio(vs_white),
        vs_black,
        vs_black_level: WcagLevel::from_ratio(vs_black),
    }
}

/// One swatch in a 9-step tint/shade ramp (`"100"`..`"900"`).
#[derive(Debug, Clone, PartialEq)]
pub struct RampStep {
    pub label: &'static str,
    pub color: ColorValue,
    pub hex: String,
    pub rgba: [u8; 4],
}

/// Generate a 9-step tint and shade ramp (`"100"`..`"900"`) centered on `color` at `"500"`.
pub fn tint_shade_ramp(color: ColorValue) -> Vec<RampStep> {
    const STEPS: [(&str, f64); 9] = [
        ("100", 0.80),
        ("200", 0.60),
        ("300", 0.40),
        ("400", 0.20),
        ("500", 0.00),
        ("600", -0.20),
        ("700", -0.40),
        ("800", -0.60),
        ("900", -0.80),
    ];

    let r = sanitize_unit(color.red);
    let g = sanitize_unit(color.green);
    let b = sanitize_unit(color.blue);
    let a = sanitize_unit(color.alpha);

    STEPS
        .into_iter()
        .map(|(label, delta)| {
            let step_color = if delta == 0.0 {
                ColorValue::new(r, g, b, a)
            } else if delta > 0.0 {
                ColorValue::new(
                    r + (1.0 - r) * delta,
                    g + (1.0 - g) * delta,
                    b + (1.0 - b) * delta,
                    a,
                )
            } else {
                let keep = 1.0 + delta;
                ColorValue::new(r * keep, g * keep, b * keep, a)
            };
            RampStep {
                label,
                color: step_color,
                hex: ColorFormat::Hex.string(&step_color),
                rgba: color_to_rgba_u8(step_color),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wcag_black_and_white_contrast_is_21_aaa() {
        let black = color_from_rgb(0, 0, 0);
        let white = color_from_rgb(255, 255, 255);
        let ratio = contrast_ratio(black, white);
        assert!((ratio - 21.0).abs() < 1e-9);
        assert_eq!(WcagLevel::from_ratio(ratio), WcagLevel::Aaa);
        assert_eq!(WcagLevel::from_ratio(ratio).badge(), "AAA");
        assert_eq!(contrast_ratio(white, black), ratio);
    }

    #[test]
    fn wcag_mid_gray_and_levels() {
        let mid_gray = color_from_rgb(118, 118, 118);
        let report = contrast_report(mid_gray);
        assert!(report.vs_white >= 4.5 && report.vs_white < 4.6);
        assert_eq!(report.vs_white_level, WcagLevel::Aa);
        assert!(report.vs_black >= 4.5 && report.vs_black < 4.7);
        assert_eq!(report.vs_black_level, WcagLevel::Aa);

        assert_eq!(WcagLevel::from_ratio(3.2), WcagLevel::AaLarge);
        assert_eq!(WcagLevel::from_ratio(3.2).badge(), "AA Large");
        assert_eq!(WcagLevel::from_ratio(2.9), WcagLevel::Fail);
        assert_eq!(WcagLevel::from_ratio(f64::NAN), WcagLevel::Fail);
    }

    #[test]
    fn tint_shade_ramp_endpoints_and_center_identity() {
        let base = ColorValue::new(0.2, 0.4, 0.8, 0.75);
        let ramp = tint_shade_ramp(base);
        assert_eq!(ramp.len(), 9);
        assert_eq!(ramp[0].label, "100");
        assert_eq!(ramp[4].label, "500");
        assert_eq!(ramp[8].label, "900");
        assert_eq!(ramp[4].color, base);

        for step in &ramp {
            assert!((step.color.alpha - 0.75).abs() < 1e-12);
            assert_eq!(step.rgba[3], 191);
        }

        assert!(ramp[0].color.red > base.red);
        assert!(ramp[0].color.green > base.green);
        assert!(ramp[0].color.blue > base.blue);
        assert!(ramp[8].color.red < base.red);
        assert!(ramp[8].color.green < base.green);
        assert!(ramp[8].color.blue < base.blue);
    }

    #[test]
    fn tint_shade_ramp_sanitizes_out_of_range_and_nan_channels() {
        let raw = ColorValue {
            red: 2.0,
            green: -0.5,
            blue: f64::NAN,
            alpha: 1.5,
        };
        let ramp = tint_shade_ramp(raw);
        assert_eq!(ramp.len(), 9);

        let center = &ramp[4];
        assert_eq!(center.label, "500");
        assert_eq!(center.color, ColorValue::new(1.0, 0.0, 0.0, 1.0));
        assert_eq!(center.hex, "#FF0000");
        assert_eq!(center.rgba, [255, 0, 0, 255]);

        for step in &ramp {
            assert!(step.color.red.is_finite() && (0.0..=1.0).contains(&step.color.red));
            assert!(step.color.green.is_finite() && (0.0..=1.0).contains(&step.color.green));
            assert!(step.color.blue.is_finite() && (0.0..=1.0).contains(&step.color.blue));
            assert!(step.color.alpha.is_finite() && (0.0..=1.0).contains(&step.color.alpha));
            assert_eq!(step.rgba[3], 255);
            assert_eq!(step.hex.len(), 7);
        }
    }

    #[test]
    fn format_color_rgb_opaque_and_translucent() {
        let opaque = color_from_rgb(255, 87, 51);
        assert_eq!(format_color_rgb(opaque), "rgb(255, 87, 51)");

        let translucent = ColorValue::new(1.0, 0.0, 0.5, 0.5);
        assert_eq!(format_color_rgb(translucent), "rgba(255, 0, 128, 0.5)");

        let non_finite = ColorValue {
            red: f64::NAN,
            green: f64::INFINITY,
            blue: -1.0,
            alpha: 2.0,
        };
        assert_eq!(color_to_rgba_u8(non_finite), [0, 0, 0, 255]);
    }

    #[test]
    fn picker_color_format_cycling_parse_and_serde() {
        let mut fmt = PickerColorFormat::default();
        assert_eq!(fmt, PickerColorFormat::Hex);
        assert_eq!(fmt.key(), "hex");
        assert_eq!(fmt.label(), "HEX");

        fmt = fmt.next();
        assert_eq!(fmt, PickerColorFormat::Rgb);
        fmt = fmt.next();
        assert_eq!(fmt, PickerColorFormat::Hsl);
        fmt = fmt.next();
        assert_eq!(fmt, PickerColorFormat::Oklch);
        fmt = fmt.next();
        assert_eq!(fmt, PickerColorFormat::Hex);
        assert_eq!(fmt.prev(), PickerColorFormat::Oklch);

        for item in PickerColorFormat::ALL {
            let parsed = PickerColorFormat::parse(&format!("  {}  ", item.label()));
            assert_eq!(parsed, Some(item));
            let json = serde_json::to_string(&item).unwrap();
            let roundtrip: PickerColorFormat = serde_json::from_str(&json).unwrap();
            assert_eq!(roundtrip, item);
        }
        assert_eq!(PickerColorFormat::parse("cmyk"), None);

        let coral = color_from_rgb(255, 87, 51);
        assert_eq!(PickerColorFormat::Hex.format(coral), "#FF5733");
        assert_eq!(PickerColorFormat::Rgb.format(coral), "rgb(255, 87, 51)");
        assert!(PickerColorFormat::Hsl.format(coral).starts_with("hsl("));
        assert!(PickerColorFormat::Oklch.format(coral).starts_with("oklch("));
    }
}
