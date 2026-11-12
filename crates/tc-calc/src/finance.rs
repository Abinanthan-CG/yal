//! Compound growth, loans and returns: `$1,000 after 3 years at 7%`.
//!
//! Mirrors upstream `CalcFinance.swift`.
//!
//! The phrase carries three things: the **principal**, the **term** and the
//! **rate**. They come in either order - `for 3 years at 7%` or
//! `at 7% for 3 years` - and `after` and `over` both read as `for`.
//!
//! What is asked for comes first: the interest, the present value, a payment
//! and how often it is made, or the capital an investment needs.

use crate::catalog::Catalog;
use crate::duration;
use crate::parser::Parser;
use crate::result::{CalcResult, Payload};
use crate::token::{operands_of, Operator, Token};
use crate::value::{ParseError, UnsupportedArea, Value};

/// The card of a money phrase about growth, loans and returns.
pub fn evaluate(
    tokens: &[Token],
    query: &str,
    catalog: &dyn Catalog,
) -> Result<Option<CalcResult>, UnsupportedArea> {
    if let Some(result) = investment_return(tokens, query, catalog)? {
        return Ok(Some(result));
    }
    if !contains(tokens, "at") || !tokens.iter().any(|token| *token == percent()) {
        return Ok(None);
    }
    if let Some(result) = investment_required(tokens, query, catalog)? {
        return Ok(Some(result));
    }
    terms_answer(tokens, query, catalog)
}

/// What the phrase asks for.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Question {
    /// What the amount will be.
    FutureValue,
    /// What the amount will have earned.
    Interest,
    /// What the amount is worth now.
    PresentValue,
    /// What has to be paid, and how often.
    Payment(Frequency),
    /// What the payments will have earned.
    LoanInterest(Frequency),
}

/// How often a payment is made.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Frequency {
    /// Every month.
    Monthly,
    /// Every year.
    Annual,
    /// All of it at once.
    Total,
}

impl Frequency {
    /// The months the payments cover, as upstream's `CalcFinance.Frequency.months`.
    fn months(self, term: f64) -> f64 {
        match self {
            Self::Monthly => 1.0,
            Self::Annual => 12.0,
            Self::Total => term,
        }
    }

    /// The badge the card wears.
    fn badge(self) -> &'static str {
        match self {
            Self::Monthly => "Monthly Payment",
            Self::Annual => "Annual Payment",
            Self::Total => "Total Payment",
        }
    }

    /// The badge a card about the interest wears.
    fn interest_badge(self) -> &'static str {
        match self {
            Self::Monthly => "Monthly Interest",
            Self::Annual => "Annual Interest",
            Self::Total => "Total Interest",
        }
    }
}

/// The principal, the term and the rate.
struct Terms {
    /// The amount the phrase is about.
    principal: Value,
    /// How many years the term runs for.
    years: f64,
    /// The yearly rate, as a fraction.
    rate: f64,
}

/// The answer to a phrase about terms.
fn terms_answer(
    tokens: &[Token],
    query: &str,
    catalog: &dyn Catalog,
) -> Result<Option<CalcResult>, UnsupportedArea> {
    let mut rest = tokens.to_vec();
    let ask = question(&mut rest);
    let periods_per_year = compounding(&mut rest);
    let Some(terms) = terms(&rest, catalog)? else {
        return Ok(None);
    };

    let mut answer = terms.principal;
    let badge: &str;
    match ask {
        Question::FutureValue | Question::Interest | Question::PresentValue => {
            let growth = (1.0 + terms.rate / periods_per_year).powf(periods_per_year * terms.years);
            match ask {
                Question::Interest => {
                    answer.amount *= growth - 1.0;
                    badge = "Interest";
                }
                Question::PresentValue => {
                    answer.amount /= growth;
                    badge = "Present Value";
                }
                _ => {
                    answer.amount *= growth;
                    badge = "Future Value";
                }
            }
        }
        Question::Payment(frequency) | Question::LoanInterest(frequency) => {
            let months = terms.years * 12.0;
            if months <= 0.0 {
                return Ok(None);
            }
            let monthly_rate = terms.rate / 12.0;
            let monthly = if monthly_rate == 0.0 {
                1.0 / months
            } else {
                monthly_rate / (1.0 - (1.0 + monthly_rate).powf(-months))
            };
            if ask == Question::LoanInterest(frequency) {
                // Interest is what the payments add over the principal, spread like the
                // payments are.
                answer.amount *= (monthly * months - 1.0) * frequency.months(months) / months;
                badge = frequency.interest_badge();
            } else {
                answer.amount *= monthly * frequency.months(months);
                badge = frequency.badge();
            }
        }
    }
    if !answer.amount.is_finite() {
        return Ok(None);
    }
    Ok(Some(CalcResult::phrase(
        query,
        amount_payload(answer),
        badge,
    )))
}

/// What the phrase asks for, and the words it asked it with.
fn question(rest: &mut Vec<Token>) -> Question {
    let leads: [(&[&str], Question); 3] = [
        (&["interest", "on"], Question::Interest),
        (&["present", "value", "of"], Question::PresentValue),
        (
            &["interest", "repayment", "on"],
            Question::LoanInterest(Frequency::Total),
        ),
    ];
    for (lead, question) in leads {
        if starts_with(rest, lead) {
            rest.drain(0..lead.len());
            return question;
        }
    }
    let frequency = match rest.first() {
        Some(Token::Ident(name)) => match name.as_str() {
            "monthly" => Frequency::Monthly,
            "annual" | "yearly" => Frequency::Annual,
            "total" => Frequency::Total,
            _ => return Question::FutureValue,
        },
        _ => return Question::FutureValue,
    };
    if rest.len() <= 3 {
        return Question::FutureValue;
    }
    let Some(Token::Ident(noun)) = rest.get(1) else {
        return Question::FutureValue;
    };
    if ["payment", "payments", "repayment", "repayments"].contains(&noun.as_str()) {
        if rest.get(2) == Some(&Token::Ident("on".to_string())) {
            rest.drain(0..3);
            return Question::Payment(frequency);
        }
        return Question::FutureValue;
    }
    if noun != "interest" {
        return Question::FutureValue;
    }
    let tail = if rest.get(2) == Some(&Token::Ident("repayment".to_string())) {
        1
    } else {
        0
    };
    if rest.get(2 + tail) != Some(&Token::Ident("on".to_string())) {
        return Question::FutureValue;
    }
    rest.drain(0..3 + tail);
    Question::LoanInterest(frequency)
}

/// How often the interest is added.
///
/// A trailing `compounding quarterly` sets it; yearly otherwise.
fn compounding(rest: &mut Vec<Token>) -> f64 {
    let periods: [(&str, f64); 6] = [
        ("daily", 365.0),
        ("weekly", 52.0),
        ("monthly", 12.0),
        ("quarterly", 4.0),
        ("yearly", 1.0),
        ("annually", 1.0),
    ];
    if rest.len() <= 2 {
        return 1.0;
    }
    let Some(Token::Ident(name)) = rest.last() else {
        return 1.0;
    };
    let Some((_, count)) = periods.iter().find(|(period, _)| period == name) else {
        return 1.0;
    };
    let before = rest[rest.len() - 2].clone();
    if before != Token::Ident("compounding".to_string())
        && before != Token::Ident("compounded".to_string())
    {
        return 1.0;
    }
    rest.truncate(rest.len() - 2);
    *count
}

/// The principal, the term and the rate.
fn terms(tokens: &[Token], catalog: &dyn Catalog) -> Result<Option<Terms>, UnsupportedArea> {
    let normalized: Vec<Token> = tokens
        .iter()
        .map(|token| {
            if token == &Token::Ident("after".to_string())
                || token == &Token::Ident("over".to_string())
            {
                Token::Ident("for".to_string())
            } else {
                token.clone()
            }
        })
        .collect();
    let (principal, term, rate) = if let Some(parts) = operands_of(&normalized, &term_first()) {
        (parts[0].clone(), parts[1].clone(), parts[2].clone())
    } else if let Some(parts) = operands_of(&normalized, &rate_first()) {
        (parts[0].clone(), parts[2].clone(), parts[1].clone())
    } else {
        return Ok(None);
    };
    let Some(amount) = value_of(&principal, catalog)? else {
        return Ok(None);
    };
    if amount.is_percent {
        return Ok(None);
    }
    let Some(years) = years_of(&term, catalog) else {
        return Ok(None);
    };
    if years <= 0.0 || years > 1000.0 {
        return Ok(None);
    }
    let Some(percent) = scalar_of(&rate, catalog) else {
        return Ok(None);
    };
    Ok(Some(Terms {
        principal: amount,
        years,
        rate: percent / 100.0,
    }))
}

/// `for 3 years at 7%`.
fn term_first() -> Vec<Option<Token>> {
    vec![
        None,
        Some(word("for")),
        None,
        Some(word("at")),
        None,
        Some(percent()),
    ]
}

/// `at 7% for 3 years`.
fn rate_first() -> Vec<Option<Token>> {
    vec![
        None,
        Some(word("at")),
        None,
        Some(percent()),
        Some(word("for")),
        None,
    ]
}

/// The term in years.
fn years_of(tokens: &[Token], catalog: &dyn Catalog) -> Option<f64> {
    let duration = duration::read(tokens, catalog)?;
    let year = duration::year(catalog)?;
    Some(duration.years(&year))
}

/// The return on an investment: `500 invested 1000 returned`.
fn investment_return(
    tokens: &[Token],
    query: &str,
    catalog: &dyn Catalog,
) -> Result<Option<CalcResult>, UnsupportedArea> {
    if !contains(tokens, "invested") && !contains(tokens, "grew") {
        return Ok(None);
    }
    let mut rest = tokens.to_vec();
    for suffix in [
        vec![word("as"), word("a"), percent()],
        vec![word("as"), percent()],
    ] {
        if rest.len() > suffix.len() && rest[rest.len() - suffix.len()..] == suffix[..] {
            rest.truncate(rest.len() - suffix.len());
        }
    }
    if let Some(parts) = operands_of(
        &rest,
        &[None, Some(word("invested")), None, Some(word("returned"))],
    ) {
        let Some(gain) = ratio(&parts[1], &parts[0], catalog) else {
            return Ok(None);
        };
        return Ok(percent_card((gain - 1.0) * 100.0, query, "Return"));
    }
    let periods_per_year: [(&str, f64); 7] = [
        ("annual", 1.0),
        ("annualized", 1.0),
        ("yearly", 1.0),
        ("quarterly", 4.0),
        ("monthly", 12.0),
        ("weekly", 52.0),
        ("daily", 365.0),
    ];
    if rest.len() <= 3 {
        return Ok(None);
    }
    let Some(Token::Ident(lead)) = rest.first() else {
        return Ok(None);
    };
    let Some((_, periods)) = periods_per_year.iter().find(|(name, _)| name == lead) else {
        return Ok(None);
    };
    if rest[1] != word("return") || rest[2] != word("on") {
        return Ok(None);
    }
    let normalized: Vec<Token> = rest[3..]
        .iter()
        .map(|token| {
            if token == &word("over") {
                word("after")
            } else {
                token.clone()
            }
        })
        .collect();
    let parts = operands_of(
        &normalized,
        &[
            None,
            Some(word("invested")),
            None,
            Some(word("returned")),
            Some(word("after")),
            None,
        ],
    )
    .or_else(|| {
        operands_of(
            &normalized,
            &[
                None,
                Some(word("that")),
                Some(word("grew")),
                Some(word("to")),
                None,
                Some(word("after")),
                None,
            ],
        )
    });
    let Some(parts) = parts else {
        return Ok(None);
    };
    let Some(gain) = ratio(&parts[1], &parts[0], catalog) else {
        return Ok(None);
    };
    if gain <= 0.0 {
        return Ok(None);
    }
    let Some(years) = years_of(&parts[2], catalog) else {
        return Ok(None);
    };
    if years <= 0.0 {
        return Ok(None);
    }
    let rate = gain.powf(1.0 / (years * periods)) - 1.0;
    Ok(percent_card(rate * 100.0, query, "Return"))
}

/// The capital whose yield pays a rate: `investment required for $50,000/year at 4%`.
fn investment_required(
    tokens: &[Token],
    query: &str,
    catalog: &dyn Catalog,
) -> Result<Option<CalcResult>, UnsupportedArea> {
    let Some(parts) = operands_of(
        tokens,
        &[
            Some(word("investment")),
            Some(word("required")),
            Some(word("for")),
            None,
            Some(word("at")),
            None,
            Some(percent()),
        ],
    ) else {
        return Ok(None);
    };
    let Some(rate) = scalar_of(&parts[1], catalog) else {
        return Ok(None);
    };
    if rate <= 0.0 {
        return Ok(None);
    }
    let Some(mut yearly) = yearly_amount(&parts[0], catalog)? else {
        return Ok(None);
    };
    if yearly.is_percent {
        return Ok(None);
    }
    yearly.amount /= rate / 100.0;
    if !yearly.amount.is_finite() {
        return Ok(None);
    }
    Ok(Some(CalcResult::phrase(
        query,
        amount_payload(yearly),
        "Investment",
    )))
}

/// The amount a year, as `investment required for $50,000/year at 4%` writes it.
///
/// Upstream divides the money by the year and multiplies it back by one, which
/// cancels; the year is read here and dropped, and the money is kept.
fn yearly_amount(
    tokens: &[Token],
    catalog: &dyn Catalog,
) -> Result<Option<Value>, UnsupportedArea> {
    let mut amount_tokens = tokens;
    if tokens.len() >= 3 {
        let split = tokens.len() - 2;
        if tokens[split] == Token::Op(Operator::Divide) {
            if let Token::Ident(name) = &tokens[split + 1] {
                if duration::time_unit(name, catalog).is_some() {
                    amount_tokens = &tokens[..split];
                }
            }
        }
    }
    value_of(amount_tokens, catalog)
}

/// One amount as a multiple of another.
fn ratio(numerator: &[Token], denominator: &[Token], catalog: &dyn Catalog) -> Option<f64> {
    let numerator = value_of(numerator, catalog).ok().flatten();
    let denominator = value_of(denominator, catalog).ok().flatten();
    let (Some(numerator), Some(denominator)) = (numerator, denominator) else {
        return None;
    };
    if denominator.amount == 0.0 {
        return None;
    }
    Some(numerator.amount / denominator.amount)
}

/// The card of a percentage answer.
fn percent_card(value: f64, query: &str, badge: &str) -> Option<CalcResult> {
    if !value.is_finite() {
        return None;
    }
    Some(CalcResult::phrase(query, Payload::percentage(value), badge))
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
        Ok(value) => Some(value.effective()),
        Err(_) => None,
    }
}

/// Whether the tokens hold a word.
fn contains(tokens: &[Token], word_text: &str) -> bool {
    tokens
        .iter()
        .any(|token| token == &Token::Ident(word_text.to_string()))
}

/// Whether the tokens start with these words.
fn starts_with(tokens: &[Token], lead: &[&str]) -> bool {
    if tokens.len() < lead.len() {
        return false;
    }
    lead.iter()
        .enumerate()
        .all(|(index, text)| tokens[index] == Token::Ident((*text).to_string()))
}

/// A word token.
fn word(text: &str) -> Token {
    Token::Ident(text.to_string())
}

/// The `%` token.
fn percent() -> Token {
    Token::Op(Operator::Percent)
}
