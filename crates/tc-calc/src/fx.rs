//! Clean-room currency & cryptocurrency conversion engine, mixed-currency
//! arithmetic, and live rate-feed JSON parsers.

use std::collections::HashMap;
use std::path::Path;

use crate::catalog::Catalog;
use crate::context::{CalcContext, ExchangeRates};
use crate::currency::{self, CurrencyDef};
use crate::formatter::Formatter;
use crate::parser::Parser;
use crate::result::{CalcResult, Payload};
use crate::token::{Operator, Token};

/// Public zero-API-key fiat exchange-rate endpoint (USD base, 160+ currencies).
pub const FIAT_FEED_URL: &str = "https://open.er-api.com/v6/latest/USD";

/// Public zero-API-key CoinGecko simple price endpoint for major coins.
pub const CRYPTO_FEED_URL: &str = "https://api.coingecko.com/api/v3/simple/price?ids=bitcoin,ethereum,solana,binancecoin,ripple,cardano,dogecoin,avalanche-2,polkadot,matic-network,litecoin,bitcoin-cash,stellar,monero,tron,shiba-inu,tether,usd-coin,dash,eos,ethereum-classic,neo&vs_currencies=usd";

/// Refresh interval for cached live rates (24 hours in seconds).
pub const RATES_REFRESH_SECS: i64 = 24 * 3600;

/// Format a monetary amount with sub-cent widening so tiny cross-rates
/// (e.g. `100 IDR to USD` or `100 USD to BTC`) do not collapse to `0.00`.
pub fn format_money_amount(value: f64) -> String {
    if !value.is_finite() {
        return "0.00".to_string();
    }
    let abs = value.abs();
    if abs == 0.0 || abs < 1e-12 {
        return "0.00".to_string();
    }
    if abs >= 0.01 {
        return Formatter::currency(value);
    }
    // Widen decimals until at least 2 significant digits appear (up to 8 decimals).
    for decimals in 3..=8 {
        let formatted = format!("{value:.decimals$}");
        let trimmed = formatted.trim_end_matches('0');
        let sig_digits = trimmed
            .chars()
            .filter(|c| c.is_ascii_digit())
            .skip_while(|&c| c == '0')
            .count();
        if sig_digits >= 2 || decimals == 8 {
            return trimmed.trim_end_matches('.').to_string();
        }
    }
    Formatter::currency(value)
}

/// Build a money `Payload` with smart sub-cent widening.
pub fn money_payload(value: f64, code: &str) -> Payload {
    let text = format_money_amount(value);
    Payload::Value {
        display: format!("{} {code}", Formatter::grouped(&text)),
        copy_text: format!("{text} {code}"),
    }
}

/// Evaluate currency conversions (`100 usd in inr`, `0.5 btc in usd`,
/// `USD1K in EUR`, `13 million idr to usd`) and mixed-currency sums
/// (`100 usd + 50 eur`, `100 usd + 50 eur in inr`).
pub fn evaluate(
    tokens: &[Token],
    query: &str,
    catalog: &dyn Catalog,
    ctx: &CalcContext,
) -> Option<CalcResult> {
    let rates = ctx.rates.as_deref()?;

    if let Some(card) = evaluate_conversion(tokens, catalog, rates) {
        return Some(card);
    }
    if let Some(card) = evaluate_mixed_currency_sum(tokens, query, catalog, rates) {
        return Some(card);
    }
    None
}

/// Parse `<expr/amount with currency> (to|in|as|->) <target currency>` or
/// adjacent `<amount> <from_currency> <to_currency>` (e.g. `0.5 btc usd`).
fn evaluate_conversion(
    tokens: &[Token],
    catalog: &dyn Catalog,
    rates: &ExchangeRates,
) -> Option<CalcResult> {
    if tokens.len() < 2 {
        return None;
    }

    // Form A: `<source...> (to|in|as|->) <target_currency>`
    if let Some((source_tokens, target_curr)) = split_currency_target(tokens) {
        if let Some((amount, from_curr)) = eval_currency_source(source_tokens, catalog, rates) {
            // Never steal if the target word is a known unit AND from is not a currency.
            let Some(converted) = rates.convert(amount, from_curr.code, target_curr.code) else {
                return Some(CalcResult::new(
                    format!("{} {}", Formatter::display(amount), from_curr.code),
                    Payload::Error {
                        message: format!("No exchange rate for {}.", target_curr.code),
                    },
                ));
            };
            let mut card = CalcResult::with_badges(
                format!("{} {}", Formatter::display(amount), from_curr.code),
                from_curr.name,
                target_curr.name,
                money_payload(converted, target_curr.code),
            );
            card.is_currency = true;
            return Some(card);
        }
    }

    // Form B: Bare pair without connector: `100 usd inr`, `0.5 btc usd`, `$100 eur`.
    // Only match when the prefix is a plain single amount + currency (no binary operators),
    // so `100 usd + 92 eur` falls through to mixed-currency addition instead.
    if let Some(last_curr) = token_as_currency(tokens.last()?) {
        let prefix = &tokens[..tokens.len() - 1];
        let has_binary_op = prefix.iter().any(|t| {
            matches!(
                t,
                Token::Op(
                    Operator::Add | Operator::Subtract | Operator::Multiply | Operator::Divide
                )
            )
        });
        if !prefix.is_empty() && !has_binary_op && !ends_with_connector(prefix) {
            if let Some((amount, from_curr)) = eval_currency_source(prefix, catalog, rates) {
                if from_curr.code != last_curr.code {
                    let converted = rates.convert(amount, from_curr.code, last_curr.code)?;
                    let mut card = CalcResult::with_badges(
                        format!("{} {}", Formatter::display(amount), from_curr.code),
                        from_curr.name,
                        last_curr.name,
                        money_payload(converted, last_curr.code),
                    );
                    card.is_currency = true;
                    return Some(card);
                }
            }
        }
    }

    None
}

/// Evaluate mixed-currency addition/subtraction (`100 usd + 50 eur` answers in
/// the first currency; `100 usd + 50 eur in inr` is handled by `evaluate_conversion`).
fn evaluate_mixed_currency_sum(
    tokens: &[Token],
    query: &str,
    catalog: &dyn Catalog,
    rates: &ExchangeRates,
) -> Option<CalcResult> {
    let (amount, target_curr) = eval_mixed_sum_terms(tokens, catalog, rates)?;
    let mut card = CalcResult::with_badges(
        Formatter::expression(query),
        "Expression",
        target_curr.name,
        money_payload(amount, target_curr.code),
    );
    card.is_currency = true;
    Some(card)
}

fn eval_currency_source(
    tokens: &[Token],
    catalog: &dyn Catalog,
    rates: &ExchangeRates,
) -> Option<(f64, CurrencyDef)> {
    if tokens.is_empty() {
        return None;
    }

    // Bare single currency word (`usd to inr` -> implied 1 USD).
    if tokens.len() == 1 {
        let curr = token_as_currency(&tokens[0])?;
        return Some((1.0, curr));
    }

    // Standard single-currency expression parsed by `Parser` (handles `$100`,
    // `USD1K`, `13 million idr`, `(100 + 25) usd`, `20% off $100`, etc.).
    let mut parser = Parser::new(tokens);
    if let Ok(val) = parser.parse(catalog) {
        if !val.is_boolean {
            if let Some(curr) = val.currency {
                return Some((val.amount, curr));
            }
        }
    }

    // Fallback: mixed-currency sum on the LHS (e.g. `100 usd + 50 eur in inr`).
    eval_mixed_sum_terms(tokens, catalog, rates)
}

fn eval_mixed_sum_terms(
    tokens: &[Token],
    catalog: &dyn Catalog,
    rates: &ExchangeRates,
) -> Option<(f64, CurrencyDef)> {
    // Split at top-level `+` / `-` operators.
    let mut segments: Vec<(f64, &[Token])> = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    let mut sign = 1.0_f64;

    for (i, tok) in tokens.iter().enumerate() {
        match tok {
            Token::Op(Operator::Open) => depth += 1,
            Token::Op(Operator::Close) => depth = depth.saturating_sub(1),
            Token::Op(Operator::Add) if depth == 0 && i > start => {
                segments.push((sign, &tokens[start..i]));
                sign = 1.0;
                start = i + 1;
            }
            Token::Op(Operator::Subtract) if depth == 0 && i > start => {
                segments.push((sign, &tokens[start..i]));
                sign = -1.0;
                start = i + 1;
            }
            _ => {}
        }
    }
    if start < tokens.len() {
        segments.push((sign, &tokens[start..]));
    }
    if segments.len() < 2 {
        return None;
    }

    let mut anchor_curr: Option<CurrencyDef> = None;
    let mut saw_distinct = false;
    let mut total = 0.0_f64;

    for (term_sign, seg) in segments {
        let mut p = Parser::new(seg);
        let val = p.parse(catalog).ok()?;
        if val.is_boolean || val.unit.is_some() {
            return None;
        }
        match val.currency {
            Some(curr) => {
                let target = *anchor_curr.get_or_insert(curr);
                if curr.code != target.code {
                    saw_distinct = true;
                }
                let in_target = rates.convert(val.amount, curr.code, target.code)?;
                total += term_sign * in_target;
            }
            None => {
                // Plain scalar added to money stays in anchor currency.
                anchor_curr?;
                total += term_sign * val.amount;
            }
        }
    }

    if !saw_distinct {
        return None;
    }
    Some((total, anchor_curr?))
}

fn split_currency_target(tokens: &[Token]) -> Option<(&[Token], CurrencyDef)> {
    let n = tokens.len();
    if n < 3 {
        return None;
    }
    // Check 1-word target currency after connector (`to`, `in`, `as`, `->`).
    if is_connector(&tokens[n - 2]) {
        if let Some(curr) = token_as_currency(&tokens[n - 1]) {
            return Some((&tokens[..n - 2], curr));
        }
    }
    // Check 2-word currency name after connector (e.g., `us dollar`, `indian rupee`).
    if n >= 4 && is_connector(&tokens[n - 3]) {
        if let (Token::Ident(a), Token::Ident(b)) = (&tokens[n - 2], &tokens[n - 1]) {
            let phrase = format!("{a} {b}");
            if let Some(curr) = currency::by_name(&phrase) {
                return Some((&tokens[..n - 3], curr));
            }
        }
    }
    None
}

fn token_as_currency(token: &Token) -> Option<CurrencyDef> {
    let Token::Ident(name) = token else {
        return None;
    };
    // Keep unit collisions owned by units (e.g., `cup` is volume, not Cuban peso).
    if crate::units::by_name(name).is_some()
        && !matches!(
            name.to_ascii_lowercase().as_str(),
            "pound" | "pounds" | "lb" | "lbs" | "sol" | "btc" | "eth"
        )
    {
        return None;
    }
    // `pound`/`pounds` stays weight unless explicitly paired with another currency;
    // currency symbols/codes like `gbp`, `usd`, `inr`, `eur`, `btc`, `eth`, `sol` always match.
    if matches!(name.to_ascii_lowercase().as_str(), "lb" | "lbs") {
        return None;
    }
    currency::by_name(name)
}

fn is_connector(token: &Token) -> bool {
    match token {
        Token::Arrow => true,
        Token::Ident(s) => matches!(s.as_str(), "to" | "in" | "as"),
        _ => false,
    }
}

fn ends_with_connector(tokens: &[Token]) -> bool {
    tokens.last().is_some_and(is_connector)
}

/// Parse the JSON payload from `https://open.er-api.com/v6/latest/USD`.
pub fn parse_open_er_api_json(json: &str) -> Option<(HashMap<String, f64>, String)> {
    let val: serde_json::Value = serde_json::from_str(json).ok()?;
    if val.get("result").and_then(|v| v.as_str()) != Some("success") {
        return None;
    }
    let rates_obj = val.get("rates").and_then(|v| v.as_object())?;
    let mut rates = HashMap::with_capacity(rates_obj.len());
    for (code, rate_val) in rates_obj {
        if let Some(rate) = rate_val.as_f64() {
            if rate.is_finite() && rate > 0.0 {
                rates.insert(code.to_ascii_uppercase(), rate);
            }
        }
    }
    if rates.is_empty() {
        return None;
    }
    let updated_date = val
        .get("time_last_update_utc")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "live".to_string());
    Some((rates, updated_date))
}

/// Parse the JSON payload from CoinGecko `/simple/price` into `TICKER -> USD price`.
pub fn parse_coingecko_simple_json(json: &str) -> Option<HashMap<String, f64>> {
    const ID_TO_CODE: &[(&str, &str)] = &[
        ("bitcoin", "BTC"),
        ("ethereum", "ETH"),
        ("solana", "SOL"),
        ("binancecoin", "BNB"),
        ("ripple", "XRP"),
        ("cardano", "ADA"),
        ("dogecoin", "DOGE"),
        ("avalanche-2", "AVAX"),
        ("polkadot", "DOT"),
        ("matic-network", "POL"),
        ("litecoin", "LTC"),
        ("bitcoin-cash", "BCH"),
        ("stellar", "XLM"),
        ("monero", "XMR"),
        ("tron", "TRX"),
        ("shiba-inu", "SHIB"),
        ("tether", "USDT"),
        ("usd-coin", "USDC"),
        ("dash", "DASH"),
        ("eos", "EOS"),
        ("ethereum-classic", "ETC"),
        ("neo", "NEO"),
    ];

    let val: serde_json::Value = serde_json::from_str(json).ok()?;
    let obj = val.as_object()?;
    let mut prices = HashMap::new();
    for &(id, code) in ID_TO_CODE {
        if let Some(usd) = obj
            .get(id)
            .and_then(|entry| entry.get("usd"))
            .and_then(|v| v.as_f64())
        {
            if usd.is_finite() && usd > 0.0 {
                prices.insert(code.to_string(), usd);
            }
        }
    }
    if prices.is_empty() {
        None
    } else {
        Some(prices)
    }
}

/// Load a cached `ExchangeRates` snapshot from disk, falling back to bundled rates.
pub fn load_rates_cache(path: &Path) -> ExchangeRates {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<ExchangeRates>(&text).ok())
        .filter(|r| !r.per_usd.is_empty())
        .unwrap_or_else(ExchangeRates::bundled)
}

/// Persist an `ExchangeRates` snapshot atomically to disk.
pub fn save_rates_cache(path: &Path, rates: &ExchangeRates) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create rates dir: {e}"))?;
    }
    let json =
        serde_json::to_string_pretty(rates).map_err(|e| format!("encode rates json: {e}"))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json).map_err(|e| format!("write temp rates: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("commit rates file: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::FullCatalog;
    use crate::tokenizer::tokenize;
    use jiff::tz::TimeZone;
    use jiff::Timestamp;
    use std::sync::Arc;

    fn test_ctx() -> CalcContext {
        let mut rates = ExchangeRates::bundled();
        rates.per_usd.insert("USD".into(), 1.0);
        rates.per_usd.insert("EUR".into(), 0.92);
        rates.per_usd.insert("GBP".into(), 0.79);
        rates.per_usd.insert("INR".into(), 86.5);
        rates.per_usd.insert("JPY".into(), 157.0);
        rates.per_usd.insert("BTC".into(), 0.00001); // 1 BTC = $100,000
        CalcContext::fixed(Timestamp::UNIX_EPOCH, TimeZone::UTC).with_rates(Some(Arc::new(rates)))
    }

    fn eval_fx(query: &str) -> Option<CalcResult> {
        let catalog = FullCatalog;
        let tokens = tokenize(query, &catalog)?;
        evaluate(&tokens, query, &catalog, &test_ctx())
    }

    #[test]
    fn converts_fiat_and_crypto_pairs() {
        let r = eval_fx("100 usd in inr").expect("100 usd in inr");
        assert_eq!(
            r.payload,
            Payload::Value {
                display: "8,650.00 INR".to_string(),
                copy_text: "8650.00 INR".to_string(),
            }
        );

        let btc = eval_fx("0.5 btc in usd").expect("0.5 btc in usd");
        assert_eq!(
            btc.payload,
            Payload::Value {
                display: "50,000.00 USD".to_string(),
                copy_text: "50000.00 USD".to_string(),
            }
        );

        let bare = eval_fx("0.5 btc usd").expect("bare pair");
        assert_eq!(
            bare.payload,
            Payload::Value {
                display: "50,000.00 USD".to_string(),
                copy_text: "50000.00 USD".to_string(),
            }
        );
    }

    #[test]
    fn converts_mixed_currency_sums_and_shorthand() {
        // 100 USD + 92 EUR (= 100 USD) = 200 USD
        let sum = eval_fx("100 usd + 92 eur").expect("mixed sum");
        assert_eq!(
            sum.payload,
            Payload::Value {
                display: "200.00 USD".to_string(),
                copy_text: "200.00 USD".to_string(),
            }
        );

        // 100 USD + 92 EUR in GBP (= 200 * 0.79 = 158 GBP)
        let converted = eval_fx("100 usd + 92 eur in gbp").expect("mixed sum converted");
        assert_eq!(
            converted.payload,
            Payload::Value {
                display: "158.00 GBP".to_string(),
                copy_text: "158.00 GBP".to_string(),
            }
        );

        let k = eval_fx("USD1K in EUR").expect("USD1K");
        assert_eq!(
            k.payload,
            Payload::Value {
                display: "920.00 EUR".to_string(),
                copy_text: "920.00 EUR".to_string(),
            }
        );
    }

    #[test]
    fn sub_cent_amounts_widen_instead_of_collapsing_to_zero() {
        let small = format_money_amount(0.00053);
        assert_eq!(small, "0.00053");
        let tiny_btc = eval_fx("100 usd in btc").expect("100 usd in btc");
        match tiny_btc.payload {
            Payload::Value { display, .. } => assert_eq!(display, "0.001 BTC"),
            other => panic!("unexpected payload: {other:?}"),
        }
    }

    #[test]
    fn parses_open_er_api_and_coingecko_payloads() {
        let fiat = r#"{"result":"success","time_last_update_utc":"Sun, 22 Nov 2026","rates":{"USD":1,"EUR":0.91,"INR":85.5}}"#;
        let (map, date) = parse_open_er_api_json(fiat).expect("valid fiat json");
        assert_eq!(map.get("INR"), Some(&85.5));
        assert!(date.contains("2026"));

        let crypto = r#"{"bitcoin":{"usd":95000.0},"ethereum":{"usd":3200.5}}"#;
        let cmap = parse_coingecko_simple_json(crypto).expect("valid crypto json");
        assert_eq!(cmap.get("BTC"), Some(&95000.0));
        assert_eq!(cmap.get("ETH"), Some(&3200.5));
    }
}
