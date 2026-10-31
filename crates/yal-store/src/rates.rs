//! The exchange-rate table.
//!
//! The schema lives in migration 3 (see `schema.rs`). The writes here are
//! whole-table affairs: an update either produced a complete, validated
//! set — one shared vintage, every code stamped — or nothing was written,
//! so a conversion can never answer from a half-fetched world.

use crate::{Store, StoreResult, params};

/// A stored rate: the currency code, units of it per one US dollar, and
/// the vintage day the source stamped.
pub type RateRow = (String, f64, String);

impl Store {
    /// Replace the whole rates table, in one transaction.
    pub fn replace_rates(&self, rows: &[RateRow]) -> StoreResult<()> {
        self.conn.execute_batch("BEGIN IMMEDIATE")?;
        let result = self.rewrite_rates(rows);
        match result {
            Ok(()) => self.conn.execute_batch("COMMIT")?,
            Err(error) => {
                self.conn.execute_batch("ROLLBACK").ok();
                return Err(error);
            }
        }
        Ok(())
    }

    /// The delete-and-refill, so the transaction's rollback can undo it.
    fn rewrite_rates(&self, rows: &[RateRow]) -> StoreResult<()> {
        self.conn.execute("DELETE FROM rates", [])?;
        let mut statement = self.conn.prepare_cached(
            "INSERT INTO rates (code, per_usd, as_of) VALUES (?1, ?2, ?3)",
        )?;
        for (code, per_usd, as_of) in rows {
            statement.execute(params![code, per_usd, as_of])?;
        }
        Ok(())
    }

    /// Every stored rate, in code order.
    pub fn rates(&self) -> StoreResult<Vec<RateRow>> {
        let mut statement =
            self.conn.prepare("SELECT code, per_usd, as_of FROM rates ORDER BY code")?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?, row.get::<_, String>(2)?))
        })?;
        let mut rates = Vec::new();
        for rate in rows {
            rates.push(rate?);
        }
        Ok(rates)
    }

    /// The vintage day the stored rows share, if the table holds any.
    ///
    /// All rows are written with one vintage, so taking the newest is a
    /// formality that also holds if a table was written by an older
    /// update with a different source.
    pub fn rates_vintage(&self) -> StoreResult<Option<String>> {
        let vintage: Option<String> = self
            .conn
            .query_row("SELECT MAX(as_of) FROM rates", [], |row| row.get(0))?;
        Ok(vintage)
    }
}

#[cfg(test)]
mod tests {
    use crate::Store;

    #[test]
    fn writes_and_reads_whole_tables() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(store.rates().unwrap().len(), 0);
        assert_eq!(store.rates_vintage().unwrap(), None);

        store
            .replace_rates(&[
                ("EUR".into(), 0.90, "2026-10-30".into()),
                ("JPY".into(), 150.0, "2026-10-30".into()),
            ])
            .unwrap();
        let rates = store.rates().unwrap();
        assert_eq!(rates.len(), 2);
        assert_eq!(rates[0].0, "EUR");
        assert_eq!(rates_vintage(&store), Some("2026-10-30".into()));
    }

    #[test]
    fn claims_the_freshest_vintage() {
        let store = Store::open_in_memory().unwrap();
        store
            .replace_rates(&[("EUR".into(), 0.92, "2026-10-31".into())])
            .unwrap();
        store
            .replace_rates(&[
                ("EUR".into(), 0.90, "2026-10-30".into()),
                ("GBP".into(), 0.79, "2026-10-30".into()),
            ])
            .unwrap();
        // The refill removed the previous table entirely.
        assert_eq!(store.rates().unwrap().len(), 2);
        assert_eq!(rates_vintage(&store), Some("2026-10-30".into()));
    }

    fn rates_vintage(store: &Store) -> Option<String> {
        store.rates_vintage().unwrap()
    }
}
