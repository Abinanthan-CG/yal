//! The engine: a query in, a card out.
//!
//! Mirrors upstream `CalcEngine.swift` for the part of the calculator this crate
//! implements: the tokenizer, the expression parser and the number formatting.
//!
//! Upstream's engine is a dispatcher over many features - dates, currency, units,
//! percentages, finance and more. Those live outside this crate, so the engine answers
//! either a card or [`CalcOutcome::Unsupported`], and never a made-up answer.

use crate::catalog::Catalog;
use crate::formatter::Formatter;
use crate::math::Math;
use crate::parser::Parser;
use crate::result::{CalcResult, Payload};
use crate::rounding;
use crate::token::{NumberBase, Operator, Token};
use crate::tokenizer::tokenize;
use crate::units;
use crate::value::{ParseError, UnsupportedArea};

/// Every integer up to 2^53 is exactly representable as a double.
const MAX_EXACT_INTEGER: f64 = 9_007_199_254_740_992.0;

/// The longest query the calculator answers.
const MAX_QUERY_LENGTH: usize = 256;

/// The answer of a query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CalcOutcome {
    /// The card the calculator shows.
    Card(CalcResult),
    /// The query is not a calculation, so upstream answers `nil` and shows no card.
    NoCard,
    /// The query needs an area this crate does not implement.
    Unsupported(UnsupportedArea),
}

impl CalcOutcome {
    /// The card, if there is one.
    pub fn card(&self) -> Option<&CalcResult> {
        match self {
            Self::Card(result) => Some(result),
            _ => None,
        }
    }

    /// The text the card shows, if there is a card.
    pub fn display(&self) -> Option<&str> {
        match self.card()?.payload {
            Payload::Value { ref display, .. } => Some(display),
            Payload::Error { .. } => None,
        }
    }

    /// The text that lands on the pasteboard, if there is a card.
    pub fn copy_text(&self) -> Option<&str> {
        match self.card()?.payload {
            Payload::Value { ref copy_text, .. } => Some(copy_text),
            Payload::Error { .. } => None,
        }
    }
}

/// Evaluates a query, as upstream's `CalcEngine.evaluate`.
///
/// Upstream answers `nil` for a query that is not a calculation, which is
/// [`CalcOutcome::NoCard`] here.
pub fn evaluate(query: &str, catalog: &dyn Catalog) -> CalcOutcome {
    let trimmed = without_question_mark(query.trim());
    if trimmed.is_empty() || trimmed.chars().count() > MAX_QUERY_LENGTH {
        return CalcOutcome::NoCard;
    }
    // The English number format is the canonical one, so the query is read as it stands.
    let Some(tokens) = tokenize(&trimmed, catalog) else {
        return CalcOutcome::NoCard;
    };
    if tokens.is_empty() {
        return CalcOutcome::NoCard;
    }
    match evaluate_tokens(&tokens, &trimmed, catalog) {
        Ok(Some(result)) => CalcOutcome::Card(result),
        Ok(None) => CalcOutcome::NoCard,
        Err(area) => CalcOutcome::Unsupported(area),
    }
}

/// Evaluates a tokenized query, as upstream's `CalcEngine.evaluate` does once the query
/// is tokenized.
fn evaluate_tokens(
    tokens: &[Token],
    query: &str,
    catalog: &dyn Catalog,
) -> Result<Option<CalcResult>, UnsupportedArea> {
    // A trailing operator keeps the last complete prefix on the card while the user
    // still types.
    if let Some(result) = partial_result(tokens, query, catalog)? {
        return Ok(Some(result));
    }

    // A lone literal reads as an app search, so no card - except a radix one ("0xff").
    if tokens.len() == 1 {
        return Ok(lone_literal(tokens, query));
    }

    // "255 to hex": an expression on the left, like a unit conversion.
    if let Some(result) = base_conversion(tokens, query, catalog)? {
        return Ok(Some(result));
    }

    // The unit conversions, before the typed arithmetic: a unit makes the plain
    // parser stop, and the conversions are what a query with a unit is asking for.
    if let Some(result) = units::evaluate(tokens, query, catalog) {
        return Ok(Some(result));
    }

    // Typed arithmetic.
    if let Some(result) = quantity_result(tokens, query, catalog, false)? {
        return Ok(Some(result));
    }

    // Rounding and fraction phrasings.
    if let Some(result) = rounding::evaluate(tokens, query, catalog)? {
        return Ok(Some(result));
    }

    Ok(None)
}

/// The card of a lone literal, as upstream's `CalcEngine.evaluate` shows it.
fn lone_literal(tokens: &[Token], query: &str) -> Option<CalcResult> {
    match &tokens[0] {
        Token::IntLiteral(value, base) if *base != NumberBase::Decimal => {
            let text = value.to_string();
            Some(CalcResult::with_badges(
                query.to_string(),
                base.name(),
                "Decimal",
                Payload::Value {
                    display: Formatter::grouped(&text),
                    copy_text: text,
                },
            ))
        }
        Token::CompactNumber(value) => Some(CalcResult::with_badges(
            query.to_string(),
            "Expression",
            "Result",
            Payload::number(*value),
        )),
        _ => None,
    }
}

/// The card of a trailing operator, as upstream's `CalcEngine.partialResult`.
fn partial_result(
    tokens: &[Token],
    query: &str,
    catalog: &dyn Catalog,
) -> Result<Option<CalcResult>, UnsupportedArea> {
    let Some(trailing) = tokens.last() else {
        return Ok(None);
    };
    let Some(operator_text) = partial_operator_text(trailing) else {
        return Ok(None);
    };
    let prefix = &tokens[..tokens.len() - 1];
    if prefix.is_empty() {
        return Ok(None);
    }
    let prefix_query = token_query(prefix);
    if prefix.len() == 1 {
        if let Some(value) = decimal_literal(&prefix[0]) {
            return Ok(Some(CalcResult::with_badges(
                Formatter::expression(query),
                "Expression",
                "Result",
                Payload::number(value),
            )));
        }
    }
    if let Some(result) = quantity_result(prefix, &prefix_query, catalog, true)? {
        return Ok(Some(result.with_expression(format!(
            "{} {operator_text}",
            result.expression
        ))));
    }
    // A conversion's echo drops its target, so echo the typed text; the badges name both.
    if let Some(result) = evaluate_tokens(prefix, &prefix_query, catalog)? {
        return Ok(Some(result.with_expression(Formatter::expression(query))));
    }
    let mut parser = Parser::new(prefix);
    match parser.parse(catalog) {
        Ok(value) if !value.is_boolean => Ok(Some(CalcResult::with_badges(
            Formatter::expression(query),
            "Expression",
            "Result",
            Payload::number(value.effective()),
        ))),
        Ok(_) => Ok(None),
        Err(ParseError::NotACalculation) => Ok(None),
        Err(ParseError::Unsupported(area)) => Err(area),
    }
}

/// The text a trailing operator is echoed with, as upstream's
/// `CalcEngine.partialOperatorText`.
fn partial_operator_text(token: &Token) -> Option<&'static str> {
    match token {
        Token::Op(Operator::Multiply) => Some("×"),
        Token::Op(Operator::Divide) => Some("÷"),
        Token::Op(Operator::Add) => Some("+"),
        Token::Op(Operator::Subtract) => Some("-"),
        Token::Op(Operator::Power) => Some("^"),
        _ => None,
    }
}

/// The card of a calculation, as upstream's `CalcQuantity.evaluate` shows it for a
/// scalar or a boolean.
///
/// `preserve_standalone` is upstream's `preserveStandaloneUnit`: a lone value that is
/// not a calculation only earns a card when it is asked for.
fn quantity_result(
    tokens: &[Token],
    query: &str,
    catalog: &dyn Catalog,
    preserve_standalone: bool,
) -> Result<Option<CalcResult>, UnsupportedArea> {
    let mut parser = Parser::new(tokens);
    let value = match parser.parse(catalog) {
        Ok(value) => value,
        Err(ParseError::NotACalculation) => return Ok(None),
        Err(ParseError::Unsupported(area)) => return Err(area),
    };
    if !value.is_boolean {
        if parser.operation_count() == 0 && (preserve_standalone || tokens.len() <= 1) {
            return Ok(None);
        }
        return Ok(Some(CalcResult::with_badges(
            Formatter::expression(query),
            "Expression",
            "Result",
            Payload::number(value.effective()),
        )));
    }
    let text = if value.amount == 0.0 { "false" } else { "true" };
    Ok(Some(CalcResult {
        expression: expression_text(tokens, catalog),
        source_badge: Some("Expression".to_string()),
        target_badge: Some("Boolean".to_string()),
        payload: Payload::Value {
            display: text.to_string(),
            copy_text: text.to_string(),
        },
        can_chain: false,
        is_currency: false,
    }))
}

/// The card of a base conversion, as upstream's `CalcEngine.baseConversion`.
fn base_conversion(
    tokens: &[Token],
    query: &str,
    catalog: &dyn Catalog,
) -> Result<Option<CalcResult>, UnsupportedArea> {
    let Some((start, radix, prefix, name)) = radix_target(tokens) else {
        return Ok(None);
    };
    let value_tokens = &tokens[..start];
    let literal_text = query.split_whitespace().next().unwrap_or(query).to_string();

    let (source, source_badge, source_text) = match value_tokens {
        [Token::IntLiteral(value, base)] => (*value, base.name().to_string(), literal_text.clone()),
        [token] => match decimal_literal(token) {
            Some(value) if is_source_value(value) => {
                (value as u64, "Decimal".to_string(), literal_text.clone())
            }
            _ => {
                let Some((value, text)) = source_expression(value_tokens, catalog)? else {
                    return Ok(None);
                };
                (value, "Decimal".to_string(), text)
            }
        },
        _ => {
            let Some((value, text)) = source_expression(value_tokens, catalog)? else {
                return Ok(None);
            };
            (value, "Decimal".to_string(), text)
        }
    };

    let output = if radix == 10 {
        Formatter::grouped(&source.to_string())
    } else {
        format!("{prefix}{}", to_radix(source, radix, true))
    };
    Ok(Some(CalcResult::with_badges(
        source_text,
        &source_badge,
        &name,
        Payload::Value {
            display: output.clone(),
            copy_text: output.replace(',', ""),
        },
    )))
}

/// The value and the echo of an expression that is converted to another base.
///
/// Upstream answers `nil` when the expression is not a whole number.
fn source_expression(
    tokens: &[Token],
    catalog: &dyn Catalog,
) -> Result<Option<(u64, String)>, UnsupportedArea> {
    let mut parser = Parser::new(tokens);
    let value = match parser.parse(catalog) {
        Ok(value) if !value.is_boolean => value.effective(),
        Ok(_) => return Ok(None),
        Err(ParseError::NotACalculation) => return Ok(None),
        Err(ParseError::Unsupported(area)) => return Err(area),
    };
    if !is_source_value(value) {
        return Ok(None);
    }
    let source = value as u64;
    Ok(Some((source, Formatter::grouped(&source.to_string()))))
}

/// Whether a value can be shown in another base, as upstream's `CalcEngine.baseConversion`.
fn is_source_value(value: f64) -> bool {
    value >= 0.0 && value.round() == value && value <= MAX_EXACT_INTEGER
}

/// The base a query converts to, as upstream's `CalcEngine.radixTarget`.
fn radix_target(tokens: &[Token]) -> Option<(usize, u32, &'static str, String)> {
    let count = tokens.len();
    if count >= 3 && is_connector(&tokens[count - 2]) {
        if let Token::Ident(name) = &tokens[count - 1] {
            if let Some(base) = NumberBase::from_name(name) {
                return Some((
                    count - 2,
                    base.radix(),
                    base.prefix(),
                    base.name().to_string(),
                ));
            }
        }
    }
    if count >= 4
        && is_connector(&tokens[count - 3])
        && tokens[count - 2] == Token::Ident("base".to_string())
    {
        if let Some(radix) = tokens[count - 1].number_value().and_then(exact_integer) {
            if (2..=36).contains(&radix) {
                let radix = radix as u32;
                return Some(match NumberBase::from_radix(radix) {
                    Some(base) => (count - 3, radix, base.prefix(), base.name().to_string()),
                    None => (count - 3, radix, "", format!("Base {radix}")),
                });
            }
        }
    }
    None
}

/// Whether a token connects a value to a target, as upstream's `CalcUnits.isConnector`.
fn is_connector(token: &Token) -> bool {
    match token {
        Token::Arrow => true,
        Token::Ident(name) => matches!(name.as_str(), "to" | "in" | "as"),
        _ => false,
    }
}

/// A number a token carries, as upstream's `CalcEngine.decimalLiteral`.
fn decimal_literal(token: &Token) -> Option<f64> {
    match token {
        Token::Number(value) | Token::CompactNumber(value) => Some(*value),
        _ => None,
    }
}

/// A double that is an exact integer.
fn exact_integer(value: f64) -> Option<i64> {
    if value.round() != value || value.abs() >= 9.2e18 {
        return None;
    }
    Some(value as i64)
}

/// The query the tokens stand for, as upstream's `CalcEngine.tokenQuery`.
fn token_query(tokens: &[Token]) -> String {
    let mut parts: Vec<String> = Vec::with_capacity(tokens.len());
    for token in tokens {
        parts.push(match token {
            Token::Number(value) | Token::CompactNumber(value) => number_text(*value),
            Token::IntLiteral(value, base) => {
                format!("{}{}", base.prefix(), to_radix(*value, base.radix(), false))
            }
            Token::Ident(name) => name.clone(),
            Token::Op(op) => op.text(),
            Token::Arrow => "->".to_string(),
            Token::Comma => ",".to_string(),
        });
    }
    parts.join(" ")
}

/// The number as upstream's `String` spells it.
fn number_text(value: f64) -> String {
    if value.round() == value && value.abs() < 9.2e18 {
        return (value as i64).to_string();
    }
    Formatter::number_text(value)
}

/// A number written in another base.
fn to_radix(value: u64, radix: u32, uppercase: bool) -> String {
    const LOWER: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    const UPPER: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let digits = if uppercase { UPPER } else { LOWER };
    if value == 0 {
        return "0".to_string();
    }
    let mut output: Vec<u8> = Vec::new();
    let mut remaining = value;
    while remaining > 0 {
        output.push(digits[(remaining % u64::from(radix)) as usize]);
        remaining /= u64::from(radix);
    }
    output.reverse();
    String::from_utf8(output).unwrap_or_default()
}

/// The normalized echo of a calculation, as upstream's `CalcQuantity.expressionText`.
fn expression_text(tokens: &[Token], catalog: &dyn Catalog) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut attach_next = true;
    for (index, token) in tokens.iter().enumerate() {
        match token {
            Token::Number(value) | Token::CompactNumber(value) => add(
                &mut parts,
                &mut attach_next,
                &Formatter::copy_text(*value),
                false,
            ),
            Token::IntLiteral(value, _) => {
                add(&mut parts, &mut attach_next, &value.to_string(), false)
            }
            Token::Ident(name) => {
                let symbol = catalog
                    .unit(name)
                    .map(|unit| unit.symbol)
                    .or_else(|| catalog.currency(name).map(|currency| currency.code))
                    .unwrap_or_else(|| name.clone());
                add(&mut parts, &mut attach_next, &symbol, false);
                attach_next = index + 1 < tokens.len()
                    && tokens[index + 1] == Token::Op(Operator::Open)
                    && Math::is_function(name);
            }
            Token::Op(Operator::Open) => {
                add(&mut parts, &mut attach_next, "(", false);
                attach_next = true;
            }
            Token::Op(Operator::Close) => add(&mut parts, &mut attach_next, ")", true),
            Token::Op(Operator::Percent) => add(&mut parts, &mut attach_next, "%", true),
            Token::Op(Operator::Factorial) => add(&mut parts, &mut attach_next, "!", true),
            Token::Op(Operator::Multiply) => add(&mut parts, &mut attach_next, "×", false),
            Token::Op(Operator::Divide) => add(&mut parts, &mut attach_next, "÷", false),
            Token::Op(op) => {
                add(&mut parts, &mut attach_next, &op.text(), false);
                if *op == Operator::Subtract || *op == Operator::Add {
                    attach_next = is_sign(index, tokens, catalog);
                }
            }
            Token::Arrow => add(&mut parts, &mut attach_next, "→", false),
            Token::Comma => add(&mut parts, &mut attach_next, ",", true),
        }
    }
    parts.join(" ")
}

/// Adds a piece to the echo, as upstream's `CalcQuantity.expressionText` does.
fn add(parts: &mut Vec<String>, attach_next: &mut bool, piece: &str, attached: bool) {
    if (*attach_next || attached) && !parts.is_empty() {
        if let Some(last) = parts.last_mut() {
            last.push_str(piece);
        }
    } else {
        parts.push(piece.to_string());
    }
    *attach_next = false;
}

/// Whether a `+` or `-` negates the operand that follows rather than joining two of
/// them, as upstream's `CalcQuantity.isSign`.
fn is_sign(index: usize, tokens: &[Token], catalog: &dyn Catalog) -> bool {
    if index == 0 {
        return true;
    }
    match &tokens[index - 1] {
        Token::Op(previous) => {
            *previous != Operator::Close
                && *previous != Operator::Percent
                && *previous != Operator::Factorial
        }
        // A word operator (`of`, `mod`, `sqrt`) introduces an operand, so the sign
        // belongs to it.
        Token::Ident(name) => {
            catalog.unit(name).is_none()
                && catalog.currency(name).is_none()
                && Math::constant(name).is_none()
        }
        _ => false,
    }
}

/// The query without its question mark, as upstream's `CalcEngine.withoutQuestionMark`.
fn without_question_mark(text: &str) -> String {
    match text.strip_suffix('?') {
        Some(stripped) => stripped.trim().to_string(),
        None => text.to_string(),
    }
}
