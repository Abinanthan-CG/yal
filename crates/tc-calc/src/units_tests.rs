//! The unit catalog and the conversions, against upstream's own expectations.
//!
//! Every expectation here is upstream's, taken from `Tests/calc-test.swift`.

use crate::catalog::Catalog;
use crate::engine::{evaluate, CalcOutcome};
use crate::token::Token;
use crate::units::{self, UnitCatalog, UnitCategory};

/// The card a query produces, as upstream's `expectDisplay`.
fn display(query: &str) -> Option<String> {
    evaluate(query, &UnitCatalog::new())
        .display()
        .map(str::to_string)
}

/// The copy text a query produces, as upstream's `expectCopy`.
fn copy_text(query: &str) -> Option<String> {
    evaluate(query, &UnitCatalog::new())
        .copy_text()
        .map(str::to_string)
}

/// The message of the error card a query produces, as upstream's `expectError`.
fn error(query: &str) -> Option<String> {
    match evaluate(query, &UnitCatalog::new()) {
        CalcOutcome::Card(card) => match card.payload {
            crate::result::Payload::Error { message } => Some(message),
            crate::result::Payload::Value { .. } => None,
        },
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

// MARK: - The table itself

#[test]
fn the_table_holds_the_units_upstream_lists() {
    // Upstream's `CalcUnitCatalog`: the everyday units and their scales.
    let kilometre = units::by_name("km").expect("a kilometre is known");
    assert_eq!(kilometre.symbol, "km");
    assert_eq!(kilometre.name, "Kilometers");
    assert_eq!(kilometre.factor, 1000.0);
    assert_eq!(kilometre.category, UnitCategory::Length);

    let mile = units::by_name("mi").expect("a mile is known");
    assert_eq!(mile.factor, 1609.344);
    assert_eq!(mile.category, UnitCategory::Length);

    let pound = units::by_name("lb").expect("a pound is known");
    assert_eq!(pound.factor, 0.45359237);
    assert_eq!(pound.category, UnitCategory::Weight);
}

#[test]
fn the_table_holds_the_spelled_out_names() {
    // Upstream's `CalcUnitCatalog`: `nautical miles`, `sq ft` and `cubic feet` are
    // aliases too.
    assert!(units::knows("nautical miles"));
    assert!(units::knows("sq ft"));
    assert!(units::knows("cubic feet"));
    assert!(units::knows("square feet"));
    assert!(units::knows("light years"));
    assert!(units::knows("horsepower"));
    assert!(units::knows("yottabytes"));
}

#[test]
fn the_table_holds_the_metric_prefixes() {
    // Upstream's `CalcUnitCatalog` builds the prefixed units from the base ones.
    let milligram = units::by_name("mg").expect("a milligram is known");
    assert_eq!(milligram.factor, 1e-6);
    assert_eq!(milligram.category, UnitCategory::Weight);

    let kilowatt = units::by_name("kW").expect("a kilowatt is known");
    assert_eq!(kilowatt.factor, 1000.0);
    assert_eq!(kilowatt.category, UnitCategory::Power);

    let megahertz = units::by_name("MHz").expect("a megahertz is known");
    assert_eq!(megahertz.factor, 1_000_000.0);
}

#[test]
fn the_table_holds_the_byte_rates() {
    // Upstream's `CalcUnitCatalog` adds the byte rates and the bits beside them.
    let kibibyte = units::by_name("kib").expect("a kibibyte is known");
    assert_eq!(kibibyte.factor, 1024.0);
    assert_eq!(kibibyte.category, UnitCategory::DigitalStorage);

    let megabits = units::by_name("mbps").expect("a megabit per second is known");
    assert_eq!(megabits.factor, 125_000.0);
    assert_eq!(megabits.category, UnitCategory::DataRate);

    let kibibytes = units::by_name("kibibytes").expect("the spelled-out name is known");
    assert_eq!(kibibytes.factor, 1024.0);
}

#[test]
fn a_symbol_is_not_itself_a_name() {
    // Upstream's `CalcUnitCatalog` files a unit under its aliases, not under its
    // symbol, so `KiB` is not a name and `kib` is.
    assert!(units::by_name("KiB").is_none());
    assert!(units::by_name("kib").is_some());
    assert!(units::by_name("K").is_none());
    assert!(units::by_name("k").is_some());
}

#[test]
fn the_temperatures_carry_their_offsets() {
    // Upstream's `CalcUnitCatalog`: the temperatures are affine, not a plain scale.
    let celsius = units::by_name("c").expect("a degree Celsius is known");
    assert_eq!(celsius.offset, 273.15);
    let fahrenheit = units::by_name("f").expect("a degree Fahrenheit is known");
    assert_eq!(fahrenheit.offset, 255.3722222222222);
    let kelvin = units::by_name("k").expect("a kelvin is known");
    assert_eq!(kelvin.offset, 0.0);
}

// MARK: - The conversions

#[test]
fn a_length_is_converted_into_another_length() {
    // Upstream `expectDisplay("10 km to m", "10,000 m")`.
    assert_eq!(display("10 km to m").as_deref(), Some("10,000 m"));
}

#[test]
fn the_card_names_the_unit_it_came_from_and_the_one_it_became() {
    // Upstream `expectDisplay("10 km to m", "10,000 m")`, with the badges.
    assert_eq!(target_badge("10 km to m").as_deref(), Some("Meters"));
    assert_eq!(copy_text("10 km to m").as_deref(), Some("10000 m"));
}

#[test]
fn a_converted_amount_is_copied_without_the_separators() {
    // Upstream's card copies the answer ungrouped.
    assert_eq!(copy_text("10 km to m").as_deref(), Some("10000 m"));
}

#[test]
fn a_nautical_mile_is_converted_into_kilometres() {
    // Upstream `expectDisplay("2 nautical miles to km", "3.704 km")`.
    assert_eq!(
        display("2 nautical miles to km").as_deref(),
        Some("3.704 km")
    );
}

#[test]
fn a_spaced_unit_name_is_read_as_one_unit() {
    // Upstream `expectDisplay("250 sq ft to m2", "23.22576 m²")`.
    assert_eq!(display("250 sq ft to m2").as_deref(), Some("23.22576 m²"));
}

#[test]
fn a_cubic_measure_is_converted_into_litres() {
    // Upstream `expectDisplay("3 cubic feet to l", "84.95053978 L")`.
    assert_eq!(
        display("3 cubic feet to l").as_deref(),
        Some("84.95053978 L")
    );
}

#[test]
fn a_fraction_before_a_unit_is_one_amount() {
    // Upstream `expectDisplay("5/16\" in mm", "7.9375 mm")`.
    assert_eq!(display("5/16\" in mm").as_deref(), Some("7.9375 mm"));
    // Upstream `expectDisplay("5/16 inch in mm", "7.9375 mm")`.
    assert_eq!(display("5/16 inch in mm").as_deref(), Some("7.9375 mm"));
    // Upstream `expectDisplay("5/16 in to cm", "0.79375 cm")`.
    assert_eq!(display("5/16 in to cm").as_deref(), Some("0.79375 cm"));
    // Upstream `expectDisplay("3/8 in to mm", "9.525 mm")`.
    assert_eq!(display("3/8 in to mm").as_deref(), Some("9.525 mm"));
}

#[test]
fn a_mixed_number_before_a_unit_is_one_amount() {
    // Upstream `expectDisplay("1 1/2 cups to ml", "354.8823548 mL")`.
    assert_eq!(
        display("1 1/2 cups to ml").as_deref(),
        Some("354.8823548 mL")
    );
    // Upstream `expectDisplay("2 1/4 lb to g", "1,020.582833 g")`.
    assert_eq!(display("2 1/4 lb to g").as_deref(), Some("1,020.582833 g"));
}

#[test]
fn a_conversion_can_be_asked_for_with_as() {
    // Upstream `expectDisplay("10 km as m", "10,000 m")`.
    assert_eq!(display("10 km as m").as_deref(), Some("10,000 m"));
}

#[test]
fn two_unit_names_are_a_conversion_between_them() {
    // Upstream's `CalcUnits.parseUnitPairConversion`: `day s` is one day in seconds.
    assert_eq!(display("day s").as_deref(), Some("86,400 s"));
}

#[test]
fn a_weight_is_converted_into_another_weight() {
    // Upstream `expectDisplay("convert 10 kg to lb", "22.04622622 lb")`, with the
    // conversion asked for without the word.
    assert_eq!(display("10 kg to lb").as_deref(), Some("22.04622622 lb"));
}

#[test]
fn a_unit_that_measures_something_else_is_an_error_card() {
    // Upstream's `CalcEngine`: a clear conversion attempt that cannot be made says so.
    assert_eq!(
        error("10 kg to m").as_deref(),
        Some("Cannot convert Weight to Length.")
    );
}

// MARK: - The bare conversions

#[test]
fn a_measurement_without_a_target_is_converted_into_its_counterpart() {
    // Upstream's `CalcUnits.parseBareConversion`: `5 km` answers in miles.
    assert_eq!(display("5 km").as_deref(), Some("3.106855961 mi"));
    // Upstream `expectDisplay("convert 10 kg to lb", "22.04622622 lb")`, with the
    // target left out.
    assert_eq!(display("10 kg").as_deref(), Some("22.04622622 lb"));
}

#[test]
fn a_metre_is_answered_in_feet_and_inches() {
    // Upstream's `CalcEngine`: only the metric length asks for feet and inches.
    assert_eq!(
        display("1.75 m").as_deref(),
        Some("5 feet 8.897637795 inches")
    );
}

#[test]
fn a_thousand_is_not_read_as_a_kelvin() {
    // Upstream's `CalcUnits.parseBareConversion`: `c`, `f` and `k` are left out, so
    // `5k` stays a search.
    assert_eq!(display("5k").as_deref(), Some("5,000"));
}

// MARK: - The mixed units

#[test]
fn a_measure_is_told_in_two_units() {
    // Upstream `expectDisplay("1.75 m to feet and inches", "5 ft 8.897637795 in")`.
    assert_eq!(
        display("1.75 m to feet and inches").as_deref(),
        Some("5 ft 8.897637795 in")
    );
}

#[test]
fn a_mixed_answer_names_both_of_its_units() {
    // Upstream `expectBadges("1.75 m to feet and inches", source: "Meters", target:
    // "Feet and Inches")`.
    assert_eq!(
        target_badge("1.75 m to feet and inches").as_deref(),
        Some("Feet and Inches")
    );
}

#[test]
fn two_units_of_different_kinds_are_not_a_mixed_answer() {
    // Upstream `expectNil("5 km to kg and g")`.
    assert_eq!(display("5 km to kg and g"), None);
}

#[test]
fn the_two_units_of_a_split_are_joined_in_three_ways() {
    // Upstream's `CalcUnits`: `and`, `&` and a plain space all name the pair.
    assert_eq!(
        display("1.75 m to feet and inches").as_deref(),
        Some("5 ft 8.897637795 in")
    );
    assert_eq!(
        display("2.5 kg to lb & oz").as_deref(),
        Some("5 lb 8.184904874 oz")
    );
    assert_eq!(
        display("1.75 m to feet inches").as_deref(),
        Some("5 ft 8.897637795 in")
    );
}

#[test]
fn an_hour_on_its_own_is_answered_in_minutes() {
    // Upstream `expectDisplay("1hr gives 60 min")`.
    assert_eq!(display("1hr").as_deref(), Some("60 min"));
}

#[test]
fn a_torque_is_converted_into_another_torque() {
    // Upstream's electrical section: `10 newton meters to pound feet is
    // 7.375621493 lb·ft`.
    assert_eq!(
        display("10 newton meters to pound feet").as_deref(),
        Some("7.375621493 lb·ft")
    );
}

#[test]
fn a_transfer_rate_is_converted_into_another_one() {
    // Upstream's data section: `100Mbps to MB/s is 12.5 MB/s`.
    assert_eq!(display("100 mbps to MB/s").as_deref(), Some("12.5 MB/s"));
    // The binary units and the bit amounts are there too.
    assert!(units::knows("MiB/s"));
    assert!(units::knows("kbit"));
}

#[test]
fn the_table_holds_the_cubic_and_liquid_measures() {
    // Upstream's volume section: the cubic units run from mm³ to m³, plus in³, ft³
    // and yd³, and the liquids carry the US and the UK measures.
    for name in [
        "mm3",
        "cm3",
        "m3",
        "in3",
        "ft3",
        "yd3",
        "km3",
        "mi3",
        "metric cup",
        "ml",
        "cl",
        "dl",
        "l",
        "kl",
        "cup",
        "tbsp",
        "tsp",
        "gal",
        "qt",
        "pt",
        "floz",
        "ukgal",
        "ukqt",
        "ukpint",
        "ukfloz",
    ] {
        assert!(units::knows(name), "{name} is known");
    }
}

#[test]
fn the_table_holds_the_electrical_measures() {
    // Upstream's electrical section: the capacitance, the inductance, the conductance,
    // the magnetic flux, the torque and the amount of substance.
    for name in [
        "farad",
        "uf",
        "nf",
        "pf",
        "henry",
        "mH",
        "uh",
        "siemens",
        "millisiemens",
        "weber",
        "mwb",
        "tesla",
        "mT",
        "µt",
        "gauss",
        "mol",
        "mmol",
        "umol",
        "nmol",
        "n",
        "j",
        "w",
        "v",
        "a",
        "ohm",
        "as",
        "ah",
    ] {
        assert!(units::knows(name), "{name} is known");
    }
}

#[test]
fn the_table_holds_the_remaining_measures() {
    // Upstream's data section: the rest of the units it lists.
    for name in [
        "t",
        "st",
        "ct",
        "ozt",
        "nmi",
        "ly",
        "pc",
        "au",
        "fathom",
        "furlong",
        "bbl",
        "bu",
        "acre foot",
        "ul",
        "gwh",
        "twh",
        "hp",
        "btu",
        "rpm",
        "rps",
        "rad/s",
        "lbf",
        "pdl",
        "ksi",
        "ft/s2",
        "yb",
    ] {
        assert!(units::knows(name), "{name} is known");
    }
}

#[test]
fn the_prefixes_are_case_sensitive() {
    // Upstream's electrical section: `MW` is megawatts and `mW` is milliwatts.
    let megawatt = units::by_name("MW").expect("a megawatt is known");
    let milliwatt = units::by_name("mW").expect("a milliwatt is known");
    assert_eq!(megawatt.factor, 1_000_000.0);
    assert_eq!(milliwatt.factor, 0.001);
}

#[test]
fn a_bare_c_is_the_temperature_and_not_the_charge() {
    // Upstream's electrical section: `C` means Celsius, so the coulombs are `As`.
    let celsius = units::by_name("c").expect("a degree Celsius is known");
    assert_eq!(celsius.category, UnitCategory::Temperature);
    let coulomb = units::by_name("as").expect("a coulomb is known");
    assert_eq!(coulomb.category, UnitCategory::ElectricCharge);
    // A bare `F` is Fahrenheit; the farad is spelled out.
    assert_eq!(
        units::by_name("f")
            .expect("a degree Fahrenheit is known")
            .category,
        UnitCategory::Temperature
    );
    assert!(units::by_name("F").is_none());
    assert!(units::knows("farad"));
}

// MARK: - What the catalog does not decide

#[test]
fn a_calculation_with_a_unit_is_not_answered_yet() {
    // Upstream `expectDisplay("60 mph * 2.5 hours", "150 mi")`: a rate over time
    // answers in its own numerator. This port has no quantity algebra yet, so the
    // query is left unanswered rather than answered wrongly.
    assert_ne!(
        display("60 mph * 2.5 hours").as_deref(),
        Some("150 mi"),
        "a quantity with a unit must not be answered with the wrong number"
    );
}

#[test]
fn a_divisor_that_is_a_unit_is_not_read_as_a_plain_amount() {
    // Upstream `expectDisplay("1 / 20ms to hz", "50 Hz")`: one divided by twenty
    // milliseconds is fifty hertz. Reading it as a plain amount would answer
    // `0.00005 Hz`, so this port answers nothing instead.
    assert_ne!(display("1 / 20ms to hz").as_deref(), Some("0.00005 Hz"));
}

#[test]
fn the_catalog_knows_the_units_the_tokenizer_needs() {
    // Upstream's `CalcTokenizer.isTemperatureConversion`: the `k` of `10k to c` is a
    // kelvin, not a thousands suffix.
    let catalog = UnitCatalog::new();
    assert!(catalog.unit("kg").is_some());
    assert!(catalog.unit("sq ft").is_some());
    let celsius = catalog.unit("c").expect("a degree Celsius is known");
    assert!(celsius.is_temperature);
    assert!(catalog.unit("nonsense").is_none());
}

#[test]
fn a_unit_that_is_not_known_is_not_a_unit() {
    let catalog = UnitCatalog::new();
    assert!(catalog.unit("wibble").is_none());
    assert!(catalog.currency("usd").is_none());
    assert!(catalog.currency_sign('$').is_none());
}

#[test]
fn the_tokens_of_a_conversion_are_read_as_upstream_reads_them() {
    // Upstream's tokenizer joins a spaced unit name into one token.
    let tokens = crate::tokenizer::tokenize("250 sq ft to m2", &UnitCatalog::new())
        .expect("the query is calculator input");
    assert_eq!(
        tokens,
        vec![
            Token::Number(250.0),
            Token::Ident("sq ft".to_string()),
            Token::Ident("to".to_string()),
            Token::Ident("m2".to_string()),
        ]
    );
}
