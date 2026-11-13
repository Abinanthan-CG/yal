//! The value the calculator works with, and the ways a calculation can fail.

use std::fmt;

use crate::currency::CurrencyDef;
use crate::measure::Measurement;

/// The scalar projection of upstream's `CalcValue`.
///
/// Upstream's value carries a unit or a currency as well; the units live in this
/// crate too, so a value carries its money and its measurement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Value {
    /// The amount the value carries.
    pub amount: f64,
    /// Whether the value was written as a percentage.
    pub is_percent: bool,
    /// Whether the value is the answer of a comparison.
    pub is_boolean: bool,
    /// The currency the amount is in, if it is money.
    pub currency: Option<CurrencyDef>,
    /// The unit the amount is in, if it is a measurement.
    pub unit: Option<Measurement>,
}

impl Value {
    /// A plain scalar value.
    pub fn scalar(amount: f64) -> Self {
        Self {
            amount,
            is_percent: false,
            is_boolean: false,
            currency: None,
            unit: None,
        }
    }

    /// An amount of money, as upstream's `CalcValue(kind: .currency(def))`.
    pub fn money(amount: f64, currency: CurrencyDef) -> Self {
        Self {
            amount,
            is_percent: false,
            is_boolean: false,
            currency: Some(currency),
            unit: None,
        }
    }

    /// An amount of a measure, as upstream's `CalcValue(kind: .unit(def))`.
    pub fn measurement(amount: f64, unit: Measurement) -> Self {
        Self {
            amount,
            is_percent: false,
            is_boolean: false,
            currency: None,
            unit: Some(unit),
        }
    }

    /// Whether the value is a plain amount, as upstream's `CalcValue.Kind.scalar`.
    ///
    /// A value that carries a measure or a currency is not a plain amount, so the
    /// phrases that want one do not take it.
    pub fn is_scalar(&self) -> bool {
        self.unit.is_none() && self.currency.is_none()
    }

    /// The amount the value stands for, as upstream's `CalcValue.effective`.
    ///
    /// A percentage is a hundredth of the amount it carries.
    pub fn effective(self) -> f64 {
        if self.is_percent {
            self.amount / 100.0
        } else {
            self.amount
        }
    }
}

/// A part of the calculator that lives outside this crate.
///
/// The units and the currencies are ported separately, so a calculation that needs one
/// of them says so instead of answering a made-up number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedArea {
    /// The name of the area, as the parity ledger names it.
    pub area: &'static str,
    /// What in the calculation needed it.
    pub detail: String,
}

impl UnsupportedArea {
    /// The unit area, reached by a word the catalog names a unit.
    pub fn units(detail: impl Into<String>) -> Self {
        Self {
            area: "units",
            detail: detail.into(),
        }
    }

    /// The currency area, reached by a word the catalog names a currency.
    pub fn currency(detail: impl Into<String>) -> Self {
        Self {
            area: "currency",
            detail: detail.into(),
        }
    }
}

/// The ways a calculation can fail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// The tokens are not a calculation.
    ///
    /// Upstream answers `nil` here, which means the query earns no card.
    NotACalculation,
    /// The calculation needs an area this crate does not implement.
    Unsupported(UnsupportedArea),
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotACalculation => formatter.write_str("not a calculation"),
            Self::Unsupported(area) => write!(
                formatter,
                "{} are not supported: {}",
                area.area, area.detail
            ),
        }
    }
}

impl std::error::Error for ParseError {}
