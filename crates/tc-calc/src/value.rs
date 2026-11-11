//! The value the calculator works with, and the ways a calculation can fail.

use std::fmt;

/// The scalar projection of upstream's `CalcValue`.
///
/// Upstream's value carries a unit or a currency as well; both live outside this crate,
/// so only the scalar part is modelled here.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Value {
    /// The amount the value carries.
    pub amount: f64,
    /// Whether the value was written as a percentage.
    pub is_percent: bool,
    /// Whether the value is the answer of a comparison.
    pub is_boolean: bool,
}

impl Value {
    /// A plain scalar value.
    pub fn scalar(amount: f64) -> Self {
        Self {
            amount,
            is_percent: false,
            is_boolean: false,
        }
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
