//! The unit a value carries, and the arithmetic between units.
//!
//! Mirrors upstream `CalcUnitExpression.swift`, the base units and the product unit
//! of `CalcUnits.swift`, and the unit branches of `CalcExpressionParser.swift`.
//!
//! A measurement is the unit itself: the symbol it is written with, the scale onto
//! its base, and the dimension it has. Two measurements of one dimension can be added,
//! and the last one typed decides the unit of the answer. A rate and a time multiply
//! into the numerator of the rate, so `60 mph * 2.5 hours` is `150 mi`.
//!
//! A compound unit is written down once and kept: the symbols it is made of are
//! interned, so the same compound is the same measurement every time it is met.

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

use crate::catalog::Catalog;
use crate::formatter::Formatter;
use crate::units::{Dimension, UnitCategory, UnitDef};

/// A unit a value carries, as upstream's `UnitDef`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Measurement {
    /// The symbol the unit is written with, such as `kg` or `kg/m³`.
    pub symbol: &'static str,
    /// The long label for the card's badge, such as `Kilograms`.
    pub name: &'static str,
    /// What the unit measures.
    pub category: UnitCategory,
    /// The scale onto the base unit.
    pub factor: f64,
    /// The offset onto the base unit; only the temperatures need one.
    pub offset: f64,
    /// The dimension the unit has.
    pub dimension: Option<Dimension>,
}

impl Measurement {
    /// The measurement of a unit in the catalog.
    pub fn of(unit: &'static UnitDef) -> Self {
        Self {
            symbol: unit.symbol.as_str(),
            name: unit.name.as_str(),
            category: unit.category,
            factor: unit.factor,
            offset: unit.offset,
            dimension: unit.dimension(),
        }
    }

    /// The measurement of the unit a name stands for, if the catalog knows one.
    ///
    /// A catalog that knows the names but not the scales answers `None`: the core
    /// needs the factor and the dimension to carry the unit.
    pub fn named(catalog: &dyn Catalog, name: &str) -> Option<Self> {
        catalog.unit_ref(name).map(Self::of)
    }

    /// A unit made of other units, as upstream's `CalcUnitExpression.combine`.
    pub fn compound(symbol: String, factor: f64, dimension: Option<Dimension>) -> Self {
        Self {
            symbol: intern(&symbol),
            name: "Compound Units",
            category: UnitCategory::Compound,
            factor,
            offset: 0.0,
            dimension,
        }
    }

    /// Whether the two measurements can be added or compared, as upstream's
    /// `UnitDef.isCompatible`.
    pub fn is_compatible_with(&self, other: &Self) -> bool {
        if self.category != UnitCategory::Compound && self.category == other.category {
            return true;
        }
        match (self.dimension, other.dimension) {
            (Some(left), Some(right)) => left == right,
            _ => false,
        }
    }

    /// The amount of this measurement, told in the other one.
    ///
    /// A measurement is an affine map onto its base, so only the temperatures carry
    /// an offset; everything else is a plain scale.
    pub fn told_in(&self, amount: f64, target: &Self) -> f64 {
        (amount * self.factor + self.offset - target.offset) / target.factor
    }
}

/// The base unit of a dimension, as upstream's `CalcUnits.baseUnits`.
///
/// A combined measurement is answered in the base unit of its dimension, so five
/// metres times four metres is twenty square metres rather than a compound.
pub fn base_unit(dimension: &Dimension) -> Option<Measurement> {
    base_units()
        .iter()
        .find(|(base, _)| base == dimension)
        .map(|(_, measurement)| *measurement)
}

/// The base unit of every dimension the catalog knows.
fn base_units() -> &'static [(Dimension, Measurement)] {
    static BASE: OnceLock<Vec<(Dimension, Measurement)>> = OnceLock::new();
    BASE.get_or_init(|| {
        let mut base: Vec<(Dimension, Measurement)> = Vec::new();
        for name in BASE_NAMES {
            let Some(unit) = crate::units::by_name(name) else {
                continue;
            };
            let Some(dimension) = unit.dimension() else {
                continue;
            };
            if base.iter().any(|(known, _)| *known == dimension) {
                continue;
            }
            base.push((dimension, Measurement::of(unit)));
        }
        base
    })
}

/// The base units of upstream's `CalcUnits.baseUnits`.
///
/// The name of each is the unit that stands for the dimension: a metre, a kilogram,
/// a second, and so on.
const BASE_NAMES: &[&str] = &[
    "m", "kg", "s", "m2", "m3", "b", "m/s", "pa", "bps", "m/s2", "n", "j", "w", "hz", "a", "v",
    "ohm", "as", "m3/s", "px", "px2", "ppi", "farad", "henry", "siemens", "wb", "tesla", "mol",
];

/// The unit a rate multiplied by a time accrues in, as upstream's
/// `CalcUnits.productUnit`.
///
/// A rate over time accrues in its own numerator, so `60 mph * 2.5 hours` is miles
/// rather than metres, and `1500 w * 2 h` is watt hours.
pub fn product_unit(left: &Measurement, right: &Measurement) -> Option<Measurement> {
    let (measure, duration) = if left.category == UnitCategory::Time {
        (right, left)
    } else {
        (left, right)
    };
    if duration.category != UnitCategory::Time {
        return None;
    }
    if let Some(accrued) = accrued_unit(measure) {
        return Some(accrued);
    }
    if duration.factor < 60.0 {
        return None;
    }
    match measure.category {
        UnitCategory::Power => by_name(if measure.factor >= 1000.0 {
            "kwh"
        } else {
            "wh"
        }),
        UnitCategory::ElectricCurrent => {
            by_name(if measure.factor <= 0.001 { "mah" } else { "ah" })
        }
        _ => None,
    }
}

/// The numerator a rate accrues in, as upstream's `CalcUnits.accruedUnit`.
fn accrued_unit(rate: &Measurement) -> Option<Measurement> {
    let symbol = rate.symbol.replace('²', "2").replace('³', "3");
    let numerator = RATE_NUMERATORS
        .iter()
        .find(|(key, _)| *key == symbol)
        .map(|(_, value)| *value)
        .or_else(|| symbol.split('/').next().filter(|part| *part != symbol))?;
    let unit = by_name(numerator).or_else(|| by_name(&numerator.to_lowercase()))?;
    let rate_dimension = rate.dimension?;
    let unit_dimension = unit.dimension?;
    let time = Dimension::new(0.0, 0.0, 1.0);
    if unit_dimension == rate_dimension.adding(time, 1.0) {
        Some(unit)
    } else {
        None
    }
}

/// The rates that accrue in a unit of their own, as upstream's
/// `CalcUnits.rateNumerators`.
const RATE_NUMERATORS: &[(&str, &str)] = &[("mph", "mi"), ("kn", "nmi")];

/// The compound of two measurements, as upstream's `CalcUnitExpression.combine`.
///
/// The symbol keeps the way the two were joined, so a kilogram over a cubic metre
/// reads `kg/m³` and a kilogram times a metre reads `kg·m`.
pub fn combine(left: &Measurement, right: &Measurement, dividing: bool) -> Option<Measurement> {
    let left_dimension = left.dimension?;
    let right_dimension = right.dimension?;
    let dimension = left_dimension.adding(right_dimension, if dividing { -1.0 } else { 1.0 });
    if dimension.currency.abs() > 1.0 {
        return None;
    }
    let factor = if dividing {
        left.factor / right.factor
    } else {
        left.factor * right.factor
    };
    if !factor.is_finite() || factor <= 0.0 {
        return None;
    }
    let right_symbol = if dividing && (right.symbol.contains('/') || right.symbol.contains('·')) {
        format!("({})", right.symbol)
    } else {
        right.symbol.to_string()
    };
    let symbol = format!(
        "{}{}{}",
        left.symbol,
        if dividing { "/" } else { "·" },
        right_symbol
    );
    Some(Measurement::compound(symbol, factor, Some(dimension)))
}

/// A measurement raised to a power, as upstream's `CalcUnitExpression.power`.
pub fn power(unit: &Measurement, exponent: f64) -> Option<Measurement> {
    let dimension = unit.dimension?;
    let raised = dimension.raised(exponent);
    if raised.currency.abs() > 1.0 || raised.currency.round() != raised.currency {
        return None;
    }
    let factor = unit.factor.powf(exponent);
    if !factor.is_finite() || factor <= 0.0 {
        return None;
    }
    let symbol = if unit.symbol.contains('/') || unit.symbol.contains('·') {
        format!("({})", unit.symbol)
    } else {
        unit.symbol.to_string()
    };
    let suffix = if exponent == 2.0 {
        "²".to_string()
    } else if exponent == 3.0 {
        "³".to_string()
    } else {
        format!("^{}", Formatter::copy_text(exponent))
    };
    Some(Measurement::compound(
        format!("{symbol}{suffix}"),
        factor,
        Some(raised),
    ))
}

/// The measurement of the unit a name stands for in the catalog.
pub fn by_name(name: &str) -> Option<Measurement> {
    crate::units::by_name(name).map(Measurement::of)
}

/// The measurement of a currency, as upstream's `CalcUnitExpression.named`.
///
/// A currency is a unit of its own dimension, so `50 usd per hour` is a compound
/// of money and time.
pub fn of_currency(currency: crate::currency::CurrencyDef) -> Measurement {
    Measurement {
        symbol: currency.code,
        name: currency.name,
        category: UnitCategory::Compound,
        factor: 1.0,
        offset: 0.0,
        dimension: Some(Dimension {
            currency: 1.0,
            ..Dimension::new(0.0, 0.0, 0.0)
        }),
    }
}

/// A unit written as an expression, as upstream's `CalcUnitExpression.factor`.
///
/// `m`, `m/s`, `m/s2` and `(m/s)2` are all written as an expression, and the
/// parser reads one of them the way it is written.
pub fn factor(tokens: &[crate::token::Token], index: usize) -> Option<(Measurement, usize)> {
    let mut parser = UnitParser { tokens, index };
    let unit = parser.factor()?;
    Some((unit, parser.index))
}

/// The reader of a unit written as an expression, as upstream's `UnitParser`.
struct UnitParser<'a> {
    /// The tokens to read.
    tokens: &'a [crate::token::Token],
    /// Where the reader is.
    index: usize,
}

impl<'a> UnitParser<'a> {
    /// The token at the cursor.
    fn current(&self) -> Option<&crate::token::Token> {
        self.tokens.get(self.index)
    }

    /// A unit, a group of units, or one of them with a power, as upstream's
    /// `UnitParser.factor`.
    fn factor(&mut self) -> Option<Measurement> {
        let unit = match self.current()? {
            crate::token::Token::Ident(name) => {
                let unit = by_name(name)?;
                self.index += 1;
                unit
            }
            crate::token::Token::Op(crate::token::Operator::Open) => {
                self.index += 1;
                let found = self.expression()?;
                if self.current() != Some(&crate::token::Token::Op(crate::token::Operator::Close)) {
                    return None;
                }
                self.index += 1;
                found
            }
            _ => return None,
        };
        if self.current() != Some(&crate::token::Token::Op(crate::token::Operator::Power)) {
            return Some(unit);
        }
        self.index += 1;
        let negative =
            self.current() == Some(&crate::token::Token::Op(crate::token::Operator::Subtract));
        if negative {
            self.index += 1;
        }
        let number = match self.current() {
            Some(crate::token::Token::Number(number)) => *number,
            _ => return None,
        };
        if number > 16.0 {
            return None;
        }
        self.index += 1;
        power(&unit, if negative { -number } else { number })
    }

    /// A run of units joined by `*` or `/`, as upstream's `UnitParser.expression`.
    fn expression(&mut self) -> Option<Measurement> {
        let mut left = self.factor()?;
        while matches!(
            self.current(),
            Some(crate::token::Token::Op(
                crate::token::Operator::Multiply | crate::token::Operator::Divide
            ))
        ) {
            let dividing =
                self.current() == Some(&crate::token::Token::Op(crate::token::Operator::Divide));
            let mut next = UnitParser {
                tokens: self.tokens,
                index: self.index + 1,
            };
            let Some(right) = next.factor() else {
                break;
            };
            let Some(result) = combine(&left, &right, dividing) else {
                break;
            };
            self.index = next.index;
            left = result;
        }
        Some(left)
    }
}

/// Keeps one copy of every symbol a compound is written with.
///
/// The symbols are short and there are few of them: a calculator meets the same
/// compound again and again, and the copy is what makes two compounds with the same
/// writing the same measurement.
fn intern(symbol: &str) -> &'static str {
    static REGISTRY: OnceLock<RwLock<HashMap<String, &'static str>>> = OnceLock::new();
    let registry = REGISTRY.get_or_init(|| RwLock::new(HashMap::new()));
    let mut symbols = registry
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(known) = symbols.get(symbol) {
        return known;
    }
    let kept: &'static str = Box::leak(symbol.to_string().into_boxed_str());
    symbols.insert(symbol.to_string(), kept);
    kept
}
