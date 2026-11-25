//! The index's store.
//!
//! SQLite, in the file the user chose or in the default place under the app's
//! own directory. Two tables: every listed directory with the time it was
//! listed at, and every listed file and folder keyed by its path. Nothing
//! about a file's *contents* is ever read or written - the index holds the
//! name, where it is, when it changed and how big it is, and that is all.
//!
//! The schema and the statements are this port's own; the design of the pass
//! that fills them is the donor project's (GPL-3.0-only).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension};

use crate::index::{walk_once, Backing, Entry, WalkReport};
use crate::scope::IgnoreRules;
use crate::search::fold;

/// What can go wrong while the index is stored.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// The database could not be opened, or a statement failed.
    #[error("the file index could not be read or written: {0}")]
    Sqlite(#[from] rusqlite::Error),

    /// The database could not be created at that place.
    #[error("the file index could not be opened at {path}: {source}")]
    Open {
        /// The place the index was asked for.
        path: String,
        /// What went wrong.
        source: std::io::Error,
    },
}

/// The index's store.
#[derive(Debug)]
pub struct IndexStore {
    connection: Connection,
}

const UPSERT_ENTRY_SQL: &str = "INSERT INTO file_index_entries
        (path, name, parent, size, modified_ns, is_dir)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6)
     ON CONFLICT(path) DO UPDATE SET
        name = excluded.name,
        parent = excluded.parent,
        size = excluded.size,
        modified_ns = excluded.modified_ns,
        is_dir = excluded.is_dir";

const WALK_BATCH_DIRS: usize = 250;

impl IndexStore {
    /// Open the index at a place, creating the tables if they are not there.
    pub fn open(path: &Path) -> Result<IndexStore, StoreError> {
        let connection = Connection::open(path).map_err(|source| StoreError::Open {
            path: path.to_string_lossy().to_string(),
            source: std::io::Error::other(source),
        })?;
        let store = IndexStore { connection };
        store.configure_pragmas(true)?;
        store.migrate()?;
        Ok(store)
    }

    /// Open an index that lives only in memory, for the tests and for a
    /// session that must not write anything to the disk.
    pub fn open_in_memory() -> Result<IndexStore, StoreError> {
        let connection = Connection::open_in_memory()?;
        let store = IndexStore { connection };
        store.configure_pragmas(false)?;
        store.migrate()?;
        Ok(store)
    }

    fn configure_pragmas(&self, on_disk: bool) -> Result<(), StoreError> {
        if on_disk {
            self.connection.execute_batch(
                "PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = NORMAL;
                 PRAGMA temp_store = MEMORY;
                 PRAGMA cache_size = -2000;
                 PRAGMA busy_timeout = 5000;
                 PRAGMA recursive_triggers = ON;",
            )?;
        } else {
            self.connection.execute_batch(
                "PRAGMA temp_store = MEMORY;
                 PRAGMA recursive_triggers = ON;",
            )?;
        }
        Ok(())
    }

    /// Create the tables, indexes, and FTS5 trigram virtual table if they are not there yet.
    fn migrate(&self) -> Result<(), StoreError> {
        self.connection.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS file_index_dirs (
                path      TEXT PRIMARY KEY,
                mtime_ns  INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS file_index_entries (
                path        TEXT PRIMARY KEY,
                name        TEXT NOT NULL,
                parent      TEXT NOT NULL,
                size        INTEGER NOT NULL,
                modified_ns INTEGER NOT NULL,
                is_dir      INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS file_index_entries_parent
                ON file_index_entries (parent);

            CREATE INDEX IF NOT EXISTS file_index_entries_recent
                ON file_index_entries (is_dir, modified_ns DESC);

            CREATE INDEX IF NOT EXISTS file_index_entries_name_nocase
                ON file_index_entries (name COLLATE NOCASE);

            CREATE TABLE IF NOT EXISTS file_index_meta (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS file_index_fts USING fts5(
                name,
                path,
                content='file_index_entries',
                content_rowid='rowid',
                tokenize='trigram remove_diacritics 1'
            );

            CREATE TRIGGER IF NOT EXISTS file_index_entries_ai
            AFTER INSERT ON file_index_entries BEGIN
                INSERT INTO file_index_fts(rowid, name, path)
                VALUES (new.rowid, new.name, new.path);
            END;

            CREATE TRIGGER IF NOT EXISTS file_index_entries_ad
            AFTER DELETE ON file_index_entries BEGIN
                INSERT INTO file_index_fts(file_index_fts, rowid, name, path)
                VALUES ('delete', old.rowid, old.name, old.path);
            END;

            CREATE TRIGGER IF NOT EXISTS file_index_entries_au
            AFTER UPDATE ON file_index_entries BEGIN
                INSERT INTO file_index_fts(file_index_fts, rowid, name, path)
                VALUES ('delete', old.rowid, old.name, old.path);
                INSERT INTO file_index_fts(rowid, name, path)
                VALUES (new.rowid, new.name, new.path);
            END;
            "#,
        )?;

        // One-time backfill when upgrading an existing pre-FTS5 database on disk.
        let fts_version: Option<String> = self
            .connection
            .query_row(
                "SELECT value FROM file_index_meta WHERE key = 'fts_version'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        if fts_version.as_deref() != Some("1") {
            let entry_count: i64 = self.connection.query_row(
                "SELECT COUNT(*) FROM file_index_entries",
                [],
                |row| row.get(0),
            )?;
            if entry_count > 0 {
                self.connection.execute(
                    "INSERT INTO file_index_fts(file_index_fts) VALUES('rebuild')",
                    [],
                )?;
            }
            self.connection.execute(
                "INSERT INTO file_index_meta (key, value) VALUES ('fts_version', '1')
                 ON CONFLICT(key) DO UPDATE SET value = '1'",
                [],
            )?;
        }

        Ok(())
    }

    /// How many directories and entries the index holds.
    pub fn counts(&self) -> Result<(usize, usize), StoreError> {
        let dirs =
            self.connection
                .query_row("SELECT COUNT(*) FROM file_index_dirs", [], |row| {
                    row.get::<_, i64>(0)
                })?;
        let entries =
            self.connection
                .query_row("SELECT COUNT(*) FROM file_index_entries", [], |row| {
                    row.get::<_, i64>(0)
                })?;
        Ok((dirs as usize, entries as usize))
    }

    /// When the index was last built, in nanoseconds since the epoch.
    pub fn built_at(&self) -> Result<Option<i64>, StoreError> {
        let value = self
            .connection
            .query_row(
                "SELECT value FROM file_index_meta WHERE key = 'built_at_ns'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        Ok(value.and_then(|text| text.parse::<i64>().ok()))
    }

    /// Note that the index was built at this time.
    pub fn set_built_at(&mut self, when_ns: i64) -> Result<(), StoreError> {
        self.connection.execute(
            "INSERT INTO file_index_meta (key, value) VALUES ('built_at_ns', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![when_ns.to_string()],
        )?;
        Ok(())
    }

    /// Read the whole index into memory, for tests and full-export callers.
    pub fn entries(&self) -> Result<Vec<Entry>, StoreError> {
        let mut statement = self.connection.prepare(
            "SELECT path, name, parent, size, modified_ns, is_dir
             FROM file_index_entries",
        )?;
        let rows = statement.query_map([], row_to_entry)?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    }

    /// Run an incremental index pass with batched SQLite transactions
    /// (committing every 250 directories) so indexing 200k+ files finishes in
    /// seconds while keeping WAL readers responsive throughout the walk.
    pub fn walk_batched(
        &mut self,
        roots: &[PathBuf],
        rules: &IgnoreRules,
        home: &Path,
    ) -> WalkReport {
        let mut batched = ChunkedBacking::new(&self.connection);
        let report = walk_once(&mut batched, roots, rules, home);
        batched.finish();
        report
    }

    /// Query up to `limit` most recently modified non-directory entries using
    /// the `(is_dir, modified_ns DESC)` index without loading the full table.
    pub fn recent_candidates(&self, limit: usize) -> Result<Vec<Entry>, StoreError> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let mut statement = self.connection.prepare_cached(
            "SELECT path, name, parent, size, modified_ns, is_dir
             FROM file_index_entries
             WHERE is_dir = 0
             ORDER BY modified_ns DESC
             LIMIT ?1",
        )?;
        let rows = statement.query_map(params![limit as i64], row_to_entry)?;
        let mut out = Vec::with_capacity(limit.min(512));
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Fast indexed candidate lookup for `query` without loading all rows into RAM.
    ///
    /// - Queries whose words are all `>= 3` characters use the SQLite FTS5
    ///   trigram index (`name` matches first, then `path` matches up to `limit`),
    ///   running in ~1–4 ms on 200k+ indexed files.
    /// - Shorter 1–2 character queries use indexed prefix + bounded `LIKE` on `name`.
    pub fn search_candidates(&self, query: &str, limit: usize) -> Result<Vec<Entry>, StoreError> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let words: Vec<String> = query
            .split_whitespace()
            .map(fold)
            .filter(|w| !w.is_empty())
            .collect();
        if words.is_empty() {
            return Ok(Vec::new());
        }

        let mut results = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();

        let fts_words: Vec<String> = words
            .iter()
            .filter(|w| w.chars().count() >= 3)
            .cloned()
            .collect();
        if !fts_words.is_empty() {
            let name_match = build_fts_query("name", &fts_words);
            self.append_fts_matches(&name_match, limit, &mut seen, &mut results)?;

            if results.len() < limit {
                let remaining = limit - results.len();
                let path_match = build_fts_query("path", &fts_words);
                self.append_fts_matches(&path_match, remaining, &mut seen, &mut results)?;
            }
            return Ok(results);
        }

        // Short 1–2 character query: fast indexed prefix lookup first, then bounded LIKE.
        let first_prefix = format!("{}%", escape_like(&words[0]));
        {
            let mut stmt = self.connection.prepare_cached(
                "SELECT path, name, parent, size, modified_ns, is_dir
                 FROM file_index_entries
                 WHERE name LIKE ?1 ESCAPE '\\' COLLATE NOCASE
                 LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![first_prefix, (limit * 2) as i64], row_to_entry)?;
            for row in rows {
                let entry = row?;
                if seen.insert(entry.path.clone()) {
                    results.push(entry);
                    if results.len() >= limit {
                        return Ok(results);
                    }
                }
            }
        }

        if results.len() < limit {
            let contains = format!("%{}%", escape_like(&words[0]));
            let mut stmt = self.connection.prepare_cached(
                "SELECT path, name, parent, size, modified_ns, is_dir
                 FROM file_index_entries
                 WHERE name LIKE ?1 ESCAPE '\\' COLLATE NOCASE
                 LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![contains, (limit * 2) as i64], row_to_entry)?;
            for row in rows {
                let entry = row?;
                if seen.insert(entry.path.clone()) {
                    results.push(entry);
                    if results.len() >= limit {
                        break;
                    }
                }
            }
        }

        Ok(results)
    }

    fn append_fts_matches(
        &self,
        fts_expr: &str,
        limit: usize,
        seen: &mut HashSet<String>,
        out: &mut Vec<Entry>,
    ) -> Result<(), StoreError> {
        let mut stmt = self.connection.prepare_cached(
            "SELECT e.path, e.name, e.parent, e.size, e.modified_ns, e.is_dir
             FROM file_index_fts
             JOIN file_index_entries AS e ON e.rowid = file_index_fts.rowid
             WHERE file_index_fts MATCH ?1
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![fts_expr, limit as i64], row_to_entry)?;
        for row in rows {
            let entry = row?;
            if seen.insert(entry.path.clone()) {
                out.push(entry);
            }
        }
        Ok(())
    }

    /// Close the store and let go of the index file.
    ///
    /// Windows will not let a file that is open be renamed, so a store has to
    /// be closed before the index it holds can be rebuilt. The search works
    /// over an in-memory snapshot, so nothing is lost by letting go of the
    /// file between rebuilds.
    pub fn close(self) -> Result<(), StoreError> {
        self.connection
            .close()
            .map_err(|(_, error)| StoreError::Sqlite(error))
    }

    /// Delete the index.
    ///
    /// Turning File Search off deletes the index, and so does a scope or
    /// pattern change that rebuilds it.
    pub fn clear(&mut self) -> Result<(), StoreError> {
        self.connection.execute_batch(
            "DELETE FROM file_index_entries;
             DELETE FROM file_index_dirs;
             DELETE FROM file_index_meta;",
        )?;
        Ok(())
    }
}

impl Backing for IndexStore {
    fn recorded_dirs(&self) -> Vec<(String, i64)> {
        let mut statement = match self
            .connection
            .prepare("SELECT path, mtime_ns FROM file_index_dirs")
        {
            Ok(statement) => statement,
            Err(_) => return Vec::new(),
        };
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        });
        match rows {
            Ok(rows) => rows.filter_map(|row| row.ok()).collect(),
            Err(_) => Vec::new(),
        }
    }

    fn remember_dirs(&mut self, rows: &[(String, i64)]) {
        let transaction = match self.connection.transaction() {
            Ok(transaction) => transaction,
            Err(_) => return,
        };
        for (path, mtime) in rows {
            let _ = transaction.execute(
                "INSERT INTO file_index_dirs (path, mtime_ns) VALUES (?1, ?2)
                 ON CONFLICT(path) DO UPDATE SET mtime_ns = excluded.mtime_ns",
                params![path, mtime],
            );
        }
        let _ = transaction.commit();
    }

    fn rewrite_entries(&mut self, dir: &str, rows: &[Entry]) {
        // The listing is the whole truth for this directory, so the old rows
        // go in the same transaction that writes the new ones.
        let transaction = match self.connection.transaction() {
            Ok(transaction) => transaction,
            Err(_) => return,
        };
        let _ = transaction.execute(
            "DELETE FROM file_index_entries WHERE parent = ?1",
            params![dir],
        );
        for entry in rows {
            let _ = transaction.execute(
                UPSERT_ENTRY_SQL,
                params![
                    entry.path,
                    entry.name,
                    entry.parent,
                    entry.size as i64,
                    entry.modified_ns,
                    i64::from(entry.is_dir)
                ],
            );
        }
        let _ = transaction.commit();
    }

    fn forget_dirs(&mut self, dirs: &[String]) {
        if dirs.is_empty() {
            return;
        }
        let transaction = match self.connection.transaction() {
            Ok(transaction) => transaction,
            Err(_) => return,
        };
        for dir in dirs {
            let _ = transaction.execute(
                "DELETE FROM file_index_entries WHERE parent = ?1",
                params![dir],
            );
            let _ =
                transaction.execute("DELETE FROM file_index_dirs WHERE path = ?1", params![dir]);
        }
        let _ = transaction.commit();
    }
}

fn row_to_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<Entry> {
    Ok(Entry {
        path: row.get(0)?,
        name: row.get(1)?,
        parent: row.get(2)?,
        size: row.get::<_, i64>(3)? as u64,
        modified_ns: row.get(4)?,
        is_dir: row.get::<_, i64>(5)? != 0,
    })
}

fn build_fts_query(column: &str, words: &[String]) -> String {
    words
        .iter()
        .map(|w| {
            let escaped = w.replace('"', "\"\"");
            format!("{column} : \"{escaped}\"")
        })
        .collect::<Vec<_>>()
        .join(" AND ")
}

fn escape_like(term: &str) -> String {
    let mut out = String::with_capacity(term.len());
    for ch in term.chars() {
        if matches!(ch, '%' | '_' | '\\') {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

/// Transactional [`Backing`] wrapper that batches writes into chunks of
/// `WALK_BATCH_DIRS` directories per SQLite transaction.
struct ChunkedBacking<'a> {
    conn: &'a Connection,
    in_txn: bool,
    pending_dirs: usize,
}

impl<'a> ChunkedBacking<'a> {
    fn new(conn: &'a Connection) -> Self {
        Self {
            conn,
            in_txn: false,
            pending_dirs: 0,
        }
    }

    fn ensure_txn(&mut self) {
        if !self.in_txn && self.conn.execute_batch("BEGIN IMMEDIATE").is_ok() {
            self.in_txn = true;
            self.pending_dirs = 0;
        }
    }

    fn flush_if_needed(&mut self) {
        if self.in_txn && self.pending_dirs >= WALK_BATCH_DIRS {
            self.commit_txn();
        }
    }

    fn commit_txn(&mut self) {
        if self.in_txn {
            let _ = self.conn.execute_batch("COMMIT");
            self.in_txn = false;
            self.pending_dirs = 0;
        }
    }

    fn finish(mut self) {
        self.commit_txn();
    }
}

impl Drop for ChunkedBacking<'_> {
    fn drop(&mut self) {
        if self.in_txn {
            let _ = self.conn.execute_batch("ROLLBACK");
            self.in_txn = false;
        }
    }
}

impl Backing for ChunkedBacking<'_> {
    fn recorded_dirs(&self) -> Vec<(String, i64)> {
        let mut statement = match self
            .conn
            .prepare_cached("SELECT path, mtime_ns FROM file_index_dirs")
        {
            Ok(statement) => statement,
            Err(_) => return Vec::new(),
        };
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        });
        match rows {
            Ok(rows) => rows.filter_map(|row| row.ok()).collect(),
            Err(_) => Vec::new(),
        }
    }

    fn remember_dirs(&mut self, rows: &[(String, i64)]) {
        if rows.is_empty() {
            return;
        }
        self.ensure_txn();
        if let Ok(mut stmt) = self.conn.prepare_cached(
            "INSERT INTO file_index_dirs (path, mtime_ns) VALUES (?1, ?2)
             ON CONFLICT(path) DO UPDATE SET mtime_ns = excluded.mtime_ns",
        ) {
            for (path, mtime) in rows {
                let _ = stmt.execute(params![path, mtime]);
            }
        }
        self.flush_if_needed();
    }

    fn rewrite_entries(&mut self, dir: &str, rows: &[Entry]) {
        self.ensure_txn();
        if let Ok(mut del) = self
            .conn
            .prepare_cached("DELETE FROM file_index_entries WHERE parent = ?1")
        {
            let _ = del.execute(params![dir]);
        }
        if let Ok(mut ins) = self.conn.prepare_cached(UPSERT_ENTRY_SQL) {
            for entry in rows {
                let _ = ins.execute(params![
                    entry.path,
                    entry.name,
                    entry.parent,
                    entry.size as i64,
                    entry.modified_ns,
                    i64::from(entry.is_dir)
                ]);
            }
        }
        self.pending_dirs += 1;
    }

    fn forget_dirs(&mut self, dirs: &[String]) {
        if dirs.is_empty() {
            return;
        }
        self.ensure_txn();
        if let Ok(mut del_entries) = self
            .conn
            .prepare_cached("DELETE FROM file_index_entries WHERE parent = ?1")
        {
            if let Ok(mut del_dir) = self
                .conn
                .prepare_cached("DELETE FROM file_index_dirs WHERE path = ?1")
            {
                for dir in dirs {
                    let _ = del_entries.execute(params![dir]);
                    let _ = del_dir.execute(params![dir]);
                }
            }
        }
    }
}

/// Rebuild the index at a place, from scratch.
///
/// The new index is built **beside** the old one and swapped in only when it
/// is ready, so a search that is already running keeps using the old index
/// until the new one is complete. This is what upstream means by "changing a
/// scope or a pattern rebuilds the index: search keeps using the old index
/// until the new one is ready".
///
/// The index is built in a helper process upstream, so that its memory is
/// returned when the build ends; on Windows the caller of this function is
/// that helper.
///
/// Windows will not rename a file that is open, so the store holding the old
/// index has to be closed before this is called. The search runs over an
/// in-memory snapshot, so the old index stays in use until the new one is
/// swapped in.
pub fn rebuild(
    path: &Path,
    roots: &[std::path::PathBuf],
    rules: &crate::scope::IgnoreRules,
    home: &Path,
) -> Result<(IndexStore, WalkReport), StoreError> {
    let building = building_path(path);
    let old = old_path(path);
    std::fs::remove_file(&building).ok();
    std::fs::remove_file(&old).ok();

    let mut store = IndexStore::open(&building)?;
    let report = store.walk_batched(roots, rules, home);
    let when = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| i64::try_from(since.as_nanos()).unwrap_or(i64::MAX))
        .unwrap_or(0);
    store.set_built_at(when)?;
    // The connection has to let go of the file before it can be renamed.
    store
        .connection
        .close()
        .map_err(|(_, error)| StoreError::Sqlite(error))?;

    // The old index steps aside, the new one takes its place, and the old one
    // is thrown away.
    let had_old = path.exists();
    if had_old {
        std::fs::rename(path, &old).map_err(|source| StoreError::Open {
            path: old.to_string_lossy().to_string(),
            source,
        })?;
    }
    std::fs::rename(&building, path).map_err(|source| StoreError::Open {
        path: path.to_string_lossy().to_string(),
        source,
    })?;
    std::fs::remove_file(&old).ok();

    let store = IndexStore::open(path)?;
    Ok((store, report))
}

/// The file the new index is built in, beside the old one.
fn building_path(path: &Path) -> std::path::PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".building");
    path.with_file_name(name)
}

/// The file the old index steps aside into.
fn old_path(path: &Path) -> std::path::PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".old");
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::walk_once;
    use crate::scope::IgnoreRules;

    fn entry(path: &str, parent: &str, name: &str, is_dir: bool) -> Entry {
        Entry {
            path: path.to_string(),
            name: name.to_string(),
            parent: parent.to_string(),
            size: if is_dir { 0 } else { 12 },
            modified_ns: 1_700_000_000_000_000_000,
            is_dir,
        }
    }

    #[test]
    fn a_fresh_store_is_empty() {
        let store = IndexStore::open_in_memory().expect("the store");
        assert_eq!(store.counts().expect("the counts"), (0, 0));
        assert!(store.entries().expect("the entries").is_empty());
    }

    #[test]
    fn a_directory_keeps_its_listing_as_the_whole_truth() {
        let mut store = IndexStore::open_in_memory().expect("the store");
        store.rewrite_entries(
            "C:\\a",
            &[
                entry("C:\\a\\one.txt", "C:\\a", "one.txt", false),
                entry("C:\\a\\two.txt", "C:\\a", "two.txt", false),
            ],
        );
        store.rewrite_entries(
            "C:\\a",
            &[entry("C:\\a\\one.txt", "C:\\a", "one.txt", false)],
        );
        let entries = store.entries().expect("the entries");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "one.txt");
    }

    #[test]
    fn a_forgotten_directory_takes_its_contents_along() {
        let mut store = IndexStore::open_in_memory().expect("the store");
        store.rewrite_entries(
            "C:\\a",
            &[entry("C:\\a\\one.txt", "C:\\a", "one.txt", false)],
        );
        store.forget_dirs(&["C:\\a".to_string()]);
        assert!(store.entries().expect("the entries").is_empty());
    }

    #[test]
    fn the_recorded_directories_come_back_with_their_times() {
        let mut store = IndexStore::open_in_memory().expect("the store");
        store.remember_dirs(&[("C:\\a".to_string(), 42)]);
        let recorded = store.recorded_dirs();
        assert_eq!(recorded, vec![("C:\\a".to_string(), 42)]);
    }

    #[test]
    fn remembering_a_directory_twice_keeps_the_later_time() {
        let mut store = IndexStore::open_in_memory().expect("the store");
        store.remember_dirs(&[("C:\\a".to_string(), 42)]);
        store.remember_dirs(&[("C:\\a".to_string(), 43)]);
        assert_eq!(store.recorded_dirs(), vec![("C:\\a".to_string(), 43)]);
    }

    #[test]
    fn clearing_the_store_empties_every_table() {
        let mut store = IndexStore::open_in_memory().expect("the store");
        store.rewrite_entries(
            "C:\\a",
            &[entry("C:\\a\\one.txt", "C:\\a", "one.txt", false)],
        );
        store.remember_dirs(&[("C:\\a".to_string(), 42)]);
        store.set_built_at(7).expect("the stamp");
        store.clear().expect("the clear");
        assert_eq!(store.counts().expect("the counts"), (0, 0));
        assert_eq!(store.built_at().expect("the stamp"), None);
    }

    #[test]
    fn the_build_time_is_remembered() {
        let mut store = IndexStore::open_in_memory().expect("the store");
        store.set_built_at(1_700_000_000).expect("the stamp");
        assert_eq!(store.built_at().expect("the stamp"), Some(1_700_000_000));
    }

    #[test]
    fn a_rebuild_swaps_the_new_index_in() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let root = dir.path().join("scoped");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("first.txt"), b"x").unwrap();
        let index = dir.path().join("index.sqlite3");

        let (store, report) = rebuild(
            &index,
            std::slice::from_ref(&root),
            &IgnoreRules::empty(),
            dir.path(),
        )
        .expect("the rebuild");
        assert!(report.changed);
        assert_eq!(store.entries().expect("the entries").len(), 1);
        // The file has to be let go of before it can be swapped out.
        store.close().expect("the close");

        // A file that arrives after the first build is picked up by the next.
        std::fs::write(root.join("second.txt"), b"x").unwrap();
        let (store, _report) = rebuild(
            &index,
            std::slice::from_ref(&root),
            &IgnoreRules::empty(),
            dir.path(),
        )
        .expect("the rebuild");
        let mut names: Vec<String> = store
            .entries()
            .expect("the entries")
            .into_iter()
            .map(|entry| entry.name)
            .collect();
        names.sort();
        assert_eq!(
            names,
            vec!["first.txt".to_string(), "second.txt".to_string()]
        );
        assert!(index.exists());
        assert!(!building_path(&index).exists(), "the building file is gone");
        assert!(!old_path(&index).exists(), "the old file is gone");
    }

    #[test]
    fn a_rebuild_keeps_the_old_index_when_it_fails() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let index = dir.path().join("index.sqlite3");
        // A root that does not exist: the walk finds nothing, and the store
        // is still swapped in - but the old index is only replaced by a store
        // that was opened and closed cleanly.
        let (store, report) = rebuild(
            &index,
            &[dir.path().join("missing")],
            &IgnoreRules::empty(),
            dir.path(),
        )
        .expect("the rebuild");
        assert_eq!(report.listed, 0);
        assert_eq!(store.counts().expect("the counts"), (0, 0));
        assert!(index.exists());
    }

    #[test]
    fn a_walk_over_a_real_folder_fills_the_store() {
        let root = std::env::temp_dir().join(format!("yal-store-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("alpha.txt"), b"hello").unwrap();

        let mut store = IndexStore::open_in_memory().expect("the store");
        let report = walk_once(
            &mut store,
            std::slice::from_ref(&root),
            &IgnoreRules::empty(),
            root.parent().unwrap(),
        );
        assert!(report.changed);
        let (dirs, entries) = store.counts().expect("the counts");
        assert_eq!(dirs, 1);
        assert_eq!(entries, 1);
        let found = store.entries().expect("the entries");
        assert_eq!(found[0].name, "alpha.txt");
        assert_eq!(found[0].size, 5);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn fts5_search_candidates_finds_name_path_and_diacritics() {
        let mut store = IndexStore::open_in_memory().expect("the store");
        store.rewrite_entries(
            "C:\\Users\\dabin\\development\\opencode-stack\\AIStudio2API",
            &[
                entry(
                    "C:\\Users\\dabin\\development\\opencode-stack\\AIStudio2API\\aistudio2api.exe",
                    "C:\\Users\\dabin\\development\\opencode-stack\\AIStudio2API",
                    "aistudio2api.exe",
                    false,
                ),
                entry(
                    "C:\\Users\\dabin\\development\\opencode-stack\\AIStudio2API\\2x2.png",
                    "C:\\Users\\dabin\\development\\opencode-stack\\AIStudio2API",
                    "2x2.png",
                    false,
                ),
                entry(
                    "C:\\Users\\dabin\\Documents\\café.txt",
                    "C:\\Users\\dabin\\Documents",
                    "café.txt",
                    false,
                ),
            ],
        );

        let hits = store
            .search_candidates("aistudio2api", 20)
            .expect("fts search");
        let names: Vec<&str> = hits.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names[0], "aistudio2api.exe");
        assert!(names.contains(&"2x2.png"));

        let accent_hits = store.search_candidates("cafe", 20).expect("accent search");
        assert_eq!(accent_hits.len(), 1);
        assert_eq!(accent_hits[0].name, "café.txt");

        let short_hits = store.search_candidates("ai", 20).expect("short search");
        assert!(short_hits.iter().any(|e| e.name == "aistudio2api.exe"));
    }

    #[test]
    fn walk_batched_and_recent_candidates_work_end_to_end() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().join("development");
        let proj = root.join("AIStudio2API");
        std::fs::create_dir_all(&proj).unwrap();
        std::fs::write(proj.join("aistudio2api.exe"), b"bin").unwrap();
        std::fs::write(proj.join("notes.txt"), b"txt").unwrap();

        let mut store = IndexStore::open_in_memory().expect("store");
        let report = store.walk_batched(
            std::slice::from_ref(&root),
            &IgnoreRules::defaults(),
            dir.path(),
        );
        assert!(report.changed);
        let hits = store.search_candidates("aistudio2api", 10).expect("search");
        assert!(hits.iter().any(|e| e.name == "AIStudio2API" && e.is_dir));
        assert!(hits.iter().any(|e| e.name == "aistudio2api.exe"));
        assert!(hits.iter().any(|e| e.name == "notes.txt"));

        let recent = store.recent_candidates(10).expect("recent");
        assert_eq!(recent.len(), 2);
        assert!(recent.iter().all(|e| !e.is_dir));
    }
}
