//! A shared amount: `85 + 20% split 3`, `split $120 between 4`, `120 / 4 people`.
//!
//! Mirrors upstream `CalcSplit.swift`.
//!
//! The amount is shared out before it is rounded and before any `to`, so
//! `$1 / 6 split 2` is the same as `$1 / 6 / 2`. The card says who the share is
//! for: `Per Person`.

use crate::catalog::Catalog;
use crate::formatter::Formatter;
use crate::percent;
use crate::result::{CalcResult, Payload};
use crate::token::{Operator, Token};
use crate::value::{UnsupportedArea, Value};

/// The words that join an amount to its headcount.
const JOINERS: [&str; 5] = ["between", "among", "by", "into", "in"];

/// The nouns that name the people an amount is shared between.
const HEADCOUNTS: [&str; 8] = [
    "people", "persons", "person", "ways", "way", "guests", "friends", "diners",
];

/// The longest query the splitter looks at.
const MAX_QUERY_LENGTH: usize = 128;

/// The largest headcount it shares between.
const MAX_HEADCOUNT: u32 = 10_000;

/// A shared amount: the amount and the headcount it is shared between.
struct Phrase {
    /// The amount, as the user wrote it.
    amount: String,
    /// How many share it.
    count: u32,
}

/// Whether the tokens mention a split at all.
///
/// A query without these words pays nothing more than a scan of its tokens.
pub fn mentions(tokens: &[Token]) -> bool {
    tokens.iter().any(|token| match token {
        Token::Ident(name) => {
            name == "split"
                || name == "between"
                || name == "among"
                || HEADCOUNTS.contains(&name.as_str())
        }
        _ => false,
    })
}

/// The card of a shared amount.
pub fn evaluate(query: &str, catalog: &dyn Catalog) -> Result<Option<CalcResult>, UnsupportedArea> {
    let Some(phrase) = parse(query) else {
        return Ok(None);
    };
    let Some(tokens) = crate::tokenizer::tokenize(&phrase.amount, catalog) else {
        return Ok(None);
    };
    if tokens.is_empty() {
        return Ok(None);
    }
    let count = f64::from(phrase.count);
    // A tip, a discount or a markup is shared out as it is.
    if let Some(amount) = percent::amount(&tokens, catalog)? {
        return Ok(Some(share_out(query, amount, count)));
    }
    // The rest is divided before it is rounded: `(85 + 20%) / 3`.
    let divided = divide(&tokens, count);
    let Some(each) = crate::engine::quantity_result(&divided, &phrase.amount, catalog, true)?
    else {
        return Ok(None);
    };
    if !each.can_chain {
        return Ok(None);
    }
    let mut result = each;
    result.expression = Formatter::collapsed(query);
    result.target_badge = Some("Per Person".to_string());
    Ok(Some(result))
}

/// The card of an amount shared out.
fn share_out(query: &str, amount: Value, count: f64) -> CalcResult {
    let mut value = amount;
    value.amount = amount.amount / count;
    let payload = match value.currency {
        Some(currency) => Payload::money(value.amount, currency.code),
        None => Payload::number(value.amount),
    };
    CalcResult::phrase(query, payload, "Per Person")
}

/// The tokens of an amount with the headcount divided into it.
fn divide(tokens: &[Token], count: f64) -> Vec<Token> {
    let mut divided = vec![Token::Op(Operator::Open)];
    divided.extend(tokens.to_vec());
    divided.extend([
        Token::Op(Operator::Close),
        Token::Op(Operator::Divide),
        Token::Number(count),
    ]);
    divided
}

/// The shared amount a query asks for.
fn parse(query: &str) -> Option<Phrase> {
    if query.chars().count() > MAX_QUERY_LENGTH {
        return None;
    }
    let words: Vec<&str> = query.split_whitespace().collect();
    if words.len() < 3 {
        return None;
    }
    let lower: Vec<String> = words.iter().map(|word| word.to_lowercase()).collect();

    // `split 120 between 4`.
    if lower[0] == "split" {
        if let Some(joiner) = lower
            .iter()
            .enumerate()
            .skip(2)
            .find(|(_, word)| JOINERS.contains(&word.as_str()))
            .map(|(index, _)| index)
        {
            return phrase(&words[1..joiner], &lower[joiner + 1..]);
        }
    }
    // `120 split 4`, `120 between 4`, `120 among 4`.
    if let Some(marker) = lower
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, word)| ["split", "between", "among"].contains(&word.as_str()))
        .map(|(index, _)| index)
    {
        let mut tail: Vec<String> = lower[marker + 1..].to_vec();
        if let Some(first) = tail.first() {
            if JOINERS.contains(&first.as_str()) {
                tail.remove(0);
            }
        }
        return phrase(&words[..marker], &tail);
    }
    // `120 / 4 people`: the divide is already there, so only the trailing noun is dropped.
    let last = lower.len() - 1;
    if HEADCOUNTS.contains(&lower[last].as_str())
        && lower[last - 1].parse::<u32>().is_ok()
        && (lower[last - 2] == "/" || lower[last - 2] == "÷")
    {
        return phrase(&words[..last - 2], &[lower[last - 1].clone()]);
    }
    None
}

/// The phrase an amount and a headcount make.
fn phrase(amount: &[&str], tail: &[String]) -> Option<Phrase> {
    let mut tail = tail.to_vec();
    if let Some(last) = tail.last() {
        if HEADCOUNTS.contains(&last.as_str()) {
            tail.pop();
        }
    }
    if amount.is_empty() || tail.len() != 1 {
        return None;
    }
    let count: u32 = tail[0].parse().ok()?;
    if !(1..=MAX_HEADCOUNT).contains(&count) {
        return None;
    }
    Some(Phrase {
        amount: amount.join(" "),
        count,
    })
}
