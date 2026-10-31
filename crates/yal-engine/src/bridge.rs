//! The bridge between the file index's walker and the store.
//!
//! `yal-index` may not know the store, and the store may not know the
//! index: the application layer is the one place that is allowed to see
//! both. Because neither side can own the `impl (the trait lives in
//! yal-index and the type in yal-store, and Rust's coherence rules only
//! let a foreign trait be implemented for a foreign type from nearer),
//! the store gets a thin local wrapper here, [`StoreBacking`], and the
//! snapshot helpers that turn indexed rows into query material live
//! beside it.

use std::path::PathBuf;
use yal_index::walk::Backing;
use yal_store::Store;

/// The store behind the walker's [`Backing`] interface.
///
/// The trait's `&mut self` is only a formality here — every operation is
/// a transaction of reads and writes the store offers through `&self` —
/// so the wrapper forwards and swallows nothing: an index write that
/// fails is a warning, and the pass continues, which is the index's own
/// best-effort contract.
struct StoreBacking<'a>(&'a Store);

impl Backing for StoreBacking<'_> {
    fn recorded_dirs(&self) -> Vec<(String, i64)> {
        self.0.indexed_dirs().unwrap_or_default()
    }

    fn remember_dirs(&mut self, rows: &[(String, i64)]) {
        if let Err(error) = self.0.record_dirs(rows) {
            tracing::warn!(error = %error, "the index could not record directories");
        }
    }

    fn rewrite_files(&mut self, dir: &str, rows: &[(String, String)]) {
        if let Err(error) = self.0.rewrite_dir_files(dir, rows) {
            tracing::warn!(error = %error, "the index could not rewrite a directory's files");
        }
    }

    fn forget_dirs(&mut self, dirs: &[String]) {
        if let Err(error) = self.0.forget_dirs(dirs) {
            tracing::warn!(error = %error, "the index could not forget directories");
        }
    }
}

/// Whether the whole-drive index answers queries: the setting exists so
/// the user can turn the supplement off, and a missing setting means on —
/// the index is silent about nothing until it is told.
pub fn file_index_enabled(store: &Store) -> bool {
    store
        .setting("file_index")
        .ok()
        .flatten()
        .as_deref()
        != Some("off")
}

/// Load the query snapshot out of the index tables.
///
/// The snapshot is the list of paths only; the few hits a query finds
/// have their name and directory recomputed from the path, which keeps
/// the load a single cursor and keeps the empty-index case free.
pub fn snapshot(store: &Store) -> Vec<PathBuf> {
    if !file_index_enabled(store) {
        return Vec::new();
    }
    store
        .indexed_paths()
        .unwrap_or_default()
        .into_iter()
        .map(PathBuf::from)
        .collect()
}

/// Run one refresh pass over the whole fixed drives.
///
/// The command line calls this for `file-index --refresh`, which is how
/// the walk's cost is measured and recorded; the walker thread calls it
/// on its own schedule.
pub fn refresh_once(store: &Store) -> yal_index::walk::WalkReport {
    let roots = yal_index::walk::fixed_drive_roots();
    let mut backing = StoreBacking(store);
    yal_index::walk::walk_once(&mut backing, &roots)
}

/// The stored exchange rates, as a table the evaluator can apply.
///
/// The rows were written whole, so they share a vintage; taking the
/// newest as the table's own is a formality that also holds if an older
/// update stored a different source. A rate that is not finite and
/// positive has no currency meaning, so it is not applied.
pub fn rates(store: &Store) -> Option<yal_calc::RatesTable> {
    let rows = store.rates().ok()?;
    let mut table = yal_calc::RatesTable::default();
    for (code, per_usd, as_of) in rows {
        if per_usd.is_finite() && per_usd > 0.0 {
            table.rates.insert(code, per_usd);
            table.vintage = table.vintage.max(as_of);
        }
    }
    if table.rates.is_empty() {
        return None;
    }
    Some(table)
}

/// Shape a fetched document into rate rows with their shared vintage.
///
/// The reply's shape, observed live (2026-10-31): `{ "amount": 1.0,
/// "base": "USD", "date": "2026-10-30", "rates": { "AUD": 1.42, ... } }`.
/// `base_date` is accepted as an older spelling. It is accepted only
/// whole: three-letter uppercase codes, finite positive rates, a
/// present vintage — one doubtful row throws the whole reply out, and
/// nothing is stored, because a half-fetched table is a lie about the
/// world. The US dollar anchors the table and is set as itself; a
/// `base=USD` reply lists no row of its own.
pub fn rates_from_document(
    document: &serde_json::Value,
) -> Result<(Vec<yal_store::rates::RateRow>, String), String> {
    let rates = document
        .get("rates")
        .and_then(|value| value.as_object())
        .ok_or_else(|| "the reply carries no rates table".to_string())?;
    let vintage = ["date", "base_date"]
        .iter()
        .find_map(|field| document.get(*field).and_then(|value| value.as_str()))
        .ok_or_else(|| "the reply carries no vintage day".to_string())
        .map(str::to_string)?;

    let mut rows = Vec::new();
    for (code, value) in rates {
        if code.len() != 3 || !code.chars().all(|c| c.is_ascii_uppercase()) {
            return Err(format!("`{code}` is not a three-letter currency code"));
        }
        let per_usd = value
            .as_f64()
            .ok_or_else(|| format!("`{code}` is not a plain rate"))?;
        if !per_usd.is_finite() || per_usd <= 0.0 {
            return Err(format!("`{code}` carries no usable rate"));
        }
        rows.push((code.clone(), per_usd, vintage.clone()));
    }
    if rows.is_empty() {
        return Err("the reply listed no currencies".to_string());
    }
    rows.push(("USD".to_string(), 1.0, vintage.clone()));
    Ok((rows, vintage))
}

/// Fetch a fresh rate table once and store it whole.
///
/// This is the launcher's one network act, and it costs the user an
/// explicit ask — the command line or the palette's act, never a
/// schedule. The fetch holds fifteen seconds and two megabytes; the
/// reply is validated whole before a row is written. Returns how many
/// codes the table now holds and its vintage day.
pub fn update_rates(store: &Store) -> Result<(usize, String), String> {
    let document = yal_system::net::fetch_json(
        yal_system::net::DEFAULT_RATES_URL,
        std::time::Duration::from_secs(15),
        2_000_000,
    )
    .map_err(|error| error.to_string())?;
    let (rows, vintage) = rates_from_document(&document)?;
    store.replace_rates(&rows).map_err(|error| error.to_string())?;
    Ok((rows.len(), vintage))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn shapes_a_fetched_table_offline() {
        // The observed live shape: the reply stamps its vintage `date`.
        let document = json!({
            "amount": 1.0,
            "base": "USD",
            "date": "2026-10-30",
            "rates": { "EUR": 0.92, "JPY": 157.2 }
        });
        let (rows, vintage) = rates_from_document(&document).unwrap();
        assert_eq!(vintage, "2026-10-30");
        // EUR and JPY, plus the USD row the reply omitted.
        assert_eq!(rows.len(), 3);
        assert!(rows.iter().any(|(code, code_usd, _)| code == "USD" && *code_usd == 1.0));
        assert!(rows.iter().any(|(code, code_eur, _)| code == "EUR" && *code_eur == 0.92));
    }

    #[test]
    fn the_less_common_vintage_spelling_is_accepted() {
        let document = json!({
            "base": "USD", "base_date": "2026-10-30",
            "rates": { "EUR": 0.92 }
        });
        let (_, vintage) = rates_from_document(&document).unwrap();
        assert_eq!(vintage, "2026-10-30");
    }

    #[test]
    fn a_doubtful_reply_is_thrown_out_in_full() {
        assert!(rates_from_document(&json!({ "rates": { "EUR": "free" } })).is_err());
        assert!(rates_from_document(&json!({ "rates": { "euri": 0.92 } })).is_err());
        assert!(rates_from_document(&json!({ "rates": { "EUR": -0.92 } })).is_err());
        assert!(rates_from_document(&json!({ "rates": { "EUR": 0.0 } })).is_err());
        assert!(rates_from_document(&json!({ "rates": {} })).is_err());
        assert!(rates_from_document(&json!({ "EUR": 0.92 })).is_err());
    }

    #[test]
    fn the_stored_table_round_trips() {
        let store = yal_store::Store::open_in_memory().unwrap();
        assert!(rates(&store).is_none());

        // The observed spelling of the vintage field; the fallback is
        // covered by `the_less_common_vintage_spelling_is_accepted`.
        let document = json!({ "date": "2026-10-30", "rates": { "EUR": 0.92 } });
        let (rows, vintage) = rates_from_document(&document).unwrap();
        store.replace_rates(&rows).unwrap();

        let table = rates(&store).unwrap();
        assert_eq!(table.vintage, vintage);
        assert!((table.rates["EUR"] - 0.92).abs() < 1e-9);

        // And the extended registry answers a conversion fresh.
        let expr = yal_calc::parse("100 usd in eur").unwrap();
        let registry = yal_calc::CurrencyRegistry::new().with_rates(&table);
        let value = yal_calc::Evaluator::with(chrono::Utc::now(), registry).eval(&expr);
        match value.unwrap() {
            yal_calc::Value::Currency { amount, code } => {
                assert_eq!(code, "EUR");
                assert!((amount - 92.0).abs() < 1e-9);
            }
            other => panic!("expected a currency, got {other:?}"),
        }
    }
}
