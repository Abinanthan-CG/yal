//! The math functions and constants of the calculator.
//!
//! Mirrors upstream `CalcMath.swift`.

use crate::token::Operator;

/// Every integer up to 2^53 is exactly representable as a double.
const MAX_EXACT_INTEGER: f64 = 9_007_199_254_740_992.0;

/// The functions, constants and bit operations of the calculator.
///
/// Mirrors upstream `CalcMath`.
pub struct Math;

impl Math {
    /// The names that take several arguments, as upstream's `CalcMath.multipleArguments`.
    pub const MULTIPLE_ARGUMENTS: [&'static str; 19] = [
        "hypot", "round", "log", "gcd", "lcm", "atan2", "pow", "root", "fmod", "min", "max", "sum",
        "avg", "mean", "average", "median", "count", "stddev", "stdev",
    ];

    /// The names that keep the unit of their first argument, as upstream's
    /// `CalcMath.measurements`.
    pub const MEASUREMENTS: [&'static str; 11] = [
        "hypot", "round", "min", "max", "sum", "avg", "mean", "average", "median", "stddev",
        "stdev",
    ];

    /// Whether a name is a function of the calculator, as upstream's `CalcMath.isFunction`.
    pub fn is_function(name: &str) -> bool {
        single_argument(name).is_some() || Self::MULTIPLE_ARGUMENTS.contains(&name)
    }

    /// Whether a name is a one-argument function, as upstream's `CalcMath.functions`.
    ///
    /// Upstream's parser reaches for the one-argument functions by name, so a name that
    /// only takes several arguments is not one of them.
    pub fn is_single_argument(name: &str) -> bool {
        single_argument(name).is_some()
    }

    /// The value of a named constant, as upstream's `CalcMath.constants`.
    pub fn constant(name: &str) -> Option<f64> {
        Some(match name {
            "pi" | "π" => std::f64::consts::PI,
            "e" => std::f64::consts::E,
            "tau" | "τ" => 2.0 * std::f64::consts::PI,
            "phi" => (1.0 + 5.0_f64.sqrt()) / 2.0,
            "half" => 0.5,
            _ => return None,
        })
    }

    /// Applies the one-argument function a name stands for.
    ///
    /// Upstream keeps these in `CalcMath.functions`; upstream's answer is `nil` for a
    /// name that is not one of them.
    pub fn apply_single(name: &str, value: f64) -> Option<f64> {
        single_argument(name).map(|function| function(value))
    }

    /// The factorial of a non-negative integer, as upstream's `CalcMath.factorial`.
    ///
    /// 170! is the last value representable as a double.
    pub fn factorial(value: f64) -> Option<f64> {
        if value < 0.0 || value.round() != value || value > 170.0 {
            return None;
        }
        let mut result = 1.0;
        let mut next = 2.0;
        while next <= value {
            result *= next;
            next += 1.0;
        }
        Some(result)
    }

    /// Applies a named function to its arguments, as upstream's `CalcMath.evaluate`.
    ///
    /// Upstream answers `nil` for an unknown name, a wrong number of arguments, or a
    /// result that is not finite.
    pub fn evaluate(name: &str, values: &[f64]) -> Option<f64> {
        if values.is_empty() || !values.iter().all(|value| value.is_finite()) {
            return None;
        }
        let first = values[0];
        let result = match name {
            "min" => values.iter().copied().fold(f64::INFINITY, f64::min),
            "max" => values.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            "sum" => values.iter().sum(),
            "avg" | "mean" | "average" => {
                let count = values.len() as f64;
                values.iter().fold(0.0, |sum, value| sum + value / count)
            }
            "median" => {
                let mut sorted = values.to_vec();
                sorted.sort_by(|left, right| {
                    left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal)
                });
                let middle = sorted.len() / 2;
                if sorted.len() % 2 == 1 {
                    sorted[middle]
                } else {
                    (sorted[middle - 1] + sorted[middle]) / 2.0
                }
            }
            "count" => values.len() as f64,
            "stddev" | "stdev" => {
                // The sample deviation, as spreadsheets' STDEV: a typed list is a sample,
                // not a population.
                if values.len() < 2 {
                    return None;
                }
                let count = values.len() as f64;
                let mean = values.iter().fold(0.0, |sum, value| sum + value / count);
                let squares = values
                    .iter()
                    .fold(0.0, |sum, value| sum + (value - mean) * (value - mean));
                (squares / (count - 1.0)).sqrt()
            }
            "hypot" => values
                .iter()
                .copied()
                .fold(0.0, |left: f64, right: f64| left.hypot(right)),
            "gcd" | "lcm" => return Self::greatest_common_multiple(name, values),
            _ => {
                if values.len() == 1 {
                    let function = single_argument(name)?;
                    function(first)
                } else {
                    if values.len() != 2 {
                        return None;
                    }
                    let second = values[1];
                    match name {
                        "round" => {
                            let digits = exact_integer(second)?;
                            if !(-308..=308).contains(&digits) {
                                return None;
                            }
                            let factor = 10.0_f64.powf(second);
                            let scaled = first * factor;
                            if scaled.is_finite() {
                                scaled.round() / factor
                            } else {
                                first
                            }
                        }
                        "log" => {
                            if first <= 0.0 || second <= 0.0 || second == 1.0 {
                                return None;
                            }
                            first.ln() / second.ln()
                        }
                        "atan2" => first.atan2(second),
                        "pow" => first.powf(second),
                        "root" => {
                            if second == 0.0 {
                                return None;
                            }
                            if first < 0.0 && second % 2.0 != 0.0 && second.round() == second {
                                -(-first).powf(1.0 / second)
                            } else {
                                first.powf(1.0 / second)
                            }
                        }
                        "fmod" => first % second,
                        _ => return None,
                    }
                }
            }
        };
        if result.is_finite() {
            Some(result)
        } else {
            None
        }
    }

    /// The greatest common divisor or the least common multiple, as upstream's
    /// `CalcMath.evaluate` does for `gcd` and `lcm`.
    fn greatest_common_multiple(name: &str, values: &[f64]) -> Option<f64> {
        let mut accumulator: i64 = if name == "gcd" { 0 } else { 1 };
        for value in values {
            let integer = exact_integer(*value)?;
            let positive = integer.abs();
            let mut left = accumulator;
            let mut right = positive;
            while right != 0 {
                let next = left % right;
                left = right;
                right = next;
            }
            if name == "gcd" {
                accumulator = left;
            } else if left == 0 {
                accumulator = 0;
            } else {
                accumulator = accumulator.checked_div(left)?.checked_mul(positive)?;
            }
        }
        if (accumulator.unsigned_abs() as f64) < MAX_EXACT_INTEGER {
            Some(accumulator as f64)
        } else {
            None
        }
    }

    /// A bit operation on two integers, as upstream's `CalcMath.bitwise`.
    ///
    /// Upstream answers `nil` when an operand is not an exact integer or the result
    /// leaves the exactly representable range.
    pub fn bitwise(op: Operator, left: f64, right: f64) -> Option<f64> {
        let lhs = exact_integer(left)?;
        let rhs = exact_integer(right)?;
        let result: i64 = match op {
            Operator::BitAnd => lhs & rhs,
            Operator::BitOr => lhs | rhs,
            Operator::BitXor => lhs ^ rhs,
            Operator::BitNot => !lhs,
            Operator::ShiftLeft => {
                if !(0..64).contains(&rhs) {
                    return None;
                }
                let shifted = lhs.wrapping_shl(rhs as u32);
                if shifted >> rhs != lhs {
                    return None;
                }
                shifted
            }
            Operator::ShiftRight => {
                if !(0..64).contains(&rhs) {
                    return None;
                }
                lhs >> rhs
            }
            _ => return None,
        };
        if (result.unsigned_abs() as f64) < MAX_EXACT_INTEGER {
            Some(result as f64)
        } else {
            None
        }
    }
}

/// The one-argument function a name stands for, as upstream's `CalcMath.functions`.
fn single_argument(name: &str) -> Option<fn(f64) -> f64> {
    Some(match name {
        "sqrt" => f64::sqrt,
        "log" => f64::log10,
        "ln" => f64::ln,
        "sin" => f64::sin,
        "cos" => f64::cos,
        "tan" => f64::tan,
        "abs" => f64::abs,
        "floor" => f64::floor,
        "ceil" => f64::ceil,
        "round" => f64::round,
        "cot" => |value| 1.0 / value.tan(),
        "sec" => |value| 1.0 / value.cos(),
        "csc" => |value| 1.0 / value.sin(),
        "asin" => f64::asin,
        "acos" => f64::acos,
        "atan" => f64::atan,
        "arcsin" => f64::asin,
        "arccos" => f64::acos,
        "arctan" => f64::atan,
        "sinh" => f64::sinh,
        "cosh" => f64::cosh,
        "tanh" => f64::tanh,
        "asinh" => f64::asinh,
        "acosh" => f64::acosh,
        "atanh" => f64::atanh,
        "cbrt" => f64::cbrt,
        "exp" => f64::exp,
        "log2" => f64::log2,
        "sign" => |value| {
            if value > 0.0 {
                1.0
            } else if value < 0.0 {
                -1.0
            } else {
                0.0
            }
        },
        "trunc" => f64::trunc,
        _ => return None,
    })
}

/// A double that is an exact integer inside the exactly representable range.
fn exact_integer(value: f64) -> Option<i64> {
    if value.abs() >= MAX_EXACT_INTEGER {
        return None;
    }
    if value.round() != value {
        return None;
    }
    Some(value as i64)
}
