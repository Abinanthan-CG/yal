//! Runtime evaluation context for the calculator: clock, local time zone,
//! exchange rates, and user settings (such as CSS `rem` pixel base).

use std::collections::HashMap;
use std::sync::Arc;

use jiff::tz::TimeZone;
use jiff::{Timestamp, Zoned};
use serde::{Deserialize, Serialize};

/// Snapshot of exchange rates expressed as units of each currency per 1 USD.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExchangeRates {
    /// Base currency code (always `"USD"`).
    pub base: String,
    /// ISO date string when rates were captured (`YYYY-MM-DD`).
    pub updated_date: String,
    /// Unix timestamp (seconds) when rates were fetched (`0` for bundled fallback).
    pub fetched_at_secs: i64,
    /// Units of currency per 1 USD (e.g. `EUR => 0.92`, `INR => 86.5`, `BTC => 0.0000105`).
    pub per_usd: HashMap<String, f64>,
}

impl ExchangeRates {
    /// Indicative bundled fallback rates so currency & crypto conversions work
    /// even when offline, until the live background feed updates them.
    pub fn bundled() -> Self {
        let pairs: &[(&str, f64)] = &[
            // Major & regional fiat currencies (per 1 USD, indicative fallback snapshot)
            ("USD", 1.0),
            ("EUR", 0.92),
            ("GBP", 0.79),
            ("JPY", 154.0),
            ("CHF", 0.88),
            ("CAD", 1.39),
            ("AUD", 1.53),
            ("NZD", 1.69),
            ("CNY", 7.24),
            ("HKD", 7.78),
            ("SGD", 1.34),
            ("INR", 84.5),
            ("KRW", 1395.0),
            ("TWD", 32.5),
            ("THB", 34.5),
            ("MYR", 4.45),
            ("IDR", 15850.0),
            ("PHP", 58.6),
            ("VND", 25400.0),
            ("PKR", 278.0),
            ("BDT", 119.5),
            ("LKR", 293.0),
            ("NPR", 135.2),
            ("BRL", 5.80),
            ("MXN", 20.3),
            ("ARS", 1005.0),
            ("CLP", 975.0),
            ("COP", 4400.0),
            ("PEN", 3.78),
            ("SEK", 10.9),
            ("NOK", 11.0),
            ("DKK", 6.87),
            ("PLN", 4.08),
            ("CZK", 23.8),
            ("HUF", 385.0),
            ("RON", 4.58),
            ("BGN", 1.80),
            ("TRY", 34.4),
            ("RUB", 98.0),
            ("UAH", 41.3),
            ("ILS", 3.74),
            ("AED", 3.6725),
            ("SAR", 3.75),
            ("QAR", 3.64),
            ("KWD", 0.307),
            ("BHD", 0.376),
            ("OMR", 0.385),
            ("EGP", 49.3),
            ("ZAR", 18.1),
            ("NGN", 1650.0),
            ("KES", 129.0),
            ("MAD", 9.95),
            ("ISK", 138.0),
            // Major cryptocurrencies (units of coin per 1 USD = 1 / USD price)
            ("BTC", 1.0 / 92_000.0),
            ("ETH", 1.0 / 3_100.0),
            ("SOL", 1.0 / 210.0),
            ("BNB", 1.0 / 620.0),
            ("XRP", 1.0 / 1.10),
            ("ADA", 1.0 / 0.75),
            ("DOGE", 1.0 / 0.38),
            ("AVAX", 1.0 / 35.0),
            ("DOT", 1.0 / 6.0),
            ("POL", 1.0 / 0.45),
            ("LTC", 1.0 / 85.0),
            ("BCH", 1.0 / 440.0),
            ("XLM", 1.0 / 0.25),
            ("XMR", 1.0 / 160.0),
            ("TRX", 1.0 / 0.20),
            ("SHIB", 1.0 / 0.000025),
            ("USDT", 1.0),
            ("USDC", 1.0),
            ("DASH", 1.0 / 28.0),
            ("EOS", 1.0 / 0.65),
            ("ETC", 1.0 / 26.0),
            ("NEO", 1.0 / 13.0),
        ];
        let mut per_usd = HashMap::with_capacity(pairs.len());
        for &(code, rate) in pairs {
            per_usd.insert(code.to_string(), rate);
        }
        Self {
            base: "USD".to_string(),
            updated_date: "bundled".to_string(),
            fetched_at_secs: 0,
            per_usd,
        }
    }

    /// Look up units per 1 USD for an ISO/crypto code (case-insensitive).
    pub fn rate(&self, code: &str) -> Option<f64> {
        let key = code.trim().to_ascii_uppercase();
        self.per_usd
            .get(&key)
            .copied()
            .filter(|r| r.is_finite() && *r > 0.0)
    }

    /// Convert `amount` from `from_code` into `to_code`.
    pub fn convert(&self, amount: f64, from_code: &str, to_code: &str) -> Option<f64> {
        let from = self.rate(from_code)?;
        let to = self.rate(to_code)?;
        let result = (amount / from) * to;
        if result.is_finite() {
            Some(result)
        } else {
            None
        }
    }

    /// Merge live fiat rates (`code -> per_usd`) over the existing table.
    pub fn merge_fiat(&mut self, rates: HashMap<String, f64>, updated_date: String, now_secs: i64) {
        for (code, rate) in rates {
            if rate.is_finite() && rate > 0.0 {
                self.per_usd.insert(code.to_ascii_uppercase(), rate);
            }
        }
        self.per_usd.insert("USD".to_string(), 1.0);
        self.updated_date = updated_date;
        self.fetched_at_secs = now_secs;
    }

    /// Merge crypto USD prices (`code -> usd_price_per_coin`) by inverting to per-USD.
    pub fn merge_crypto_usd_prices(&mut self, usd_prices: HashMap<String, f64>) {
        for (code, usd_price) in usd_prices {
            if usd_price.is_finite() && usd_price > 0.0 {
                let per_usd = 1.0 / usd_price;
                if per_usd.is_finite() && per_usd > 0.0 {
                    self.per_usd.insert(code.to_ascii_uppercase(), per_usd);
                }
            }
        }
    }
}

/// Environment inputs injected into `evaluate_with_context`.
#[derive(Debug, Clone)]
pub struct CalcContext {
    /// Current moment in UTC.
    pub now_utc: Timestamp,
    /// User's local time zone (for `now`, `today`, `time diff`, and ISO conversion).
    pub local_tz: TimeZone,
    /// Exchange rates table (fiat + crypto). `None` disables currency conversions.
    pub rates: Option<Arc<ExchangeRates>>,
    /// Base pixels per `1rem` / `1em` for CSS unit conversions (default: 16.0).
    pub rem_px: f64,
    /// Use 24-hour clock output when `true`, 12-hour AM/PM when `false`.
    pub use_24h_clock: bool,
}

impl CalcContext {
    /// Live host environment context using the system clock, system time zone,
    /// and bundled fallback rates.
    pub fn live() -> Self {
        Self {
            now_utc: Timestamp::now(),
            local_tz: TimeZone::system(),
            rates: Some(Arc::new(ExchangeRates::bundled())),
            rem_px: 16.0,
            use_24h_clock: false,
        }
    }

    /// Deterministic context pinned to a UTC timestamp and time zone for tests.
    pub fn fixed(now_utc: Timestamp, tz: TimeZone) -> Self {
        Self {
            now_utc,
            local_tz: tz,
            rates: Some(Arc::new(ExchangeRates::bundled())),
            rem_px: 16.0,
            use_24h_clock: false,
        }
    }

    /// Return a copy using a specific rates snapshot.
    pub fn with_rates(mut self, rates: Option<Arc<ExchangeRates>>) -> Self {
        self.rates = rates;
        self
    }

    /// Return a copy using a custom CSS `rem` pixel size.
    pub fn with_rem_px(mut self, rem_px: f64) -> Self {
        if rem_px.is_finite() && rem_px > 0.0 {
            self.rem_px = rem_px;
        }
        self
    }

    /// The current moment in the user's local time zone.
    pub fn local_now(&self) -> Zoned {
        self.now_utc.to_zoned(self.local_tz.clone())
    }
}

impl Default for CalcContext {
    fn default() -> Self {
        Self::live()
    }
}
