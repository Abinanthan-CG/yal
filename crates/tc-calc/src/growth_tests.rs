//! The tests of the phrases about a steady rate.
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
fn an_amount_after_a_term_at_a_rate_every_period_compounds() {
    assert_eq!(
        display("20 after 5 years at 5% every year").as_deref(),
        Some("25.52563125")
    );
    assert_eq!(
        target_badge("20 after 5 years at 5% every year").as_deref(),
        Some("Future Value")
    );
}

#[test]
fn a_decay_shrinks_the_amount() {
    assert_eq!(
        display("20 after 5 years at 5% decay every year").as_deref(),
        Some("15.47561875")
    );
}

#[test]
fn the_period_may_be_longer_than_a_year() {
    assert_eq!(
        display("20 after 10 years at 5% every 2 years").as_deref(),
        Some("25.52563125")
    );
}

#[test]
fn the_time_it_takes_to_reach_a_value_is_a_duration() {
    assert_eq!(
        display("time from 20 to 30 at 5% every year").as_deref(),
        Some("8.310386223 yr")
    );
    assert_eq!(
        target_badge("time from 20 to 30 at 5% every year").as_deref(),
        Some("Duration")
    );
}

#[test]
fn a_steady_amount_each_step_is_a_duration_too() {
    assert_eq!(
        display("time from 20 to 30 at 2 every day").as_deref(),
        Some("5 day")
    );
}

#[test]
fn the_rate_that_explains_a_change_is_a_percentage() {
    assert_eq!(
        display("growth every year from 20 to 30 over 5 years").as_deref(),
        Some("8.44717712%")
    );
    assert_eq!(
        target_badge("growth every year from 20 to 30 over 5 years").as_deref(),
        Some("Growth Rate")
    );
}

#[test]
fn a_phrase_without_a_period_is_not_a_steady_rate() {
    assert!(matches!(outcome("20 after 5 years"), CalcOutcome::NoCard));
}

#[test]
fn the_same_amount_compounds_the_same_way_with_or_without_a_period() {
    // Upstream's finance phrases answer `20 after 5 years at 5%` on their own, and
    // the growth phrases answer the same amount when the period is named.
    assert_eq!(
        display("20 after 5 years at 5%").as_deref(),
        Some("25.52563125")
    );
}
