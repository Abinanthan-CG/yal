//! Change at a steady rate: `20 after 5 years at 5% every year`.
//!
//! Mirrors upstream `CalcGrowth.swift`.
//!
//! A phrase about a steady rate names three things: the amount it starts from,
//! the **period** the rate applies to (`every year`, `every 2 years`) and how
//! long it runs. `growth` and `decay` say which way it goes.

use crate::catalog::Catalog;
use crate::duration;
use crate::parser::Parser;
use crate::result::{CalcResult, Payload};
use crate::token::{operands_of, Operator, Token};
use crate::value::{ParseError, UnsupportedArea};

/// The card of a phrase about a steady rate.
pub fn evaluate(
    tokens: &[Token],
    query: &str,
    catalog: &dyn Catalog,
) -> Result<Option<CalcResult>, UnsupportedArea> {
    if !contains(tokens, "every") {
        return Ok(None);
    }
    if let Some(result) = compounded(tokens, query, catalog)? {
        return Ok(Some(result));
    }
    if let Some(result) = time_to_reach(tokens, query, catalog)? {
        return Ok(Some(result));
    }
    rate_over(tokens, query, catalog)
}

/// The way the amount changes: `growth`, `decay`, or neither.
fn modifiers() -> Vec<Vec<Token>> {
    vec![Vec::new(), vec![word("growth")], vec![word("decay")]]
}

/// The card of a compounded change.
fn compounded(
    tokens: &[Token],
    query: &str,
    catalog: &dyn Catalog,
) -> Result<Option<CalcResult>, UnsupportedArea> {
    for modifier in modifiers() {
        let mut pattern: Vec<Option<Token>> = vec![
            None,
            Some(word("after")),
            None,
            Some(word("at")),
            None,
            Some(percent()),
        ];
        pattern.extend(modifier.iter().cloned().map(Some));
        pattern.push(Some(word("every")));
        pattern.push(None);
        let Some(parts) = operands_of(tokens, &pattern) else {
            continue;
        };
        let Some(mut start) = value_of(&parts[0], catalog)? else {
            continue;
        };
        if start.is_percent {
            continue;
        }
        let Some(per_period) = rate(&parts[2], &modifier, catalog) else {
            continue;
        };
        let Some(periods) = duration_ratio(&parts[1], &parts[3], catalog) else {
            continue;
        };
        if periods < 0.0 {
            continue;
        }
        start.amount *= (1.0 + per_period).powf(periods);
        if !start.amount.is_finite() {
            return Ok(None);
        }
        let payload = match start.currency {
            Some(currency) => Payload::money(start.amount, currency.code),
            None => Payload::number(start.amount),
        };
        return Ok(Some(CalcResult::phrase(query, payload, "Future Value")));
    }
    Ok(None)
}

/// The card of the time it takes to reach a value.
fn time_to_reach(
    tokens: &[Token],
    query: &str,
    catalog: &dyn Catalog,
) -> Result<Option<CalcResult>, UnsupportedArea> {
    let Some(Token::Ident(lead)) = tokens.first() else {
        return Ok(None);
    };
    if tokens.len() <= 1 || tokens[1] != word("from") {
        return Ok(None);
    }
    let unit = duration::time_unit(lead, catalog);
    if lead != "time" && unit.is_none() {
        return Ok(None);
    }
    let rest = &tokens[2..];
    let Some((periods, period)) =
        compounded_periods(rest, catalog).or_else(|| linear_periods(rest, catalog))
    else {
        return Ok(None);
    };
    if !periods.is_finite() || periods < 0.0 {
        return Ok(None);
    }
    let Some(duration) = duration::read(&period, catalog) else {
        return Ok(None);
    };
    let mut duration = duration;
    duration.amount *= periods;
    // The lead unit names the unit the answer is told in: `time from 20 to 30`
    // is told in years, because the period is a year.
    let unit = match unit {
        Some(unit) => unit,
        None => duration.unit.clone(),
    };
    let amount = if unit.symbol == duration.unit.symbol {
        duration.amount
    } else {
        crate::units::convert(duration.amount, &duration.unit, &unit)
    };
    Ok(Some(CalcResult::phrase(
        query,
        Payload::measurement(amount, &unit),
        "Duration",
    )))
}

/// The number of periods it takes to reach a value, and the period itself.
fn compounded_periods(tokens: &[Token], catalog: &dyn Catalog) -> Option<(f64, Vec<Token>)> {
    for modifier in modifiers() {
        let mut pattern: Vec<Option<Token>> = vec![
            None,
            Some(word("to")),
            None,
            Some(word("at")),
            None,
            Some(percent()),
        ];
        pattern.extend(modifier.iter().cloned().map(Some));
        pattern.push(Some(word("every")));
        pattern.push(None);
        let parts = operands_of(tokens, &pattern)?;
        let per_period = rate(&parts[2], &modifier, catalog)?;
        if per_period <= -1.0 || per_period == 0.0 {
            continue;
        }
        let growth = scalar_ratio(&parts[1], &parts[0], catalog)?;
        if growth <= 0.0 {
            continue;
        }
        return Some(((growth.ln() / (1.0 + per_period).ln()), parts[3].clone()));
    }
    None
}

/// The number of steps it takes to reach a value at a steady amount.
fn linear_periods(tokens: &[Token], catalog: &dyn Catalog) -> Option<(f64, Vec<Token>)> {
    let parts = operands_of(
        tokens,
        &[
            None,
            Some(word("to")),
            None,
            Some(word("at")),
            None,
            Some(word("every")),
            None,
        ],
    )?;
    let start = scalar_of(&parts[0], catalog)?;
    let end = scalar_of(&parts[1], catalog)?;
    let step = scalar_of(&parts[2], catalog)?;
    if step == 0.0 {
        return None;
    }
    Some((((end - start) / step), parts[3].clone()))
}

/// The card of the rate that explains a change.
fn rate_over(
    tokens: &[Token],
    query: &str,
    catalog: &dyn Catalog,
) -> Result<Option<CalcResult>, UnsupportedArea> {
    let Some(Token::Ident(lead)) = tokens.first() else {
        return Ok(None);
    };
    if lead != "growth" && lead != "decay" {
        return Ok(None);
    }
    let parts = operands_of(
        tokens,
        &[
            Some(word(lead)),
            Some(word("every")),
            None,
            Some(word("from")),
            None,
            Some(word("to")),
            None,
            Some(word("over")),
            None,
        ],
    );
    let Some(parts) = parts else {
        return Ok(None);
    };
    let Some(change) = scalar_ratio(&parts[2], &parts[1], catalog) else {
        return Ok(None);
    };
    if change <= 0.0 {
        return Ok(None);
    }
    let Some(periods) = duration_ratio(&parts[3], &parts[0], catalog) else {
        return Ok(None);
    };
    if periods <= 0.0 {
        return Ok(None);
    }
    let per_period = change.powf(1.0 / periods) - 1.0;
    let value = if lead == "decay" {
        -per_period
    } else {
        per_period
    } * 100.0;
    if !value.is_finite() {
        return Ok(None);
    }
    Ok(Some(CalcResult::phrase(
        query,
        Payload::percentage(value),
        "Growth Rate",
    )))
}

/// The rate of one period, as a fraction.
///
/// `growth` makes it positive and `decay` makes it negative, so a rate written
/// the other way round still tells the right story.
fn rate(tokens: &[Token], trailing: &[Token], catalog: &dyn Catalog) -> Option<f64> {
    let value = scalar_of(tokens, catalog)?;
    let fraction = value / 100.0;
    match trailing {
        [] => Some(fraction),
        [Token::Ident(name)] if name == "growth" => Some(fraction.abs()),
        [Token::Ident(name)] if name == "decay" => Some(-fraction.abs()),
        _ => None,
    }
}

/// One duration as a multiple of another.
fn duration_ratio(
    numerator: &[Token],
    denominator: &[Token],
    catalog: &dyn Catalog,
) -> Option<f64> {
    let numerator = duration::read(numerator, catalog)?;
    let denominator = duration::read(denominator, catalog)?;
    duration::ratio(&numerator, &denominator)
}

/// One amount as a multiple of another.
fn scalar_ratio(numerator: &[Token], denominator: &[Token], catalog: &dyn Catalog) -> Option<f64> {
    let numerator = scalar_of(numerator, catalog)?;
    let denominator = scalar_of(denominator, catalog)?;
    if denominator == 0.0 {
        return None;
    }
    Some(numerator / denominator)
}

/// The number of a run of tokens.
fn scalar_of(tokens: &[Token], catalog: &dyn Catalog) -> Option<f64> {
    let mut parser = Parser::new(tokens);
    match parser.parse(catalog) {
        Ok(value) if value.is_scalar() => Some(value.effective()),
        _ => None,
    }
}

/// The value of a run of tokens.
fn value_of(
    tokens: &[Token],
    catalog: &dyn Catalog,
) -> Result<Option<crate::value::Value>, UnsupportedArea> {
    let mut parser = Parser::new(tokens);
    match parser.parse(catalog) {
        Ok(value) if !value.is_boolean => Ok(Some(value)),
        Ok(_) => Ok(None),
        Err(ParseError::NotACalculation) => Ok(None),
        Err(ParseError::Unsupported(area)) => Err(area),
    }
}

/// Whether the tokens hold a word.
fn contains(tokens: &[Token], word_text: &str) -> bool {
    tokens
        .iter()
        .any(|token| token == &Token::Ident(word_text.to_string()))
}

/// A word token.
fn word(text: &str) -> Token {
    Token::Ident(text.to_string())
}

/// The `%` token.
fn percent() -> Token {
    Token::Op(Operator::Percent)
}
