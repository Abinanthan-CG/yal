//! The tests of the reductions over a spoken list and of the math asked in words.
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
fn a_list_is_reduced_by_the_statistic_asked_for() {
    assert_eq!(display("median of 3, 1, 2").as_deref(), Some("2"));
    assert_eq!(target_badge("median of 3, 1, 2").as_deref(), Some("Median"));
    assert_eq!(
        display("count of 4, 8, 15, 16, 23, 42").as_deref(),
        Some("6")
    );
    assert_eq!(
        target_badge("count of 4, 8, 15, 16, 23, 42").as_deref(),
        Some("Count")
    );
}

#[test]
fn the_standard_deviation_is_the_sample_one() {
    assert_eq!(
        display("standard deviation of 2, 4, 4, 4, 5, 5, 7, 9").as_deref(),
        Some("2.138089935")
    );
}

#[test]
fn a_list_may_be_separated_by_and() {
    assert_eq!(display("sum of 10 and 20").as_deref(), Some("30"));
    assert_eq!(display("larger of 3 and 7").as_deref(), Some("7"));
    assert_eq!(
        target_badge("larger of 3 and 7").as_deref(),
        Some("Maximum")
    );
}

#[test]
fn a_midpoint_is_the_average_of_the_two() {
    assert_eq!(display("midpoint between 10 and 30").as_deref(), Some("20"));
}

#[test]
fn the_greatest_common_divisor_and_the_least_common_multiple_are_told_apart() {
    assert_eq!(display("gcd of 12 and 18").as_deref(), Some("6"));
    assert_eq!(display("gcf of 20 and 30").as_deref(), Some("10"));
    assert_eq!(
        display("lowest common multiple of 4 and 6").as_deref(),
        Some("12")
    );
}

#[test]
fn a_whole_number_is_told_whether_it_is_prime() {
    assert_eq!(display("is 97 prime").as_deref(), Some("true"));
    assert_eq!(display("is 91 a prime number").as_deref(), Some("false"));
    assert_eq!(target_badge("is 97 prime").as_deref(), Some("Prime Number"));
}

#[test]
fn a_prime_check_works_for_the_whole_range() {
    // Upstream: "Prime checks work for every whole number up to
    // 9,007,199,254,740,992 and answer instantly."
    assert_eq!(
        display("is 9007199254740992 prime").as_deref(),
        Some("false")
    );
    assert_eq!(display("is 2147483647 prime").as_deref(), Some("true"));
    assert_eq!(display("is 2147483649 prime").as_deref(), Some("false"));
    assert_eq!(display("is 104729 prime").as_deref(), Some("true"));
}

#[test]
fn a_remainder_is_what_is_left_of_a_division() {
    assert_eq!(
        display("remainder of 20 divided by 3").as_deref(),
        Some("2")
    );
    assert_eq!(
        target_badge("remainder of 20 divided by 3").as_deref(),
        Some("Remainder")
    );
}

#[test]
fn a_value_is_held_between_two_bounds() {
    assert_eq!(display("clamp 20 between 5 and 15").as_deref(), Some("15"));
    assert_eq!(
        target_badge("clamp 20 between 5 and 15").as_deref(),
        Some("Clamped")
    );
}

#[test]
fn a_proportion_is_solved_for_the_missing_term() {
    assert_eq!(
        display("6 is to 600 as 8 is to what").as_deref(),
        Some("800")
    );
    assert_eq!(
        target_badge("6 is to 600 as 8 is to what").as_deref(),
        Some("Proportion")
    );
}

#[test]
fn the_combinations_and_the_permutations_are_told_apart() {
    assert_eq!(display("5 choose 3").as_deref(), Some("10"));
    assert_eq!(target_badge("5 choose 3").as_deref(), Some("Combinations"));
    assert_eq!(display("5 permutation 3").as_deref(), Some("60"));
    assert_eq!(
        target_badge("5 permutation 3").as_deref(),
        Some("Permutations")
    );
}

#[test]
fn a_logarithm_is_answered_in_both_orders() {
    assert_eq!(display("logarithm 81 base 3").as_deref(), Some("4"));
    assert_eq!(
        display("1000 is 10 to the power of what").as_deref(),
        Some("3")
    );
    assert_eq!(
        target_badge("logarithm 81 base 3").as_deref(),
        Some("Logarithm")
    );
}

#[test]
fn a_root_is_answered_in_the_degree_first_order() {
    assert_eq!(display("5 root of 32").as_deref(), Some("2"));
    assert_eq!(target_badge("5 root of 32").as_deref(), Some("Root"));
}

#[test]
fn a_multiplier_is_written_with_an_x() {
    assert_eq!(display("5 as multiplier of 1").as_deref(), Some("5x"));
    assert_eq!(display("10 to 20 as multiplier").as_deref(), Some("2x"));
    assert_eq!(
        target_badge("5 as multiplier of 1").as_deref(),
        Some("Multiplier")
    );
}

#[test]
fn a_prime_answer_does_not_chain_into_the_next_calculation() {
    let result = match outcome("is 97 prime") {
        CalcOutcome::Card(result) => result,
        other => panic!("the query answered {other:?}"),
    };
    assert!(!result.can_chain);
}
