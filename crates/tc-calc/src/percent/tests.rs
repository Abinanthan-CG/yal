//! The tests of the percentage phrasings.
//!
//! Every expectation in this file is read out of upstream Tinycast's own test suite,
//! `upstream/Tests/calc-test.swift`, or out of `upstream/docs/features/calculator.md`.
//! The test names name the upstream behaviour they pin.

use crate::catalog::NameCatalog;
use crate::engine::{evaluate, CalcOutcome};
use crate::result::{CalcResult, Payload};

/// The outcome of a query, with the empty catalog upstream's own tests run with.
fn outcome(query: &str) -> CalcOutcome {
    let catalog = NameCatalog::new();
    evaluate(query, &catalog)
}

/// The card of a query.
fn card(query: &str) -> CalcResult {
    match outcome(query) {
        CalcOutcome::Card(result) => result,
        other => panic!("query {query:?} answered {other:?}"),
    }
}

/// Asserts the text a card shows.
fn assert_display(query: &str, expected: &str) {
    let result = card(query);
    match result.payload {
        Payload::Value { display, .. } => assert_eq!(display, expected, "query {query:?}"),
        Payload::Error { message } => panic!("query {query:?} answered the error {message:?}"),
    }
}

/// Asserts the text that lands on the pasteboard.
fn assert_copy(query: &str, expected: &str) {
    let result = card(query);
    match result.payload {
        Payload::Value { copy_text, .. } => assert_eq!(copy_text, expected, "query {query:?}"),
        Payload::Error { message } => panic!("query {query:?} answered the error {message:?}"),
    }
}

/// Asserts the two word-name pills of a card.
fn assert_badges(query: &str, source: &str, target: &str) {
    let result = card(query);
    assert_eq!(
        result.source_badge.as_deref(),
        Some(source),
        "query {query:?} source badge"
    );
    assert_eq!(
        result.target_badge.as_deref(),
        Some(target),
        "query {query:?} target badge"
    );
}

#[test]
fn a_discount_off_an_amount_is_the_amount_less_the_percentage() {
    assert_display("20% off 500", "400");
    assert_badges("20% off 500", "Expression", "Discounted");
}

#[test]
fn a_discount_off_of_an_amount_reads_the_same() {
    assert_display("20% off of 100", "80");
}

#[test]
fn take_a_percentage_off_an_amount_is_a_discount() {
    assert_display("take 20% off 100", "80");
    assert_badges("take 20% off 100", "Expression", "Discounted");
}

#[test]
fn a_tip_is_the_tip_alone() {
    assert_display("15% tip on 42", "6.3");
    assert_display("20% tip of 80", "16");
    assert_badges("15% tip on 42", "Expression", "Tip");
}

#[test]
fn a_markup_on_an_amount_is_the_amount_plus_the_percentage() {
    assert_display("15% on 80", "92");
    assert_badges("15% on 80", "Expression", "Increased");
}

#[test]
fn a_percentage_from_an_amount_is_the_share_of_it() {
    assert_display("20% from 100", "20");
    assert_badges("20% from 100", "Expression", "Result");
}

#[test]
fn a_word_between_the_percentage_and_the_relation_only_names_it() {
    assert_display("20% discount off 100", "80");
    assert_display("20% markup on 50", "60");
}

#[test]
fn a_tax_on_an_amount_is_added_to_it() {
    assert_display("10% tax on 100", "110");
}

#[test]
fn a_tax_of_an_amount_is_the_share_of_it() {
    assert_display("10% tax of 110", "11");
}

#[test]
fn a_tax_in_a_price_is_already_inside_it() {
    assert_display("10% tax in 110", "10");
    assert_display("20% vat in 120", "20");
    assert_display("10% gst in 110", "10");
    assert_badges("20% vat in 120", "Expression", "Tax");
}

#[test]
fn sales_tax_is_a_tax() {
    assert_display("10% sales tax on 100", "110");
}

#[test]
fn a_query_that_ends_in_a_tax_is_a_total() {
    assert_display("100 + 10% tax", "110");
    assert_badges("100 + 10% tax", "Expression", "Total");
}

#[test]
fn thirty_is_twenty_percent_of_what_solves_for_the_whole() {
    assert_display("30 is 20% of what", "150");
    assert_badges("30 is 20% of what", "Expression", "Total");
}

#[test]
fn twenty_percent_of_what_is_thirty_solves_for_the_whole() {
    assert_display("20% of what is 30", "150");
}

#[test]
fn the_off_and_on_forms_solve_for_the_whole_too() {
    assert_display("20% off what is 30", "37.5");
    assert_display("20% on what is 30", "25");
    assert_display("90 is 10% off what", "100");
}

#[test]
fn an_amount_as_a_percentage_of_another_is_a_share() {
    assert_display("50 as % of 200", "25%");
    assert_display("30 as a % of 120", "25%");
    assert_display("50 is what % of 200", "25%");
    assert_display("what % is 25 of 100", "25%");
    assert_display("10 is % of 20", "50%");
    assert_display("10 is what % 20", "50%");
    assert_display("10 as % 20", "50%");
    assert_display("4 out of 10", "40%");
}

#[test]
fn a_share_of_the_two_together_is_a_third() {
    assert_display("10 is what % with 20", "33.33333333%");
    assert_copy("10 is what % with 20", "33.33333333%");
}

#[test]
fn a_discount_reads_as_the_change_it_makes() {
    assert_display("10 as discount off 20", "50%");
    assert_display("45 is what % off 50", "10%");
}

#[test]
fn a_markup_reads_as_the_change_it_makes() {
    assert_display("66 is what % on 60", "10%");
}

#[test]
fn a_change_from_one_amount_to_another_is_a_percentage() {
    assert_display("% change from 10 to 20", "100%");
    assert_display("% increase from 10 to 20", "100%");
    assert_display("% decrease from 20 to 10", "-50%");
    assert_display("what % change is 10 to 20", "100%");
}

#[test]
fn a_change_between_two_amounts_is_a_percentage() {
    assert_display("80 to 100 is what %", "25%");
    assert_display("80 to 60 as %", "-25%");
    assert_badges("80 to 100 is what %", "Expression", "Percent Change");
}

#[test]
fn the_growth_that_reached_a_total_is_a_percentage() {
    assert_display("15 as % after adding 3", "25%");
    assert_display("15 is what % after +3", "25%");
}

#[test]
fn a_fraction_read_as_a_percentage() {
    assert_display("0.125 as %", "12.5%");
    assert_display("3/8 as percent", "37.5%");
    assert_display("1/8 is what %", "12.5%");
}

#[test]
fn the_rule_of_three_with_an_amount_asked_for() {
    assert_display("if 30 is 20%, what is 50%", "75");
    assert_badges("if 30 is 20%, what is 50%", "Expression", "Proportion");
}

#[test]
fn the_rule_of_three_with_a_percentage_asked_for() {
    assert_display("if 30 is 20% what is 60", "40%");
    assert_display("if 20% is 30, what is 50%", "75");
}

#[test]
fn a_ratio_is_reduced() {
    assert_display("ratio of 3 to 5", "3 : 5");
    assert_display("ratio of 4 to 6", "2 : 3");
    assert_display("ratio of 1920 to 1080", "16 : 9");
    assert_badges("ratio of 3 to 5", "Expression", "Ratio");
}

#[test]
fn a_ratio_does_not_chain_into_the_next_calculation() {
    let result = card("ratio of 3 to 5");
    assert!(!result.can_chain);
}

#[test]
fn a_plain_percentage_is_left_to_the_arithmetic() {
    assert_display("20% of 450", "90");
    assert_display("450 + 20%", "540");
    assert_display("20%", "0.2");
}

#[test]
fn a_percentage_that_is_not_a_phrase_earns_no_card() {
    assert!(matches!(outcome("50 as % of"), CalcOutcome::NoCard));
}
