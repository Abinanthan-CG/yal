//! The tests of the money phrases about growth, loans and returns.
//!
//! Every expectation in this file is read out of upstream Tinycast's own test
//! suite, `upstream/Tests/calc-test.swift`, and out of
//! `upstream/docs/features/calculator.md`.

use crate::engine::{evaluate, CalcOutcome};
use crate::result::Payload;
use crate::FullCatalog;

/// The outcome of a query, with the catalog that knows the units and the money.
fn outcome(query: &str) -> CalcOutcome {
    evaluate(query, &FullCatalog)
}

/// The text a card shows.
fn display(query: &str) -> Option<String> {
    match outcome(query) {
        CalcOutcome::Card(result) => match result.payload {
            Payload::Value { display, .. } => Some(display),
            Payload::Error { message } => Some(message),
        },
        _ => None,
    }
}

/// The badge a card shows on its right.
fn target_badge(query: &str) -> Option<String> {
    match outcome(query) {
        CalcOutcome::Card(result) => result.target_badge,
        _ => None,
    }
}

#[test]
fn an_amount_after_a_term_at_a_rate_is_worth_more() {
    assert_eq!(
        display("$2,000 after 5 years at 4%").as_deref(),
        Some("2,433.31 USD")
    );
    assert_eq!(
        target_badge("$2,000 after 5 years at 4%").as_deref(),
        Some("Future Value")
    );
}

#[test]
fn the_term_and_the_rate_come_in_either_order() {
    assert_eq!(
        display("$2,000 at 4% for 5 years compounding quarterly").as_deref(),
        Some("2,440.38 USD")
    );
}

#[test]
fn a_term_may_be_written_in_months() {
    assert_eq!(
        display("$2,000 for 18 months at 4%").as_deref(),
        Some("2,121.19 USD")
    );
}

#[test]
fn the_interest_is_what_the_amount_earned() {
    assert_eq!(
        display("interest on $2,000 after 5 years at 4%").as_deref(),
        Some("433.31 USD")
    );
    assert_eq!(
        target_badge("interest on $2,000 after 5 years at 4%").as_deref(),
        Some("Interest")
    );
}

#[test]
fn a_payment_is_what_has_to_be_paid() {
    assert_eq!(
        display("monthly payment on $20,000 over 5 years at 6%").as_deref(),
        Some("386.66 USD")
    );
    assert_eq!(
        target_badge("monthly payment on $20,000 over 5 years at 6%").as_deref(),
        Some("Monthly Payment")
    );
}

#[test]
fn the_total_payment_is_all_of_them() {
    assert_eq!(
        display("total payment on $20,000 over 5 years at 6%").as_deref(),
        Some("23,199.36 USD")
    );
    assert_eq!(
        target_badge("total payment on $20,000 over 5 years at 6%").as_deref(),
        Some("Total Payment")
    );
}

#[test]
fn a_loan_without_interest_is_the_principal_divided() {
    assert_eq!(
        display("monthly payment on 12000 over 1 year at 0%").as_deref(),
        Some("1,000")
    );
}

#[test]
fn the_interest_of_a_loan_is_what_the_payments_add() {
    assert_eq!(
        display("total interest on 10000 for 5 years at 6%").as_deref(),
        Some("1,599.680918")
    );
    assert_eq!(
        display("monthly interest on 10000 for 5 years at 6%").as_deref(),
        Some("26.66134863")
    );
    assert_eq!(
        target_badge("monthly interest on 10000 for 5 years at 6%").as_deref(),
        Some("Monthly Interest")
    );
}

#[test]
fn the_present_value_is_what_the_amount_is_worth_now() {
    assert_eq!(
        display("present value of $1000 after 5 years at 5%").as_deref(),
        Some("783.53 USD")
    );
    assert_eq!(
        target_badge("present value of $1000 after 5 years at 5%").as_deref(),
        Some("Present Value")
    );
}

#[test]
fn an_investment_that_returned_is_a_percentage() {
    assert_eq!(
        display("500 invested 1000 returned").as_deref(),
        Some("100%")
    );
    assert_eq!(
        target_badge("500 invested 1000 returned").as_deref(),
        Some("Return")
    );
}

#[test]
fn the_return_can_be_a_yearly_one() {
    assert_eq!(
        display("annual return on 500 invested 1000 returned after 5 years").as_deref(),
        Some("14.8698355%")
    );
    assert_eq!(
        display("annual return on 500 that grew to 1000 over 5 years").as_deref(),
        Some("14.8698355%")
    );
}

#[test]
fn the_capital_an_investment_needs_is_the_income_over_the_rate() {
    assert_eq!(
        display("investment required for $50000/year at 4%").as_deref(),
        Some("1,250,000.00 USD")
    );
    assert_eq!(
        target_badge("investment required for $50000/year at 4%").as_deref(),
        Some("Investment")
    );
}

#[test]
fn a_phrase_without_a_rate_is_not_a_loan() {
    assert!(matches!(outcome("2000 after 5 years"), CalcOutcome::NoCard));
}
