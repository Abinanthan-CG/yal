//! The currency table and the money a calculation carries.
//!
//! Mirrors upstream `CalcCurrency.swift` and `CurrencyData.generated.swift`.
//!
//! A currency is an ISO 4217 code and the label the card wears: `USD` is
//! `US Dollar`, `JPY` is `Japanese Yen`. The words that name one are the code
//! itself (`usd`), the nouns exactly one currency uses (`afghani`), and the
//! contested ones (`dollars`, `pounds`, `francs`), which the standards bodies
//! never settled and which upstream writes down by hand.
//!
//! The amounts of two currencies cannot be compared or added without the rate
//! feed, which the port does not have yet: a calculation that mixes two
//! currencies says so rather than answering a made-up number.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::catalog::{Catalog, CurrencyInfo, UnitInfo};

pub mod table;

/// One currency: the ISO 4217 code and the label the card wears.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CurrencyDef {
    /// The ISO 4217 code, such as `USD`.
    pub code: &'static str,
    /// The label the card wears, such as `US Dollar`.
    pub name: &'static str,
}

/// Hand-written because no standards body assigns a shared noun.
///
/// Upstream's `CalcCurrency.contested`.
const CONTESTED: &[(&str, &[&str])] = &[
    ("USD", &["dollar", "dollars"]),
    ("CHF", &["franc", "francs"]),
    ("GBP", &["pound", "pounds"]),
    ("MXN", &["peso", "pesos"]),
    ("INR", &["rupee", "rupees"]),
    ("KES", &["shilling", "shillings"]),
    ("AED", &["dirham", "dirhams"]),
    ("KRW", &["won"]),
    ("RON", &["leu", "lei"]),
    ("RUB", &["ruble", "rubles"]),
    ("SAR", &["riyal", "riyals"]),
];

/// ISO 4217's own names where CLDR differs; the standard is the source of truth.
///
/// Upstream's `CalcCurrency.isoNames`.
const ISO_NAMES: &[(&str, &[&str])] = &[("CNY", &["rmb", "renminbi"])];

/// Codes daily use spells from CLDR's sign, not from ISO 4217.
///
/// Upstream's `CalcCurrency.signCodes`.
const SIGN_CODES: &[(&str, &[&str])] = &[("TWD", &["ntd"])];

/// The coins no standards body names.
///
/// Upstream's `CalcCurrency.crypto`.
const CRYPTO: &[(&str, &str, &[&str])] = &[
    ("ADA", "Cardano", &["cardano"]),
    ("AVAX", "Avalanche", &["avalanche"]),
    ("BCH", "Bitcoin Cash", &[]),
    ("BNB", "BNB", &["binance"]),
    ("BSV", "Bitcoin SV", &[]),
    ("BTC", "Bitcoin", &["bitcoin"]),
    ("DASH", "Dash", &[]),
    ("DOGE", "Dogecoin", &["dogecoin"]),
    ("DOT", "Polkadot", &["polkadot"]),
    ("EOS", "EOS", &[]),
    ("ETC", "Ethereum Classic", &[]),
    ("ETH", "Ethereum", &["ethereum", "ether"]),
    ("LTC", "Litecoin", &["litecoin"]),
    ("LUNA", "Terra", &["terra"]),
    ("NEO", "Neo", &[]),
    ("POL", "Polygon", &["polygon"]),
    ("SHIB", "Shiba Inu", &["shiba"]),
    ("SOL", "Solana", &["solana"]),
    ("TRX", "TRON", &["tron"]),
    ("USDT", "Tether", &["tether"]),
    ("XLM", "Stellar", &["stellar"]),
    ("XMR", "Monero", &["monero"]),
    ("XRP", "XRP", &["ripple"]),
];

/// The table, built once.
fn table() -> &'static HashMap<String, CurrencyDef> {
    static TABLE: OnceLock<HashMap<String, CurrencyDef>> = OnceLock::new();
    TABLE.get_or_init(build_table)
}

/// Builds the table, as upstream's `CalcCurrency.byName` does.
///
/// The generated data comes first, so the hand-written tables win: a ticker beats
/// a noun, so `sol` is Solana and `soles` stays PEN.
fn build_table() -> HashMap<String, CurrencyDef> {
    let mut defs: HashMap<&'static str, CurrencyDef> = HashMap::new();
    let mut table: HashMap<String, CurrencyDef> = HashMap::new();
    for (code, name) in table::FIAT {
        let def = CurrencyDef { code, name };
        defs.insert(code, def);
        table.insert(code.to_lowercase(), def);
    }
    for (word, code) in table::ALIASES {
        if let Some(def) = defs.get(code) {
            table.insert((*word).to_string(), *def);
        }
    }
    for (code, name, words) in CRYPTO {
        let def = CurrencyDef { code, name };
        defs.insert(code, def);
        table.insert(code.to_lowercase(), def);
        for word in *words {
            table.insert((*word).to_string(), def);
        }
    }
    for (code, words) in CONTESTED {
        if let Some(def) = defs.get(code) {
            for word in *words {
                table.insert((*word).to_string(), *def);
            }
        }
    }
    for (code, words) in ISO_NAMES {
        if let Some(def) = defs.get(code) {
            for word in *words {
                table.insert((*word).to_string(), *def);
            }
        }
    }
    for (code, words) in SIGN_CODES {
        if let Some(def) = defs.get(code) {
            for word in *words {
                table.insert((*word).to_string(), *def);
            }
        }
    }
    table
}

/// The currency filed under `name`, if the table knows one.
///
/// The name may be the code, a noun, or a sign that has been folded into a code.
pub fn by_name(name: &str) -> Option<CurrencyDef> {
    table().get(&name.to_lowercase()).copied()
}

/// Whether the table knows a currency under `name`.
pub fn knows(name: &str) -> bool {
    by_name(name).is_some()
}

/// The currency a sign stands for, if the table knows one.
pub fn sign_code(sign: char) -> Option<&'static str> {
    table::SIGNS
        .iter()
        .find(|(glyph, _)| *glyph == sign)
        .map(|(_, code)| *code)
}

/// The catalog of every currency in the table.
///
/// It knows no units: the units live in [`crate::units::UnitCatalog`], and
/// [`crate::catalog::FullCatalog`] serves both.
#[derive(Debug, Clone, Default)]
pub struct CurrencyCatalog;

impl CurrencyCatalog {
    /// A catalog with every currency in it.
    pub fn new() -> Self {
        Self
    }
}

impl Catalog for CurrencyCatalog {
    fn unit(&self, _name: &str) -> Option<UnitInfo> {
        None
    }

    fn currency(&self, name: &str) -> Option<CurrencyInfo> {
        by_name(name).map(|def| CurrencyInfo {
            code: def.code.to_string(),
            name: def.name.to_string(),
        })
    }

    fn currency_sign(&self, sign: char) -> Option<String> {
        sign_code(sign).map(str::to_string)
    }

    fn currency_definition(&self, name: &str) -> Option<CurrencyDef> {
        by_name(name)
    }
}
