//! Currency data and conversion.
//!
//! The bundled rates are **indicative reference values**: a snapshot taken
//! when the build was assembled, not live quotes. A conversion is only as
//! current as the table that answered it, so a table carries its vintage
//! and the UI is expected to say so.
//!
//! Freshness comes from the store. An explicit act (`yal rates-update`,
//! or the palette's update act) fetches a fresh table once and stores it;
//! this registry then applies it — codes the bundle ships get their rate
//! overridden, codes it does not are added. A code the bundle has no
//! symbol for is displayed as its bare code rather than a guessed
//! symbol, which is the honest form.
//!
//! The base currency is the US dollar.

use std::collections::HashMap;

/// The date the bundled reference rates were captured.
pub const RATES_AS_OF: &str = "2026-01-15";

/// A currency and its rate against the base currency.
#[derive(Debug, Clone, PartialEq)]
pub struct CurrencyDef {
    /// The ISO 4217 code.
    pub code: String,
    /// The display name.
    pub name: String,
    /// The symbol shown next to an amount.
    pub symbol: String,
    /// Units of this currency per one US dollar.
    pub per_usd: f64,
}

/// One bundled row, as compile-time data. The owned shape is built from
/// these when the registry is constructed.
pub struct Bundled {
    /// The ISO 4217 code.
    pub code: &'static str,
    /// The display name.
    pub name: &'static str,
    /// The symbol shown next to an amount.
    pub symbol: &'static str,
    /// Units of this currency per one US dollar.
    pub per_usd: f64,
}

/// The bundled currency table.
pub const CURRENCIES: &[Bundled] = &[
    Bundled { code: "USD", name: "US Dollar", symbol: "$", per_usd: 1.0 },
    Bundled { code: "EUR", name: "Euro", symbol: "€", per_usd: 0.92 },
    Bundled { code: "GBP", name: "British Pound", symbol: "£", per_usd: 0.79 },
    Bundled { code: "JPY", name: "Japanese Yen", symbol: "¥", per_usd: 157.2 },
    Bundled { code: "CHF", name: "Swiss Franc", symbol: "₣", per_usd: 0.89 },
    Bundled { code: "CAD", name: "Canadian Dollar", symbol: "$", per_usd: 1.36 },
    Bundled { code: "AUD", name: "Australian Dollar", symbol: "$", per_usd: 1.52 },
    Bundled { code: "CNY", name: "Chinese Yuan", symbol: "¥", per_usd: 7.31 },
    Bundled { code: "HKD", name: "Hong Kong Dollar", symbol: "$", per_usd: 7.81 },
    Bundled { code: "SGD", name: "Singapore Dollar", symbol: "$", per_usd: 1.35 },
    Bundled { code: "INR", name: "Indian Rupee", symbol: "₹", per_usd: 86.5 },
    Bundled { code: "KRW", name: "South Korean Won", symbol: "₩", per_usd: 1475.0 },
    Bundled { code: "BRL", name: "Brazilian Real", symbol: "R$", per_usd: 5.42 },
    Bundled { code: "MXN", name: "Mexican Peso", symbol: "$", per_usd: 17.1 },
    Bundled { code: "SEK", name: "Swedish Krona", symbol: "kr", per_usd: 10.6 },
    Bundled { code: "NOK", name: "Norwegian Krone", symbol: "kr", per_usd: 10.8 },
    Bundled { code: "DKK", name: "Danish Krone", symbol: "kr", per_usd: 6.87 },
    Bundled { code: "PLN", name: "Polish Zloty", symbol: "zł", per_usd: 4.01 },
    Bundled { code: "TRY", name: "Turkish Lira", symbol: "₺", per_usd: 34.5 },
    Bundled { code: "ZAR", name: "South African Rand", symbol: "R", per_usd: 18.7 },
    Bundled { code: "NZD", name: "New Zealand Dollar", symbol: "$", per_usd: 1.68 },
    Bundled { code: "THB", name: "Thai Baht", symbol: "฿", per_usd: 34.4 },
    Bundled { code: "IDR", name: "Indonesian Rupiah", symbol: "Rp", per_usd: 16250.0 },
    Bundled { code: "ILS", name: "Israeli Shekel", symbol: "₪", per_usd: 3.72 },
    Bundled { code: "AED", name: "UAE Dirham", symbol: "د.إ", per_usd: 3.67 },
];

impl CurrencyDef {
    /// An owned currency from a bundled row.
    fn from_bundled(b: &Bundled) -> Self {
        CurrencyDef {
            code: b.code.to_string(),
            name: b.name.to_string(),
            symbol: b.symbol.to_string(),
            per_usd: b.per_usd,
        }
    }
}

/// A stored rates table: codes to units per US dollar, all stamped with
/// one vintage day. The bars are all-or-nothing — a half-fetched table is
/// a lie about the world, so either the whole table means something or
/// nothing is applied.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RatesTable {
    /// The vintage day shared by every row, ISO-formatted.
    pub vintage: String,
    /// Units of each currency per one US dollar, by code.
    pub rates: HashMap<String, f64>,
}

/// A registry of currencies, resolved by code.
#[derive(Debug, Clone)]
pub struct CurrencyRegistry {
    by_code: HashMap<String, CurrencyDef>,
}

impl Default for CurrencyRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl CurrencyRegistry {
    /// Build the registry from the bundled table.
    pub fn new() -> Self {
        Self {
            by_code: CURRENCIES
                .iter()
                .map(|b| (b.code.to_string(), CurrencyDef::from_bundled(b)))
                .collect(),
        }
    }

    /// Apply a stored table over the bundled one.
    ///
    /// A stored code that is also bundled keeps its name and symbol and
    /// takes the fresh rate; a stored code the bundle does not ship is
    /// added, shown by its bare code.
    pub fn with_rates(mut self, table: &RatesTable) -> Self {
        for (code, per_usd) in &table.rates {
            match self.by_code.get_mut(code) {
                Some(def) => def.per_usd = *per_usd,
                None => {
                    self.by_code.insert(
                        code.clone(),
                        CurrencyDef {
                            code: code.clone(),
                            name: code.clone(),
                            symbol: code.clone(),
                            per_usd: *per_usd,
                        },
                    );
                }
            }
        }
        self
    }

    /// Resolve a currency by its three-letter code, case-insensitively.
    pub fn resolve(&self, code: &str) -> Option<&CurrencyDef> {
        self.by_code.get(&code.to_uppercase())
    }
}

/// Convert an amount between two currencies.
pub fn convert(
    amount: f64,
    from: &CurrencyDef,
    to: &CurrencyDef,
) -> Option<f64> {
    if from.per_usd <= 0.0 || to.per_usd <= 0.0 {
        return None;
    }
    let in_usd = amount / from.per_usd;
    Some(in_usd * to.per_usd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_codes_case_insensitively() {
        let registry = CurrencyRegistry::new();
        assert_eq!(registry.resolve("eur").map(|c| c.code.as_str()), Some("EUR"));
        assert!(registry.resolve("xyz").is_none());
    }

    #[test]
    fn converts_through_the_us_dollar() {
        let registry = CurrencyRegistry::new();
        let usd = registry.resolve("USD").unwrap();
        let eur = registry.resolve("EUR").unwrap();
        let euros = convert(100.0, usd, eur).unwrap();
        assert!((euros - 92.0).abs() < 1e-9);
        let back = convert(euros, eur, usd).unwrap();
        assert!((back - 100.0).abs() < 1e-9);
    }

    #[test]
    fn identity_conversion_is_stable() {
        let registry = CurrencyRegistry::new();
        let gbp = registry.resolve("GBP").unwrap();
        let pounds = convert(42.0, gbp, gbp).unwrap();
        assert!((pounds - 42.0).abs() < 1e-9);
    }

    #[test]
    fn a_stored_table_overrides_and_extends() {
        let rates = RatesTable {
            vintage: "2026-10-30".into(),
            rates: HashMap::from([("EUR".into(), 0.90), ("ZWL".into(), 322.0)]),
        };
        let registry = CurrencyRegistry::new().with_rates(&rates);

        // Bundled codes keep their names and take the fresh rate.
        let eur = registry.resolve("EUR").unwrap();
        assert_eq!(eur.name, "Euro");
        assert!((eur.per_usd - 0.90).abs() < 1e-9);

        // Unknown codes join, shown by their bare code.
        let zwl = registry.resolve("zwl").unwrap();
        assert_eq!(zwl.name, "ZWL");
        assert_eq!(zwl.symbol, "ZWL");
        assert!((zwl.per_usd - 322.0).abs() < 1e-9);
    }

    #[test]
    fn an_added_code_converts() {
        let rates = RatesTable {
            vintage: "2026-10-30".into(),
            rates: HashMap::from([("ZWL".into(), 5.0)]),
        };
        let registry = CurrencyRegistry::new().with_rates(&rates);
        let usd = registry.resolve("USD").unwrap();
        let zwl = registry.resolve("ZWL").unwrap();
        let converted = convert(100.0, usd, zwl).unwrap();
        assert!((converted - 500.0).abs() < 1e-9);
    }
}
