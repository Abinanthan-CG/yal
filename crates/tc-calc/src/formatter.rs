//! The number formatting of the calculator.
//!
//! Mirrors upstream `CalcFormatter.swift`. The formatting is hand-rolled and
//! locale-independent, so every locale renders identically.

/// Every integer up to 2^53 is exactly representable as a double.
const MAX_EXACT_INTEGER: f64 = 9_007_199_254_740_992.0;

/// The steps a timespan is broken into, largest first.
const TIMESPAN_STEPS: [(f64, &str); 5] = [
    (604_800.0, "wk"),
    (86_400.0, "day"),
    (3_600.0, "hr"),
    (60.0, "min"),
    (1.0, "s"),
];

/// The formatter of the calculator's numbers, as upstream's `CalcFormatter`.
pub struct Formatter;

impl Formatter {
    /// The expression as the card echoes it, as upstream's `CalcFormatter.expression`.
    ///
    /// The spacing is collapsed and the operators are shown with their pretty glyphs.
    pub fn expression(query: &str) -> String {
        Self::collapsed(query).replace('*', "×").replace('/', "÷")
    }

    /// The query with its spacing collapsed, as upstream's `CalcResult.phrase` shows it.
    ///
    /// The operators keep the glyphs they were typed with.
    pub fn collapsed(query: &str) -> String {
        query.split_whitespace().collect::<Vec<&str>>().join(" ")
    }

    /// The human-facing form of a number, as upstream's `CalcFormatter.display`.
    ///
    /// It is the copy text with thousands separators: at most ten significant digits,
    /// trailing zeros trimmed.
    pub fn display(value: f64) -> String {
        Self::grouped(&Self::copy_text(value))
    }

    /// The form of a number that lands on the pasteboard, as upstream's
    /// `CalcFormatter.copyText`.
    ///
    /// Every integer up to 2^53 is written out in full; past that the precision is
    /// genuinely gone, so exponent form is the honest answer there.
    pub fn copy_text(value: f64) -> String {
        let normalized = if value == 0.0 { 0.0 } else { value };
        if normalized.round() == normalized && normalized.abs() <= MAX_EXACT_INTEGER {
            return (normalized as i64).to_string();
        }
        format_g(normalized, 10)
    }

    /// The money form of a number, as upstream's `CalcFormatter.currency`.
    ///
    /// Two decimals, widening below a cent. Never `%g`.
    pub fn currency(value: f64) -> String {
        let magnitude = value.abs();
        // Below ~1e-9 the digits are noise, and a literal "0.00" avoids "-0.00".
        if magnitude < 1e-9 {
            return "0.00".to_string();
        }
        if magnitude >= 0.01 {
            return format!("{value:.2}");
        }
        let decimals = (3 - magnitude.log10().floor() as i32).max(0) as usize;
        let mut text = format!("{value:.decimals$}");
        while text.ends_with('0') {
            text.pop();
        }
        text
    }

    /// Whole feet plus the remaining inches, as upstream's
    /// `CalcFormatter.compoundFeetInches`.
    pub fn compound_feet_inches(feet: f64) -> String {
        let sign = if feet < 0.0 { "-" } else { "" };
        let magnitude = feet.abs();
        let whole_feet = magnitude.trunc();
        let inches = (magnitude - whole_feet) * 12.0;
        let feet_part = if whole_feet == 0.0 {
            String::new()
        } else {
            format!(
                "{sign}{} {}",
                Self::display(whole_feet),
                if whole_feet == 1.0 { "foot" } else { "feet" }
            )
        };
        let inch_text = Self::display(inches);
        let inch_part = format!(
            "{inch_text} {}",
            if inch_text == "1" { "inch" } else { "inches" }
        );
        if feet_part.is_empty() {
            format!("{sign}{inch_part}")
        } else {
            format!("{feet_part} {inch_part}")
        }
    }

    /// Seconds as the largest units that fit, as upstream's `CalcFormatter.timespan`.
    ///
    /// `8,700` becomes `2 hr 25 min`.
    pub fn timespan(seconds: f64) -> String {
        if !seconds.is_finite() {
            return Self::display(seconds);
        }
        let sign = if seconds < 0.0 { "-" } else { "" };
        let mut remainder = seconds.abs().round();
        let mut parts: Vec<String> = Vec::new();
        for (step, symbol) in TIMESPAN_STEPS {
            if remainder >= step {
                let count = (remainder / step).trunc();
                remainder -= count * step;
                parts.push(format!(
                    "{} {symbol}",
                    Self::grouped(&format!("{count:.0}"))
                ));
            }
        }
        // Sub-second input has no whole part to show, so it keeps its own precision.
        if parts.is_empty() {
            return format!("{} s", Self::display(seconds));
        }
        format!("{sign}{}", parts.join(" "))
    }

    /// Inserts a `,` every three integer digits, as upstream's `CalcFormatter.grouped`.
    ///
    /// Exponent-form strings pass through untouched.
    pub fn grouped(text: &str) -> String {
        let bytes = text.as_bytes();
        if bytes.contains(&b'e') || bytes.contains(&b'E') {
            return text.to_string();
        }
        let sign_count = usize::from(bytes.first() == Some(&b'-'));
        let integer = bytes.iter().take_while(|byte| **byte != b'.').count();
        if integer - sign_count <= 3 {
            return text.to_string();
        }
        let mut output: Vec<u8> = Vec::with_capacity(bytes.len() + integer / 3);
        for (index, byte) in bytes.iter().enumerate() {
            if index > sign_count && index < integer && (integer - index) % 3 == 0 {
                output.push(b',');
            }
            output.push(*byte);
        }
        String::from_utf8(output).unwrap_or_else(|_| text.to_string())
    }

    /// The way upstream's `String(Double)` spells a number.
    ///
    /// Swift prints the shortest round-tripping digits, positionally for an exponent of
    /// -4 to 16 and in exponential form otherwise, and keeps a trailing `.0`.
    pub fn number_text(value: f64) -> String {
        if value.is_nan() {
            return "nan".to_string();
        }
        if value.is_infinite() {
            return if value < 0.0 {
                "-inf".to_string()
            } else {
                "inf".to_string()
            };
        }
        if value == 0.0 {
            return "0.0".to_string();
        }
        let exponential = format!("{value:e}");
        let (mantissa, exponent) = exponential
            .split_once('e')
            .unwrap_or((exponential.as_str(), "0"));
        let exponent: i32 = exponent.parse().unwrap_or(0);
        if (-4..=16).contains(&exponent) {
            let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
            positional(&digits, exponent)
        } else {
            let sign = if exponent < 0 { '-' } else { '+' };
            format!("{mantissa}e{sign}{:02}", exponent.abs())
        }
    }
}

/// Formats a double the way C's `%.*g` does, which is what upstream's
/// `String(format:)` produces.
fn format_g(value: f64, precision: usize) -> String {
    if value.is_nan() {
        return "nan".to_string();
    }
    if value.is_infinite() {
        return if value < 0.0 {
            "-inf".to_string()
        } else {
            "inf".to_string()
        };
    }
    let places = if precision == 0 { 1 } else { precision };
    // The exponent the value takes in `%e` style with `places - 1` fractional digits.
    let exponential = format!("{value:.places$e}", places = places - 1);
    let (mantissa, exponent) = exponential
        .split_once('e')
        .unwrap_or((exponential.as_str(), "0"));
    let exponent: i32 = exponent.parse().unwrap_or(0);
    if places as i32 > exponent && exponent >= -4 {
        let decimals = (places as i32 - 1 - exponent) as usize;
        trim_trailing_zeros(&format!("{value:.decimals$}"))
    } else {
        let mantissa = trim_trailing_zeros(mantissa);
        let sign = if exponent < 0 { '-' } else { '+' };
        format!("{mantissa}e{sign}{:02}", exponent.abs())
    }
}

/// Drops the trailing zeros of a formatted number, and the point when it is left alone.
fn trim_trailing_zeros(text: &str) -> String {
    if !text.contains('.') {
        return text.to_string();
    }
    let trimmed = text.trim_end_matches('0');
    trimmed.strip_suffix('.').unwrap_or(trimmed).to_string()
}

/// Writes digits and an exponent the way Swift spells a double positionally.
fn positional(digits: &str, exponent: i32) -> String {
    if exponent >= 0 {
        let whole = exponent as usize + 1;
        if digits.len() > whole {
            format!("{}.{}", &digits[..whole], &digits[whole..])
        } else {
            format!("{}{}.0", digits, "0".repeat(whole - digits.len()))
        }
    } else {
        format!("0.{}{}", "0".repeat((-exponent - 1) as usize), digits)
    }
}
