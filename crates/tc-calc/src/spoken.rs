//! Math asked in words: `is 97 prime`, `5 choose 3`, `6 is to 600 as 8 is to what`.
//!
//! Mirrors upstream `CalcSpokenMath.swift`.
//!
//! The phrases are read in upstream's order - the remainder, the primality,
//! the clamp, the root, the proportion, the combinations and permutations, the
//! logarithm and the multiplier - and each one is a shape the query has to
//! have.

use crate::catalog::Catalog;
use crate::math::Math;
use crate::parser::Parser;
use crate::result::{CalcResult, Payload};
use crate::token::{operands_of, Operator, Token};
use crate::value::{ParseError, UnsupportedArea, Value};

/// The words that mean a phrase of this kind is being asked for.
const KEYWORDS: [&str; 14] = [
    "remainder",
    "prime",
    "clamp",
    "root",
    "to",
    "choose",
    "combination",
    "combinations",
    "permutation",
    "permutations",
    "logarithm",
    "log",
    "what",
    "multiplier",
];

/// The card of a phrase of math asked in words.
pub fn evaluate(
    tokens: &[Token],
    query: &str,
    catalog: &dyn Catalog,
) -> Result<Option<CalcResult>, UnsupportedArea> {
    if !tokens.iter().any(|token| match token {
        Token::Ident(name) => KEYWORDS.contains(&name.as_str()),
        _ => false,
    }) {
        return Ok(None);
    }
    if let Some(result) = remainder(tokens, query, catalog)? {
        return Ok(Some(result));
    }
    if let Some(result) = prime(tokens, query, catalog)? {
        return Ok(Some(result));
    }
    if let Some(result) = clamp(tokens, query, catalog)? {
        return Ok(Some(result));
    }
    if let Some(result) = root(tokens, query, catalog)? {
        return Ok(Some(result));
    }
    if let Some(result) = proportion(tokens, query, catalog)? {
        return Ok(Some(result));
    }
    if let Some(result) = combinatorics(tokens, query, catalog)? {
        return Ok(Some(result));
    }
    if let Some(result) = logarithm(tokens, query, catalog)? {
        return Ok(Some(result));
    }
    multiplier(tokens, query, catalog)
}

/// The remainder of a division: `remainder of 20 divided by 3`.
fn remainder(
    tokens: &[Token],
    query: &str,
    catalog: &dyn Catalog,
) -> Result<Option<CalcResult>, UnsupportedArea> {
    let Some(parts) = operands_of(
        tokens,
        &[
            Some(word("remainder")),
            Some(word("of")),
            None,
            Some(Token::Op(Operator::Divide)),
            None,
        ],
    ) else {
        return Ok(None);
    };
    let Some(dividend) = scalar_of(&parts[0], catalog) else {
        return Ok(None);
    };
    let Some(divisor) = scalar_of(&parts[1], catalog) else {
        return Ok(None);
    };
    if divisor == 0.0 {
        return Ok(None);
    }
    let result = dividend % divisor;
    if !result.is_finite() {
        return Ok(None);
    }
    Ok(Some(CalcResult::phrase(
        query,
        Payload::number(result),
        "Remainder",
    )))
}

/// Whether a whole number is prime: `is 97 prime`.
fn prime(
    tokens: &[Token],
    query: &str,
    catalog: &dyn Catalog,
) -> Result<Option<CalcResult>, UnsupportedArea> {
    let patterns = [
        vec![
            Some(word("is")),
            None,
            Some(word("a")),
            Some(word("prime")),
            Some(word("number")),
        ],
        vec![Some(word("is")), None, Some(word("a")), Some(word("prime"))],
        vec![Some(word("is")), None, Some(word("prime"))],
        vec![Some(word("is")), Some(word("prime")), None],
    ];
    let Some(parts) = first_match(tokens, &patterns) else {
        return Ok(None);
    };
    let Some(value) = scalar_of(&parts[0], catalog) else {
        return Ok(None);
    };
    if value < 0.0 || value.round() != value || value > 9_007_199_254_740_992.0 {
        return Ok(None);
    }
    let text = if is_prime(value as u64) {
        "true"
    } else {
        "false"
    };
    let mut result = CalcResult::phrase(query, Payload::text(text), "Prime Number");
    result.can_chain = false;
    Ok(Some(result))
}

/// Whether a whole number is prime, as upstream's `CalcSpokenMath.isPrime`.
///
/// Miller-Rabin with the bases that are deterministic for every 64-bit
/// whole number.
fn is_prime(number: u64) -> bool {
    const BASES: [u64; 12] = [2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37];
    if number < 2 {
        return false;
    }
    for base in BASES {
        if number % base == 0 {
            return number == base;
        }
    }
    let mut odd = number - 1;
    let mut twos = 0u32;
    while odd % 2 == 0 {
        odd /= 2;
        twos += 1;
    }
    'bases: for base in BASES {
        let mut x = power(base, odd, number);
        if x == 1 || x == number - 1 {
            continue;
        }
        for _ in 1..twos {
            x = multiply(x, x, number);
            if x == number - 1 {
                continue 'bases;
            }
        }
        return false;
    }
    true
}

/// A number times another, modulo a third, without overflowing.
fn multiply(left: u64, right: u64, modulo: u64) -> u64 {
    ((left as u128 * right as u128) % modulo as u128) as u64
}

/// A number to a power, modulo a third.
fn power(base: u64, exponent: u64, modulo: u64) -> u64 {
    let mut result = 1u64;
    let mut base = base % modulo;
    let mut exponent = exponent;
    while exponent > 0 {
        if exponent & 1 == 1 {
            result = multiply(result, base, modulo);
        }
        base = multiply(base, base, modulo);
        exponent >>= 1;
    }
    result
}

/// A value held between two bounds: `clamp 20 between 5 and 15`.
fn clamp(
    tokens: &[Token],
    query: &str,
    catalog: &dyn Catalog,
) -> Result<Option<CalcResult>, UnsupportedArea> {
    if tokens.first() != Some(&word("clamp")) {
        return Ok(None);
    }
    let patterns = [
        vec![
            Some(word("clamp")),
            None,
            Some(word("between")),
            None,
            Some(word("and")),
            None,
        ],
        vec![
            Some(word("clamp")),
            None,
            Some(word("from")),
            None,
            Some(word("to")),
            None,
        ],
    ];
    let parts = if let Some(matched) = first_match(tokens, &patterns) {
        matched
    } else if tokens.len() == 4 && tokens[1..].iter().all(number_value) {
        tokens[1..]
            .iter()
            .map(|token| vec![token.clone()])
            .collect()
    } else {
        return Ok(None);
    };
    let Some(span) = value_of(&combined(&parts[2], Operator::Subtract, &parts[1]), catalog)? else {
        return Ok(None);
    };
    if span.amount < 0.0 {
        return Ok(None);
    }
    let mut call = vec![word("max"), Token::Op(Operator::Open)];
    call.extend(parts[1].to_vec());
    call.push(Token::Comma);
    call.push(word("min"));
    call.push(Token::Op(Operator::Open));
    call.extend(parts[0].to_vec());
    call.push(Token::Comma);
    call.extend(parts[2].to_vec());
    call.push(Token::Op(Operator::Close));
    call.push(Token::Op(Operator::Close));
    let Some(clamped) = value_of(&call, catalog)? else {
        return Ok(None);
    };
    Ok(Some(CalcResult::phrase(
        query,
        amount_payload(clamped),
        "Clamped",
    )))
}

/// A root: `5 root of 32` is the fifth root.
fn root(
    tokens: &[Token],
    query: &str,
    catalog: &dyn Catalog,
) -> Result<Option<CalcResult>, UnsupportedArea> {
    let Some(parts) = operands_of(tokens, &[None, Some(word("root")), Some(word("of")), None])
    else {
        return Ok(None);
    };
    let Some(degree) = scalar_of(&parts[0], catalog) else {
        return Ok(None);
    };
    if degree < 2.0 || degree.round() != degree {
        return Ok(None);
    }
    let Some(value) = scalar_of(&parts[1], catalog) else {
        return Ok(None);
    };
    let Some(result) = Math::evaluate("root", &[value, degree]) else {
        return Ok(None);
    };
    Ok(Some(CalcResult::phrase(
        query,
        Payload::number(result),
        "Root",
    )))
}

/// A proportion, solved for the missing term: `6 is to 600 as 8 is to what`.
fn proportion(
    tokens: &[Token],
    query: &str,
    catalog: &dyn Catalog,
) -> Result<Option<CalcResult>, UnsupportedArea> {
    let missing_third = [
        vec![
            None,
            Some(word("is")),
            Some(word("to")),
            None,
            Some(word("as")),
            Some(word("what")),
            Some(word("is")),
            Some(word("to")),
            None,
        ],
        vec![
            None,
            Some(word("to")),
            None,
            Some(word("as")),
            Some(word("what")),
            Some(word("to")),
            None,
        ],
        vec![
            None,
            Some(word("to")),
            None,
            Some(word("is")),
            Some(word("what")),
            Some(word("to")),
            None,
        ],
    ];
    let missing_fourth = [
        vec![
            None,
            Some(word("is")),
            Some(word("to")),
            None,
            Some(word("as")),
            None,
            Some(word("is")),
            Some(word("to")),
            Some(word("what")),
        ],
        vec![
            None,
            Some(word("to")),
            None,
            Some(word("as")),
            None,
            Some(word("to")),
            Some(word("what")),
        ],
        vec![
            None,
            Some(word("to")),
            None,
            Some(word("is")),
            None,
            Some(word("to")),
            Some(word("what")),
        ],
    ];
    let product = if let Some(parts) = first_match(tokens, &missing_third) {
        combined(
            &combined(&parts[0], Operator::Multiply, &parts[2]),
            Operator::Divide,
            &parts[1],
        )
    } else if let Some(parts) = first_match(tokens, &missing_fourth) {
        combined(
            &combined(&parts[1], Operator::Multiply, &parts[2]),
            Operator::Divide,
            &parts[0],
        )
    } else {
        return Ok(None);
    };
    let Some(value) = value_of(&product, catalog)? else {
        return Ok(None);
    };
    if !value.amount.is_finite() {
        return Ok(None);
    }
    Ok(Some(CalcResult::phrase(
        query,
        amount_payload(value),
        "Proportion",
    )))
}

/// The combinations and permutations: `5 choose 3`.
fn combinatorics(
    tokens: &[Token],
    query: &str,
    catalog: &dyn Catalog,
) -> Result<Option<CalcResult>, UnsupportedArea> {
    let shapes: [(Vec<Option<Token>>, bool, bool); 7] = [
        (
            vec![None, Some(word("combinations")), Some(word("of")), None],
            true,
            true,
        ),
        (
            vec![None, Some(word("combination")), Some(word("of")), None],
            true,
            true,
        ),
        (
            vec![None, Some(word("permutations")), Some(word("of")), None],
            false,
            true,
        ),
        (
            vec![None, Some(word("permutation")), Some(word("of")), None],
            false,
            true,
        ),
        (vec![None, Some(word("choose")), None], true, false),
        (vec![None, Some(word("combination")), None], true, false),
        (vec![None, Some(word("permutation")), None], false, false),
    ];
    for (pattern, combination, count_first) in &shapes {
        let Some(parts) = operands_of(tokens, pattern) else {
            continue;
        };
        let (n_tokens, k_tokens) = if *count_first {
            (&parts[1], &parts[0])
        } else {
            (&parts[0], &parts[1])
        };
        let Some(n) = scalar_of(n_tokens, catalog) else {
            return Ok(None);
        };
        let Some(k) = scalar_of(k_tokens, catalog) else {
            return Ok(None);
        };
        let Some(count) = arrangements(n, k, *combination) else {
            return Ok(None);
        };
        let badge = if *combination {
            "Combinations"
        } else {
            "Permutations"
        };
        return Ok(Some(CalcResult::phrase(
            query,
            Payload::number(count),
            badge,
        )));
    }
    Ok(None)
}

/// `nCk` or `nPk` by running the product, so neither ever forms a factorial
/// larger than its answer.
fn arrangements(n: f64, k: f64, combination: bool) -> Option<f64> {
    if n.round() != n || k.round() != k || k < 0.0 || n < k || n > 1e15 {
        return None;
    }
    let steps = if combination { k.min(n - k) } else { k };
    let mut result = 1.0;
    let mut step = 0.0;
    while step < steps {
        result *= n - step;
        if combination {
            result /= step + 1.0;
        }
        if !result.is_finite() {
            return None;
        }
        step += 1.0;
    }
    Some(if combination { result.round() } else { result })
}

/// A logarithm: `logarithm 81 base 3`.
fn logarithm(
    tokens: &[Token],
    query: &str,
    catalog: &dyn Catalog,
) -> Result<Option<CalcResult>, UnsupportedArea> {
    let value_first = [
        vec![Some(word("logarithm")), None, Some(word("base")), None],
        vec![
            Some(word("logarithm")),
            Some(word("of")),
            None,
            Some(word("to")),
            Some(word("base")),
            None,
        ],
        vec![
            Some(word("logarithm")),
            Some(word("of")),
            None,
            Some(word("with")),
            Some(word("base")),
            None,
        ],
        vec![
            None,
            Some(word("is")),
            None,
            Some(Token::Op(Operator::Power)),
            Some(word("what")),
        ],
        vec![
            None,
            Some(word("is")),
            None,
            Some(word("to")),
            Some(word("what")),
            Some(word("power")),
        ],
    ];
    let base_first = [
        vec![
            Some(word("logarithm")),
            Some(word("base")),
            None,
            Some(word("of")),
            None,
        ],
        vec![
            Some(word("log")),
            Some(word("base")),
            None,
            Some(word("of")),
            None,
        ],
        vec![Some(word("logarithm")), None, Some(word("of")), None],
        vec![
            None,
            Some(word("to")),
            Some(word("what")),
            Some(word("power")),
            Some(word("is")),
            None,
        ],
        vec![
            None,
            Some(word("to")),
            Some(word("what")),
            Some(Token::Op(Operator::Power)),
            Some(word("is")),
            None,
        ],
        vec![
            None,
            Some(word("with")),
            Some(word("what")),
            Some(word("power")),
            Some(word("is")),
            None,
        ],
        vec![
            None,
            Some(word("with")),
            Some(word("what")),
            Some(Token::Op(Operator::Power)),
            Some(word("is")),
            None,
        ],
    ];
    let mut candidates: Vec<(Vec<Token>, Vec<Token>)> = Vec::new();
    for pattern in &value_first {
        if let Some(parts) = operands_of(tokens, pattern) {
            candidates.push((parts[0].clone(), parts[1].clone()));
        }
    }
    for pattern in &base_first {
        if let Some(parts) = operands_of(tokens, pattern) {
            candidates.push((parts[1].clone(), parts[0].clone()));
        }
    }
    for (value_tokens, base_tokens) in candidates {
        let Some(value) = scalar_of(&value_tokens, catalog) else {
            continue;
        };
        let Some(base) = scalar_of(&base_tokens, catalog) else {
            continue;
        };
        let Some(result) = Math::evaluate("log", &[value, base]) else {
            continue;
        };
        return Ok(Some(CalcResult::phrase(
            query,
            Payload::number(result),
            "Logarithm",
        )));
    }
    Ok(None)
}

/// A relation and what it does to a ratio.
type Relation = (&'static str, fn(f64) -> f64);

/// A multiplier: `5 as multiplier of 1` is `5x`.
fn multiplier(
    tokens: &[Token],
    query: &str,
    catalog: &dyn Catalog,
) -> Result<Option<CalcResult>, UnsupportedArea> {
    if !tokens.contains(&word("multiplier")) {
        return Ok(None);
    }
    let leads: [Vec<Token>; 3] = [
        vec![word("as"), word("multiplier")],
        vec![word("as"), word("a"), word("multiplier")],
        vec![word("is"), word("what"), word("multiplier")],
    ];
    let relations: [Relation; 3] = [
        ("of", |ratio| ratio),
        ("on", |ratio| ratio - 1.0),
        ("off", |ratio| 1.0 - ratio),
    ];
    for lead in &leads {
        for (relation, adjust) in relations {
            let mut pattern: Vec<Option<Token>> = vec![None];
            pattern.extend(lead.iter().cloned().map(Some));
            pattern.push(Some(word(relation)));
            pattern.push(None);
            let Some(parts) = operands_of(tokens, &pattern) else {
                continue;
            };
            let Some(ratio) = scalar_of(&combined(&parts[0], Operator::Divide, &parts[1]), catalog)
            else {
                continue;
            };
            return Ok(multiplier_card(adjust(ratio), query));
        }
        let mut pattern: Vec<Option<Token>> = vec![None, Some(word("to")), None];
        pattern.extend(lead.iter().cloned().map(Some));
        let Some(parts) = operands_of(tokens, &pattern) else {
            continue;
        };
        let Some(ratio) = scalar_of(&combined(&parts[1], Operator::Divide, &parts[0]), catalog)
        else {
            continue;
        };
        return Ok(multiplier_card(ratio, query));
    }
    Ok(None)
}

/// The card of a multiplier.
fn multiplier_card(value: f64, query: &str) -> Option<CalcResult> {
    if !value.is_finite() {
        return None;
    }
    let mut result = CalcResult::phrase(query, Payload::suffixed(value, "x"), "Multiplier");
    result.can_chain = false;
    Some(result)
}

/// The first pattern the query matches.
fn first_match(tokens: &[Token], patterns: &[Vec<Option<Token>>]) -> Option<Vec<Vec<Token>>> {
    for pattern in patterns {
        if let Some(parts) = operands_of(tokens, pattern) {
            return Some(parts);
        }
    }
    None
}

/// Two amounts and the operation between them, through the typed parser.
fn combined(left: &[Token], operator: Operator, right: &[Token]) -> Vec<Token> {
    let mut combined = vec![Token::Op(Operator::Open)];
    combined.extend(left.to_vec());
    combined.push(Token::Op(Operator::Close));
    combined.push(Token::Op(operator));
    combined.push(Token::Op(Operator::Open));
    combined.extend(right.to_vec());
    combined.push(Token::Op(Operator::Close));
    combined
}

/// Whether a token is a plain number.
fn number_value(token: &Token) -> bool {
    matches!(token, Token::Number(_) | Token::CompactNumber(_))
}

/// The payload of an amount, keeping the money it carries.
fn amount_payload(amount: Value) -> Payload {
    match amount.currency {
        Some(currency) => Payload::money(amount.amount, currency.code),
        None => Payload::number(amount.amount),
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

/// The number of a run of tokens.
fn scalar_of(tokens: &[Token], catalog: &dyn Catalog) -> Option<f64> {
    let mut parser = Parser::new(tokens);
    match parser.parse(catalog) {
        Ok(value) if value.is_scalar() => Some(value.effective()),
        _ => None,
    }
}

/// A word token.
fn word(text: &str) -> Token {
    Token::Ident(text.to_string())
}
