//! The tests of the currency table and of the money a calculation carries.
//!
//! Every expectation in this file is read out of upstream Tinycast's own sources:
//! `upstream/Tinycast/Features/Calculator/Model/CalcCurrency.swift`,
//! `upstream/Tinycast/Features/Calculator/Model/CurrencyData.generated.swift` and
//! `upstream/Tests/calc-test.swift`.

use crate::currency::{by_name, knows, sign_code};
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
fn the_table_holds_the_currencies_upstream_lists() {
    // Upstream's `CurrencyData.all` holds 159 fiat currencies and `CalcCurrency.crypto`
    // holds 23 coins.
    assert_eq!(crate::currency::table::FIAT.len(), 159);
    assert_eq!(crate::currency::table::SIGNS.len(), 26);
    let usd = by_name("usd").expect("the US dollar is known");
    assert_eq!(usd.code, "USD");
    assert_eq!(usd.name, "US Dollar");
    let jpy = by_name("jpy").expect("the Japanese yen is known");
    assert_eq!(jpy.code, "JPY");
    assert_eq!(jpy.name, "Japanese Yen");
    let bitcoin = by_name("bitcoin").expect("the bitcoin is known");
    assert_eq!(bitcoin.code, "BTC");
    assert_eq!(bitcoin.name, "Bitcoin");
}

#[test]
fn a_currency_is_named_by_its_code_and_its_own_nouns() {
    // Upstream's generated nouns: the ones exactly one currency uses.
    assert_eq!(by_name("afghani").map(|def| def.code), Some("AFN"));
    assert_eq!(by_name("rupiah").map(|def| def.code), Some("IDR"));
    // Upstream's hand-written contested nouns.
    assert_eq!(by_name("dollar").map(|def| def.code), Some("USD"));
    assert_eq!(by_name("pounds").map(|def| def.code), Some("GBP"));
    assert_eq!(by_name("francs").map(|def| def.code), Some("CHF"));
    assert_eq!(by_name("rupees").map(|def| def.code), Some("INR"));
    assert_eq!(by_name("won").map(|def| def.code), Some("KRW"));
}

#[test]
fn a_ticker_beats_a_noun_that_looks_like_one() {
    // Upstream: "After the generated nouns, so a ticker beats one: `sol` is Solana,
    // `soles` stays PEN."
    assert_eq!(by_name("sol").map(|def| def.code), Some("SOL"));
    assert_eq!(by_name("soles").map(|def| def.code), Some("PEN"));
}

#[test]
fn the_standard_names_the_currency_the_data_misses() {
    // Upstream's `CalcCurrency.isoNames`: ISO 4217 names CNY "Yuan Renminbi".
    assert_eq!(by_name("rmb").map(|def| def.code), Some("CNY"));
    assert_eq!(by_name("renminbi").map(|def| def.code), Some("CNY"));
    // Upstream's `CalcCurrency.signCodes`: Taiwan types the sign's code.
    assert_eq!(by_name("ntd").map(|def| def.code), Some("TWD"));
}

#[test]
fn a_sign_names_its_currency() {
    // Upstream's generated signs: only the ones CLDR gives to a single currency.
    assert_eq!(sign_code('$'), Some("usd"));
    assert_eq!(sign_code('€'), Some("eur"));
    assert_eq!(sign_code('£'), Some("gbp"));
    assert_eq!(sign_code('¥'), Some("jpy"));
    assert_eq!(sign_code('₹'), Some("inr"));
    assert_eq!(sign_code('z'), None);
}

#[test]
fn a_word_several_countries_share_is_not_guessed_at() {
    // Upstream: "If a word is still ambiguous, Tinycast shows no card. For example,
    // krona could be Swedish or Icelandic, so Tinycast doesn't guess."
    assert!(!knows("krona"));
    // The plural that only one currency uses is still known.
    assert_eq!(by_name("kronor").map(|def| def.code), Some("SEK"));
}

#[test]
fn an_unknown_word_is_not_a_currency() {
    assert!(!knows("wibble"));
    assert!(!knows("zzz"));
}

#[test]
fn an_amount_of_money_keeps_its_currency() {
    assert_eq!(display("100 usd").as_deref(), Some("100.00 USD"));
    assert_eq!(copy_text("100 usd").as_deref(), Some("100 USD"));
    assert_eq!(target_badge("100 usd").as_deref(), Some("US Dollar"));
}

#[test]
fn a_sign_before_an_amount_is_the_same_money() {
    assert_eq!(display("$100").as_deref(), Some("100.00 USD"));
    assert_eq!(display("€100").as_deref(), Some("100.00 EUR"));
}

#[test]
fn a_discount_off_money_keeps_the_money() {
    assert_eq!(
        display("20% discount off $100").as_deref(),
        Some("80.00 USD")
    );
    assert_eq!(display("25% off $60").as_deref(), Some("45.00 USD"));
}

#[test]
fn a_tax_on_money_keeps_the_money() {
    assert_eq!(display("$100 + 10% tax").as_deref(), Some("110.00 USD"));
    assert_eq!(target_badge("$100 + 10% tax").as_deref(), Some("Total"));
}

#[test]
fn a_total_keeps_its_currency() {
    assert_eq!(display("$66 is 10% on what").as_deref(), Some("60.00 USD"));
}

#[test]
fn a_plain_amount_beside_money_stays_in_the_money() {
    assert_eq!(display("100 usd + 50").as_deref(), Some("150.00 USD"));
}

#[test]
fn two_currencies_cannot_be_mixed_without_the_rates() {
    // Upstream needs the rate feed for this; the port does not have it yet, so the
    // answer is no card rather than a made-up number.
    assert!(matches!(outcome("100 usd + 5 eur"), CalcOutcome::NoCard));
}
