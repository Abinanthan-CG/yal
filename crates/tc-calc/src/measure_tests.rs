//! The arithmetic that carries a unit, against upstream's own expectations.
//!
//! Every expectation in this file is upstream's, taken from `Tests/calc-test.swift` and
//! from the calculator page of the upstream documentation.

use crate::engine::{evaluate, CalcOutcome};
use crate::units::UnitCatalog;

/// The text a card shows, as upstream's `expectDisplay`.
fn display(query: &str) -> Option<String> {
    evaluate(query, &UnitCatalog::new())
        .display()
        .map(str::to_string)
}

/// The text that lands on the pasteboard, as upstream's `expectCopy`.
fn copy_text(query: &str) -> Option<String> {
    evaluate(query, &UnitCatalog::new())
        .copy_text()
        .map(str::to_string)
}

/// The expression a card echoes, as upstream's `expectExpression`.
fn expression(query: &str) -> Option<String> {
    match evaluate(query, &UnitCatalog::new()) {
        CalcOutcome::Card(card) => Some(card.expression),
        _ => None,
    }
}

/// The badge a card shows on its right, as upstream's `expectBadges`.
fn target_badge(query: &str) -> Option<String> {
    match evaluate(query, &UnitCatalog::new()) {
        CalcOutcome::Card(card) => card.target_badge.clone(),
        _ => None,
    }
}

/// Asserts the text a card shows, as upstream's `expectDisplay`.
fn assert_display(query: &str, expected: &str) {
    assert_eq!(display(query).as_deref(), Some(expected), "query {query:?}");
}

/// Asserts that a query earns no card, as upstream's `expectNil`.
fn assert_nil(query: &str) {
    assert_eq!(display(query), None, "query {query:?}");
}

// MARK: - The last unit typed decides the unit of the answer

#[test]
fn the_last_unit_typed_decides_the_unit_of_the_answer() {
    // Upstream's `Tests/calc-test.swift`: the answer is in the unit that was typed
    // last, so `10kg + 10g` is grams and `500g + 1kg` is kilograms.
    assert_display("10kg + 5kg", "15 kg");
    assert_display("10kg + 10g", "10,010 g");
    assert_display("10kg + 500g", "10,500 g");
    assert_display("500g + 1kg", "1.5 kg");
    assert_display("10lb + 5kg", "9.5359237 kg");
    assert_display("1m + 50cm", "150 cm");
    assert_display("2hr + 30min", "150 min");
    assert_display("1GiB + 512MiB", "1,536 MiB");
    assert_display("1L - 250mL", "750 mL");
    assert_display("-5kg + 2kg", "-3 kg");
    assert_display("-(2kg + 500g)", "-2,500 g");
    assert_display("1m² + 10ft²", "20.76391042 ft²");
    assert_display("1L + 1cup", "5.226752838 cup");
    assert_display("1GB + 1GiB", "1.931322575 GiB");
    assert_display("90deg + 1rad", "2.570796327 rad");
    assert_display("60mph + 10kmh", "106.56064 km/h");
    assert_display("1bar + 10psi", "24.50377377 psi");
    assert_display("1Gbps + 500Mbps", "1,500 Mbps");
}

#[test]
fn the_copy_text_carries_the_unit_too() {
    // Upstream's `Tests/calc-test.swift`: the copy text is the answer with its unit.
    assert_eq!(copy_text("10kg + 5kg").as_deref(), Some("15 kg"));
    assert_eq!(copy_text("500g + 1kg").as_deref(), Some("1.5 kg"));
}

#[test]
fn the_card_names_the_unit_it_answers_in() {
    // Upstream's `Tests/calc-test.swift`: the right badge is the unit of the answer.
    assert_eq!(target_badge("10kg + 5kg").as_deref(), Some("Kilograms"));
    assert_eq!(target_badge("5kg / 2kg").as_deref(), Some("Result"));
}

#[test]
fn the_expression_echoes_the_units_as_symbols() {
    // Upstream's `Tests/calc-test.swift`: the echo is normalized.
    assert_eq!(expression("10kg + 5kg").as_deref(), Some("10 kg + 5 kg"));
    assert_eq!(expression("5 feet 3 inches").as_deref(), Some("5 ft 3 in"));
    assert_eq!(expression("10kg - 5kg").as_deref(), Some("10 kg - 5 kg"));
    assert_eq!(expression("-5kg + 2kg").as_deref(), Some("-5 kg + 2 kg"));
}

// MARK: - Composite notation

#[test]
fn two_quantities_written_side_by_side_are_one_amount() {
    // Upstream's `Tests/calc-test.swift`: composite notation answers in the leading
    // unit, because the invisible `+` converts the second into the first.
    assert_display("5 feet 3 inches", "5.25 ft");
    assert_display("1hr 30min", "1.5 hr");
}

#[test]
fn a_half_typed_unit_earns_no_card() {
    // Upstream's `Tests/calc-test.swift`: `1hr 30` is one keystroke short of
    // `1hr 30min`, and answering `31 hr` would be worse than nothing.
    assert_nil("1hr 30");
    assert_nil("5 feet 3");
}

// MARK: - The unit beside a bare amount

#[test]
fn a_bare_amount_takes_the_unit_next_to_it() {
    // Upstream's `Tests/calc-test.swift`.
    assert_display("5kg + 5", "10 kg");
    assert_display("10kg + 2 * 5kg", "20 kg");
    assert_display("20kg / 2 + 3kg", "13 kg");
    assert_display("5kg * 3", "15 kg");
    assert_display("10kg / 4", "2.5 kg");
    assert_display("1kg / 3", "0.3333333333 kg");
}

// MARK: - Two measures of one kind

#[test]
fn two_measures_of_one_kind_divide_into_a_plain_ratio() {
    // Upstream's `Tests/calc-test.swift`: the quotient of two of the same kind is a
    // plain number, not a measure.
    assert_display("5kg / 2kg", "2.5");
    assert_display("5kg / 500g", "10");
    assert_display("(10kg * 3) / 5kg", "6");
    assert_display("10kg / (5kg / 2)", "4");
}

// MARK: - The arithmetic between measures

#[test]
fn the_arithmetic_between_measures_keeps_the_unit() {
    // Upstream's `Tests/calc-test.swift`.
    assert_display("2 * 5kg", "10 kg");
    assert_display("(10kg + 5kg) * 2", "30 kg");
    assert_display("2 * (3kg + 500g)", "7,000 g");
    assert_display("20kg / (2 + 3)", "4 kg");
    assert_display("10kg * (2 + 3)", "50 kg");
    assert_display("10kg / (2 * 5)", "1 kg");
    assert_display("(2kg + 500g) * 4", "10,000 g");
    assert_display("(20kg - 5kg) / 3", "5 kg");
    assert_display("2(3)kg", "6 kg");
    assert_display("2*(3)kg", "6 kg");
    assert_display("2(3)kg x 2", "12 kg");
}

#[test]
fn a_measure_multiplied_by_a_percentage_is_a_measure() {
    // Upstream's `Tests/calc-test.swift`.
    assert_display("10kg + 20%", "12 kg");
    assert_display("10kg - 20%", "8 kg");
    assert_display("10kg * 20%", "2 kg");
    assert_display("10kg * 3%", "0.3 kg");
    assert_display("3% * 10kg", "0.3 kg");
    assert_display("10kg * 0%", "0 kg");
    assert_display("10kg * -3%", "-0.3 kg");
    assert_display("10kg / 25%", "40 kg");
    assert_display("10kg / 200%", "5 kg");
    assert_display("10kg * 3% + 1kg", "1.3 kg");
    assert_display("(10kg + 5kg) * 3%", "0.45 kg");
    assert_display("19m + 47%", "27.93 m");
    assert_nil("10kg / 0%");
}

// MARK: - Measures that make a new measure

#[test]
fn two_measures_of_different_kinds_make_a_new_one() {
    // Upstream's `Tests/calc-test.swift`: the answer is told in the base unit of the
    // dimension it lands on.
    assert_display("5m * 4m", "20 m²");
    assert_display("2m * 3m * 4m", "24 m³");
    assert_display("2kg / 4m3", "0.5 kg/m³");
    assert_display("1kg * 1m", "1 kg·m");
}

#[test]
fn a_root_keeps_the_unit() {
    // Upstream's `Tests/calc-test.swift`: the root raises the unit as well.
    assert_display("square root of 25m2", "5 m");
    assert_display("cube root of -8m3", "-2 m");
    assert_display("(2m)^2", "4 m²");
}

#[test]
fn a_rate_multiplied_by_a_time_keeps_the_numerator_of_the_rate() {
    // Upstream's `Tests/calc-test.swift`: a rate over time accrues in its own
    // numerator, so miles stay miles and kilometres stay kilometres.
    assert_display("90km/h * 20min", "30 km");
    assert_display("60 mph * 2.5 hours", "150 mi");
    assert_display("10 l/min * 30 s", "5 L");
}

#[test]
fn a_plain_amount_over_a_unit_is_the_inverse_of_that_unit() {
    // Upstream's `Tests/calc-test.swift`: `1 / 20ms` is fifty hertz.
    assert_display("1 / 20ms", "50 Hz");
}

// MARK: - The card of a measure in progress

#[test]
fn a_trailing_operator_keeps_the_measure_on_the_card() {
    // Upstream's `Tests/calc-test.swift`: an incomplete expression keeps the last
    // complete result, and the unit with it.
    assert_display("10kg *", "10 kg");
    assert_display("10kg + 500g +", "10,500 g");
    assert_display("(10kg + 500g) *", "10,500 g");
    assert_display("10kg * 3% +", "0.3 kg");
    assert_eq!(target_badge("10kg *").as_deref(), Some("Kilograms"));
}

// MARK: - A unit that is not a unit

#[test]
fn a_unit_wins_the_collision_with_the_currency_of_the_same_name() {
    // Upstream's `Tests/calc-test.swift`: `pounds` is a measure, not money.
    assert_display("10 pounds + 5 pounds", "15 lb");
}

// MARK: - The conversion at the end of a calculation

#[test]
fn a_calculation_is_told_in_the_unit_that_is_named_after_it() {
    // Upstream's `Tests/calc-test.swift`: the calculation is made first, and the
    // answer is then told in the unit that was named.
    assert_display("(1kg + 500g) to lb", "3.306933933 lb");
    assert_display("10kg + 500g to lb", "23.14853753 lb");
    assert_display("(10lb + 5kg) to kg", "9.5359237 kg");
    assert_display("10kg * 3% to g", "300 g");
    assert_display("100km / 2h to km/h", "50 km/h");
    assert_display("1500w * 2h to kwh", "3 kWh");
}

#[test]
fn the_card_of_a_converted_calculation_names_the_expression_and_the_unit() {
    // Upstream's `Tests/calc-test.swift`: the badges are the expression and the unit.
    assert_eq!(
        target_badge("(1kg + 500g) to lb").as_deref(),
        Some("Pounds")
    );
    assert_eq!(
        target_badge("10kg + 5kg").as_deref(),
        Some("Kilograms"),
        "a calculation names the unit it answered in"
    );
}

// MARK: - The examples of the documentation

#[test]
fn the_examples_of_the_documentation_answer_as_they_are_written() {
    // Every example of the unit section of upstream's calculator page.
    assert_display("5feet + 1m", "2.524 m");
    assert_display("10kg + 500g", "10,500 g");
    assert_display("1kg + 500g + 2lb", "5.306933933 lb");
    assert_display("10kg + 500g to lb", "23.14853753 lb");
    assert_display("5kg+5", "10 kg");
    assert_display("10kg + 20%", "12 kg");
    assert_display("10kg * 3%", "0.3 kg");
    assert_display("10kg / 25%", "40 kg");
    assert_display("100 / s to hz", "100 Hz");
    assert_display("sqrt(25m2)", "5 m");
    assert_display("cube root of -8m3", "-2 m");
    assert_display("100km / 2h to km/h", "50 km/h");
    assert_display("1GB / 100mbps to s", "80 s");
    assert_display("1500w * 2h to kwh", "3 kWh");
    assert_display("1 / 20ms to hz", "50 Hz");
    assert_display("3 months to days", "91.310625 day");
    assert_display("12 months to years", "1 yr");
    assert_display("m in ft", "3.280839895 ft");
    assert_display("2.5 kg to lb oz", "5 lb 8.184904874 oz");
    assert_display("2.75 hours in hours and minutes", "2 hr 45 min");
    assert_display("100 months to years and months", "8 yr 4 mo");
    assert_display("100 days to weeks and days", "14 week 2 day");
    assert_display("2kg / 4m3", "0.5 kg/m³");
    assert_display("pi (2m)^2", "12.56637061 m²");
    assert_display("5w * 3h 30min", "17.5 Wh");
    assert_display("2kw * 3h 30min", "7 kWh");
    assert_display("500mA * 3h 30min", "1,750 mAh");
    assert_display("12V * 2Ah to wh", "24 Wh");
    assert_display("90km / 1h 30min to km/h", "60 km/h");
}

#[test]
fn the_documentation_loses_a_multiplication_sign_in_its_markdown() {
    // The documentation writes `5m 4m` and `90km/h 20min`; the multiplication sign is
    // read as emphasis and is lost. Upstream's own tests write the sign out, and
    // without it the two quantities stand side by side and are one amount.
    assert_display("5m * 4m", "20 m²");
    assert_display("2m * 3m * 4m to l", "24,000 L");
    assert_display("90km/h * 20min to km", "30 km");
    assert_display("60 mph * 2.5 hours", "150 mi");
    assert_display("10 l/min * 30 s", "5 L");
    assert_display("5 feet 3 inches", "5.25 ft");
}

#[test]
fn an_amount_of_money_and_a_measure_are_two_different_things() {
    // Upstream's `Tests/calc-test.swift`: `$10 + 5` is `15.00 USD`, and a measure
    // beside an amount of money is not a calculation.
    let catalog = crate::catalog::FullCatalog;
    assert_eq!(evaluate("$10 + 5", &catalog).display(), Some("15.00 USD"));
}

#[test]
fn a_fraction_right_before_a_unit_is_one_amount() {
    // Upstream's `Tests/calc-test.swift`: `5/16"` is a sixteenth of an inch, not five
    // divided by sixteen inches.
    assert_display("5/16\"", "0.79375 cm");
}

// MARK: - The functions of a measurement

#[test]
fn the_functions_of_a_measurement_answer_in_the_unit_of_the_first_argument() {
    // Upstream's `Tests/calc-test.swift`: the arguments are told in the unit of the
    // first one, so `min(1km,999m)` is `0.999 km` rather than `1`.
    assert_display("hypot(3m,400cm)", "5 m");
    assert_display("min(1km,999m)", "0.999 km");
    assert_display("sum(1km,500m)", "1.5 km");
    assert_display("round(2.567km,1)", "2.6 km");
    assert_display("median(3 km, 1 km, 2 km)", "2 km");
}

#[test]
fn the_echo_of_a_function_of_a_measurement_keeps_the_units() {
    // Upstream's `Tests/calc-test.swift`.
    assert_eq!(
        expression("min(1km,999m)").as_deref(),
        Some("min(1 km, 999 m)")
    );
    assert_eq!(
        expression("round(2.567km,1)").as_deref(),
        Some("round(2.567 km, 1)")
    );
    assert_eq!(
        expression("hypot(3m,400cm)").as_deref(),
        Some("hypot(3 m, 400 cm)")
    );
}

#[test]
fn the_measures_combine_through_the_whole_calculation() {
    // Upstream's `Tests/calc-test.swift`.
    assert_display("(1m + 50cm) to ft", "4.921259843 ft");
    assert_display("(2dm)^3 to l", "8 L");
    assert_display("pi * (10cm)^2 * 30cm to l", "9.424777961 L");
    assert_display("4/3 * pi * (10cm)^3 to l", "4.188790205 L");
    assert_display("500l / (2m * 1m) to cm", "25 cm");
    assert_display("(300ppi * 2.54cm) / 300px", "1");
    assert_display("pi * (2m)^2 to m2", "12.56637061 m²");
    assert_display("(2m)^0.5", "1.414213562 m^0.5");
}

// MARK: - A unit beside another unit name

#[test]
fn a_unit_beside_another_unit_name_makes_a_compound_of_the_two() {
    // Upstream's documentation: `60 miles per hour` is `60 mi/hr`, and the money of
    // a rate is a compound of money and time.
    assert_display("60 miles per hour", "60 mi/hr");
    assert_display("10 kg/m3", "10 kg/m³");
    assert_display("60 mi/hr to km/h", "96.56064 km/h");
}

#[test]
fn the_money_of_a_rate_is_a_compound_of_money_and_time() {
    // Upstream's `Tests/calc-test.swift`: `50 usd per hour` is `50 USD/hr`.
    let catalog = crate::catalog::FullCatalog;
    assert_eq!(
        evaluate("50 usd per hour", &catalog).display(),
        Some("50 USD/hr")
    );
}

#[test]
fn the_operators_can_be_written_as_words() {
    // Upstream's documentation.
    assert_display("10 plus 5", "15");
    assert_display("10 minus 3", "7");
    assert_display("6 times 7", "42");
    assert_display("10 divided by 2", "5");
    assert_display("10 multiplied by 3", "30");
    assert_display("2 to the power of 10", "1,024");
    assert_display("3 raised to 4", "81");
    assert_display("2 exponent 10", "1,024");
    assert_display("3 to the 4", "81");
    assert_display("4 squared", "16");
    assert_display("3 cubed", "27");
    assert_display("20 percent of 50", "10");
}

#[test]
fn a_list_of_measures_is_reduced_in_the_unit_of_the_first_one() {
    // Upstream's documentation: the items are told in the unit of the first one.
    assert_display("sum of 1 km and 500 m", "1.5 km");
    assert_display("average of 1 km and 500 m", "0.75 km");
    assert_display("min of 1km and 999m", "0.999 km");
    assert_display("median of 3 km, 1 km, 2 km", "2 km");
    assert_display("stdev(1km,500m)", "0.3535533906 km");
}

#[test]
fn two_measures_of_one_kind_can_be_compared() {
    // Upstream's documentation: `1km == 1000m` is true.
    assert_display("1km == 1000m", "true");
    assert_display("1km != 1000m", "false");
    assert_display("1km >= 1000m", "true");
}

#[test]
fn a_share_of_a_measure_is_a_percentage() {
    // Upstream's `Tests/calc-test.swift`: `750 m as % of 3 km` is `25%`. The `as`
    // there is a word, not the unit of a coulomb.
    assert_display("750 m as % of 3 km", "25%");
    assert_display("50 as % of 200", "25%");
    assert_display("50 is what % of 200", "25%");
}
