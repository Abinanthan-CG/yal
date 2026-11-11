//! The tests of the calculator core.
//!
//! Every expectation in this file is read out of upstream Tinycast's own test suite,
//! `upstream/Tests/calc-test.swift`, or out of the upstream source the behaviour comes
//! from. The test names name the upstream behaviour they pin.

use crate::catalog::NameCatalog;
use crate::engine::{evaluate, CalcOutcome};
use crate::formatter::Formatter;
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

/// Asserts the expression a card echoes.
fn assert_expression(query: &str, expected: &str) {
    assert_eq!(card(query).expression, expected, "query {query:?}");
}

/// Asserts that a query earns no card at all, as upstream's `expectNil`.
fn assert_no_card(query: &str) {
    assert!(
        matches!(outcome(query), CalcOutcome::NoCard),
        "query {query:?} should earn no card"
    );
}

// MARK: - Arithmetic

#[test]
fn upstream_multiplies_and_divides() {
    assert_display("2+2", "4");
    assert_display("5*7", "35");
    assert_display("100/4", "25");
    assert_display("2.5 * 4", "10");
    assert_display("1,000 + 234", "1,234");
}

#[test]
fn upstream_power_is_right_associative() {
    // Upstream: `2^3^2` is "512", right-associative.
    assert_display("2^10", "1,024");
    assert_display("2^3^2", "512");
    // Upstream: `2**3**2` is "512" too, the same as "^".
    assert_display("2**2", "4");
    assert_display("2**10", "1,024");
    assert_display("2**3**2", "512");
}

#[test]
fn upstream_multiply_binds_tighter_than_add() {
    assert_display("(5+2)*3", "21");
    // Upstream: `2(3+1)+1` is "9", an implicit "*" binds like an explicit one.
    assert_display("4(2+3)", "20");
    assert_display("(2+3)(2+3)", "25");
    assert_display("2(3+1)+1", "9");
    assert_display("6/2x(1+2)", "9");
}

#[test]
fn upstream_unary_minus_binds_looser_than_power() {
    assert_display("-5+3", "-2");
    // Upstream: `-2^2` is "-4", the sign binds looser than "^".
    assert_display("-2^2", "-4");
}

#[test]
fn upstream_division_keeps_its_fraction() {
    assert_display("10/4", "2.5");
    assert_display("1/3", "0.3333333333");
}

#[test]
fn upstream_x_is_a_multiplication() {
    assert_display("3x3", "9");
    assert_display("3 x 3", "9");
    assert_display("3X3", "9");
    assert_display("3 x -2", "-6");
    assert_display("2xpi", "6.283185307");
}

#[test]
fn upstream_shift_binds_looser_than_multiply() {
    // Upstream: `2 * (3 + 4) << 1` is "28".
    assert_display("2 * (3 + 4) << 1", "28");
    assert_display("1 << 8", "256");
    assert_display("256 >> 2", "64");
}

#[test]
fn upstream_bitwise_operations() {
    assert_display("6 & 3", "2");
    assert_display("5 xor 3", "6");
    assert_display("5 XOR 3", "6");
    assert_display("~1", "-2");
    assert_display("2 ⊻ 3", "1");
}

#[test]
fn upstream_mod_is_remainder_with_the_dividend_sign() {
    assert_display("10 mod 3", "1");
    assert_display("17 mod 5", "2");
    // Upstream: `-10 mod 3` is "-1", the sign follows the dividend.
    assert_display("-10 mod 3", "-1");
    // Upstream: `2 + 10 mod 3` is "3", the same precedence as "*" and "/".
    assert_display("2 + 10 mod 3", "3");
}

#[test]
fn upstream_factorial_is_a_postfix() {
    assert_display("5!", "120");
    // Upstream: `3!!` is "720", the chained postfix.
    assert_display("3!!", "720");
}

#[test]
fn upstream_comparisons_answer_a_boolean() {
    assert_display("1 ≤ 2", "true");
    assert_display("2 ≠ 3", "true");
    assert_display("1 | 2 == 3", "true");
    assert_display("~1 == -2", "true");
    assert_display("5 mod 2 == 1", "true");
}

#[test]
fn upstream_magnitude_words_reach_trillion() {
    // Upstream: `2 trillion` is "2,000,000,000,000" and `1.5 quadrillion` copies
    // as "1500000000000000".
    assert_display("2 trillion", "2,000,000,000,000");
    assert_copy("1.5 quadrillion", "1500000000000000");
}

#[test]
fn upstream_the_operators_can_be_written_out() {
    assert_display("10 divided by 2", "5");
    assert_display("10 plus 5", "15");
    assert_display("10 minus 3", "7");
    assert_display("6 times 7", "42");
}

#[test]
fn upstream_powers_can_be_written_out() {
    assert_display("2 to the power of 10", "1,024");
    assert_display("3 raised to 4", "81");
    assert_display("4 squared", "16");
    assert_display("3 cubed + 1", "28");
    assert_display("root 4 of 81", "3");
    assert_display("log 81 base 3", "4");
}

#[test]
fn upstream_the_whole_function_list() {
    assert_display("lcm(4,6)", "12");
    assert_display("pow(2,10)", "1,024");
    assert_display("cbrt(27)", "3");
    assert_display("exp(1)", "2.718281828");
    assert_display("log2(1024)", "10");
    assert_display("sign(-5)", "-1");
    assert_display("trunc(3.7)", "3");
    assert_display("asin(1)", "1.570796327");
    assert_display("sinh(1)", "1.175201194");
    assert_display("tanh(0)", "0");
    assert_display("median(1, 5, 2, 8)", "3.5");
}

#[test]
fn upstream_rounding_words() {
    assert_display("pi to 2 decimal places", "3.14");
    assert_display("3.3 to nearest quarter", "3.25");
}

// MARK: - Number formatting

#[test]
fn upstream_compact_suffix_scales_by_a_thousand() {
    assert_display("10k", "10,000");
    assert_copy("10k", "10000");
    assert_display("2.5K", "2,500");
    assert_display("10k + 500", "10,500");
    assert_display("10k * 2", "20,000");
    assert_badges("10k", "Expression", "Result");
}

#[test]
fn upstream_magnitude_words_scale_the_literal() {
    assert_display("13 million", "13,000,000");
    assert_copy("1.5 billion", "1500000000");
    assert_display("2 Thousand + 1", "2,001");
    assert_display("3million / 2", "1,500,000");
    assert_badges("13 million", "Expression", "Result");
}

#[test]
fn upstream_exponent_notation() {
    assert_display("1e6 + 1", "1,000,001");
    assert_display("1.5e-3 * 2", "0.003");
    assert_display("2.5e8 / 2", "125,000,000");
    assert_display("1E6 + 1", "1,000,001");
    // Upstream: a lone shorthand literal cards like "10k".
    assert_display("1e6", "1,000,000");
    assert_display("1e3k + 1", "1,000,001");
    assert_display("1e5", "100,000");
    assert_display("2e10", "20,000,000,000");
    assert_display("1E5", "100,000");
    assert_display("1.5e3", "1,500");
    assert_display("3e+2", "300");
    assert_display("2e10/2", "10,000,000,000");
}

#[test]
fn upstream_integers_are_exact_up_to_two_to_the_fifty_third() {
    assert_display("2^49", "562,949,953,421,312");
    assert_display("2^50", "1,125,899,906,842,624");
    assert_copy("2^50", "1125899906842624");
    // Upstream: "999999999999999 + 1" is exactly the old cutoff.
    assert_display("999999999999999 + 1", "1,000,000,000,000,000");
    // Upstream: past 2^53 the precision is gone, so exponent form is the honest answer.
    assert_display("123456789 * 123456789", "1.524157875e+16");
}

#[test]
fn upstream_copy_text_keeps_the_exponent_form() {
    assert_copy("1e-5", "1e-05");
    assert_display("1e305k", "1e+308");
}

#[test]
fn upstream_copy_text_normalizes_minus_zero() {
    assert_copy("-0 * 1234", "0");
}

#[test]
fn upstream_copy_text_keeps_the_exact_integer() {
    assert_copy("-9007199254740992 + 0", "-9007199254740992");
    assert_copy("1234567*1", "1234567");
}

#[test]
fn upstream_grouping_and_signs() {
    assert_display("1234567*1", "1,234,567");
    assert_display("-1234.5-0.25", "-1,234.75");
    // Upstream: a detached `e` is the constant, not an exponent.
    assert_display("1 e", "2.718281828");
}

// MARK: - The math functions

#[test]
fn upstream_square_root() {
    assert_display("sqrt(64)", "8");
    assert_display("sqrt 64", "8");
    // Upstream: a bare argument is one operand, so `sqrt 64 + 36` is "44".
    assert_display("sqrt 64 + 36", "44");
    assert_display("SQRT(64)", "8");
    assert_display("2sqrt(9)", "6");
}

#[test]
fn upstream_logarithms() {
    assert_display("log(1000)", "3");
    assert_display("ln(e)", "1");
    assert_display("log(8,2)", "3");
}

#[test]
fn upstream_rounding_functions() {
    assert_display("abs(-4)", "4");
    assert_display("floor(2.7)", "2");
    assert_display("ceil(2.1)", "3");
    assert_display("round(2.5)", "3");
    assert_display("round(3.14159,2)", "3.14");
    assert_display("round(1234,-2)", "1,200");
}

#[test]
fn upstream_constants_multiply_implicitly() {
    assert_display("2*pi", "6.283185307");
    assert_display("π*2", "6.283185307");
    assert_display("2pi", "6.283185307");
    assert_display("2π", "6.283185307");
    assert_display("e^2", "7.389056099");
    // Upstream: `10π ^e` is "224.5915772", a constant binds looser than "^".
    assert_display("10π ^e", "224.5915772");
}

#[test]
fn upstream_functions_with_several_arguments() {
    assert_display("hypot(3,4)", "5");
    assert_display("2hypot(3,4)", "10");
    assert_display("gcd(12,18,8)", "2");
    assert_display("atan2(1,1)*4", "3.141592654");
    // Upstream: `root(-8,3)` is "-2", an odd root keeps the sign.
    assert_display("root(-8,3)", "-2");
}

// MARK: - Number bases

#[test]
fn upstream_base_conversion() {
    assert_display("255 to hex", "0xFF");
    assert_display("255 to octal", "0o377");
    assert_display("255 to binary", "0b11111111");
    assert_display("0xff to decimal", "255");
    assert_display("0b1010 to decimal", "10");
    assert_badges("255 to hex", "Decimal", "Hexadecimal");
    assert_badges("0xff to decimal", "Hexadecimal", "Decimal");
}

#[test]
fn upstream_bare_radix_literal_echoes_decimal() {
    assert_display("0xff", "255");
    assert_badges("0xff", "Hexadecimal", "Decimal");
    assert_badges("0o17", "Octal", "Decimal");
}

#[test]
fn upstream_base_conversion_of_an_expression() {
    assert_display("1e5 to hex", "0x186A0");
    assert_display("2*128 to hex", "0x100");
    assert_display("10*5 to hex", "0x32");
    assert_display("256 as hex", "0x100");
}

#[test]
fn upstream_base_conversion_to_an_arbitrary_base() {
    assert_display("255 to base 7", "513");
    assert_badges("255 to base 7", "Decimal", "Base 7");
    assert_display("35 to base 36", "Z");
    assert_display("255 to base 16", "0xFF");
}

// MARK: - Rounding and fractions

#[test]
fn upstream_rounds_to_a_step() {
    assert_display("round 47 to nearest 5", "45");
    assert_display("round 12.3 to nearest 0.5", "12.5");
    assert_display("round 56 up to nearest 10", "60");
    assert_display("round 51 to next 10", "60");
    assert_display("round 59 down to last 10", "50");
    assert_display("round 56 up to 10", "60");
}

#[test]
fn upstream_rounds_a_direction() {
    assert_display("2.5 rounded", "3");
    assert_display("2.5 rounded down", "2");
    assert_display("2.1 rounded up", "3");
    assert_display("44 rounded up to nearest 10", "50");
    assert_display("round 56.2 up", "57");
    assert_display("round 56.8 down", "56");
    assert_badges("2.5 rounded", "Expression", "Rounded");
}

#[test]
fn upstream_rounds_to_decimal_places() {
    assert_display("2/3 to 3 dp", "0.667");
    assert_display("round 56.789 to 1 digit", "56.8");
    assert_display("round 3.14159 to 2 decimal points", "3.14");
    // Upstream: `round 3.14159 to nearest 8th` is "3.125".
    assert_display("round 3.14159 to nearest 8th", "3.125");
}

#[test]
fn upstream_answers_a_fraction() {
    assert_display("0.75 to fraction", "3/4");
    assert_display("0.625 as fraction", "5/8");
    assert_display("6/8 as a fraction", "3/4");
    assert_badges("0.625 as fraction", "Expression", "Fraction");
}

// MARK: - The queries upstream rejects

#[test]
fn upstream_rejects_an_unknown_word() {
    assert_no_card("safari");
    assert_no_card("1password");
    assert_no_card("x");
    assert_no_card("x3");
    assert_no_card("3x");
    assert_no_card("10 to mi");
}

#[test]
fn upstream_rejects_a_lone_literal() {
    // Upstream: a lone literal reads as an app search, so no card.
    assert_no_card("45");
    assert_no_card("3.14");
    assert_no_card("pi");
    assert_no_card("e");
}

#[test]
fn upstream_rejects_a_malformed_expression() {
    assert_no_card("sqrt()");
    // Upstream: a factorial needs an integer.
    assert_no_card("2.5!");
    assert_no_card("");
    assert_no_card("-(1 == 1)");
    assert_no_card("sqrt(1 == 1)");
    assert_no_card("10 mod 0");
}

#[test]
fn upstream_rejects_a_function_call_it_cannot_answer() {
    assert_no_card("gcd(1.5,2)");
    assert_no_card("round(1,9999)");
}

#[test]
fn upstream_rejects_a_shift_it_cannot_answer() {
    assert_no_card("1 << 64");
    assert_no_card("1 << -1");
    assert_no_card("1 << 63");
    // Upstream: a value past 2^53 is not exactly representable.
    assert_no_card("9007199254740993 & 1");
    assert_no_card("0x20000000000001 & 1");
}

#[test]
fn upstream_rejects_a_literal_that_overflows() {
    assert_no_card("1e400");
    assert_no_card("1e308k");
    assert_no_card("-1e308k");
    assert_no_card("1e308k to hex");
    assert_no_card("1e308 * 2 > 1");
    assert_no_card("(1e308 * 2) ^ 0");
    assert_no_card("1e5e5");
    assert_no_card("1e308 billion");
}

#[test]
fn upstream_rejects_a_magnitude_word_it_cannot_read() {
    assert_no_card("million");
    assert_no_card("2 * million");
    assert_no_card("(2 + 3) million");
    assert_no_card("13 millions");
    assert_no_card("13 million2");
}

#[test]
fn upstream_rejects_a_rounding_it_cannot_answer() {
    // Upstream: a bare step needs a direction to be one.
    assert_no_card("round 56 to 10");
    assert_no_card("round up");
    assert_no_card("round 5 to");
    assert_no_card("round is next round to");
    assert_no_card("pi as fraction");
}

#[test]
fn upstream_rejects_a_query_that_is_too_long() {
    let long = format!("{}1", "1+".repeat(100));
    assert_display(&long, "101");
    let too_long = format!("{}1", "1+".repeat(128));
    assert_no_card(&too_long);
}

// MARK: - Partial results

#[test]
fn upstream_keeps_the_last_complete_result_while_typing() {
    assert_display("45+", "45");
    assert_expression("45+", "45+");
    assert_display("10 +", "10");
    assert_display("10 -", "10");
    assert_display("10 *", "10");
    assert_display("10 x", "10");
    assert_expression("10k +", "10k +");
    assert_expression("13 million +", "13 million +");
    assert_expression("(2)+", "(2)+");
    assert_expression("255 to hex +", "255 to hex +");
}

#[test]
fn upstream_keeps_the_base_badge_while_typing() {
    assert_badges("0O17 +", "Octal", "Decimal");
    assert_badges("0xff -", "Hexadecimal", "Decimal");
}

#[test]
fn upstream_keeps_the_converted_value_while_typing() {
    assert_copy("1.00000000004 * 1e12 to hex +", "0xE8D4A51028");
}

// MARK: - The seam to the units and the currencies

#[test]
fn a_known_unit_is_reported_as_unsupported() {
    let catalog = NameCatalog::new().unit("kg", "kg", "Kilogram");
    match evaluate("10 kg", &catalog) {
        CalcOutcome::Unsupported(area) => {
            assert_eq!(area.area, "units");
            assert_eq!(area.detail, "kg");
        }
        other => panic!("a unit query answered {other:?}"),
    }
}

#[test]
fn a_known_currency_is_reported_as_unsupported() {
    let catalog = NameCatalog::new().currency("usd", "US Dollar");
    match evaluate("10 usd", &catalog) {
        CalcOutcome::Unsupported(area) => {
            assert_eq!(area.area, "currency");
            assert_eq!(area.detail, "usd");
        }
        other => panic!("a currency query answered {other:?}"),
    }
}

#[test]
fn an_unknown_word_is_not_a_unit() {
    let catalog = NameCatalog::new().unit("kg", "kg", "Kilogram");
    match evaluate("10 zzz", &catalog) {
        CalcOutcome::NoCard => {}
        other => panic!("an unknown word answered {other:?}"),
    }
}

// MARK: - More of the upstream suite

#[test]
fn upstream_a_spoken_root() {
    // Upstream: `2 square root of 9` is "6".
    assert_display("2 square root of 9", "6");
    // Upstream: `cube root of 8%` is "0.430886938" and the sign of an odd root is kept.
    assert_display("cube root of 8%", "0.430886938");
    assert_display("cube root of -8%", "-0.430886938");
}

#[test]
fn upstream_reads_the_odd_spaces() {
    // Upstream: a non-breaking space and a thin space separate the terms.
    assert_display("2\u{00A0}+\u{2009}2", "4");
}

#[test]
fn upstream_a_compact_literal_mods() {
    assert_display("10k mod 3", "1");
}

#[test]
fn upstream_echoes_the_pretty_glyphs() {
    // Upstream: the card echoes "3×3".
    assert_expression("3*3", "3×3");
}

#[test]
fn upstream_keeps_the_result_for_every_trailing_operator() {
    assert_display("10 /", "10");
    assert_display("10 ^", "10");
    assert_copy("10k +", "10000");
}

#[test]
fn upstream_rejects_a_half_typed_expression() {
    assert_no_card("+");
    assert_no_card("10 + nonsense");
    assert_no_card("10 + (");
    // Upstream: a stray English word is a search, not a partial expression.
    assert_no_card("10 of");
    assert_no_card("pages per");
    assert_no_card("world times 5");
}

#[test]
fn upstream_writes_the_operators_out() {
    assert_display("10 multiplied by 3", "30");
}

#[test]
fn upstream_the_rest_of_the_function_list() {
    assert_display("cot(1)", "0.6420926159");
    assert_display("csc(1)", "1.188395106");
    assert_display("acos(1)", "0");
    assert_display("arctan(1)", "0.7853981634");
    assert_display("exp(0)", "1");
}

#[test]
fn upstream_the_rest_of_the_constants() {
    assert_display("2 tau", "12.56637061");
    assert_display("phi * 2", "3.236067977");
}

#[test]
fn upstream_reads_a_comma_inside_a_number() {
    // Upstream: "10,5" is the number 105, and a lone number earns no card.
    assert_no_card("10,5");
}

#[test]
fn upstream_adds_decimals() {
    assert_display("5 + 3", "8");
    assert_display("1.5 + 3", "4.5");
    assert_display("99.99 + 0.01", "100");
}
// MARK: - The formatter on its own

#[test]
fn upstream_compounds_feet_and_inches() {
    // Upstream: one metre is "3 feet 3.37007874 inches".
    assert_eq!(
        Formatter::compound_feet_inches(1.0 / 0.3048),
        "3 feet 3.37007874 inches"
    );
}

#[test]
fn upstream_breaks_a_timespan_into_its_largest_units() {
    // Upstream: 8,700 seconds is "2 hr 25 min".
    assert_eq!(Formatter::timespan(8_700.0), "2 hr 25 min");
    assert_eq!(Formatter::timespan(90.0), "1 min 30 s");
}

#[test]
fn upstream_money_widens_below_a_cent() {
    assert_eq!(Formatter::currency(15.0), "15.00");
    assert_eq!(Formatter::currency(0.5), "0.50");
    assert_eq!(Formatter::currency(0.005), "0.005");
    assert_eq!(Formatter::currency(0.0), "0.00");
}
