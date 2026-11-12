//! The percentage phrasings the plain arithmetic misses: `15% tip on 42`,
//! `20% off 100`, `50 as % of 200`, `30 is 20% what is 40`.
//!
//! Mirrors upstream `CalcPercent.swift`.
//!
//! The rules, in upstream's own order:
//!
//! - **A word between the percentage and of, on or off only names it.** So
//!   `20% discount off 100` is `20% off 100` - except `tip`, which has a
//!   meaning of its own, and so do `tax in`, `vat in` and `gst in`, which are
//!   the tax already inside a price.
//! - **A tip is the tip on its own**; a tax is added to the amount; a
//!   discount is taken off it.
//! - **The card labels the answer** - `Tip`, `Discounted`, `Total`,
//!   `Increased`, `Result`, `Percentage`, `Percent Change`, `Proportion`,
//!   `Ratio` - instead of a generic one.
//!
//! The order the phrases are tried in is upstream's: an amount scaled by a
//! percentage first, then the rule of three, then a trailing tax, then the
//! percentages, then the rule of three that answers a percentage, and last
//! the ratio.

use crate::catalog::Catalog;
use crate::formatter::Formatter;
use crate::parser::Parser;
use crate::result::{CalcResult, Payload};
use crate::token::{operands_of, Operator, Token};
use crate::value::{ParseError, UnsupportedArea, Value};

/// The words that name a percentage without changing it: `20% discount off 100`
/// is `20% off 100`.
const KEPT_WORDS: [&str; 11] = [
    "tip", "of", "on", "off", "is", "what", "with", "change", "increase", "decrease", "after",
];

/// The words that name a tax.
const TAX_WORDS: [&str; 3] = ["tax", "vat", "gst"];

/// The relations a comment word may stand between.
const RELATIONS: [&str; 4] = ["of", "on", "off", "in"];

/// A word token.
fn word(text: &str) -> Token {
    Token::Ident(text.to_string())
}

/// The `%` token.
fn percent() -> Token {
    Token::Op(Operator::Percent)
}

/// The card of a percentage query.
///
/// Upstream answers `nil` when the query is not a percentage query.
pub fn evaluate(
    tokens: &[Token],
    query: &str,
    catalog: &dyn Catalog,
) -> Result<Option<CalcResult>, UnsupportedArea> {
    let tokens = without_comment(tokens, catalog);
    if let Some(result) = adjusted(&tokens, query, catalog)? {
        return Ok(Some(result));
    }
    if let Some(result) = rule_of_three_amount(&tokens, query, catalog)? {
        return Ok(Some(result));
    }
    if let Some(result) = with_tax(&tokens, query, catalog)? {
        return Ok(Some(result));
    }
    if let Some(result) = percentage(&tokens, query, catalog)? {
        return Ok(Some(result));
    }
    if let Some(result) = rule_of_three_percentage(&tokens, query, catalog)? {
        return Ok(Some(result));
    }
    parse_ratio(&tokens, query, catalog)
}

/// Removes the words that only name a percentage.
///
/// `20% discount off 100` is `20% off 100`, and `20% sales tax on 100` is
/// `20% tax on 100`. The words with a meaning of their own are kept: `tip`,
/// and a tax that is `in` a price. A word the catalog names as a unit or a
/// currency is kept as well, so a unit is never mistaken for a comment.
fn without_comment(tokens: &[Token], catalog: &dyn Catalog) -> Vec<Token> {
    let mut tokens = tokens.to_vec();
    // "sales tax" is one name: the "sales" is the comment.
    if let Some(sales) = tokens.iter().position(|token| *token == word("sales")) {
        if sales + 1 < tokens.len() && tokens[sales + 1] == word("tax") {
            tokens.remove(sales);
        }
    }
    let Some(index) = tokens.iter().position(|token| *token == percent()) else {
        return tokens;
    };
    if index + 2 >= tokens.len() {
        return tokens;
    }
    let Token::Ident(name) = tokens[index + 1].clone() else {
        return tokens;
    };
    if KEPT_WORDS.contains(&name.as_str()) {
        return tokens;
    }
    if catalog.unit(&name).is_some() || catalog.currency(&name).is_some() {
        return tokens;
    }
    let relation = tokens[index + 2].clone();
    let names_included_tax = TAX_WORDS.contains(&name.as_str()) && relation == word("in");
    if names_included_tax {
        return tokens;
    }
    if !RELATIONS.iter().any(|text| relation == word(text)) {
        return tokens;
    }
    tokens.remove(index + 1);
    tokens
}

/// An amount scaled by a percentage.
struct Adjustment {
    /// The shape the query has to have.
    pattern: Vec<Option<Token>>,
    /// The label the card carries.
    badge: &'static str,
    /// Which of the two operands is the amount.
    amount_index: usize,
    /// What the percentage does to the amount.
    factor: fn(f64) -> f64,
}

/// The adjustments, in upstream's order.
fn adjustments() -> Vec<Adjustment> {
    let mut table: Vec<Adjustment> = vec![
        Adjustment {
            pattern: vec![
                None,
                Some(percent()),
                Some(word("tip")),
                Some(word("on")),
                None,
            ],
            badge: "Tip",
            amount_index: 1,
            factor: |percentage| percentage / 100.0,
        },
        Adjustment {
            pattern: vec![
                None,
                Some(percent()),
                Some(word("tip")),
                Some(word("of")),
                None,
            ],
            badge: "Tip",
            amount_index: 1,
            factor: |percentage| percentage / 100.0,
        },
        Adjustment {
            pattern: vec![None, Some(percent()), Some(word("off")), None],
            badge: "Discounted",
            amount_index: 1,
            factor: |percentage| 1.0 - percentage / 100.0,
        },
        Adjustment {
            pattern: vec![
                None,
                Some(percent()),
                Some(word("off")),
                Some(word("of")),
                None,
            ],
            badge: "Discounted",
            amount_index: 1,
            factor: |percentage| 1.0 - percentage / 100.0,
        },
        Adjustment {
            pattern: vec![
                Some(word("take")),
                None,
                Some(percent()),
                Some(word("off")),
                None,
            ],
            badge: "Discounted",
            amount_index: 1,
            factor: |percentage| 1.0 - percentage / 100.0,
        },
        Adjustment {
            pattern: vec![None, Some(percent()), Some(word("of")), None],
            badge: "Result",
            amount_index: 1,
            factor: |percentage| percentage / 100.0,
        },
        Adjustment {
            pattern: vec![None, Some(percent()), Some(word("from")), None],
            badge: "Result",
            amount_index: 1,
            factor: |percentage| percentage / 100.0,
        },
        Adjustment {
            pattern: vec![None, Some(percent()), Some(word("on")), None],
            badge: "Increased",
            amount_index: 1,
            factor: |percentage| 1.0 + percentage / 100.0,
        },
        Adjustment {
            pattern: vec![
                None,
                Some(word("is")),
                None,
                Some(percent()),
                Some(word("of")),
                Some(word("what")),
            ],
            badge: "Total",
            amount_index: 0,
            factor: |percentage| 100.0 / percentage,
        },
        Adjustment {
            pattern: vec![
                None,
                Some(word("is")),
                None,
                Some(percent()),
                Some(word("off")),
                Some(word("what")),
            ],
            badge: "Total",
            amount_index: 0,
            factor: |percentage| 1.0 / (1.0 - percentage / 100.0),
        },
        Adjustment {
            pattern: vec![
                None,
                Some(word("is")),
                None,
                Some(percent()),
                Some(word("on")),
                Some(word("what")),
            ],
            badge: "Total",
            amount_index: 0,
            factor: |percentage| 1.0 / (1.0 + percentage / 100.0),
        },
        Adjustment {
            pattern: vec![
                None,
                Some(percent()),
                Some(word("of")),
                Some(word("what")),
                Some(word("is")),
                None,
            ],
            badge: "Total",
            amount_index: 1,
            factor: |percentage| 100.0 / percentage,
        },
        Adjustment {
            pattern: vec![
                None,
                Some(percent()),
                Some(word("off")),
                Some(word("what")),
                Some(word("is")),
                None,
            ],
            badge: "Total",
            amount_index: 1,
            factor: |percentage| 1.0 / (1.0 - percentage / 100.0),
        },
        Adjustment {
            pattern: vec![
                None,
                Some(percent()),
                Some(word("on")),
                Some(word("what")),
                Some(word("is")),
                None,
            ],
            badge: "Total",
            amount_index: 1,
            factor: |percentage| 1.0 / (1.0 + percentage / 100.0),
        },
    ];
    // The tax already inside a price: `20% vat in 120` is 20.
    for name in TAX_WORDS {
        table.push(Adjustment {
            pattern: vec![
                None,
                Some(percent()),
                Some(word(name)),
                Some(word("in")),
                None,
            ],
            badge: "Tax",
            amount_index: 1,
            factor: |percentage| percentage / (100.0 + percentage),
        });
    }
    table
}

/// The card of a query whose amount is scaled by a percentage.
fn adjusted(
    tokens: &[Token],
    query: &str,
    catalog: &dyn Catalog,
) -> Result<Option<CalcResult>, UnsupportedArea> {
    let Some((value, badge)) = adjusted_value(tokens, catalog)? else {
        return Ok(None);
    };
    Ok(Some(CalcResult::phrase(
        query,
        amount_payload(value, value.amount),
        badge,
    )))
}

/// The amount of a discount, tip or markup phrase, unrounded.
///
/// Upstream's `CalcPercent.amount`, for a caller that shares it out.
pub fn amount(tokens: &[Token], catalog: &dyn Catalog) -> Result<Option<Value>, UnsupportedArea> {
    let tokens = without_comment(tokens, catalog);
    Ok(adjusted_value(&tokens, catalog)?.map(|(value, _)| value))
}

/// The scaled amount of a discount, tip or markup phrase, and the badge it earns.
fn adjusted_value(
    tokens: &[Token],
    catalog: &dyn Catalog,
) -> Result<Option<(Value, &'static str)>, UnsupportedArea> {
    for adjustment in adjustments() {
        let Some(parts) = operands_of(tokens, &adjustment.pattern) else {
            continue;
        };
        if parts.len() <= adjustment.amount_index {
            continue;
        }
        let Some(amount) = value_of(&parts[adjustment.amount_index], catalog)? else {
            continue;
        };
        if amount.is_percent {
            continue;
        }
        let Some(percentage) = scalar_of(&parts[1 - adjustment.amount_index], catalog) else {
            continue;
        };
        let scaled = amount.amount * (adjustment.factor)(percentage);
        if !scaled.is_finite() {
            continue;
        }
        let mut value = amount;
        value.amount = scaled;
        value.is_percent = false;
        return Ok(Some((value, adjustment.badge)));
    }
    Ok(None)
}

/// The payload of an amount, keeping the money it carries.
///
/// Upstream's `.quantity(value)` shows money as money and a plain amount as a
/// plain number.
fn amount_payload(amount: Value, scaled: f64) -> Payload {
    match amount.currency {
        Some(currency) => Payload::money(scaled, currency.code),
        None => Payload::number(scaled),
    }
}

/// The card of a query that ends in a tax: `100 + 10% tax` is 110.
fn with_tax(
    tokens: &[Token],
    query: &str,
    catalog: &dyn Catalog,
) -> Result<Option<CalcResult>, UnsupportedArea> {
    if tokens.len() < 4 {
        return Ok(None);
    }
    let Some(Token::Ident(name)) = tokens.last() else {
        return Ok(None);
    };
    if !TAX_WORDS.contains(&name.as_str()) {
        return Ok(None);
    }
    if tokens[tokens.len() - 2] != percent() {
        return Ok(None);
    }
    let Some(value) = value_of(&tokens[..tokens.len() - 1], catalog)? else {
        return Ok(None);
    };
    if value.is_percent {
        return Ok(None);
    }
    Ok(Some(CalcResult::phrase(
        query,
        amount_payload(value, value.amount),
        "Total",
    )))
}

/// A percentage of, between or as.
struct Ratio {
    /// The shape the query has to have.
    pattern: Vec<Option<Token>>,
    /// The label the card carries.
    badge: &'static str,
    /// What the two parts make.
    kind: RatioKind,
}

/// What a ratio pattern makes of its two parts.
#[derive(Clone, Copy, PartialEq, Eq)]
enum RatioKind {
    /// The first part's share of the second.
    Share,
    /// What is left of the whole once the first part is taken off it.
    Discounted,
    /// What the first part is of the second, more than the whole.
    MarkedUp,
    /// The second part's share of the first, less the whole.
    ChangeOf,
    /// The first part's share of the two together.
    Together,
    /// What adding the second part grew the first by.
    Growth,
    /// The first part, read as a percentage of itself.
    Scalar,
}

/// The ratio patterns, in upstream's order.
fn ratios() -> Vec<Ratio> {
    vec![
        Ratio {
            pattern: vec![
                None,
                Some(word("as")),
                Some(percent()),
                Some(word("of")),
                None,
            ],
            badge: "Percentage",
            kind: RatioKind::Share,
        },
        Ratio {
            pattern: vec![
                None,
                Some(word("as")),
                Some(word("a")),
                Some(percent()),
                Some(word("of")),
                None,
            ],
            badge: "Percentage",
            kind: RatioKind::Share,
        },
        Ratio {
            pattern: vec![
                None,
                Some(word("is")),
                Some(word("what")),
                Some(percent()),
                Some(word("of")),
                None,
            ],
            badge: "Percentage",
            kind: RatioKind::Share,
        },
        Ratio {
            pattern: vec![
                None,
                Some(word("is")),
                Some(word("what")),
                Some(percent()),
                Some(word("off")),
                None,
            ],
            badge: "Percent Change",
            kind: RatioKind::Discounted,
        },
        Ratio {
            pattern: vec![
                None,
                Some(word("is")),
                Some(word("what")),
                Some(percent()),
                Some(word("on")),
                None,
            ],
            badge: "Percent Change",
            kind: RatioKind::MarkedUp,
        },
        Ratio {
            pattern: vec![
                None,
                Some(word("to")),
                None,
                Some(word("is")),
                Some(word("what")),
                Some(percent()),
            ],
            badge: "Percent Change",
            kind: RatioKind::ChangeOf,
        },
        Ratio {
            pattern: vec![
                None,
                Some(word("to")),
                None,
                Some(word("as")),
                Some(percent()),
            ],
            badge: "Percent Change",
            kind: RatioKind::ChangeOf,
        },
        Ratio {
            pattern: vec![
                Some(word("what")),
                Some(percent()),
                Some(word("is")),
                None,
                Some(word("of")),
                None,
            ],
            badge: "Percentage",
            kind: RatioKind::Share,
        },
        Ratio {
            pattern: vec![
                None,
                Some(word("is")),
                Some(percent()),
                Some(word("of")),
                None,
            ],
            badge: "Percentage",
            kind: RatioKind::Share,
        },
        Ratio {
            pattern: vec![
                None,
                Some(word("is")),
                Some(word("what")),
                Some(percent()),
                None,
            ],
            badge: "Percentage",
            kind: RatioKind::Share,
        },
        Ratio {
            pattern: vec![None, Some(word("as")), Some(percent()), None],
            badge: "Percentage",
            kind: RatioKind::Share,
        },
        Ratio {
            pattern: vec![
                None,
                Some(word("is")),
                Some(word("what")),
                Some(percent()),
                Some(word("with")),
                None,
            ],
            badge: "Percentage",
            kind: RatioKind::Together,
        },
        Ratio {
            pattern: vec![
                None,
                Some(word("as")),
                Some(percent()),
                Some(word("with")),
                None,
            ],
            badge: "Percentage",
            kind: RatioKind::Together,
        },
        Ratio {
            pattern: vec![None, Some(word("out")), Some(word("of")), None],
            badge: "Percentage",
            kind: RatioKind::Share,
        },
        Ratio {
            pattern: vec![
                None,
                Some(word("as")),
                Some(word("discount")),
                Some(word("off")),
                None,
            ],
            badge: "Percent Change",
            kind: RatioKind::Discounted,
        },
        Ratio {
            pattern: vec![
                None,
                Some(word("is")),
                Some(word("what")),
                Some(word("discount")),
                Some(word("off")),
                None,
            ],
            badge: "Percent Change",
            kind: RatioKind::Discounted,
        },
        Ratio {
            pattern: vec![
                Some(percent()),
                Some(word("change")),
                Some(word("from")),
                None,
                Some(word("to")),
                None,
            ],
            badge: "Percent Change",
            kind: RatioKind::ChangeOf,
        },
        Ratio {
            pattern: vec![
                Some(percent()),
                Some(word("increase")),
                Some(word("from")),
                None,
                Some(word("to")),
                None,
            ],
            badge: "Percent Change",
            kind: RatioKind::ChangeOf,
        },
        Ratio {
            pattern: vec![
                Some(percent()),
                Some(word("decrease")),
                Some(word("from")),
                None,
                Some(word("to")),
                None,
            ],
            badge: "Percent Change",
            kind: RatioKind::ChangeOf,
        },
        Ratio {
            pattern: vec![
                Some(word("what")),
                Some(percent()),
                Some(word("change")),
                Some(word("is")),
                None,
                Some(word("to")),
                None,
            ],
            badge: "Percent Change",
            kind: RatioKind::ChangeOf,
        },
        Ratio {
            pattern: vec![
                None,
                Some(word("as")),
                Some(percent()),
                Some(word("after")),
                Some(word("adding")),
                None,
            ],
            badge: "Percent Change",
            kind: RatioKind::Growth,
        },
        Ratio {
            pattern: vec![
                None,
                Some(word("as")),
                Some(percent()),
                Some(word("after")),
                None,
            ],
            badge: "Percent Change",
            kind: RatioKind::Growth,
        },
        Ratio {
            pattern: vec![
                None,
                Some(word("is")),
                Some(word("what")),
                Some(percent()),
                Some(word("after")),
                Some(word("adding")),
                None,
            ],
            badge: "Percent Change",
            kind: RatioKind::Growth,
        },
        Ratio {
            pattern: vec![
                None,
                Some(word("is")),
                Some(word("what")),
                Some(percent()),
                Some(word("after")),
                None,
            ],
            badge: "Percent Change",
            kind: RatioKind::Growth,
        },
        Ratio {
            pattern: vec![None, Some(word("as")), Some(percent())],
            badge: "Percentage",
            kind: RatioKind::Scalar,
        },
        Ratio {
            pattern: vec![None, Some(word("is")), Some(word("what")), Some(percent())],
            badge: "Percentage",
            kind: RatioKind::Scalar,
        },
    ]
}

/// The card of a query that answers a percentage.
fn percentage(
    tokens: &[Token],
    query: &str,
    catalog: &dyn Catalog,
) -> Result<Option<CalcResult>, UnsupportedArea> {
    for ratio in ratios() {
        let Some(parts) = operands_of(tokens, &ratio.pattern) else {
            continue;
        };
        let Some(value) = ratio_value(&ratio, &parts, catalog)? else {
            continue;
        };
        if !value.is_finite() {
            continue;
        }
        return Ok(Some(CalcResult::phrase(
            query,
            Payload::percentage(value),
            ratio.badge,
        )));
    }
    Ok(None)
}

/// What a ratio pattern makes of its two parts.
fn ratio_value(
    ratio: &Ratio,
    parts: &[Vec<Token>],
    catalog: &dyn Catalog,
) -> Result<Option<f64>, UnsupportedArea> {
    // `0.125 as %` has only one part: the number itself, read as a percentage.
    if ratio.kind == RatioKind::Scalar {
        if parts.is_empty() {
            return Ok(None);
        }
        return Ok(scalar_of(&parts[0], catalog).map(|value| value * 100.0));
    }
    if parts.len() < 2 {
        return Ok(None);
    }
    let value = match ratio.kind {
        RatioKind::Share => share(&parts[0], &parts[1], catalog)?,
        RatioKind::Discounted => share(&parts[0], &parts[1], catalog)?.map(|share| 100.0 - share),
        RatioKind::MarkedUp => share(&parts[0], &parts[1], catalog)?.map(|share| share - 100.0),
        RatioKind::ChangeOf => share(&parts[1], &parts[0], catalog)?.map(|share| share - 100.0),
        RatioKind::Together => {
            let together = together(&parts[0], &parts[1]);
            share(&parts[0], &together, catalog)?
        }
        RatioKind::Growth => {
            let base = difference(&parts[0], &parts[1]);
            share(&parts[1], &base, catalog)?
        }
        RatioKind::Scalar => None,
    };
    Ok(value)
}

/// The two parts as one sum, as upstream's `CalcPercent.share(_:with:)` builds it.
fn together(part: &[Token], other: &[Token]) -> Vec<Token> {
    let mut together = vec![Token::Op(Operator::Open)];
    together.extend(part.to_vec());
    together.extend([
        Token::Op(Operator::Close),
        Token::Op(Operator::Add),
        Token::Op(Operator::Open),
    ]);
    together.extend(other.to_vec());
    together.push(Token::Op(Operator::Close));
    together
}

/// The first part less the second, as upstream's `CalcPercent.growth(_:after:)` builds it.
fn difference(part: &[Token], other: &[Token]) -> Vec<Token> {
    let mut difference = vec![Token::Op(Operator::Open)];
    difference.extend(part.to_vec());
    difference.extend([
        Token::Op(Operator::Close),
        Token::Op(Operator::Subtract),
        Token::Op(Operator::Open),
    ]);
    difference.extend(other.to_vec());
    difference.push(Token::Op(Operator::Close));
    difference
}

/// One part's share of the whole the two make together.
///
/// `10 is what % with 20` is a third: the whole is what the two make together.
fn share(
    part: &[Token],
    whole: &[Token],
    catalog: &dyn Catalog,
) -> Result<Option<f64>, UnsupportedArea> {
    let mut quotient = vec![Token::Op(Operator::Open)];
    quotient.extend(part.to_vec());
    quotient.extend([
        Token::Op(Operator::Close),
        Token::Op(Operator::Divide),
        Token::Op(Operator::Open),
    ]);
    quotient.extend(whole.to_vec());
    quotient.push(Token::Op(Operator::Close));
    Ok(scalar_of(&quotient, catalog).map(|value| value * 100.0))
}

/// The card of a rule of three: `if 30 is 20%, what is 50%` is 75.
fn rule_of_three_amount(
    tokens: &[Token],
    query: &str,
    catalog: &dyn Catalog,
) -> Result<Option<CalcResult>, UnsupportedArea> {
    let Some((known, known_percent, asked)) = rule_of_three_parts(tokens, true) else {
        return Ok(None);
    };
    let Some(value) = value_of(&known, catalog)? else {
        return Ok(None);
    };
    if value.is_percent {
        return Ok(None);
    }
    let Some(given) = scalar_of(&known_percent, catalog) else {
        return Ok(None);
    };
    if given == 0.0 {
        return Ok(None);
    }
    let Some(asked) = scalar_of(&asked, catalog) else {
        return Ok(None);
    };
    let scaled = value.amount * asked / given;
    if !scaled.is_finite() {
        return Ok(None);
    }
    Ok(Some(CalcResult::phrase(
        query,
        Payload::number(scaled),
        "Proportion",
    )))
}

/// The card of a rule of three that answers a percentage.
fn rule_of_three_percentage(
    tokens: &[Token],
    query: &str,
    catalog: &dyn Catalog,
) -> Result<Option<CalcResult>, UnsupportedArea> {
    let Some((known, known_percent, asked)) = rule_of_three_parts(tokens, false) else {
        return Ok(None);
    };
    let Some(ratio) = share(&asked, &known, catalog)? else {
        return Ok(None);
    };
    let Some(given) = scalar_of(&known_percent, catalog) else {
        return Ok(None);
    };
    let value = ratio / 100.0 * given;
    if !value.is_finite() {
        return Ok(None);
    }
    Ok(Some(CalcResult::phrase(
        query,
        Payload::percentage(value),
        "Percentage",
    )))
}

/// The known amount, its percentage, and what is asked.
///
/// Upstream reads the two orders `30 is 20%` and `20% is 30` alike.
fn rule_of_three_parts(
    tokens: &[Token],
    asked_is_percent: bool,
) -> Option<(Vec<Token>, Vec<Token>, Vec<Token>)> {
    let mut words: Vec<Token> = tokens.to_vec();
    if words.first() == Some(&word("if")) {
        words.remove(0);
    }
    words.retain(|token| *token != Token::Comma);
    let ask: Vec<Option<Token>> = if asked_is_percent {
        vec![Some(word("what")), Some(word("is")), None, Some(percent())]
    } else {
        vec![Some(word("what")), Some(word("is")), None]
    };
    let mut first: Vec<Option<Token>> = vec![None, Some(word("is")), None, Some(percent())];
    first.extend(ask.clone());
    if let Some(parts) = operands_of(&words, &first) {
        if parts.len() >= 3 {
            return Some((parts[0].clone(), parts[1].clone(), parts[2].clone()));
        }
    }
    let mut second: Vec<Option<Token>> = vec![None, Some(percent()), Some(word("is")), None];
    second.extend(ask);
    if let Some(parts) = operands_of(&words, &second) {
        if parts.len() >= 3 {
            return Some((parts[1].clone(), parts[0].clone(), parts[2].clone()));
        }
    }
    None
}

/// The card of a ratio: `ratio of 3 to 4` is `3 : 4`.
fn parse_ratio(
    tokens: &[Token],
    query: &str,
    catalog: &dyn Catalog,
) -> Result<Option<CalcResult>, UnsupportedArea> {
    if tokens.len() < 5 || tokens[0] != word("ratio") || tokens[1] != word("of") {
        return Ok(None);
    }
    let Some(to_index) = tokens.iter().position(|token| *token == word("to")) else {
        return Ok(None);
    };
    if to_index < 2 || to_index + 1 >= tokens.len() {
        return Ok(None);
    }
    let Some(a) = scalar_of(&tokens[2..to_index], catalog) else {
        return Ok(None);
    };
    let Some(b) = scalar_of(&tokens[to_index + 1..], catalog) else {
        return Ok(None);
    };
    // Integers only, so the reduced pair stays exact.
    if a.round() != a
        || b.round() != b
        || a.abs() > 1e15
        || b.abs() > 1e15
        || (a == 0.0 && b == 0.0)
    {
        return Ok(None);
    }
    let divisor = greatest_common_divisor(a.abs(), b.abs());
    if divisor <= 0.0 {
        return Ok(None);
    }
    let text = format!(
        "{} : {}",
        Formatter::display(a / divisor),
        Formatter::display(b / divisor)
    );
    let mut result = CalcResult::phrase(query, Payload::text(&text), "Ratio");
    result.can_chain = false;
    Ok(Some(result))
}

/// The greatest common divisor of two numbers.
fn greatest_common_divisor(a: f64, b: f64) -> f64 {
    let (mut x, mut y) = (a, b);
    while y > 0.0 {
        let next = x % y;
        x = y;
        y = next;
    }
    x
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

/// The number of a run of tokens.
fn scalar_of(tokens: &[Token], catalog: &dyn Catalog) -> Option<f64> {
    let mut parser = Parser::new(tokens);
    match parser.parse(catalog) {
        Ok(value) => Some(value.effective()),
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests;
