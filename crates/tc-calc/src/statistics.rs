//! Reductions over a spoken list: `average of 10, 20 and 30`, `median of 3, 1, 2`.
//!
//! Mirrors upstream `CalcStatistics.swift`.
//!
//! The phrase is lowered to the function call it names, so a list is reduced by
//! the same arithmetic as `median(3, 1, 2)` - and the badge says which
//! reduction ran, so `min` and `max` are told apart on the card.

use crate::catalog::Catalog;
use crate::parser::Parser;
use crate::result::{CalcResult, Payload};
use crate::token::{Operator, Token};
use crate::value::{ParseError, UnsupportedArea, Value};

/// A reduction and the words that ask for it.
struct Statistic {
    /// The words the phrase starts with.
    head: Vec<String>,
    /// The function the phrase is lowered to.
    function: &'static str,
    /// The badge the card wears.
    badge: &'static str,
}

/// The card of a reduction over a spoken list.
pub fn evaluate(
    tokens: &[Token],
    query: &str,
    catalog: &dyn Catalog,
) -> Result<Option<CalcResult>, UnsupportedArea> {
    if tokens.len() < 3 {
        return Ok(None);
    }
    let Some(Token::Ident(lead)) = tokens.first() else {
        return Ok(None);
    };
    let statistics = statistics();
    if !statistics.iter().any(|statistic| {
        statistic
            .head
            .first()
            .map(|word| word == lead)
            .unwrap_or(false)
    }) {
        return Ok(None);
    }
    for statistic in &statistics {
        if !starts_with(tokens, &statistic.head) {
            continue;
        }
        let items = split_list(&tokens[statistic.head.len()..]);
        if items.len() < 2 {
            return Ok(None);
        }
        // Lowered to the function call, so a list is reduced exactly as `sum(…)` is.
        let mut call = vec![
            Token::Ident(statistic.function.to_string()),
            Token::Op(Operator::Open),
        ];
        for (index, item) in items.iter().enumerate() {
            if index > 0 {
                call.push(Token::Comma);
            }
            call.extend(item.to_vec());
        }
        call.push(Token::Op(Operator::Close));
        let Some(value) = value_of(&call, catalog)? else {
            return Ok(None);
        };
        return Ok(Some(CalcResult::phrase(
            query,
            amount_payload(value),
            statistic.badge,
        )));
    }
    Ok(None)
}

/// The reductions, in upstream's order.
fn statistics() -> Vec<Statistic> {
    let rows: [(&str, &str, &str); 25] = [
        ("average of", "average", "Average"),
        ("avg of", "average", "Average"),
        ("mean of", "average", "Average"),
        ("midpoint between", "average", "Average"),
        ("midpoint of", "average", "Average"),
        ("halfway between", "average", "Average"),
        ("sum of", "sum", "Sum"),
        ("total of", "sum", "Sum"),
        ("min of", "min", "Minimum"),
        ("smaller of", "min", "Minimum"),
        ("lesser of", "min", "Minimum"),
        ("max of", "max", "Maximum"),
        ("larger of", "max", "Maximum"),
        ("greater of", "max", "Maximum"),
        ("median of", "median", "Median"),
        ("count of", "count", "Count"),
        ("standard deviation of", "stddev", "Standard Deviation"),
        ("std dev of", "stddev", "Standard Deviation"),
        ("stddev of", "stddev", "Standard Deviation"),
        ("gcd of", "gcd", "Result"),
        ("gcf of", "gcd", "Result"),
        ("greatest common divisor of", "gcd", "Result"),
        ("greatest common factor of", "gcd", "Result"),
        ("lcm of", "lcm", "Result"),
        ("lowest common multiple of", "lcm", "Result"),
    ];
    let mut table: Vec<Statistic> = rows
        .iter()
        .map(|(phrase, function, badge)| Statistic {
            head: phrase.split(' ').map(str::to_string).collect(),
            function,
            badge,
        })
        .collect();
    // Upstream's own list ends with the third spelling of the least common multiple.
    table.push(Statistic {
        head: "least common multiple of"
            .split(' ')
            .map(str::to_string)
            .collect(),
        function: "lcm",
        badge: "Result",
    });
    table
}

/// The items of a list, split on a top-level `,` or `and`.
///
/// So `sum of 2*3, 4` stays two operands.
fn split_list(tokens: &[Token]) -> Vec<Vec<Token>> {
    let mut items: Vec<Vec<Token>> = Vec::new();
    let mut current: Vec<Token> = Vec::new();
    let mut depth = 0i32;
    for token in tokens {
        if *token == Token::Op(Operator::Open) {
            depth += 1;
        }
        if *token == Token::Op(Operator::Close) {
            depth -= 1;
        }
        if depth == 0 && (*token == Token::Comma || *token == Token::Ident("and".to_string())) {
            if current.is_empty() {
                return Vec::new();
            }
            items.push(current);
            current = Vec::new();
        } else {
            current.push(token.clone());
        }
    }
    if current.is_empty() {
        return Vec::new();
    }
    items.push(current);
    items
}

/// Whether the tokens start with these words.
fn starts_with(tokens: &[Token], head: &[String]) -> bool {
    if tokens.len() < head.len() {
        return false;
    }
    head.iter()
        .enumerate()
        .all(|(index, word)| tokens[index] == Token::Ident(word.clone()))
}

/// The payload of an amount, keeping the money and the measure it carries.
fn amount_payload(amount: Value) -> Payload {
    match (amount.currency, amount.unit) {
        (Some(currency), _) => Payload::money(amount.amount, currency.code),
        (None, Some(unit)) => Payload::measured(amount.amount, &unit),
        (None, None) => Payload::number(amount.amount),
    }
}

/// The value of a run of tokens.
fn value_of(tokens: &[Token], catalog: &dyn Catalog) -> Result<Option<Value>, UnsupportedArea> {
    let mut parser = Parser::new(tokens);
    match parser.parse(catalog) {
        Ok(value) if !value.is_boolean => Ok(Some(value)),
        Ok(_) => Ok(None),
        Err(ParseError::NotACalculation) => Ok(None),
        Err(ParseError::Unsupported(area)) => Err(area),
    }
}
