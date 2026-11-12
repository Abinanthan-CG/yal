//! The tests of the shared amounts.
//!
//! Every expectation in this file is read out of upstream Tinycast's own sources:
//! `upstream/Tinycast/Features/Calculator/Model/CalcSplit.swift`,
//! `upstream/docs/features/calculator.md` and `upstream/Tests/calc-test.swift`.

use crate::catalog::NameCatalog;
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

/// The text that lands on the pasteboard.
fn copy_text(query: &str) -> Option<String> {
    match outcome(query) {
        CalcOutcome::Card(result) => match result.payload {
            Payload::Value { copy_text, .. } => Some(copy_text),
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
fn an_amount_shared_between_four_is_a_quarter_of_it() {
    assert_eq!(display("Split 120 between 4").as_deref(), Some("30"));
    assert_eq!(
        target_badge("Split 120 between 4").as_deref(),
        Some("Per Person")
    );
}

#[test]
fn the_split_word_can_come_first() {
    assert_eq!(display("split 120 between 4").as_deref(), Some("30"));
}

#[test]
fn the_noun_of_the_people_can_be_left_out() {
    assert_eq!(display("120 split 4").as_deref(), Some("30"));
    assert_eq!(display("120 split 4 people").as_deref(), Some("30"));
    assert_eq!(display("120 split among 4").as_deref(), Some("30"));
}

#[test]
fn a_divide_by_a_people_is_the_same_share() {
    assert_eq!(display("120 / 4 people").as_deref(), Some("30"));
}

#[test]
fn a_share_of_money_keeps_the_money() {
    assert_eq!(display("$120 split 4").as_deref(), Some("30.00 USD"));
}

#[test]
fn the_currency_can_be_named_before_the_amount() {
    assert_eq!(
        display("Eur 99 among 3 friends").as_deref(),
        Some("33.00 EUR")
    );
}

#[test]
fn a_calculation_is_shared_out_after_it_is_calculated() {
    // `85 + 20%` is 102, and a third of that is 34.
    assert_eq!(display("85 + 20% split 3").as_deref(), Some("34"));
}

#[test]
fn a_tip_is_shared_out_as_it_is() {
    // The tip on 85 is 17, and a third of that is 5.666666667.
    assert_eq!(
        display("20% tip on 85 split 3").as_deref(),
        Some("5.666666667")
    );
    assert_eq!(
        copy_text("20% tip on 85 split 3").as_deref(),
        Some("5.666666667")
    );
}

#[test]
fn a_query_that_does_not_mention_a_split_is_left_alone() {
    let catalog = NameCatalog::new();
    assert!(matches!(
        evaluate("120 + 4", &catalog),
        CalcOutcome::Card(_)
    ));
    assert!(matches!(evaluate("120 4", &catalog), CalcOutcome::NoCard));
}

#[test]
fn a_headcount_of_one_is_a_share_of_itself() {
    assert_eq!(display("120 split 1").as_deref(), Some("120"));
}
