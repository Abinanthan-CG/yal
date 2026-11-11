//! A quantity told in two units of one kind, the larger whole.
//!
//! Mirrors upstream `CalcMixedUnits.swift`: `1.75 m to feet and inches` answers
//! `5 ft 8.897637795 in`, and `2.5 kg to lb & oz` answers `5 lb 8.184904874 oz`.

use crate::catalog::Catalog;
use crate::formatter::Formatter;
use crate::units::{UnitCategory, UnitDef};

/// The two units a mixed answer is told in, as upstream's `CalcMixedUnits.pair`.
///
/// The names must be of one kind and of different sizes; the temperature and the
/// compound units are left out.
pub fn pair(catalog: &dyn Catalog, target: &str) -> Option<(UnitDef, UnitDef)> {
    let names: Vec<&str> = target
        .split(' ')
        .filter(|name| !name.is_empty() && *name != "and" && *name != "&")
        .collect();
    if names.len() != 2 {
        return None;
    }
    let first = catalog.unit_definition(names[0])?;
    let second = catalog.unit_definition(names[1])?;
    if first.category != second.category
        || first.factor == second.factor
        || first.category == UnitCategory::Temperature
        || first.category == UnitCategory::Compound
    {
        return None;
    }
    Some(if first.factor > second.factor {
        (first, second)
    } else {
        (second, first)
    })
}

/// The answer, as upstream's `CalcMixedUnits.format`.
///
/// The durations keep whole smaller units, as `timespan` does; anything else keeps its
/// fraction.
pub fn format(amount: f64, from: &UnitDef, into: &(UnitDef, UnitDef)) -> Option<String> {
    let (larger, smaller) = into;
    let ratio = larger.factor / smaller.factor;
    let mut total = amount.abs() * from.factor / smaller.factor;
    if !total.is_finite() {
        return None;
    }
    if from.category == UnitCategory::Time {
        total = total.round();
    }
    let mut whole = (total / ratio).floor();
    let mut remainder = total - whole * ratio;
    if remainder < ratio * 1e-10 {
        remainder = 0.0;
    }
    // Display rounding can lift a remainder to a whole larger unit: 11.9999999999 in is
    // a foot.
    if let Ok(shown) = Formatter::copy_text(remainder).parse::<f64>() {
        if shown >= ratio {
            whole += 1.0;
            remainder = 0.0;
        }
    }
    let mut parts: Vec<String> = Vec::new();
    if whole > 0.0 {
        parts.push(format!("{} {}", Formatter::display(whole), larger.symbol));
    }
    if remainder > 0.0 || parts.is_empty() {
        parts.push(format!(
            "{} {}",
            Formatter::display(remainder),
            smaller.symbol
        ));
    }
    Some(format!(
        "{}{}",
        if amount < 0.0 { "-" } else { "" },
        parts.join(" ")
    ))
}
