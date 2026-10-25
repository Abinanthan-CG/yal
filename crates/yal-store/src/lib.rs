//! The local store.
//!
//! One SQLite database holds everything the launcher remembers: settings,
//! snippets, quicklinks, bookmarks, aliases, clipboard history, usage history
//! and the application index. The store owns the schema and the migrations;
//! callers work with the typed records in [`records`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, params};

pub mod records;
pub mod schema;

pub use records::{
    Alias, Bookmark, ClipboardEntry, ClipboardKind, IndexedApplication, Quicklink, Snippet,
    UsageRecord,
};

/// A store failure.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("stored timestamp is not a valid RFC 3339 value: {0}")]
    InvalidTimestamp(String),
}

/// Result alias for store operations.
pub type StoreResult<T> = Result<T, StoreError>;

/// The default location of the store, under the user's data directory.
pub fn default_path() -> Option<PathBuf> {
    dirs::data_dir().map(|dir| {
        // The launcher was named legacy-store before it was named yal, and
        // this is its first store under the new name. One rename carries
        // the whole data folder across, and runs only once: after it, the
        // new folder exists and the legacy one is gone. A data folder that
        // was already migrated, or was never created, is left alone.
        let legacy_dir = dir.join("legacy-store");
        let current_dir = dir.join("yal");
        if !current_dir.exists() && legacy_dir.exists() {
            std::fs::rename(&legacy_dir, &current_dir).ok();
        }
        // The folder came over whole, and carries the database file under
        // its old name; the file follows the folder.
        let legacy_file = current_dir.join("legacy.db");
        let current_file = current_dir.join("yal.db");
        if !current_file.exists() && legacy_file.exists() {
            std::fs::rename(&legacy_file, &current_file).ok();
        }
        current_file
    })
}

/// The local store.
pub struct Store {
    conn: Connection,
    path: Option<std::path::PathBuf>,
}

impl Store {
    /// Open the store at a path, creating the parent directory if needed.
    pub fn open(path: &Path) -> StoreResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        let store = Self {
            conn,
            path: Some(path.to_path_buf()),
        };
        store.migrate()?;
        Ok(store)
    }

    /// Where the store lives.
    ///
    /// `None` for the in-memory store, which has nothing worth connecting
    /// to twice: the due watcher skips it, because an in-memory store has
    /// no other process to hear about anyway.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Open a store that lives only in memory, for tests.
    pub fn open_in_memory() -> StoreResult<Self> {
        let conn = Connection::open_in_memory()?;
        let store = Self { conn, path: None };
        store.migrate()?;
        Ok(store)
    }

    /// Apply any migrations the database has not yet seen.
    pub fn migrate(&self) -> StoreResult<()> {
        self.conn.execute_batch(schema::BOOTSTRAP)?;
        let current: i64 = self
            .conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_version",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        for (index, statement) in schema::MIGRATIONS.iter().enumerate() {
            let version = index as i64 + 1;
            if version <= current {
                continue;
            }
            self.conn.execute_batch(statement)?;
            self.conn.execute(
                "INSERT INTO schema_version (version, applied_at) VALUES (?1, ?2)",
                params![version, Utc::now().to_rfc3339()],
            )?;
        }
        Ok(())
    }

    /// The schema version currently recorded in the database.
    pub fn schema_version(&self) -> StoreResult<i64> {
        Ok(self.conn.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version",
            [],
            |row| row.get(0),
        )?)
    }

    // ---- settings -------------------------------------------------------

    /// Read a setting.
    pub fn setting(&self, key: &str) -> StoreResult<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?)
    }

    /// Write a setting, replacing any previous value.
    pub fn set_setting(&self, key: &str, value: &str) -> StoreResult<()> {
        self.conn.execute(
            "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![key, value, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    /// Remove a setting.
    pub fn clear_setting(&self, key: &str) -> StoreResult<()> {
        self.conn
            .execute("DELETE FROM settings WHERE key = ?1", params![key])?;
        Ok(())
    }

    /// Every setting, as key/value pairs.
    pub fn settings(&self) -> StoreResult<Vec<(String, String)>> {
        let mut statement = self
            .conn
            .prepare("SELECT key, value FROM settings ORDER BY key")?;
        let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Seed the quicklinks every installation starts with, once.
    ///
    /// The search templates are stored as ordinary quicklinks, so the user
    /// can rename, edit or delete them like any other. The `{}` in a target
    /// is where the palette query goes.
    pub fn seed_default_quicklinks(&self) -> StoreResult<()> {
        if self.setting("quicklinks-seeded")?.is_some() {
            return Ok(());
        }

        let now = Utc::now();
        let templates: &[(&str, &str)] = &[
            ("Google", "https://www.google.com/search?q={}"),
            ("Bing", "https://www.bing.com/search?q={}"),
            ("DuckDuckGo", "https://duckduckgo.com/?q={}"),
            ("YouTube", "https://www.youtube.com/results?search_query={}"),
            ("Wikipedia", "https://en.wikipedia.org/w/index.php?search={}"),
            ("GitHub", "https://github.com/search?q={}"),
            ("Stack Overflow", "https://stackoverflow.com/search?q={}"),
            ("Reddit", "https://www.reddit.com/search/?q={}"),
            ("Amazon", "https://www.amazon.com/s?k={}"),
            ("X", "https://x.com/search?q={}"),
            ("Maps", "https://www.google.com/maps/search/{}"),
        ];

        for (index, (name, url)) in templates.iter().enumerate() {
            self.upsert_quicklink(&Quicklink {
                id: format!("template-{index}"),
                name: (*name).to_string(),
                url: (*url).to_string(),
                created_at: now,
            })?;
        }

        self.set_setting("quicklinks-seeded", "true")
    }

    // ---- usage and frecency --------------------------------------------

    /// Record that a result was chosen.
    pub fn record_use(&self, key: &str, at: DateTime<Utc>) -> StoreResult<()> {
        self.conn.execute(
            "INSERT INTO usage (key, count, last_used) VALUES (?1, 1, ?2)
             ON CONFLICT(key) DO UPDATE SET count = count + 1, last_used = excluded.last_used",
            params![key, at.to_rfc3339()],
        )?;
        Ok(())
    }

    /// The usage record for one key.
    pub fn usage(&self, key: &str) -> StoreResult<Option<UsageRecord>> {
        Ok(self
            .conn
            .query_row(
                "SELECT key, count, last_used FROM usage WHERE key = ?1",
                params![key],
                |row| {
                    Ok(UsageRecord {
                        key: row.get(0)?,
                        count: row.get::<_, i64>(1)? as u32,
                        last_used: parse_timestamp(&row.get::<_, String>(2)?)?,
                    })
                },
            )
            .optional()?)
    }

    /// Usage records for many keys at once.
    pub fn usage_for(&self, keys: &[String]) -> StoreResult<HashMap<String, UsageRecord>> {
        let mut map = HashMap::new();
        if keys.is_empty() {
            return Ok(map);
        }
        let mut statement = self.conn.prepare(
            "SELECT key, count, last_used FROM usage WHERE key IN (
                 SELECT value FROM json_each(?1))",
        )?;
        let json = serde_json::to_string(keys).map_err(|err| {
            StoreError::InvalidTimestamp(format!("could not encode keys: {err}"))
        })?;
        let rows = statement.query_map(params![json], |row| {
            Ok(UsageRecord {
                key: row.get(0)?,
                count: row.get::<_, i64>(1)? as u32,
                last_used: parse_timestamp(&row.get::<_, String>(2)?)?,
            })
        })?;
        for row in rows {
            let record = row?;
            map.insert(record.key.clone(), record);
        }
        Ok(map)
    }

    // ---- snippets --------------------------------------------------------

    /// Insert or replace a snippet.
    pub fn upsert_snippet(&self, snippet: &Snippet) -> StoreResult<()> {
        self.conn.execute(
            "INSERT INTO snippets (id, name, keyword, expansion, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                keyword = excluded.keyword,
                expansion = excluded.expansion,
                updated_at = excluded.updated_at",
            params![
                snippet.id,
                snippet.name,
                snippet.keyword,
                snippet.expansion,
                snippet.created_at.to_rfc3339(),
                snippet.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// Every snippet, ordered by keyword.
    pub fn snippets(&self) -> StoreResult<Vec<Snippet>> {
        let mut statement = self.conn.prepare(
            "SELECT id, name, keyword, expansion, created_at, updated_at
             FROM snippets ORDER BY keyword",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(Snippet {
                id: row.get(0)?,
                name: row.get(1)?,
                keyword: row.get(2)?,
                expansion: row.get(3)?,
                created_at: parse_timestamp(&row.get::<_, String>(4)?)?,
                updated_at: parse_timestamp(&row.get::<_, String>(5)?)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Remove a snippet.
    pub fn delete_snippet(&self, id: &str) -> StoreResult<()> {
        self.conn
            .execute("DELETE FROM snippets WHERE id = ?1", params![id])?;
        Ok(())
    }

    // ---- quicklinks ------------------------------------------------------

    /// Insert or replace a quicklink.
    pub fn upsert_quicklink(&self, quicklink: &Quicklink) -> StoreResult<()> {
        self.conn.execute(
            "INSERT INTO quicklinks (id, name, url, created_at) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, url = excluded.url",
            params![
                quicklink.id,
                quicklink.name,
                quicklink.url,
                quicklink.created_at.to_rfc3339()
            ],
        )?;
        Ok(())
    }

    /// Every quicklink.
    pub fn quicklinks(&self) -> StoreResult<Vec<Quicklink>> {
        let mut statement = self
            .conn
            .prepare("SELECT id, name, url, created_at FROM quicklinks ORDER BY name")?;
        let rows = statement.query_map([], |row| {
            Ok(Quicklink {
                id: row.get(0)?,
                name: row.get(1)?,
                url: row.get(2)?,
                created_at: parse_timestamp(&row.get::<_, String>(3)?)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Remove a quicklink.
    pub fn delete_quicklink(&self, id: &str) -> StoreResult<()> {
        self.conn
            .execute("DELETE FROM quicklinks WHERE id = ?1", params![id])?;
        Ok(())
    }

    // ---- bookmarks -------------------------------------------------------

    /// Insert or replace a bookmark.
    pub fn upsert_bookmark(&self, bookmark: &Bookmark) -> StoreResult<()> {
        self.conn.execute(
            "INSERT INTO bookmarks (id, name, url, created_at) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, url = excluded.url",
            params![
                bookmark.id,
                bookmark.name,
                bookmark.url,
                bookmark.created_at.to_rfc3339()
            ],
        )?;
        Ok(())
    }

    /// Every bookmark.
    pub fn bookmarks(&self) -> StoreResult<Vec<Bookmark>> {
        let mut statement = self
            .conn
            .prepare("SELECT id, name, url, created_at FROM bookmarks ORDER BY name")?;
        let rows = statement.query_map([], |row| {
            Ok(Bookmark {
                id: row.get(0)?,
                name: row.get(1)?,
                url: row.get(2)?,
                created_at: parse_timestamp(&row.get::<_, String>(3)?)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    // ---- aliases ---------------------------------------------------------

    /// Insert or replace an alias.
    pub fn upsert_alias(&self, alias: &Alias) -> StoreResult<()> {
        self.conn.execute(
            "INSERT INTO aliases (id, term, target, created_at) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET term = excluded.term, target = excluded.target",
            params![
                alias.id,
                alias.term,
                alias.target,
                alias.created_at.to_rfc3339()
            ],
        )?;
        Ok(())
    }

    /// Remove an alias.
    pub fn clear_alias(&self, id: &str) -> StoreResult<()> {
        self.conn
            .execute("DELETE FROM aliases WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Every alias.
    pub fn aliases(&self) -> StoreResult<Vec<Alias>> {        let mut statement = self
            .conn
            .prepare("SELECT id, term, target, created_at FROM aliases ORDER BY term")?;
        let rows = statement.query_map([], |row| {
            Ok(Alias {
                id: row.get(0)?,
                term: row.get(1)?,
                target: row.get(2)?,
                created_at: parse_timestamp(&row.get::<_, String>(3)?)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    // ---- clipboard -------------------------------------------------------

    /// Append a clipboard entry, trimming the history to `keep` entries.
    pub fn push_clipboard(&self, entry: &ClipboardEntry, keep: usize) -> StoreResult<()> {
        self.conn.execute(
            "INSERT INTO clipboard (text, kind, source_app, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![
                entry.text,
                entry.kind.as_str(),
                entry.source_app,
                entry.created_at.to_rfc3339()
            ],
        )?;
        self.conn.execute(
            "DELETE FROM clipboard WHERE id NOT IN (
                 SELECT id FROM clipboard ORDER BY created_at DESC LIMIT ?1)",
            params![keep as i64],
        )?;
        Ok(())
    }

    /// The clipboard history, newest first.
    pub fn clipboard(&self, limit: usize) -> StoreResult<Vec<ClipboardEntry>> {
        let mut statement = self.conn.prepare(
            "SELECT id, text, kind, source_app, created_at
             FROM clipboard ORDER BY created_at DESC LIMIT ?1",
        )?;
        let rows = statement.query_map(params![limit as i64], |row| {
            Ok(ClipboardEntry {
                id: row.get(0)?,
                text: row.get(1)?,
                kind: row.get::<_, String>(2)?.parse().unwrap_or(ClipboardKind::Text),
                source_app: row.get(3)?,
                created_at: parse_timestamp(&row.get::<_, String>(4)?)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Empty the clipboard history.
    pub fn clear_clipboard(&self) -> StoreResult<()> {
        self.conn.execute("DELETE FROM clipboard", [])?;
        Ok(())
    }

    // ---- application index ----------------------------------------------

    /// Insert or replace an indexed application.
    pub fn upsert_application(&self, application: &IndexedApplication) -> StoreResult<()> {
        self.conn.execute(
            "INSERT INTO applications (id, name, path, source, keywords, last_indexed)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                path = excluded.path,
                source = excluded.source,
                keywords = excluded.keywords,
                last_indexed = excluded.last_indexed",
            params![
                application.id,
                application.name,
                application.path,
                application.source,
                application.keywords,
                application.last_indexed.to_rfc3339()
            ],
        )?;
        Ok(())
    }

    /// Every indexed application.
    pub fn applications(&self) -> StoreResult<Vec<IndexedApplication>> {
        let mut statement = self.conn.prepare(
            "SELECT id, name, path, source, keywords, last_indexed
             FROM applications ORDER BY name",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(IndexedApplication {
                id: row.get(0)?,
                name: row.get(1)?,
                path: row.get(2)?,
                source: row.get(3)?,
                keywords: row.get(4)?,
                last_indexed: parse_timestamp(&row.get::<_, String>(5)?)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Remove applications that are no longer installed.
    pub fn prune_applications(&self, keep_ids: &[String]) -> StoreResult<()> {
        if keep_ids.is_empty() {
            self.conn.execute("DELETE FROM applications", [])?;
            return Ok(());
        }
        let json = serde_json::to_string(keep_ids).map_err(|err| {
            StoreError::InvalidTimestamp(format!("could not encode identifiers: {err}"))
        })?;
        self.conn.execute(
            "DELETE FROM applications WHERE id NOT IN (SELECT value FROM json_each(?1))",
            params![json],
        )?;
        Ok(())
    }
}

/// Parse a stored RFC 3339 timestamp.
fn parse_timestamp(value: &str) -> Result<DateTime<Utc>, rusqlite::Error> {
    DateTime::parse_from_rfc3339(value)
        .map(|at| at.with_timezone(&Utc))
        .map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(err),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Store {
        Store::open_in_memory().unwrap()
    }

    #[test]
    fn migration_records_the_current_version() {
        let store = store();
        assert_eq!(store.schema_version().unwrap(), schema::CURRENT_VERSION);
    }

    #[test]
    fn migration_is_idempotent() {
        let store = store();
        store.migrate().unwrap();
        store.migrate().unwrap();
        assert_eq!(store.schema_version().unwrap(), schema::CURRENT_VERSION);
    }

    #[test]
    fn settings_round_trip() {
        let store = store();
        assert_eq!(store.setting("theme").unwrap(), None);
        store.set_setting("theme", "dark").unwrap();
        assert_eq!(store.setting("theme").unwrap().as_deref(), Some("dark"));
        store.set_setting("theme", "light").unwrap();
        assert_eq!(store.setting("theme").unwrap().as_deref(), Some("light"));
        store.clear_setting("theme").unwrap();
        assert_eq!(store.setting("theme").unwrap(), None);
    }

    #[test]
    fn usage_counts_accumulate() {
        let store = store();
        let now = Utc::now();
        store.record_use("calc", now).unwrap();
        store.record_use("calc", now).unwrap();
        let usage = store.usage("calc").unwrap().unwrap();
        assert_eq!(usage.count, 2);
    }

    #[test]
    fn usage_can_be_fetched_for_many_keys() {
        let store = store();
        let now = Utc::now();
        store.record_use("a", now).unwrap();
        store.record_use("b", now).unwrap();
        let map = store
            .usage_for(&["a".into(), "b".into(), "missing".into()])
            .unwrap();
        assert_eq!(map.len(), 2);
        assert!(map.contains_key("a"));
        assert!(!map.contains_key("missing"));
    }

    #[test]
    fn snippets_round_trip() {
        let store = store();
        let now = Utc::now();
        let snippet = Snippet {
            id: "sig".into(),
            name: "Signature".into(),
            keyword: "sig".into(),
            expansion: "Best regards".into(),
            created_at: now,
            updated_at: now,
        };
        store.upsert_snippet(&snippet).unwrap();
        let all = store.snippets().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].expansion, "Best regards");

        let mut updated = snippet.clone();
        updated.expansion = "Kind regards".into();
        store.upsert_snippet(&updated).unwrap();
        assert_eq!(store.snippets().unwrap().len(), 1);
        assert_eq!(store.snippets().unwrap()[0].expansion, "Kind regards");
    }

    #[test]
    fn clipboard_history_is_trimmed() {
        let store = store();
        let base = Utc::now();
        for index in 0..10 {
            store
                .push_clipboard(
                    &ClipboardEntry {
                        id: 0,
                        text: format!("entry {index}"),
                        kind: ClipboardKind::Text,
                        source_app: None,
                        created_at: base + chrono::Duration::seconds(index),
                    },
                    5,
                )
                .unwrap();
        }
        let history = store.clipboard(100).unwrap();
        assert_eq!(history.len(), 5);
        assert_eq!(history[0].text, "entry 9");
    }

    #[test]
    fn applications_can_be_pruned() {
        let store = store();
        let now = Utc::now();
        for id in ["a", "b"] {
            store
                .upsert_application(&IndexedApplication {
                    id: id.into(),
                    name: id.into(),
                    path: format!("{id}.exe"),
                    source: "registry".into(),
                    keywords: String::new(),
                    last_indexed: now,
                })
                .unwrap();
        }
        assert_eq!(store.applications().unwrap().len(), 2);
        store.prune_applications(&["a".into()]).unwrap();
        let remaining = store.applications().unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, "a");
    }

    #[test]
    fn aliases_and_quicklinks_round_trip() {
        let store = store();
        let now = Utc::now();
        store
            .upsert_alias(&Alias {
                id: "1".into(),
                term: "gh".into(),
                target: "GitHub".into(),
                created_at: now,
            })
            .unwrap();
        assert_eq!(store.aliases().unwrap()[0].target, "GitHub");

        store
            .upsert_quicklink(&Quicklink {
                id: "1".into(),
                name: "Repo".into(),
                url: "https://example.com".into(),
                created_at: now,
            })
            .unwrap();
        assert_eq!(store.quicklinks().unwrap()[0].url, "https://example.com");
    }
}
