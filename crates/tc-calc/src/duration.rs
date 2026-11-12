//! A duration read out of the tokens: `5 years`, `18 months`, `every year`.
//!
//! Upstream's arithmetic carries a unit on the value, so a term like `5 years`
//! is a unit value and `5 years / year` is a plain number. **This port's
//! arithmetic core does not carry units yet**, so the phrases that need a
//! duration read it themselves, the way upstream's `CalcFinance.years` and
//! `CalcGrowth` do.

use crate::catalog::Catalog;
use crate::token::{Operator, Token};
use crate::units::{UnitCategory, UnitDef};

/// A duration: an amount and the unit it is in.
#[derive(Debug, Clone, PartialEq)]
pub struct Duration {
    /// The amount of the unit.
    pub amount: f64,
    /// The unit the amount is in.
    pub unit: UnitDef,
}

impl Duration {
    /// The duration in seconds, so two durations can be compared.
    pub fn seconds(&self) -> f64 {
        self.amount * self.unit.factor
    }

    /// The duration in years.
    pub fn years(&self, year: &UnitDef) -> f64 {
        self.seconds() / year.factor
    }

    /// One unit of the same kind, the way `every year` reads.
    pub fn single(unit: UnitDef) -> Self {
        Self { amount: 1.0, unit }
    }
}

/// Reads a duration out of the tokens.
///
/// A number and a time unit, or a time unit on its own, which is how a period
/// reads: `every year` is one year.
pub fn read(tokens: &[Token], catalog: &dyn Catalog) -> Option<Duration> {
    let inner = unwrap(tokens);
    match inner {
        [Token::Number(amount), Token::Ident(name)] => {
            let unit = time_unit(name, catalog)?;
            Some(Duration {
                amount: *amount,
                unit,
            })
        }
        [Token::CompactNumber(amount), Token::Ident(name)] => {
            let unit = time_unit(name, catalog)?;
            Some(Duration {
                amount: *amount,
                unit,
            })
        }
        [Token::Ident(name)] => {
            let unit = time_unit(name, catalog)?;
            Some(Duration::single(unit))
        }
        _ => None,
    }
}

/// The time unit filed under `name`, if the catalog knows one.
pub fn time_unit(name: &str, catalog: &dyn Catalog) -> Option<UnitDef> {
    let unit = catalog.unit_definition(name)?;
    if unit.category != UnitCategory::Time {
        return None;
    }
    Some(unit)
}

/// The unit of a year, the one the terms are measured in.
pub fn year(catalog: &dyn Catalog) -> Option<UnitDef> {
    time_unit("yr", catalog)
}

/// The brackets around a group, taken off.
fn unwrap(tokens: &[Token]) -> &[Token] {
    if tokens.len() >= 2
        && tokens[0] == Token::Op(Operator::Open)
        && tokens[tokens.len() - 1] == Token::Op(Operator::Close)
    {
        &tokens[1..tokens.len() - 1]
    } else {
        tokens
    }
}

/// One duration as a multiple of another, as upstream's `CalcGrowth.ratio` reads it.
///
/// `5 years / year` is five, and `10 years / 2 years` is five.
pub fn ratio(numerator: &Duration, denominator: &Duration) -> Option<f64> {
    if denominator.seconds() == 0.0 {
        return None;
    }
    Some(numerator.seconds() / denominator.seconds())
}
