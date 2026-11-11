//! The rounding and fraction phrasings, such as `1/3 to 2 dp` and `0.75 as fraction`.
//!
//! Mirrors upstream `CalcRounding.swift`.

use crate::catalog::Catalog;
use crate::parser::Parser;
use crate::result::{CalcResult, Payload};
use crate::token::Token;
use crate::value::{ParseError, UnsupportedArea, Value};

/// The words that mark a query as a rounding or fraction query.
const KEYWORDS: [&str; 11] = [
    "round", "rounded", "nearest", "dp", "decimal", "decimals", "digit", "digits", "places",
    "fraction", "frac",
];

/// The words that stand for a step, as upstream's `CalcRounding.stepWords`.
fn step_word(name: &str) -> Option<f64> {
    Some(match name {
        "ten" => 10.0,
        "hundred" => 100.0,
        "thousand" => 1_000.0,
        "million" => 1_000_000.0,
        "half" => 0.5,
        "third" => 1.0 / 3.0,
        "quarter" => 0.25,
        "tenth" => 0.1,
        "hundredth" => 0.01,
        "thousandth" => 0.001,
        _ => return None,
    })
}

/// The word lists that name a number of decimal places, as upstream's
/// `CalcRounding.placeWords`.
fn place_words(words: &[&str]) -> bool {
    const PLACES: [[&str; 1]; 6] = [
        ["dp"],
        ["digit"],
        ["digits"],
        ["decimal"],
        ["decimals"],
        ["places"],
    ];
    const PAIRS: [[&str; 2]; 4] = [
        ["decimal", "places"],
        ["decimal", "place"],
        ["decimal", "point"],
        ["decimal", "points"],
    ];
    if words.len() == 1 && PLACES.iter().any(|candidate| candidate[0] == words[0]) {
        return true;
    }
    words.len() == 2 && PAIRS.iter().any(|candidate| candidate == words)
}

/// The direction a value is rounded in, as Swift's `FloatingPointRoundingRule`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rule {
    /// `.toNearestOrAwayFromZero`
    Nearest,
    /// `.up`, towards positive infinity.
    Up,
    /// `.down`, towards negative infinity.
    Down,
}

impl Rule {
    /// Rounds `value` the way this rule says.
    fn apply(self, value: f64) -> f64 {
        match self {
            Self::Nearest => value.round(),
            Self::Up => value.ceil(),
            Self::Down => value.floor(),
        }
    }
}

/// The step or the number of decimal places a value is rounded to.
#[derive(Debug, Clone, Copy)]
enum Precision {
    /// A multiple of the step.
    Step(f64),
    /// A number of decimal places.
    Decimals(i32),
}

impl Precision {
    /// Rounds `amount`, as upstream's `CalcRounding.Precision.apply`.
    fn apply(self, amount: f64, rule: Rule) -> f64 {
        match self {
            Self::Step(step) => rule.apply(amount / step) * step,
            Self::Decimals(places) => {
                let scale = 10.0_f64.powi(places);
                rule.apply(amount * scale) / scale
            }
        }
    }
}

/// The card of a rounding or fraction query.
///
/// Upstream answers `nil` when the query is not a rounding or fraction query.
pub fn evaluate(
    tokens: &[Token],
    query: &str,
    catalog: &dyn Catalog,
) -> Result<Option<CalcResult>, UnsupportedArea> {
    let mentions_rounding = tokens.iter().any(|token| match token {
        Token::Ident(name) => KEYWORDS.contains(&name.as_str()),
        _ => false,
    });
    if !mentions_rounding {
        return Ok(None);
    }
    if let Some(result) = fraction(tokens, query, catalog)? {
        return Ok(Some(result));
    }
    rounded(tokens, query, catalog)
}

/// The card of a rounded query, as upstream's `CalcRounding.rounded`.
fn rounded(
    tokens: &[Token],
    query: &str,
    catalog: &dyn Catalog,
) -> Result<Option<CalcResult>, UnsupportedArea> {
    let mut rest: Vec<Token> = tokens.to_vec();
    let mut rule: Option<Rule> = None;
    let leads_with_round = rest.first() == Some(&Token::Ident("round".to_string()));
    if leads_with_round {
        rest.remove(0);
        rule = direction(&mut rest);
    }
    let stops: Vec<Token> = if leads_with_round {
        vec![ident("rounded"), ident("to"), ident("up"), ident("down")]
    } else {
        vec![ident("rounded"), ident("to")]
    };
    let Some(end) = rest.iter().position(|token| stops.contains(token)) else {
        return Ok(None);
    };
    let Some(mut value) = parse_value(&rest[..end], catalog)? else {
        return Ok(None);
    };
    if value.is_percent {
        return Ok(None);
    }
    rest = rest[end..].to_vec();
    let named = rest.first() == Some(&ident("rounded"));
    if named || (leads_with_round && rest.first() != Some(&ident("to"))) {
        if rule.is_some() {
            return Ok(None);
        }
        if named {
            rest.remove(0);
        }
        rule = direction(&mut rest);
    }
    let precision = if rest.is_empty() {
        if rule.is_none() && !named {
            return Ok(None);
        }
        Precision::Step(1.0)
    } else {
        if rest.first() != Some(&ident("to")) {
            return Ok(None);
        }
        match precision(&rest[1..], &mut rule, catalog) {
            Some(precision) => precision,
            None => return Ok(None),
        }
    };
    value.amount = precision.apply(value.amount, rule.unwrap_or(Rule::Nearest));
    if !value.amount.is_finite() {
        return Ok(None);
    }
    Ok(Some(CalcResult::phrase(
        query,
        Payload::number(value.effective()),
        "Rounded",
    )))
}

/// The direction a query rounds in, as upstream's `CalcRounding.direction`.
fn direction(rest: &mut Vec<Token>) -> Option<Rule> {
    match rest.first() {
        Some(Token::Ident(name)) if name == "up" => {
            rest.remove(0);
            Some(Rule::Up)
        }
        Some(Token::Ident(name)) if name == "down" => {
            rest.remove(0);
            Some(Rule::Down)
        }
        _ => None,
    }
}

/// The step or the number of decimal places, as upstream's `CalcRounding.precision`.
///
/// `nearest 5`, `next 10`, `nearest 8th` and `2 dp` are all read here; a bare `10` is a
/// step only after `up` or `down`.
fn precision(
    tokens: &[Token],
    rule: &mut Option<Rule>,
    catalog: &dyn Catalog,
) -> Option<Precision> {
    let mut target: Vec<Token> = tokens.to_vec();
    if target.first() == Some(&ident("the")) {
        target.remove(0);
    }
    match target.first() {
        Some(Token::Ident(name)) if name == "nearest" => {
            target.remove(0);
        }
        Some(Token::Ident(name)) if name == "next" || name == "last" => {
            let implied = if name == "next" { Rule::Up } else { Rule::Down };
            if rule.is_some() && *rule != Some(implied) {
                return None;
            }
            *rule = Some(implied);
            target.remove(0);
        }
        _ => {
            if rule.is_some() {
                if let Some(step) = scalar_of(&target, catalog) {
                    if step > 0.0 {
                        return Some(Precision::Step(step));
                    }
                }
            }
            let places = target.first().and_then(Token::number_value);
            let matches = places
                .map(|places| places.round() == places && (0..=15).contains(&(places as i32)))
                .unwrap_or(false);
            if !matches {
                return None;
            }
            let words: Vec<String> = target[1..]
                .iter()
                .map(|token| match token {
                    Token::Ident(name) => name.clone(),
                    _ => String::new(),
                })
                .collect();
            let borrowed: Vec<&str> = words.iter().map(String::as_str).collect();
            if !place_words(&borrowed) {
                return None;
            }
            return Some(Precision::Decimals(places.unwrap_or(0.0) as i32));
        }
    }
    if target.len() == 1 {
        if let Token::Ident(name) = &target[0] {
            if let Some(step) = step_word(name) {
                return Some(Precision::Step(step));
            }
        }
    }
    if target.len() == 2 {
        let denominator = target[0].number_value();
        if let (Some(denominator), Token::Ident(suffix)) = (denominator, &target[1]) {
            if ["st", "nd", "rd", "th"].contains(&suffix.as_str()) && denominator >= 2.0 {
                return Some(Precision::Step(1.0 / denominator));
            }
        }
    }
    let step = scalar_of(&target, catalog)?;
    if step <= 0.0 {
        return None;
    }
    Some(Precision::Step(step))
}

/// The card of a fraction query, as upstream's `CalcRounding.fraction`.
fn fraction(
    tokens: &[Token],
    query: &str,
    catalog: &dyn Catalog,
) -> Result<Option<CalcResult>, UnsupportedArea> {
    let Some(parts) = fraction_targets()
        .iter()
        .find_map(|pattern| crate::token::operands_of(tokens, pattern))
    else {
        return Ok(None);
    };
    let Some(value) = parse_value(&parts[0], catalog)? else {
        return Ok(None);
    };
    let Some((numerator, denominator)) = simplest_fraction(value.effective()) else {
        return Ok(None);
    };
    let text = if denominator == 1 {
        numerator.to_string()
    } else {
        format!("{numerator}/{denominator}")
    };
    Ok(Some(CalcResult::phrase(
        query,
        Payload::Value {
            display: text.clone(),
            copy_text: text,
        },
        "Fraction",
    )))
}

/// The patterns a fraction query matches, as upstream's `CalcRounding.fractionTargets`.
fn fraction_targets() -> Vec<Vec<Option<Token>>> {
    let mut targets = Vec::new();
    for connector in ["as", "to", "in"] {
        for words in [vec!["fraction"], vec!["a", "fraction"], vec!["frac"]] {
            let mut pattern: Vec<Option<Token>> = vec![None, Some(ident(connector))];
            for word in words {
                pattern.push(Some(ident(word)));
            }
            targets.push(pattern);
        }
    }
    targets
}

/// The simplest fraction within a billionth of `value`, as upstream's
/// `CalcRounding.simplestFraction`.
fn simplest_fraction(value: f64) -> Option<(i64, i64)> {
    if !value.is_finite() || value.abs() >= 1e9 {
        return None;
    }
    let target = value.abs();
    let mut previous_numerator = 0.0;
    let mut numerator = 1.0;
    let mut previous_denominator = 1.0;
    let mut denominator = 0.0;
    let mut remainder = target;
    for _ in 0..32 {
        let whole = remainder.floor();
        let next_numerator = whole * numerator + previous_numerator;
        previous_numerator = numerator;
        numerator = next_numerator;
        let next_denominator = whole * denominator + previous_denominator;
        previous_denominator = denominator;
        denominator = next_denominator;
        if denominator > 10_000.0 {
            return None;
        }
        if (numerator / denominator - target).abs() <= 1e-9 * target.max(1.0) {
            let signed = if value < 0.0 { -numerator } else { numerator };
            return Some((signed as i64, denominator as i64));
        }
        remainder = 1.0 / (remainder - whole);
        if !remainder.is_finite() {
            return None;
        }
    }
    None
}

/// A token that is a word.
fn ident(name: &str) -> Token {
    Token::Ident(name.to_string())
}

/// The value of a token list, as upstream's `CalcExpressionParser.value`.
fn parse_value(tokens: &[Token], catalog: &dyn Catalog) -> Result<Option<Value>, UnsupportedArea> {
    let mut parser = Parser::new(tokens);
    match parser.parse(catalog) {
        Ok(value) if !value.is_boolean => Ok(Some(value)),
        Ok(_) => Ok(None),
        Err(ParseError::NotACalculation) => Ok(None),
        Err(ParseError::Unsupported(area)) => Err(area),
    }
}

/// The scalar value of a token list, as upstream's `CalcExpressionParser.scalar`.
fn scalar_of(tokens: &[Token], catalog: &dyn Catalog) -> Option<f64> {
    let mut parser = Parser::new(tokens);
    match parser.parse(catalog) {
        Ok(value) if !value.is_boolean => Some(value.effective()),
        _ => None,
    }
}
