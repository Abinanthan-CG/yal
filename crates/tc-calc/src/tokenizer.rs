//! The tokenizer: a query in, a list of tokens out.
//!
//! Mirrors upstream `CalcTokenizer.swift`. Upstream answers `nil` for a character that
//! cannot be calculator input - "not a calculation", not an error - and this port does
//! the same by answering `None`.

use crate::catalog::Catalog;
use crate::math::Math;
use crate::token::{NumberBase, Operator, Token};

/// Turns a query into tokens, as upstream's `CalcTokenizer.tokenize`.
///
/// Upstream answers `nil` when the query contains a character that is not calculator
/// input. Every decision that depends on the unit or currency catalog is taken through
/// the [`Catalog`] this is given.
pub fn tokenize(input: &str, catalog: &dyn Catalog) -> Option<Vec<Token>> {
    let chars: Vec<char> = input.chars().collect();
    let mut tokens: Vec<Token> = Vec::with_capacity(chars.len() / 2 + 1);
    let mut index = 0usize;
    let mut depth: i32 = 0;
    let mut function_depth: Option<i32> = None;

    while index < chars.len() {
        let ch = chars[index];
        if ch.is_whitespace() {
            index += 1;
            continue;
        }

        // Radix literals need at least one digit after the prefix, else "0" is a plain
        // number.
        if ch == '0' && index + 2 < chars.len() {
            if let Some(base) = literal_base(chars[index + 1]) {
                let start = index + 2;
                let mut end = start;
                while end < chars.len() && chars[end].is_ascii_hexdigit() {
                    end += 1;
                }
                if end > start {
                    let text: String = chars[start..end].iter().collect();
                    if let Ok(value) = u64::from_str_radix(&text, base.radix()) {
                        tokens.push(Token::IntLiteral(value, base));
                        index = end;
                        continue;
                    }
                }
            }
        }

        if ch.is_ascii_digit() {
            if let Some((value, end)) = unit_fraction(&chars, index, catalog) {
                tokens.push(Token::Number(value));
                index = end;
                continue;
            }
        }

        if ch.is_ascii_digit()
            || (ch == '.' && index + 1 < chars.len() && chars[index + 1].is_ascii_digit())
        {
            let mut text = String::new();
            let mut seen_dot = false;
            while index < chars.len() {
                let current = chars[index];
                if current.is_ascii_digit() {
                    text.push(current);
                } else if current == ','
                    && function_depth.is_none()
                    && index + 1 < chars.len()
                    && chars[index + 1].is_ascii_digit()
                {
                    // A thousands separator: it is dropped from the literal.
                } else if current == '.' && !seen_dot {
                    seen_dot = true;
                    text.push(current);
                } else {
                    break;
                }
                index += 1;
            }
            // Only while the exponent hugs the mantissa - a spaced `2 e` stays 2 x e.
            let mut is_shorthand = false;
            if index < chars.len() && (chars[index] == 'e' || chars[index] == 'E') {
                let mut digits = index + 1;
                if digits < chars.len() && (chars[digits] == '+' || chars[digits] == '-') {
                    digits += 1;
                }
                let mut end = digits;
                while end < chars.len() && chars[end].is_ascii_digit() {
                    end += 1;
                }
                if end > digits {
                    text.extend(chars[index..end].iter());
                    index = end;
                    is_shorthand = true;
                }
            }
            // An overflowing literal ("1e400") is not calculator input, so no card.
            let Ok(value) = text.parse::<f64>() else {
                return None;
            };
            if !value.is_finite() {
                return None;
            }
            // An attached `k` is x1,000; whitespace keeps Kelvin, and `10kg` stays a unit.
            if index < chars.len()
                && (chars[index] == 'k' || chars[index] == 'K')
                && is_compact_suffix(&chars, index, catalog)
            {
                let scaled = value * 1_000.0;
                if !scaled.is_finite() {
                    return None;
                }
                tokens.push(Token::CompactNumber(scaled));
                index += 1;
            } else if let Some((scale, end)) = magnitude_word(&chars, index)
                .or_else(|| currency_magnitude(&chars, index, tokens.last(), catalog))
            {
                let scaled = value * scale;
                if !scaled.is_finite() {
                    return None;
                }
                tokens.push(Token::CompactNumber(scaled));
                index = end;
            } else if is_shorthand {
                tokens.push(Token::CompactNumber(value));
            } else {
                tokens.push(Token::Number(value));
            }
            continue;
        }

        if (ch == 'x' || ch == 'X') && is_multiplication_x(&chars, index, tokens.last(), catalog) {
            tokens.push(Token::Op(Operator::Multiply));
            index += 1;
            continue;
        }

        if ch.is_alphabetic() || ch == '°' {
            let start = index;
            while index < chars.len() && chars[index].is_alphabetic() {
                index += 1;
            }
            let mut text: String = chars[start..index].iter().collect();
            if index < chars.len() && chars[index].is_ascii_digit() {
                let prefix = text.to_lowercase();
                if catalog.unit(&prefix).is_none() && catalog.currency(&prefix).is_some() {
                    tokens.push(Token::Ident(prefix));
                    continue;
                }
            }
            while index < chars.len() {
                let current = chars[index];
                if current.is_alphabetic()
                    || is_combining_mark(current)
                    || current == '°'
                    || current.is_ascii_digit()
                {
                    text.push(current);
                } else if current == '²' {
                    text.push('2');
                } else if current == '³' {
                    text.push('3');
                } else {
                    break;
                }
                index += 1;
            }
            if let Some(previous) = tokens.last() {
                if ends_operand(previous) && may_be_word_operator(&text) {
                    if let Some((word_tokens, end)) =
                        word_operator(&text.to_lowercase(), &chars, index)
                    {
                        tokens.extend(word_tokens);
                        index = end;
                        continue;
                    }
                }
            }
            if let Some(head) = spoken_root_head(&text.to_lowercase()) {
                if following_words(&["root"], &chars, index).is_some() {
                    tokens.push(Token::Ident(head.to_string()));
                    continue;
                }
            }
            match compound_unit(&chars, index, &text, catalog) {
                Some((name, end)) => {
                    tokens.push(Token::Ident(name));
                    index = end;
                }
                None => {
                    let name = if catalog.unit(&text).is_some() {
                        text.clone()
                    } else {
                        text.to_lowercase()
                    };
                    tokens.push(Token::Ident(name));
                }
            }
            continue;
        }

        // Signs are punctuation, so they fold to ISO: `EUR20 to GBP` tokenizes as
        // `20 eur to gbp`.
        if let Some(code) = catalog.currency_sign(ch) {
            tokens.push(Token::Ident(code));
            index += 1;
            continue;
        }

        if let Some(mark) = length_mark(ch) {
            if index > 0 && chars[index - 1].is_ascii_digit() {
                tokens.push(Token::Ident(mark.to_string()));
                index += 1;
                continue;
            }
        }

        if ch == '*' && index + 1 < chars.len() && chars[index + 1] == '*' {
            tokens.push(Token::Op(Operator::Power));
            index += 2;
            continue;
        }

        if index + 1 < chars.len() {
            let combined = match (ch, chars[index + 1]) {
                ('<', '<') => Some(Operator::ShiftLeft),
                ('>', '>') => Some(Operator::ShiftRight),
                ('=', '=') => Some(Operator::Equal),
                ('!', '=') => Some(Operator::NotEqual),
                ('<', '=') => Some(Operator::LessEqual),
                ('>', '=') => Some(Operator::GreaterEqual),
                _ => None,
            };
            if let Some(op) = combined {
                tokens.push(Token::Op(op));
                index += 2;
                continue;
            }
        }

        if ch == '(' {
            if function_depth.is_none() {
                if let Some(Token::Ident(name)) = tokens.last() {
                    if Math::is_function(name) {
                        function_depth = Some(depth);
                    }
                }
            }
            depth += 1;
        } else if ch == ')' {
            depth -= 1;
            if function_depth == Some(depth) {
                function_depth = None;
            }
        }

        match ch {
            '+' | '(' | ')' | '!' | '%' | '^' | '&' | '|' | '~' | '<' | '>' | '≤' | '≥' | '≠'
            | '⊻' => {
                let op = Operator::from_char(ch)?;
                tokens.push(Token::Op(op));
            }
            ',' => tokens.push(Token::Comma),
            '*' | '×' => tokens.push(Token::Op(Operator::Multiply)),
            '/' | '÷' => tokens.push(Token::Op(Operator::Divide)),
            '−' => tokens.push(Token::Op(Operator::Subtract)),
            '-' => {
                if index + 1 < chars.len() && chars[index + 1] == '>' {
                    tokens.push(Token::Arrow);
                    index += 1;
                } else {
                    tokens.push(Token::Op(Operator::Subtract));
                }
            }
            '→' => tokens.push(Token::Arrow),
            '=' => {
                // Tolerate a trailing "=" ("2+2="); anywhere else it is not calculator
                // input.
                if index != chars.len() - 1 {
                    return None;
                }
            }
            _ => return None,
        }
        index += 1;
    }
    Some(tokens)
}

/// The base a radix prefix stands for, as upstream's `CalcTokenizer.literalBase`.
fn literal_base(prefix: char) -> Option<NumberBase> {
    match prefix {
        'x' | 'X' => Some(NumberBase::Hexadecimal),
        'b' | 'B' => Some(NumberBase::Binary),
        'o' | 'O' => Some(NumberBase::Octal),
        _ => None,
    }
}

/// Whether the `x` at `index` multiplies, as upstream's `CalcTokenizer.isMultiplicationX`.
fn is_multiplication_x(
    chars: &[char],
    index: usize,
    previous: Option<&Token>,
    catalog: &dyn Catalog,
) -> bool {
    if index == 0 || chars[index - 1].is_alphabetic() {
        return false;
    }
    let Some(previous) = previous else {
        return false;
    };
    if !ends_operand(previous) {
        return false;
    }
    let mut end = index + 1;
    while end < chars.len() && chars[end].is_alphabetic() {
        end += 1;
    }
    if end > index + 1 {
        let name: String = chars[index..end].iter().collect::<String>().to_lowercase();
        if name == "xor" || catalog.currency(&name).is_some() {
            return false;
        }
    }
    let attached = !chars[index - 1].is_whitespace();
    let mut next = index + 1;
    while next < chars.len() && chars[next].is_whitespace() {
        next += 1;
    }
    if next >= chars.len() {
        return !attached;
    }
    !matches!(
        chars[next],
        ')' | '!' | '%' | '^' | '/' | ',' | '=' | '*' | '×' | '÷' | '−' | '→'
    )
}

/// Every spoken operator is two to ten letters and starts with one of these.
///
/// Mirrors upstream's `CalcTokenizer.mayBeWordOperator`.
fn may_be_word_operator(text: &str) -> bool {
    let bytes = text.as_bytes();
    (2..=10).contains(&bytes.len()) && b"pmtdscrePMTDSCRE".contains(&bytes[0])
}

/// The head of a spoken root, as upstream's `CalcTokenizer.spokenRootHead`.
fn spoken_root_head(word: &str) -> Option<&'static str> {
    match word {
        "squared" => Some("square"),
        "cubed" => Some("cube"),
        _ => None,
    }
}

/// A word operator and where it ends, as upstream's `CalcTokenizer.wordOperator`.
fn word_operator(word: &str, chars: &[char], index: usize) -> Option<(Vec<Token>, usize)> {
    let attached =
        index < chars.len() && (chars[index].is_numeric() || is_combining_mark(chars[index]));
    if attached {
        return None;
    }
    match word {
        "per" => {
            if let Some(end) = following_words(&["cent"], chars, index) {
                Some((vec![Token::Op(Operator::Percent)], end))
            } else {
                Some((vec![Token::Op(Operator::Divide)], index))
            }
        }
        "percent" | "percentage" => Some((vec![Token::Op(Operator::Percent)], index)),
        "exponent" => Some((vec![Token::Op(Operator::Power)], index)),
        "raised" => {
            let end = following_words(&["to"], chars, index)?;
            let power_end = following_words(&["the", "power", "of"], chars, end).unwrap_or(end);
            Some((vec![Token::Op(Operator::Power)], power_end))
        }
        "plus" => Some((vec![Token::Op(Operator::Add)], index)),
        "minus" => Some((vec![Token::Op(Operator::Subtract)], index)),
        "times" => Some((vec![Token::Op(Operator::Multiply)], index)),
        "squared" => Some((vec![Token::Op(Operator::Power), Token::Number(2.0)], index)),
        "cubed" => Some((vec![Token::Op(Operator::Power), Token::Number(3.0)], index)),
        "divided" | "multiplied" => {
            let end = following_words(&["by"], chars, index)?;
            let op = if word == "divided" {
                Operator::Divide
            } else {
                Operator::Multiply
            };
            Some((vec![Token::Op(op)], end))
        }
        "to" => {
            if let Some(end) = following_words(&["the", "power", "of"], chars, index) {
                return Some((vec![Token::Op(Operator::Power)], end));
            }
            // `3 to the 9`: only a digit after `the`, so `to the nearest 10` keeps
            // rounding.
            let end = following_words(&["the"], chars, index)?;
            if end >= chars.len() || !chars[end].is_whitespace() {
                return None;
            }
            let next = chars[end + 1..].iter().find(|c| !c.is_whitespace());
            if next.map(|c| c.is_numeric()) != Some(true) {
                return None;
            }
            Some((vec![Token::Op(Operator::Power)], end))
        }
        _ => None,
    }
}

/// The end of the words that follow `index`, as upstream's `CalcTokenizer.followingWords`.
fn following_words(expected: &[&str], chars: &[char], index: usize) -> Option<usize> {
    let mut end = index;
    for word in expected {
        let mut start = end;
        while start < chars.len() && chars[start].is_whitespace() {
            start += 1;
        }
        end = start;
        while end < chars.len() && chars[end].is_alphabetic() {
            end += 1;
        }
        let found: String = chars[start..end].iter().collect::<String>().to_lowercase();
        if found != *word {
            return None;
        }
    }
    Some(end)
}

/// A fraction that is only one amount because a unit follows it.
///
/// Mirrors upstream's `CalcTokenizer.unitFraction`: only before a unit, so a spaced
/// `1 / 20ms` and a bare `5/2 - 1/2` keep dividing.
fn unit_fraction(chars: &[char], start: usize, catalog: &dyn Catalog) -> Option<(f64, usize)> {
    let first = integer(chars, start)?;
    let mut whole = 0.0;
    let mut numerator = first;
    if first.1 < chars.len() && chars[first.1].is_whitespace() {
        let mut next = first.1;
        while next < chars.len() && chars[next].is_whitespace() {
            next += 1;
        }
        let part = integer(chars, next)?;
        whole = first.0;
        numerator = part;
    }
    if numerator.1 >= chars.len() || chars[numerator.1] != '/' {
        return None;
    }
    let denominator = integer(chars, numerator.1 + 1)?;
    if denominator.0 == 0.0 {
        return None;
    }
    if !names_unit(chars, denominator.1, catalog) {
        return None;
    }
    Some((whole + numerator.0 / denominator.0, denominator.1))
}

/// A whole number of at most fifteen digits, as upstream's `CalcTokenizer.integer`.
fn integer(chars: &[char], start: usize) -> Option<(f64, usize)> {
    let mut end = start;
    while end < chars.len() && chars[end].is_ascii_digit() {
        end += 1;
    }
    if end == start || end - start > 15 {
        return None;
    }
    if end < chars.len() && chars[end] == '.' {
        return None;
    }
    let text: String = chars[start..end].iter().collect();
    Some((text.parse::<f64>().ok()?, end))
}

/// Whether a unit is named from `index` on, as upstream's `CalcTokenizer.namesUnit`.
fn names_unit(chars: &[char], index: usize, catalog: &dyn Catalog) -> bool {
    if index < chars.len() && length_mark(chars[index]).is_some() {
        return true;
    }
    let mut start = index;
    while start < chars.len() && chars[start].is_whitespace() {
        start += 1;
    }
    let mut end = start;
    while end < chars.len()
        && (chars[end].is_alphabetic() || (end > start && chars[end].is_numeric()))
    {
        end += 1;
    }
    if end == start {
        return false;
    }
    let name: String = chars[start..end].iter().collect();
    if catalog.unit(&name).is_some() || catalog.unit(&name.to_lowercase()).is_some() {
        return true;
    }
    compound_unit(chars, end, &name, catalog).is_some()
}

/// The unit a length mark stands for, as upstream's `CalcTokenizer.lengthMark`.
fn length_mark(ch: char) -> Option<&'static str> {
    match ch {
        '"' | '\u{2033}' | '\u{201D}' => Some("inch"),
        '\'' | '\u{2032}' | '\u{2019}' => Some("ft"),
        _ => None,
    }
}

/// The scale of a magnitude word after a currency, as upstream's
/// `CalcTokenizer.currencyMagnitude`.
///
/// `$5M` and `EUR2bn` only read that way right after a currency, where `m` and `b`
/// cannot mean metre or byte.
fn currency_magnitude(
    chars: &[char],
    index: usize,
    previous: Option<&Token>,
    catalog: &dyn Catalog,
) -> Option<(f64, usize)> {
    let Token::Ident(code) = previous? else {
        return None;
    };
    catalog.currency(code)?;
    let mut end = index;
    while end < chars.len() && chars[end].is_alphabetic() {
        end += 1;
    }
    if end == index {
        return None;
    }
    if end < chars.len() && chars[end].is_numeric() {
        return None;
    }
    let word: String = chars[index..end].iter().collect();
    match word.as_str() {
        "m" | "M" => Some((1e6, end)),
        "b" | "B" | "bn" => Some((1e9, end)),
        "t" | "T" | "tn" => Some((1e12, end)),
        _ => None,
    }
}

/// Whether a token can end an operand, as upstream's `CalcTokenizer.endsOperand`.
fn ends_operand(token: &Token) -> bool {
    match token {
        Token::Number(_) | Token::CompactNumber(_) | Token::IntLiteral(..) => true,
        Token::Op(op) => matches!(
            op,
            Operator::Close | Operator::Factorial | Operator::Percent
        ),
        Token::Ident(name) => !matches!(
            name.as_str(),
            "to" | "in" | "into" | "of" | "mod" | "power" | "and"
        ),
        Token::Arrow | Token::Comma => false,
    }
}

/// A compound unit spelled `left/right`, as upstream's `CalcTokenizer.compoundUnit`.
///
/// Only a spelling the catalog resolves, so `6/2(1+2)` keeps dividing.
fn compound_unit(
    chars: &[char],
    index: usize,
    prefix: &str,
    catalog: &dyn Catalog,
) -> Option<(String, usize)> {
    if index >= chars.len() {
        return None;
    }
    if chars[index] != '/' && !chars[index].is_whitespace() {
        return None;
    }
    let separator = if chars[index] == '/' { "/" } else { " " };
    let mut right_start = index + 1;
    while right_start < chars.len() && chars[right_start].is_whitespace() {
        right_start += 1;
    }
    let mut end = right_start;
    while end < chars.len() && (chars[end].is_alphabetic() || chars[end].is_numeric()) {
        end += 1;
    }
    if end == right_start {
        return None;
    }
    let right: String = chars[right_start..end].iter().collect();
    let spelling = format!("{prefix}{separator}{right}")
        .replace('²', "2")
        .replace('³', "3");
    let name = if catalog.unit(&spelling).is_some() {
        spelling.clone()
    } else {
        spelling.to_lowercase()
    };
    catalog.unit(&name)?;
    Some((name, end))
}

/// Short scale only: the engine reads canonical English, where a billion is 10^9.
fn magnitude_scale(word: &str) -> Option<f64> {
    match word {
        "thousand" => Some(1e3),
        "million" => Some(1e6),
        "billion" => Some(1e9),
        "trillion" => Some(1e12),
        "quadrillion" => Some(1e15),
        _ => None,
    }
}

/// The scale of a whole magnitude word following a literal, and where it ends.
///
/// Mirrors upstream's `CalcTokenizer.magnitudeWord`.
fn magnitude_word(chars: &[char], index: usize) -> Option<(f64, usize)> {
    let mut start = index;
    while start < chars.len() && chars[start].is_whitespace() {
        start += 1;
    }
    let mut end = start;
    while end < chars.len() && chars[end].is_alphabetic() {
        end += 1;
    }
    if end == start {
        return None;
    }
    if end < chars.len() && (chars[end].is_numeric() || is_combining_mark(chars[end])) {
        return None;
    }
    let word: String = chars[start..end].iter().collect::<String>().to_lowercase();
    Some((magnitude_scale(&word)?, end))
}

/// Whether the `k` at `index` is a thousands suffix rather than Kelvin or a unit's head.
///
/// Mirrors upstream's `CalcTokenizer.isCompactSuffix`.
fn is_compact_suffix(chars: &[char], index: usize, catalog: &dyn Catalog) -> bool {
    let next = index + 1;
    if next >= chars.len() {
        return true;
    }
    if is_temperature_conversion(chars, next, catalog) {
        return false;
    }
    if !chars[next].is_alphabetic() {
        return true;
    }
    let mut end = next;
    while end < chars.len() && chars[end].is_alphabetic() {
        end += 1;
    }
    let name: String = chars[next..end].iter().collect::<String>().to_lowercase();
    catalog.currency(&name).is_some()
}

/// Whether the rest reads as a conversion into a temperature unit, keeping `k` as Kelvin.
///
/// Mirrors upstream's `CalcTokenizer.isTemperatureConversion`.
fn is_temperature_conversion(chars: &[char], index: usize, catalog: &dyn Catalog) -> bool {
    let remainder: String = chars[index..]
        .iter()
        .collect::<String>()
        .trim()
        .to_lowercase();
    for connector in ["to", "in", "->", "→"] {
        if let Some(target) = remainder.strip_prefix(connector) {
            let target = target.trim();
            if let Some(unit) = catalog.unit(target) {
                if unit.is_temperature {
                    return true;
                }
            }
        }
    }
    false
}

/// Whether a character is a combining mark.
///
/// Upstream asks the Unicode general category for a non-spacing or spacing mark; this
/// port answers for the ranges those marks live in.
fn is_combining_mark(ch: char) -> bool {
    matches!(
        ch,
        '\u{0300}'..='\u{036F}'
            | '\u{0483}'..='\u{0489}'
            | '\u{0591}'..='\u{05BD}'
            | '\u{0610}'..='\u{061A}'
            | '\u{064B}'..='\u{065F}'
            | '\u{0670}'
            | '\u{06D6}'..='\u{06DC}'
            | '\u{06DF}'..='\u{06E4}'
            | '\u{06E7}'..='\u{06E8}'
            | '\u{06EA}'..='\u{06ED}'
            | '\u{0900}'..='\u{0903}'
            | '\u{093A}'..='\u{094F}'
            | '\u{0951}'..='\u{0957}'
            | '\u{0E31}'
            | '\u{0E34}'..='\u{0E3A}'
            | '\u{0E47}'..='\u{0E4E}'
            | '\u{20D0}'..='\u{20F0}'
            | '\u{FE20}'..='\u{FE2F}'
    )
}
