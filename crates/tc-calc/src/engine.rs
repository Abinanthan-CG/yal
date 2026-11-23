//! The engine: a query in, a card out.
//!
//! Mirrors upstream `CalcEngine.swift` for the part of the calculator this crate
//! implements: the tokenizer, the expression parser and the number formatting.
//!
//! Upstream's engine is a dispatcher over many features - dates, currency, units,
//! percentages, finance and more. Those live outside this crate, so the engine answers
//! either a card or [`CalcOutcome::Unsupported`], and never a made-up answer.

use crate::catalog::Catalog;
use crate::color;
use crate::context::CalcContext;
use crate::datetime;
use crate::finance;
use crate::formatter::Formatter;
use crate::fx;
use crate::growth;
use crate::math::Math;
use crate::measure::Measurement;
use crate::parser::Parser;
use crate::percent;
use crate::result::{CalcResult, Payload};
use crate::rounding;
use crate::specialty;
use crate::split;
use crate::spoken;
use crate::statistics;
use crate::timezone;
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

/// Evaluates a query using the default clock/zone and no exchange rates
/// (preserving strict backward compatibility with rate-free callers).
pub fn evaluate(query: &str, catalog: &dyn Catalog) -> CalcOutcome {
    let ctx = CalcContext::live().with_rates(None);
    evaluate_with_context(query, catalog, &ctx)
}

/// Evaluates a query across the full Swiss-Army-Knife calculator pipeline using
/// an injected [`CalcContext`] (clock, local time zone, live/bundled exchange
/// rates, and CSS `rem` pixel base).
pub fn evaluate_with_context(query: &str, catalog: &dyn Catalog, ctx: &CalcContext) -> CalcOutcome {
    let trimmed = without_question_mark(query.trim());
    if trimmed.is_empty() || trimmed.chars().count() > MAX_QUERY_LENGTH {
        return CalcOutcome::NoCard;
    }

    // 1. Unwrap natural-language question leads (`what is ...`, `how many ...`, `convert ...`).
    if let Some(inner) = specialty::unwrap_question(&trimmed) {
        if let CalcOutcome::Card(card) = evaluate_with_context(inner, catalog, ctx) {
            return CalcOutcome::Card(card);
        }
    }

    // 2. Color preview & conversions (hex, rgb, hsl, hwb, hsv, lab, lch, oklab, oklch, cmyk).
    if let Some(card) = color::evaluate(&trimmed) {
        return CalcOutcome::Card(card);
    }

    // 3. Date, calendar, workdays, ISO weeks, holidays, clock spans, timespans, timestamps.
    if let Some(card) = datetime::evaluate(&trimmed, ctx) {
        return CalcOutcome::Card(card);
    }

    // 4. DST-aware world clocks, time zone conversions, and time differences.
    if let Some(card) = timezone::evaluate(&trimmed, ctx) {
        return CalcOutcome::Card(card);
    }

    // 5. Specialty domains: transfer time, pace, musical pitch, chemistry molar mass,
    //    physical constants, CSS/PPI, prime heights, cooking, quoted text, minor currencies,
    //    improper/mixed fractions, and specialized engineering units (dBm, Rankine, etc.).
    if let Some(card) = specialty::evaluate(&trimmed, ctx) {
        return CalcOutcome::Card(card);
    }

    // 6. Tokenized arithmetic, units, live FX currency/crypto, percentages, finance, growth, stats.
    let Some(tokens) = tokenize(&trimmed, catalog) else {
        return CalcOutcome::NoCard;
    };
    if tokens.is_empty() {
        return CalcOutcome::NoCard;
    }
    match evaluate_tokens(&tokens, &trimmed, catalog, ctx) {
        Ok(Some(result)) => CalcOutcome::Card(result),
        Ok(None) => CalcOutcome::NoCard,
        Err(area) => CalcOutcome::Unsupported(area),
    }
}

/// Evaluates a tokenized query across all math, unit, currency, finance, and
/// spoken-math layers.
fn evaluate_tokens(
    tokens: &[Token],
    query: &str,
    catalog: &dyn Catalog,
    ctx: &CalcContext,
) -> Result<Option<CalcResult>, UnsupportedArea> {
    let mut unsupported: Option<UnsupportedArea> = None;

    // A shared amount, before the partial result: `85 + 20% split 3`.
    if split::mentions(tokens) {
        if let Some(result) = layer(split::evaluate(query, catalog), &mut unsupported) {
            return Ok(Some(result));
        }
    }

    // A trailing operator keeps the last complete prefix on the card while the user
    // still types.
    if let Some(result) = layer(
        partial_result(tokens, query, catalog, ctx),
        &mut unsupported,
    ) {
        return Ok(Some(result));
    }

    // A lone literal reads as an app search, so no card - except a radix one ("0xff").
    if tokens.len() == 1 {
        return Ok(lone_literal(tokens, query));
    }

    // "255 to hex": an expression on the left, like a unit conversion.
    if let Some(result) = layer(base_conversion(tokens, query, catalog), &mut unsupported) {
        return Ok(Some(result));
    }

    // The unit conversions, before the typed arithmetic: a unit makes the plain
    // parser stop, and the conversions are what a query with a unit is asking for.
    if let Some(result) = units::evaluate(tokens, query, catalog) {
        return Ok(Some(result));
    }

    // The conversion of a calculation into a unit: `10kg + 500g to lb`.
    if let Some(result) = layer(expression_conversion(tokens, catalog), &mut unsupported) {
        return Ok(Some(result));
    }

    // Live fiat & crypto conversions and mixed-currency sums (when rates are present).
    if let Some(result) = fx::evaluate(tokens, query, catalog, ctx) {
        return Ok(Some(result));
    }

    // Typed arithmetic.
    if let Some(result) = layer(
        quantity_result(tokens, query, catalog, false),
        &mut unsupported,
    ) {
        return Ok(Some(result));
    }

    // The percentage phrasings: `15% tip on 42`, `50 as % of 200`, `20% off 500`.
    if let Some(result) = layer(percent::evaluate(tokens, query, catalog), &mut unsupported) {
        return Ok(Some(result));
    }

    // Rounding and fraction phrasings.
    if let Some(result) = layer(rounding::evaluate(tokens, query, catalog), &mut unsupported) {
        return Ok(Some(result));
    }

    // The money phrases about growth, loans and returns.
    if let Some(result) = layer(finance::evaluate(tokens, query, catalog), &mut unsupported) {
        return Ok(Some(result));
    }

    // The phrases about a steady rate.
    if let Some(result) = layer(growth::evaluate(tokens, query, catalog), &mut unsupported) {
        return Ok(Some(result));
    }

    // The reductions over a spoken list.
    if let Some(result) = layer(
        statistics::evaluate(tokens, query, catalog),
        &mut unsupported,
    ) {
        return Ok(Some(result));
    }

    // The math asked in words.
    if let Some(result) = layer(spoken::evaluate(tokens, query, catalog), &mut unsupported) {
        return Ok(Some(result));
    }

    match unsupported {
        Some(area) => Err(area),
        None => Ok(None),
    }
}

/// The card a layer produced, keeping the area it could not answer.
///
/// The first area is kept: it is the one the query met first, and it is the one
/// worth reporting when no layer answers.
fn layer(
    result: Result<Option<CalcResult>, UnsupportedArea>,
    unsupported: &mut Option<UnsupportedArea>,
) -> Option<CalcResult> {
    match result {
        Ok(Some(card)) => Some(card),
        Ok(None) => None,
        Err(area) => {
            if unsupported.is_none() {
                *unsupported = Some(area);
            }
            None
        }
    }
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
    ctx: &CalcContext,
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
    if let Some(result) = evaluate_tokens(prefix, &prefix_query, catalog, ctx)? {
        return Ok(Some(result.with_expression(Formatter::expression(query))));
    }
    let mut parser = Parser::new(prefix);
    match parser.parse(catalog) {
        Ok(value) if !value.is_boolean => Ok(Some(CalcResult::with_badges(
            Formatter::expression(query),
            "Expression",
            "Result",
            match value.currency {
                Some(currency) => Payload::money(value.amount, currency.code),
                None => Payload::number(value.effective()),
            },
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
pub(crate) fn quantity_result(
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
        if value.unit.is_none() && value.currency.is_none() {
            // A plain amount is a calculation only when something was done to it.
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
        if let Some(currency) = value.currency {
            return Ok(Some(CalcResult::with_badges(
                Formatter::expression(query),
                "Expression",
                currency.name,
                Payload::money(value.amount, currency.code),
            )));
        }
        if let Some(unit) = value.unit {
            // A bare `50cm` auto-converts into its counterpart; with an operator the
            // unit that was typed is kept.
            if !preserve_standalone
                && parser.operation_count() == 0
                && parser.dimension_count() == 1
                && ends_with_a_unit(tokens, catalog)
            {
                return Ok(None);
            }
            if parser.operation_count() == 0
                && parser.dimension_count() <= 1
                && !preserve_standalone
            {
                return Ok(None);
            }
            return Ok(Some(CalcResult::with_badges(
                expression_text(tokens, catalog),
                "Expression",
                unit.name,
                Payload::measured(value.amount, &unit),
            )));
        }
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
        color_swatch: None,
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
        Token::Ident(name) => matches!(name.as_str(), "to" | "in" | "as" | "into"),
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
/// The conversion of a calculation into a unit, as upstream's
/// `CalcQuantity.splitConversion` and `CalcExpressionParser.converted` answer it.
///
/// `10kg + 500g to lb` is `23.14853753 lb`: the calculation is made first, and the
/// answer is then told in the unit that was named.
fn expression_conversion(
    tokens: &[Token],
    catalog: &dyn Catalog,
) -> Result<Option<CalcResult>, UnsupportedArea> {
    let Some(index) = tokens.len().checked_sub(2) else {
        return Ok(None);
    };
    if !is_connector(&tokens[index]) {
        return Ok(None);
    }
    let Token::Ident(target_name) = &tokens[index + 1] else {
        return Ok(None);
    };
    let source = &tokens[..index];
    if source.is_empty() {
        return Ok(None);
    }
    // A plain amount before the connector is a simple conversion, and the conversion
    // layer answers it with its own badges.
    if is_simple_conversion_source(source) {
        return Ok(None);
    }
    let Some(target) = catalog.unit_ref(target_name) else {
        return Ok(None);
    };
    let mut parser = Parser::new(source);
    let value = match parser.parse(catalog) {
        Ok(value) => value,
        Err(_) => return Ok(None),
    };
    let Some(unit) = value.unit else {
        return Ok(None);
    };
    let target = Measurement::of(target);
    if !unit.is_compatible_with(&target) {
        return Ok(None);
    }
    let amount = unit.told_in(value.amount, &target);
    Ok(Some(CalcResult::with_badges(
        expression_text(source, catalog),
        "Expression",
        target.name,
        Payload::measured(amount, &target),
    )))
}

/// Whether the tokens before a connector are a plain amount, as upstream's
/// `CalcQuantity.isSimpleConversionSource` asks.
///
/// A simple source is answered by the conversion layer, which shows the unit it came
/// from and the unit it went to; a calculation is answered here, with the expression
/// itself as the badge.
fn is_simple_conversion_source(tokens: &[Token]) -> bool {
    match tokens.len() {
        1 => matches!(tokens[0], Token::Ident(_)),
        2 => matches!(
            (&tokens[0], &tokens[1]),
            (Token::Number(_), Token::Ident(_))
                | (Token::CompactNumber(_), Token::Ident(_))
                | (Token::Ident(_), Token::Number(_))
                | (Token::Ident(_), Token::CompactNumber(_))
                | (Token::Ident(_), Token::Ident(_))
        ),
        _ => false,
    }
}

/// Whether the last token names a unit, as upstream's standalone-unit rule asks.
///
/// A bare `50cm` is read as a request to convert it; once something is done to it,
/// the unit that was typed is the unit of the answer.
fn ends_with_a_unit(tokens: &[Token], catalog: &dyn Catalog) -> bool {
    match tokens.last() {
        Some(Token::Ident(name)) => catalog.unit_ref(name).is_some(),
        _ => false,
    }
}

/// The expression a card echoes, as upstream's `CalcQuantity.expressionText`.
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

#[cfg(test)]
mod swiss_army_tests {
    use std::sync::Arc;

    use jiff::tz::TimeZone;
    use jiff::Timestamp;

    use super::*;
    use crate::catalog::FullCatalog;
    use crate::context::ExchangeRates;

    fn full_ctx() -> CalcContext {
        let ts: Timestamp = "2026-11-22T15:39:00Z".parse().unwrap();
        let tz = TimeZone::get("Asia/Kolkata").unwrap();
        let mut rates = ExchangeRates::bundled();
        rates.per_usd.insert("USD".into(), 1.0);
        rates.per_usd.insert("EUR".into(), 0.92);
        rates.per_usd.insert("GBP".into(), 0.79);
        rates.per_usd.insert("INR".into(), 86.5);
        rates.per_usd.insert("JPY".into(), 157.0);
        rates.per_usd.insert("BTC".into(), 0.00001);
        CalcContext::fixed(ts, tz).with_rates(Some(Arc::new(rates)))
    }

    fn eval_disp(q: &str) -> String {
        let out = evaluate_with_context(q, &FullCatalog, &full_ctx());
        out.display()
            .unwrap_or_else(|| panic!("expected card for query `{q}`, got {out:?}"))
            .to_string()
    }

    #[test]
    fn swiss_army_knife_end_to_end_benchmark() {
        // Core arithmetic, percentages, tips, VAT, splits, finance
        assert_eq!(eval_disp("52% of 900"), "468");
        assert_eq!(eval_disp("20% off 80"), "64");
        assert_eq!(eval_disp("20% vat in 120"), "20");
        assert_eq!(eval_disp("25% off $60 split 3"), "15.00 USD");
        assert_eq!(eval_disp("15% decrease from 813"), "691.05");
        assert_eq!(eval_disp("what is 15% of 240?"), "36");
        assert_eq!(eval_disp("750 m as % of 3 km"), "25%");
        assert_eq!(eval_disp("1/8 is what %"), "12.5%");

        // Units, compound math, heights, specialized units
        assert_eq!(eval_disp("10ft in m"), "3.048 m");
        assert_eq!(eval_disp("6'2\" to m"), "1.8796 m");
        assert_eq!(eval_disp("23C to F"), "73.4 °F");
        assert_eq!(eval_disp("3 kg * 2 m"), "6 kg·m");
        assert_eq!(eval_disp("100 months in years and months"), "8 yr 4 mo");
        assert_eq!(eval_disp("1 fortnight in days"), "14 days");
        assert_eq!(eval_disp("1 chain in yards"), "22 yd");
        assert_eq!(eval_disp("491.67 rankine in celsius"), "0 °C");
        assert_eq!(eval_disp("30 dBm in watts"), "1 W");
        assert!(eval_disp("1 L/s in gpm").starts_with("15.8503"));
        assert_eq!(eval_disp("1 nibble in bits"), "4 bit");
        assert_eq!(eval_disp("55h in workdays"), "6.875 workdays");

        // Physics, transfer time, pace, pitch, chemistry, CSS/PPI, cooking, text
        assert_eq!(eval_disp("speed of light in km/s"), "299,792.458 km/s");
        assert!(eval_disp("1 astronomical unit / speed of light in minutes").starts_with("8.3167"));
        assert_eq!(
            eval_disp("transfer 1 TB at 100 Mbps"),
            "22 hours 13 min 20 s"
        );
        assert_eq!(eval_disp("5:00 min/km in km/h"), "12 km/h");
        assert!(eval_disp("13:24 min/mile in mph").starts_with("4.4776"));
        assert!(eval_disp("c4 to hz").starts_with("261.6255"));
        assert_eq!(eval_disp("molar mass of C6H12O6"), "180.156 g/mol");
        assert_eq!(eval_disp("2rem in px"), "32px");
        assert_eq!(eval_disp("32px in rem"), "2rem");
        assert_eq!(eval_disp("2 inches in px at 72 ppi"), "144px");
        assert_eq!(eval_disp("1920x1080 aspect ratio"), "16:9");
        assert!(eval_disp("1 cup flour in grams").starts_with("140.29"));
        assert_eq!(
            eval_disp(r#"first 10 characters of "hello world""#),
            "hello worl"
        );
        assert_eq!(eval_disp(r#"length of "hello world""#), "11");
        assert_eq!(eval_disp(r#"words in "one two three""#), "3");

        // Bases, fractions, rounding, lists with units
        assert_eq!(eval_disp("1.75 as improper"), "7/4");
        assert_eq!(eval_disp("7/4 as mixed"), "1 ¾");
        assert_eq!(eval_disp("3.14159 to nearest hundredth"), "3.14");
        assert_eq!(eval_disp("255 to base 7"), "513");
        assert_eq!(eval_disp("0xff"), "255");
        assert_eq!(eval_disp("1 << 10"), "1,024");
        assert_eq!(eval_disp("max(2 hours 30 min, 200 min)"), "3.333333333 hr");

        // Currency, crypto, minor units
        assert_eq!(eval_disp("100 usd in inr"), "8,650.00 INR");
        assert_eq!(eval_disp("0.5 btc in usd"), "50,000.00 USD");
        assert_eq!(eval_disp("$12 in cents"), "1,200 cents");
        assert_eq!(eval_disp("100 usd + 92 eur in gbp"), "158.00 GBP");

        // Dates, workdays, weeks, timestamps, world clocks, time zones
        assert_eq!(eval_disp("now"), "9:09 PM");
        assert_eq!(eval_disp("today"), "November 22, 2026");
        assert_eq!(eval_disp("tomorrow"), "November 23, 2026");
        assert_eq!(eval_disp("monday in 3 weeks"), "Monday, December 14, 2026");
        assert_eq!(eval_disp("35 days ago"), "October 18, 2026");
        assert_eq!(eval_disp("days until 25 Dec"), "33 days");
        assert_eq!(eval_disp("workhours in 2026"), "2,088 hours");
        assert_eq!(eval_disp("week 44 2026"), "Oct 26 – Nov 1, 2026");
        assert_eq!(eval_disp("10pm to 2am"), "4 hours");
        assert_eq!(eval_disp("145 mins to timespan"), "2 hours 25 min");
        assert_eq!(eval_disp("from 1700000000 to 1700003600"), "1 hour");
        assert_eq!(
            eval_disp("2024-03-15T14:30:00Z"),
            "2024-03-15T20:00:00+05:30"
        );
        assert_eq!(eval_disp("time in tokyo"), "12:39 AM");
        assert_eq!(eval_disp("5pm ldn in sf"), "9:00 AM");
        assert_eq!(eval_disp("4:00 PT in UTC"), "12:00 PM");
        assert_eq!(eval_disp("time diff Paris"), "-4.5 hours");
        assert_eq!(eval_disp("time in brisbane + 4 hours"), "5:39 AM");
        assert_eq!(eval_disp("time in JFK"), "10:39 AM");
        assert_eq!(eval_disp("time in São Paulo"), "12:39 PM");

        // Colors + live RGBA swatch
        let col = evaluate_with_context("#12F999 as rgb", &FullCatalog, &full_ctx());
        assert_eq!(col.display(), Some("rgb(18, 249, 153)"));
        assert_eq!(col.card().unwrap().color_swatch, Some([18, 249, 153, 255]));
        assert!(eval_disp("#12F999 as oklch").starts_with("oklch(0.86"));
        assert_eq!(eval_disp("tomato at 50%"), "#FF634780");
    }
}
