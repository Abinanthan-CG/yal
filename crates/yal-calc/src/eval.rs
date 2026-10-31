//! Evaluation of parsed calculator expressions.
//!
//! Values are one of four shapes: a bare number, a number with a unit, an
//! amount of currency, or an instant in time. Arithmetic between shapes is
//! only defined where it is meaningful; anything else is reported as an
//! error rather than silently producing a number.
//!
//! Month- and year-shaped date arithmetic is *calendar* arithmetic, done
//! in the machine's local zone; the fixed-factor duration units (seconds
//! through weeks) stay exact durations. Currency codes are resolved
//! against the registry the evaluator was built with, so a registry the
//! store's rates table extended answers just as the bundled one does.

use chrono::{
    DateTime, Datelike, Days, Duration, Local, MappedLocalTime, Months, NaiveDate, NaiveDateTime,
    TimeZone, Utc,
};
use chrono_tz::Tz;

use crate::currency::{CurrencyRegistry, convert as convert_currency};
use crate::parser::{BinOp, Expr, UnaryOp};
use crate::units::{UnitRegistry, convert as convert_unit, to_base};

/// A computed value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// A plain number.
    Scalar(f64),
    /// A number with a unit, e.g. `10 km`.
    Quantity { amount: f64, unit: &'static crate::units::UnitDef },
    /// An amount of currency, e.g. `100 usd`.
    Currency { amount: f64, code: String },
    /// A point in time.
    Instant(DateTime<Utc>),
}

/// A failure while evaluating.
#[derive(Debug, Clone, PartialEq)]
pub struct EvalError {
    pub message: String,
}

impl std::fmt::Display for EvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for EvalError {}

/// The evaluation environment.
pub struct Evaluator {
    units: UnitRegistry,
    currencies: CurrencyRegistry,
    now: DateTime<Utc>,
}

impl Evaluator {
    /// An evaluator whose notion of "now" is the current instant.
    pub fn new() -> Self {
        Self {
            units: UnitRegistry::new(),
            currencies: CurrencyRegistry::new(),
            now: Utc::now(),
        }
    }

    /// An evaluator with a fixed notion of "now", for deterministic tests.
    pub fn at(now: DateTime<Utc>) -> Self {
        Self {
            units: UnitRegistry::new(),
            currencies: CurrencyRegistry::new(),
            now,
        }
    }

    /// An evaluator with a fixed notion of "now" and an explicit currency
    /// registry — the one the store's rates table has been folded into.
    pub fn with(now: DateTime<Utc>, currencies: CurrencyRegistry) -> Self {
        Self {
            units: UnitRegistry::new(),
            currencies,
            now,
        }
    }

    /// Evaluate an expression.
    pub fn eval(&self, expr: &Expr) -> Result<Value, EvalError> {
        match expr {
            Expr::Number(value) => Ok(Value::Scalar(*value)),
            Expr::Instant(at) => Ok(Value::Instant(*at)),

            Expr::Quantity { amount, unit } => Ok(Value::Quantity {
                amount: *amount,
                unit,
            }),

            Expr::Currency { amount, code } => Ok(Value::Currency {
                amount: *amount,
                code: code.clone(),
            }),

            Expr::Ident(name) => self.eval_ident(name),

            Expr::Unary { op, expr } => {
                let value = self.eval(expr)?;
                match op {
                    UnaryOp::Plus => Ok(value),
                    UnaryOp::Neg => self.negate(value),
                }
            }

            Expr::Percent(expr) => {
                let value = self.eval(expr)?;
                match value {
                    Value::Scalar(amount) => Ok(Value::Scalar(amount / 100.0)),
                    Value::Quantity { amount, unit } => Ok(Value::Quantity {
                        amount: amount / 100.0,
                        unit,
                    }),
                    Value::Currency { amount, code } => Ok(Value::Currency {
                        amount: amount / 100.0,
                        code: code.clone(),
                    }),
                    Value::Instant(_) => Err(EvalError {
                        message: "a date cannot be a percentage".into(),
                    }),
                }
            }

            Expr::Binary { op, lhs, rhs } => {
                let left = self.eval(lhs)?;
                let right = self.eval(rhs)?;
                self.binary(*op, left, right)
            }

            Expr::Call { name, args } => {
                let values: Result<Vec<Value>, EvalError> =
                    args.iter().map(|arg| self.eval(arg)).collect();
                let values = values?;
                self.call(name, values)
            }

            Expr::Convert { lhs, rhs } => {
                let left = self.eval(lhs)?;
                let right = self.eval(rhs)?;
                self.convert(left, right)
            }
        }
    }

    /// Resolve an identifier: a constant, `now`, `today`, or a currency code.
    fn eval_ident(&self, name: &str) -> Result<Value, EvalError> {
        match name {
            "pi" => Ok(Value::Scalar(std::f64::consts::PI)),
            "e" => Ok(Value::Scalar(std::f64::consts::E)),
            "tau" => Ok(Value::Scalar(std::f64::consts::TAU)),
            "now" => Ok(Value::Instant(self.now)),
            "today" => {
                let local = self.now.with_timezone(&Local);
                let start = local
                    .date_naive()
                    .and_hms_opt(0, 0, 0)
                    .ok_or_else(|| EvalError {
                        message: "could not resolve the start of today".into(),
                    })?;
                Ok(Value::Instant(
                    Local
                        .from_local_datetime(&start)
                        .earliest()
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or(self.now),
                ))
            }
            other => {
                // A weekday spelled out is the ISO index the `next` and
                // `last` functions take: Monday 0 through Sunday 6.
                if let Some(index) = weekday_index(other) {
                    return Ok(Value::Scalar(index));
                }
                if let Some(currency) = self.currencies.resolve(other) {
                    return Ok(Value::Currency {
                        amount: 1.0,
                        code: currency.code.clone(),
                    });
                }
                if let Some(unit) = self.units.resolve(other) {
                    return Ok(Value::Quantity { amount: 1.0, unit });
                }
                Err(EvalError {
                    message: format!("`{other}` is not a known constant"),
                })
            }
        }
    }

    fn negate(&self, value: Value) -> Result<Value, EvalError> {
        match value {
            Value::Scalar(amount) => Ok(Value::Scalar(-amount)),
            Value::Quantity { amount, unit } => Ok(Value::Quantity {
                amount: -amount,
                unit,
            }),
            Value::Currency { amount, code } => Ok(Value::Currency {
                amount: -amount,
                code: code.clone(),
            }),
            Value::Instant(_) => Err(EvalError {
                message: "a date cannot be negated".into(),
            }),
        }
    }

    fn binary(&self, op: BinOp, left: Value, right: Value) -> Result<Value, EvalError> {
        use Value::{Currency, Instant, Quantity, Scalar};

        match op {
            BinOp::Add => match (left, right) {
                (Scalar(a), Scalar(b)) => Ok(Scalar(a + b)),
                (Scalar(a), Quantity { amount, unit }) => {
                    Ok(Quantity { amount: a + amount, unit })
                }
                (Quantity { amount, unit }, Scalar(b)) => {
                    Ok(Quantity { amount: amount + b, unit })
                }
                (Quantity { amount: a, unit: ua }, Quantity { amount: b, unit: ub }) => {
                    if ua.dimension != ub.dimension {
                        return Err(EvalError {
                            message: format!(
                                "cannot add {} to {}",
                                ub.id, ua.id
                            ),
                        });
                    }
                    let b = convert_unit(b, ub, ua).ok_or_else(|| EvalError {
                        message: format!("cannot convert {} to {}", ub.id, ua.id),
                    })?;
                    Ok(Quantity { amount: a + b, unit: ua })
                }
                (Instant(at), Quantity { amount, unit }) => {
                    if unit.dimension != crate::units::Dimension::Time {
                        return Err(EvalError {
                            message: format!("`{}` is not a duration", unit.id),
                        });
                    }
                    let seconds = to_base(amount, unit);
                    Ok(Instant(at + Duration::milliseconds((seconds * 1000.0) as i64)))
                }
                (Quantity { amount, unit }, Instant(at)) => {
                    if unit.dimension != crate::units::Dimension::Time {
                        return Err(EvalError {
                            message: format!("`{}` is not a duration", unit.id),
                        });
                    }
                    let seconds = to_base(amount, unit);
                    Ok(Instant(at + Duration::milliseconds((seconds * 1000.0) as i64)))
                }
                (Currency { amount: a, code: ca }, Currency { amount: b, code: cb }) => {
                    if ca != cb {
                        return Err(EvalError {
                            message: format!("cannot add {cb} to {ca}"),
                        });
                    }
                    Ok(Currency { amount: a + b, code: ca })
                }
                (a, b) => Err(EvalError {
                    message: format!("cannot add {a:?} and {b:?}"),
                }),
            },

            BinOp::Sub => match (left, right) {
                (Scalar(a), Scalar(b)) => Ok(Scalar(a - b)),
                (Scalar(a), Quantity { amount, unit }) => {
                    Ok(Quantity { amount: a - amount, unit })
                }
                (Quantity { amount, unit }, Scalar(b)) => {
                    Ok(Quantity { amount: amount - b, unit })
                }
                (Quantity { amount: a, unit: ua }, Quantity { amount: b, unit: ub }) => {
                    if ua.dimension != ub.dimension {
                        return Err(EvalError {
                            message: format!("cannot subtract {} from {}", ub.id, ua.id),
                        });
                    }
                    let b = convert_unit(b, ub, ua).ok_or_else(|| EvalError {
                        message: format!("cannot convert {} to {}", ub.id, ua.id),
                    })?;
                    Ok(Quantity { amount: a - b, unit: ua })
                }
                (Instant(a), Instant(b)) => {
                    let seconds = (a - b).num_milliseconds() as f64 / 1000.0;
                    Ok(Scalar(seconds))
                }
                (Instant(at), Quantity { amount, unit }) => {
                    if unit.dimension != crate::units::Dimension::Time {
                        return Err(EvalError {
                            message: format!("`{}` is not a duration", unit.id),
                        });
                    }
                    let seconds = to_base(amount, unit);
                    Ok(Instant(at - Duration::milliseconds((seconds * 1000.0) as i64)))
                }
                (Currency { amount: a, code: ca }, Currency { amount: b, code: cb }) => {
                    if ca != cb {
                        return Err(EvalError {
                            message: format!("cannot subtract {cb} from {ca}"),
                        });
                    }
                    Ok(Currency { amount: a - b, code: ca })
                }
                (a, b) => Err(EvalError {
                    message: format!("cannot subtract {b:?} from {a:?}"),
                }),
            },

            BinOp::Mul => match (left, right) {
                (Scalar(a), Scalar(b)) => Ok(Scalar(a * b)),
                (Scalar(a), Quantity { amount, unit }) => Ok(Quantity {
                    amount: a * amount,
                    unit,
                }),
                (Quantity { amount, unit }, Scalar(b)) => Ok(Quantity {
                    amount: amount * b,
                    unit,
                }),
                (Currency { amount, code }, Scalar(b)) => Ok(Currency {
                    amount: amount * b,
                    code,
                }),
                (Scalar(a), Currency { amount, code }) => Ok(Currency {
                    amount: a * amount,
                    code,
                }),
                (a, b) => Err(EvalError {
                    message: format!("cannot multiply {a:?} and {b:?}"),
                }),
            },

            BinOp::Div => match (left, right) {
                (Scalar(a), Scalar(b)) => {
                    if b == 0.0 {
                        return Err(EvalError {
                            message: "division by zero".into(),
                        });
                    }
                    Ok(Scalar(a / b))
                }
                (Quantity { amount, unit }, Scalar(b)) => {
                    if b == 0.0 {
                        return Err(EvalError {
                            message: "division by zero".into(),
                        });
                    }
                    Ok(Quantity {
                        amount: amount / b,
                        unit,
                    })
                }
                (Scalar(a), Quantity { amount, unit: _ }) => {
                    if amount == 0.0 {
                        return Err(EvalError {
                            message: "division by zero".into(),
                        });
                    }
                    Ok(Scalar(a / amount))
                }
                (Quantity { amount: a, unit: ua }, Quantity { amount: b, unit: ub }) => {
                    if ua.dimension != ub.dimension {
                        return Err(EvalError {
                            message: format!("cannot divide {} by {}", ua.id, ub.id),
                        });
                    }
                    if b == 0.0 {
                        return Err(EvalError {
                            message: "division by zero".into(),
                        });
                    }
                    Ok(Scalar(a / b))
                }
                (Currency { amount, code }, Scalar(b)) => Ok(Currency {
                    amount: amount / b,
                    code,
                }),
                (a, b) => Err(EvalError {
                    message: format!("cannot divide {a:?} by {b:?}"),
                }),
            },

            BinOp::Rem => match (left, right) {
                (Scalar(a), Scalar(b)) => {
                    if b == 0.0 {
                        return Err(EvalError {
                            message: "division by zero".into(),
                        });
                    }
                    Ok(Scalar(a % b))
                }
                (a, b) => Err(EvalError {
                    message: format!("cannot take the remainder of {a:?} and {b:?}"),
                }),
            },

            BinOp::Pow => match (left, right) {
                (Scalar(a), Scalar(b)) => Ok(Scalar(a.powf(b))),
                (a, b) => Err(EvalError {
                    message: format!("cannot raise {a:?} to {b:?}"),
                }),
            },
        }
    }

    fn call(&self, name: &str, args: Vec<Value>) -> Result<Value, EvalError> {
        // The calendar functions are the only ones that accept an
        // instant, so they are answered before the numbers-only scan that
        // every other function shares.
        match name {
            "add_month" => return self.calendar_span("month", &args),
            "add_year" => return self.calendar_span("year", &args),
            "next" => return self.shift_weekday(true, "next", &args),
            "last" => return self.shift_weekday(false, "last", &args),
            _ => {}
        }

        let mut numbers = Vec::with_capacity(args.len());
        for arg in &args {
            match arg {
                Value::Scalar(n) => numbers.push(*n),
                other => {
                    return Err(EvalError {
                        message: format!("`{name}` expects a number, got {other:?}"),
                    });
                }
            }
        }

        let one = |numbers: &Vec<f64>| -> Result<f64, EvalError> {
            numbers.first().copied().ok_or_else(|| EvalError {
                message: format!("`{name}` expects one argument"),
            })
        };

        let value = match name {
            "sqrt" => one(&numbers)?.sqrt(),
            "cbrt" => one(&numbers)?.cbrt(),
            "abs" => one(&numbers)?.abs(),
            "floor" => one(&numbers)?.floor(),
            "ceil" => one(&numbers)?.ceil(),
            "round" => round_half_away(one(&numbers)?),
            "trunc" => one(&numbers)?.trunc(),
            "sign" => {
                let n = one(&numbers)?;
                if n > 0.0 {
                    1.0
                } else if n < 0.0 {
                    -1.0
                } else {
                    0.0
                }
            }
            "sin" => one(&numbers)?.sin(),
            "cos" => one(&numbers)?.cos(),
            "tan" => one(&numbers)?.tan(),
            "asin" => one(&numbers)?.asin(),
            "acos" => one(&numbers)?.acos(),
            "atan" => one(&numbers)?.atan(),
            "ln" => one(&numbers)?.ln(),
            "log" => one(&numbers)?.log10(),
            "log2" => one(&numbers)?.log2(),
            "exp" => one(&numbers)?.exp(),
            "pow" => {
                if numbers.len() != 2 {
                    return Err(EvalError {
                        message: "`pow` expects two arguments".into(),
                    });
                }
                numbers[0].powf(numbers[1])
            }
            "min" => {
                if numbers.is_empty() {
                    return Err(EvalError {
                        message: "`min` expects at least one argument".into(),
                    });
                }
                numbers.iter().copied().fold(f64::INFINITY, f64::min)
            }
            "max" => {
                if numbers.is_empty() {
                    return Err(EvalError {
                        message: "`max` expects at least one argument".into(),
                    });
                }
                numbers.iter().copied().fold(f64::NEG_INFINITY, f64::max)
            }
            "hypot" => {
                if numbers.is_empty() {
                    return Err(EvalError {
                        message: "`hypot` expects at least one argument".into(),
                    });
                }
                numbers.iter().map(|n| n * n).sum::<f64>().sqrt()
            }
            other => {
                return Err(EvalError {
                    message: format!("`{other}` is not a known function"),
                });
            }
        };

        Ok(Value::Scalar(value))
    }

    /// Move an instant whole calendar months or years, in the local zone.
    ///
    /// `30 days` is the truth only two months out of the year; a month is
    /// a calendar span, not a factor. The move is honest on both counts:
    /// one month past the 31st of January lands on the last day of
    /// February, and the wall time the user stood at is kept. A
    /// fractional count is truncated toward zero — "one and a half
    /// months" has no calendar reading.
    fn calendar_span(&self, unit: &str, args: &[Value]) -> Result<Value, EvalError> {
        if args.len() != 2 {
            return Err(EvalError {
                message: format!("`add_{unit}` expects an instant and a number of {unit}s"),
            });
        }
        let (base, count) = match (&args[0], &args[1]) {
            (Value::Instant(base), Value::Scalar(count)) => (*base, *count),
            (Value::Instant(_), other) => {
                return Err(EvalError {
                    message: format!("`add_{unit}` expects a count, got {other:?}"),
                });
            }
            _ => {
                return Err(EvalError {
                    message: format!("`add_{unit}` expects the instant as its first argument"),
                });
            }
        };
        let count = count.trunc() as i64;

        // Whole months, counted on the local calendar: leap the year and
        // clamp the day to the month the landing lands in.
        let local = base.with_timezone(&Local);
        let months = if unit == "month" { count } else { count * 12 };
        let total = local.year() as i64 * 12 + local.month0() as i64 + months;
        let year = total.div_euclid(12);
        let month = total.rem_euclid(12) as u32 + 1;
        let day = local.day().min(days_in_month(year as i32, month));
        let date = NaiveDate::from_ymd_opt(year as i32, month, day).ok_or_else(|| EvalError {
            message: "the calendar does not reach that date".into(),
        })?;
        localize(date.and_time(local.time()))
            .map(Value::Instant)
            .ok_or_else(|| EvalError {
                message: "the wall time does not exist at that calendar date".into(),
            })
    }

    /// The next or previous named weekday, judged on the local calendar.
    ///
    /// "Next" and "last" answer for a *different day*, never today:
    /// typed on a Friday, `next friday` is next Friday. Whole days are
    /// moved and the wall time is kept, so the landing reads as the
    /// same moment cast forward a few days.
    fn shift_weekday(&self, ahead: bool, name: &str, args: &[Value]) -> Result<Value, EvalError> {
        if args.len() != 1 {
            return Err(EvalError {
                message: format!(
                    "`{name}` expects one weekday, 0 (Monday) through 6 (Sunday)"
                ),
            });
        }
        let Value::Scalar(index) = args[0] else {
            return Err(EvalError {
                message: format!("`{name}` expects a weekday, spelled or numbered"),
            });
        };
        let index = index.trunc();
        if !(0.0..=6.0).contains(&index) {
            return Err(EvalError {
                message: "the weekday is 0 (Monday) through 6 (Sunday)".into(),
            });
        }

        let local = self.now.with_timezone(&Local);
        let current = local.weekday().num_days_from_monday() as i64;
        let shift = if ahead {
            let ahead = (index as i64 - current).rem_euclid(7);
            if ahead == 0 { 7 } else { ahead }
        } else {
            let behind = (current - index as i64).rem_euclid(7);
            if behind == 0 { 7 } else { behind }
        };
        let date = local.date_naive();
        let date = if ahead {
            date.checked_add_days(Days::new(shift as u64))
        } else {
            date.checked_sub_days(Days::new(shift as u64))
        }
        .ok_or_else(|| EvalError {
            message: "the calendar does not reach that date".into(),
        })?;
        localize(date.and_time(local.time()))
            .map(Value::Instant)
            .ok_or_else(|| EvalError {
                message: "the wall time does not exist at that calendar date".into(),
            })
    }

    fn convert(&self, left: Value, right: Value) -> Result<Value, EvalError> {
        use Value::{Currency, Instant, Quantity, Scalar};

        match (left, right) {
            (Quantity { amount, unit: from }, Quantity { unit: to, .. }) => {
                let converted = convert_unit(amount, from, to).ok_or_else(|| EvalError {
                    message: format!("cannot convert {} to {}", from.id, to.id),
                })?;
                Ok(Quantity {
                    amount: converted,
                    unit: to,
                })
            }
            (Currency { amount, code: from }, Currency { code: to, .. }) => {
                let from_def = self.currencies.resolve(&from).ok_or_else(|| EvalError {
                    message: format!("`{from}` is not a known currency"),
                })?;
                let to_def = self.currencies.resolve(&to).ok_or_else(|| EvalError {
                    message: format!("`{to}` is not a known currency"),
                })?;
                let converted = convert_currency(amount, from_def, to_def).ok_or_else(|| {
                    EvalError {
                        message: format!("cannot convert {from} to {to}"),
                    }
                })?;
                Ok(Currency {
                    amount: converted,
                    code: to,
                })
            }
            (Instant(at), Scalar(_)) => Ok(Instant(at)),
            (Scalar(a), Scalar(_)) => Ok(Scalar(a)),
            (Instant(at), right) => {
                // A time zone name converts an instant between zones.
                let Some(zone) = self.timezone_name(&right) else {
                    return Err(EvalError {
                        message: "expected a time zone to convert to".into(),
                    });
                };
                let local = at.with_timezone(&zone);
                Ok(Instant(local.with_timezone(&Utc)))
            }
            (a, b) => Err(EvalError {
                message: format!("cannot convert {a:?} into {b:?}"),
            }),
        }
    }

    /// Extract a time zone name from a value used as a conversion target.
    fn timezone_name(&self, value: &Value) -> Option<Tz> {
        match value {
            Value::Scalar(_) => None,
            Value::Quantity { unit, .. } => unit.id.parse::<Tz>().ok(),
            Value::Currency { code, .. } => code.parse::<Tz>().ok(),
            Value::Instant(_) => None,
        }
    }
}

impl Default for Evaluator {
    fn default() -> Self {
        Self::new()
    }
}

/// Round half away from zero, which is what a person means by "round".
fn round_half_away(value: f64) -> f64 {
    if value < 0.0 {
        -(-value + 0.5).floor()
    } else {
        (value + 0.5).floor()
    }
}

/// The ISO index of a weekday spelled out: Monday is 0, Sunday is 6 —
/// the number the `next` and `last` functions accept.
fn weekday_index(name: &str) -> Option<f64> {
    Some(match name {
        "monday" => 0.0,
        "tuesday" => 1.0,
        "wednesday" => 2.0,
        "thursday" => 3.0,
        "friday" => 4.0,
        "saturday" => 5.0,
        "sunday" => 6.0,
        _ => return None,
    })
}

/// How many days a month has.
///
/// Reached by walking to the first of the next month and stepping back a
/// day, which keeps the leap rule in chrono's hands. The year can only
/// overflow past `i32`'s range, which no honest count reaches.
fn days_in_month(year: i32, month: u32) -> u32 {
    NaiveDate::from_ymd_opt(year, month, 1)
        .and_then(|first| first.checked_add_months(Months::new(1)))
        .and_then(|after| after.checked_sub_days(Days::new(1)))
        .map_or(30, |last| last.day())
}

/// Resolve a local naive datetime into UTC: the earlier reading of a
/// repeated wall time, and a refusal of one the zone skips — the
/// machine's clock never shows such a time, so neither do we.
fn localize(naive: NaiveDateTime) -> Option<DateTime<Utc>> {
    match Local.from_local_datetime(&naive) {
        MappedLocalTime::Single(at) => Some(Utc.from_utc_datetime(&at.naive_utc())),
        MappedLocalTime::Ambiguous(earliest, _) => Some(Utc.from_utc_datetime(&earliest.naive_utc())),
        MappedLocalTime::None => None,
    }
}

/// Convenience: evaluate an expression string.
pub fn evaluate(input: &str) -> Result<Value, EvalError> {
    let expr = crate::parser::parse(input)
        .map_err(|err| EvalError {
            message: err.message,
        })?;
    Evaluator::new().eval(&expr)
}

/// Convenience: answer a phrase or an expression.
///
/// The natural-language layer is tried first — `10 days from now` is an
/// expression shaped like English — and a miss falls through to the
/// ordinary grammar, so plain math is untouched.
pub fn evaluate_smart(input: &str) -> Result<Value, EvalError> {
    let expr = match crate::nl::parse(input) {
        Some(expr) => expr,
        None => crate::parser::parse(input).map_err(|err| EvalError {
            message: err.message,
        })?,
    };
    Evaluator::new().eval(&expr)
}

/// [`evaluate_smart`] with an explicit currency registry — the one the
/// store's rates table has been folded into, when it holds a table.
pub fn evaluate_smart_with(
    input: &str,
    currencies: CurrencyRegistry,
) -> Result<Value, EvalError> {
    let expr = match crate::nl::parse(input) {
        Some(expr) => expr,
        None => crate::parser::parse(input).map_err(|err| EvalError {
            message: err.message,
        })?,
    };
    Evaluator::with(Utc::now(), currencies).eval(&expr)
}

/// [`evaluate_smart`] with a fixed notion of "now", which is how the
/// natural-language phrases are tested deterministically.
pub fn evaluate_smart_at(input: &str, now: DateTime<Utc>) -> Result<Value, EvalError> {
    let expr = match crate::nl::parse(input) {
        Some(expr) => expr,
        None => crate::parser::parse(input).map_err(|err| EvalError {
            message: err.message,
        })?,
    };
    Evaluator::at(now).eval(&expr)
}

/// Format a value for display.
pub fn format(value: &Value) -> String {
    match value {
        Value::Scalar(n) => format_number(*n),
        Value::Quantity { amount, unit } => {
            format!("{} {}", format_number(*amount), unit.id)
        }
        Value::Currency { amount, code } => {
            format!("{} {}", format_number(*amount), code)
        }
        Value::Instant(at) => at.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
    }
}

/// Format a number without trailing noise.
pub fn format_number(value: f64) -> String {
    if value == value.trunc() && value.abs() < 1e15 {
        return format!("{}", value as i64);
    }
    let mut text = format!("{value:.10}");
    if text.contains('.') {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    text
}

/// The month and year components of an instant, used by date formatting.
pub fn date_parts(at: DateTime<Utc>) -> (i32, u32) {
    (at.with_timezone(&Local).year(), at.with_timezone(&Local).month())
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    fn scalar(input: &str) -> f64 {
        match evaluate(input).unwrap() {
            Value::Scalar(n) => n,
            other => panic!("expected a scalar for `{input}`, got {other:?}"),
        }
    }

    #[test]
    fn evaluates_arithmetic_with_precedence() {
        assert_relative_eq!(scalar("1 + 2 * 3"), 7.0);
        assert_relative_eq!(scalar("(1 + 2) * 3"), 9.0);
        assert_relative_eq!(scalar("10 - 2 - 3"), 5.0);
    }

    #[test]
    fn evaluates_power_right_associatively() {
        assert_relative_eq!(scalar("2 ^ 3 ^ 2"), 512.0);
        assert_relative_eq!(scalar("-2 ^ 2"), -4.0);
    }

    #[test]
    fn evaluates_percent() {
        assert_relative_eq!(scalar("50%"), 0.5);
        assert_relative_eq!(scalar("200 * 10%"), 20.0);
    }

    #[test]
    fn evaluates_the_percent_of_sugar() {
        assert_relative_eq!(scalar("20% of 250"), 50.0);
        // The `of` binds one factor; the addition is still additive.
        assert_relative_eq!(scalar("20% of 250 + 10"), 60.0);
        // Parentheses buy the other reading.
        assert_relative_eq!(scalar("20% of (250 + 10)"), 52.0);
    }

    #[test]
    fn modulo_survives_spaces_now() {
        assert_relative_eq!(scalar("100 % 3"), 1.0);
    }

    #[test]
    fn evaluates_functions() {
        assert_relative_eq!(scalar("sqrt(16)"), 4.0);
        assert_relative_eq!(scalar("max(1, 7, 3)"), 7.0);
        assert_relative_eq!(scalar("round(2.5)"), 3.0);
        assert_relative_eq!(scalar("round(-2.5)"), -3.0);
        assert_relative_eq!(scalar("abs(-3)"), 3.0);
    }

    #[test]
    fn evaluates_constants() {
        assert_relative_eq!(scalar("pi"), std::f64::consts::PI);
        assert_relative_eq!(scalar("e"), std::f64::consts::E);
    }

    #[test]
    fn converts_units() {
        match evaluate("1 km in miles").unwrap() {
            Value::Quantity { amount, unit } => {
                assert_eq!(unit.id, "mi");
                assert_relative_eq!(amount, 0.621_371_192_237_334, epsilon = 1e-12);
            }
            other => panic!("expected a quantity, got {other:?}"),
        }
    }

    #[test]
    fn converts_temperature() {
        match evaluate("100 c in f").unwrap() {
            Value::Quantity { amount, unit } => {
                assert_eq!(unit.id, "f");
                assert_relative_eq!(amount, 212.0, epsilon = 1e-9);
            }
            other => panic!("expected a quantity, got {other:?}"),
        }
    }

    #[test]
    fn adds_quantities_across_units() {
        match evaluate("1 km + 500 m").unwrap() {
            Value::Quantity { amount, unit } => {
                assert_eq!(unit.id, "km");
                assert_relative_eq!(amount, 1.5);
            }
            other => panic!("expected a quantity, got {other:?}"),
        }
    }

    #[test]
    fn rejects_incompatible_units() {
        assert!(evaluate("1 kg + 1 km").is_err());
    }

    #[test]
    fn converts_currency() {
        match evaluate("100 usd in eur").unwrap() {
            Value::Currency { amount, code } => {
                assert_eq!(code, "EUR");
                assert_relative_eq!(amount, 92.0);
            }
            other => panic!("expected currency, got {other:?}"),
        }
    }

    #[test]
    fn stored_rates_extend_the_registry() {
        let rates = crate::currency::RatesTable {
            vintage: "2026-10-30".into(),
            rates: std::collections::HashMap::from([("ZWL".into(), 5.0)]),
        };
        let registry = CurrencyRegistry::new().with_rates(&rates);
        let expr = crate::parser::parse("100 usd in zwl").unwrap();
        let evaluator = Evaluator::with(now_fixed(), registry);
        match evaluator.eval(&expr).unwrap() {
            Value::Currency { amount, code } => {
                assert_eq!(code, "ZWL");
                assert_relative_eq!(amount, 500.0);
            }
            other => panic!("expected currency, got {other:?}"),
        }
    }

    #[test]
    fn does_date_arithmetic() {
        let now = DateTime::from_timestamp(1_700_000_000, 0).unwrap();
        let evaluator = Evaluator::at(now);
        let expr = crate::parser::parse("now + 3 days").unwrap();
        match evaluator.eval(&expr).unwrap() {
            Value::Instant(at) => {
                assert_eq!((at - now).num_days(), 3);
            }
            other => panic!("expected an instant, got {other:?}"),
        }
    }

    /// Treated as 2026-01-31T12:00:00Z: the day chosen for the
    /// calendar-clamp proofs below.
    fn now_fixed() -> DateTime<Utc> {
        DateTime::from_timestamp(1_769_860_800, 0).unwrap()
    }

    /// Answer a phrase with the fixed "now"; the answer must be an instant.
    fn asked(input: &str) -> DateTime<Utc> {
        match evaluate_smart_at(input, now_fixed()).unwrap() {
            Value::Instant(at) => at,
            other => panic!("expected an instant for `{input}`, got {other:?}"),
        }
    }

    #[test]
    fn answers_the_duration_phrases() {
        let now = now_fixed();
        let later = asked("10 days from now");
        assert_eq!((later - now).num_seconds(), 10 * 86_400);
        let later = asked("in 2 weeks");
        assert_eq!((later - now).num_seconds(), 2 * 604_800);
        let later = asked("in 3 hours");
        assert_eq!((later - now).num_seconds(), 3 * 3_600);
        let earlier = asked("90 minutes ago");
        assert_eq!((earlier - now).num_seconds(), -90 * 60);
    }

    #[test]
    fn answers_the_day_phrases() {
        let now = now_fixed();
        assert_eq!(asked("tomorrow") - now, Duration::days(1));
        assert_eq!(asked("yesterday") - now, Duration::days(-1));
    }

    #[test]
    fn a_month_moves_the_calendar_not_a_factor() {
        let now = now_fixed();
        // The local landing is the same wall time — a month is a calendar
        // span, and its day clamps to February's last.
        let landing = asked("in 1 month");
        let landing = landing.with_timezone(&Local);
        assert_eq!(landing.time(), now.with_timezone(&Local).time());
        assert_eq!(landing.month(), 2);
        assert_eq!(landing.day(), days_in_month(2026, 2));
        // The 30-day fiction would already have walked into March.
        let expr = crate::parser::parse("now + 30 days").unwrap();
        let fiction = Evaluator::at(now).eval(&expr).unwrap();
        assert_ne!(Value::Instant(landing.with_timezone(&Utc)), fiction);
    }

    #[test]
    fn a_year_moves_the_calendar() {
        let landing = asked("in 1 year");
        assert_eq!(landing.with_timezone(&Local).year(), 2027);
    }

    #[test]
    fn the_landing_keeps_the_wall_time_and_the_zone() {
        // `next friday` answers for a *different* day, spells Friday, and
        // keeps the wall-clock time it was asked at.
        let next = asked("next friday");
        let local = next.with_timezone(&Local);
        assert_eq!(local.weekday(), chrono::Weekday::Fri);
        let now = now_fixed().with_timezone(&Local);
        assert!(local.date_naive() != now.date_naive());
        assert_eq!(local.time(), now.time());
    }

    #[test]
    fn last_weekday_walks_backwards() {
        let last = asked("last monday");
        let local = last.with_timezone(&Local);
        assert_eq!(local.weekday(), chrono::Weekday::Mon);
        assert!(last < now_fixed());
    }

    #[test]
    fn subtracts_instants_to_a_duration() {
        let now = DateTime::from_timestamp(1_700_000_000, 0).unwrap();
        let evaluator = Evaluator::at(now);
        let expr = crate::parser::parse("now - now").unwrap();
        assert_relative_eq!(match evaluator.eval(&expr).unwrap() {
            Value::Scalar(n) => n,
            other => panic!("expected a scalar, got {other:?}"),
        }, 0.0);
    }

    #[test]
    fn rejects_division_by_zero() {
        assert!(evaluate("1 / 0").is_err());
    }

    #[test]
    fn rejects_unknown_identifiers() {
        assert!(evaluate("wibble").is_err());
    }

    #[test]
    fn rejects_unknown_functions() {
        assert!(evaluate("wibble(1)").is_err());
    }
}
