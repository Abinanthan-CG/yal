//! The CSS Color 4 spaces: Oklab and Oklch.
//!
//! Upstream oracle: `Features/Clipboard/Model/ColorSpaces.swift`, an
//! extension on `ColorValue` that adds the Oklab conversions and the inverse
//! `init(lightness:chroma:hue:alpha:)`.

use crate::color::ColorValue;

/// A colour in Oklch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Oklch {
    /// The lightness, 0…1.
    pub l: f64,
    /// The chroma, 0…0.4 or so.
    pub c: f64,
    /// The hue in degrees, 0…360.
    pub h: f64,
}

/// A colour in Oklab.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Oklab {
    /// The lightness, 0…1.
    pub l: f64,
    /// The green-red axis, roughly -0.4…0.4.
    pub a: f64,
    /// The blue-yellow axis, roughly -0.4…0.4.
    pub b: f64,
}

impl ColorValue {
    /// The colour in Oklch.
    ///
    /// Upstream `ColorValue.oklch`.
    pub fn oklch(&self) -> Oklch {
        let oklab = self.oklab();
        Oklch {
            l: oklab.l,
            c: (oklab.a * oklab.a + oklab.b * oklab.b).sqrt(),
            h: hue_degrees(oklab.a, oklab.b),
        }
    }

    /// The colour in Oklab.
    ///
    /// Upstream `ColorValue.oklab`: the same idea as Lab, fitted so equal
    /// steps look equal.
    pub fn oklab(&self) -> Oklab {
        let red = linear(self.red);
        let green = linear(self.green);
        let blue = linear(self.blue);
        let long = cube_root(0.4122215 * red + 0.5363325 * green + 0.0514460 * blue);
        let medium = cube_root(0.2119035 * red + 0.6806995 * green + 0.1073970 * blue);
        let short = cube_root(0.0883025 * red + 0.2817188 * green + 0.6299787 * blue);
        Oklab {
            l: 0.2104543 * long + 0.7936178 * medium - 0.0040720 * short,
            a: 1.9779985 * long - 2.4285922 * medium + 0.4505937 * short,
            b: 0.0259040 * long + 0.7827718 * medium - 0.8086758 * short,
        }
    }

    /// A colour from its Oklch components.
    ///
    /// Upstream `ColorValue.init(lightness:chroma:hue:alpha:)`: the inverse of
    /// `oklch`, because a colour picker states its swatch in the space rather
    /// than in hex.
    pub fn from_oklch(lightness: f64, chroma: f64, hue: f64, alpha: f64) -> ColorValue {
        let radians = hue * std::f64::consts::PI / 180.0;
        let a = chroma * radians.cos();
        let b = chroma * radians.sin();
        let long = cubed(lightness + 0.3963377774 * a + 0.2158037573 * b);
        let medium = cubed(lightness - 0.1055613458 * a - 0.0638541728 * b);
        let short = cubed(lightness - 0.0894841775 * a - 1.2914855480 * b);
        ColorValue::new(
            gamma(4.0767416621 * long - 3.3077115913 * medium + 0.2309699292 * short),
            gamma(-1.2684380046 * long + 2.6097574011 * medium - 0.3413193965 * short),
            gamma(-0.0041960863 * long - 0.7034186147 * medium + 1.7076147010 * short),
            alpha,
        )
    }
}

/// Oklab is defined on light, not on the gamma-encoded numbers a hex triplet
/// carries. Upstream `ColorValue.linear(_:)`.
fn linear(channel: f64) -> f64 {
    if channel <= 0.04045 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

/// The inverse of `linear`; a channel below the split can be negative, and
/// `pow` answers NaN. Upstream `ColorValue.gamma(_:)`.
fn gamma(channel: f64) -> f64 {
    if channel <= 0.0031308 {
        channel * 12.92
    } else {
        1.055 * channel.powf(1.0 / 2.4) - 0.055
    }
}

/// `pow` is undefined for a negative base at a fractional exponent, and a
/// channel can be one. Upstream `ColorValue.cubeRoot(_:)`.
fn cube_root(value: f64) -> f64 {
    if value < 0.0 {
        -(-value).powf(1.0 / 3.0)
    } else {
        value.powf(1.0 / 3.0)
    }
}

/// Upstream `ColorValue.cubed(_:)`.
fn cubed(value: f64) -> f64 {
    value * value * value
}

/// Zero for a neutral: `atan2` over two rounding errors would still name a
/// direction. Upstream `ColorValue.hueDegrees(_:_:)`.
fn hue_degrees(a: f64, b: f64) -> f64 {
    if a.abs() <= 1e-6 && b.abs() <= 1e-6 {
        return 0.0;
    }
    let degrees = b.atan2(a) * 180.0 / std::f64::consts::PI;
    if degrees < 0.0 {
        degrees + 360.0
    } else {
        degrees
    }
}
