//! Currency data and conversion.
//!
//! The bundled rates are **indicative reference values**, not live market
//! quotes. The launcher runs local-only and does not fetch rates, so a
//! conversion is only as current as the table shipped with the build. The
//! table therefore records its own vintage so the UI can say so.
//!
//! The base currency is the US dollar.

use std::collections::HashMap;

/// The date the bundled reference rates were captured.
pub const RATES_AS_OF: &str = "2026-01-15";

/// A currency and its rate against the base currency.
#[derive(Debug, Clone, Copy)]
pub struct CurrencyDef {
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
pub const CURRENCIES: &[CurrencyDef] = &[
    CurrencyDef { code: "USD", name: "US Dollar", symbol: "$", per_usd: 1.0 },
    CurrencyDef { code: "EUR", name: "Euro", symbol: "€", per_usd: 0.92 },
    CurrencyDef { code: "GBP", name: "British Pound", symbol: "£", per_usd: 0.79 },
    CurrencyDef { code: "JPY", name: "Japanese Yen", symbol: "¥", per_usd: 157.2 },
    CurrencyDef { code: "CHF", name: "Swiss Franc", symbol: "₣", per_usd: 0.89 },
    CurrencyDef { code: "CAD", name: "Canadian Dollar", symbol: "$", per_usd: 1.36 },
    CurrencyDef { code: "AUD", name: "Australian Dollar", symbol: "$", per_usd: 1.52 },
    CurrencyDef { code: "CNY", name: "Chinese Yuan", symbol: "¥", per_usd: 7.31 },
    CurrencyDef { code: "HKD", name: "Hong Kong Dollar", symbol: "$", per_usd: 7.81 },
    CurrencyDef { code: "SGD", name: "Singapore Dollar", symbol: "$", per_usd: 1.35 },
    CurrencyDef { code: "INR", name: "Indian Rupee", symbol: "₹", per_usd: 86.5 },
    CurrencyDef { code: "KRW", name: "South Korean Won", symbol: "₩", per_usd: 1475.0 },
    CurrencyDef { code: "BRL", name: "Brazilian Real", symbol: "R$", per_usd: 5.42 },
    CurrencyDef { code: "MXN", name: "Mexican Peso", symbol: "$", per_usd: 17.1 },
    CurrencyDef { code: "SEK", name: "Swedish Krona", symbol: "kr", per_usd: 10.6 },
    CurrencyDef { code: "NOK", name: "Norwegian Krone", symbol: "kr", per_usd: 10.8 },
    CurrencyDef { code: "DKK", name: "Danish Krone", symbol: "kr", per_usd: 6.87 },
    CurrencyDef { code: "PLN", name: "Polish Zloty", symbol: "zł", per_usd: 4.01 },
    CurrencyDef { code: "TRY", name: "Turkish Lira", symbol: "₺", per_usd: 34.5 },
    CurrencyDef { code: "ZAR", name: "South African Rand", symbol: "R", per_usd: 18.7 },
    CurrencyDef { code: "NZD", name: "New Zealand Dollar", symbol: "$", per_usd: 1.68 },
    CurrencyDef { code: "THB", name: "Thai Baht", symbol: "฿", per_usd: 34.4 },
    CurrencyDef { code: "IDR", name: "Indonesian Rupiah", symbol: "Rp", per_usd: 16250.0 },
    CurrencyDef { code: "ILS", name: "Israeli Shekel", symbol: "₪", per_usd: 3.72 },
    CurrencyDef { code: "AED", name: "UAE Dirham", symbol: "د.إ", per_usd: 3.67 },
];

/// A registry of currencies, resolved by code.
#[derive(Debug, Clone)]
pub struct CurrencyRegistry {
    by_code: HashMap<&'static str, &'static CurrencyDef>,
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
            by_code: CURRENCIES.iter().map(|c| (c.code, c)).collect(),
        }
    }

    /// Resolve a currency by its three-letter code, case-insensitively.
    pub fn resolve(&self, code: &str) -> Option<&'static CurrencyDef> {
        let upper = code.to_uppercase();
        self.by_code.get(upper.as_str()).copied()
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
        assert_eq!(registry.resolve("eur").map(|c| c.code), Some("EUR"));
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
}
