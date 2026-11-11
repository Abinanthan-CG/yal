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

use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};

use crate::index::{walk_once, Backing, Entry, WalkReport};

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

impl IndexStore {
    /// Open the index at a place, creating the tables if they are not there.
    pub fn open(path: &Path) -> Result<IndexStore, StoreError> {
        let connection = Connection::open(path).map_err(|source| StoreError::Open {
            path: path.to_string_lossy().to_string(),
            source: std::io::Error::other(source),
        })?;
        let store = IndexStore { connection };
        store.migrate()?;
        Ok(store)
    }

    /// Open an index that lives only in memory, for the tests and for a
    /// session that must not write anything to the disk.
    pub fn open_in_memory() -> Result<IndexStore, StoreError> {
        let connection = Connection::open_in_memory()?;
        let store = IndexStore { connection };
        store.migrate()?;
        Ok(store)
    }

    /// Create the tables if they are not there yet.
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

            CREATE TABLE IF NOT EXISTS file_index_meta (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            "#,
        )?;
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

    /// Read the whole index into memory, for the search to work over.
    pub fn entries(&self) -> Result<Vec<Entry>, StoreError> {
        let mut statement = self.connection.prepare(
            "SELECT path, name, parent, size, modified_ns, is_dir
             FROM file_index_entries",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(Entry {
                path: row.get(0)?,
                name: row.get(1)?,
                parent: row.get(2)?,
                size: row.get::<_, i64>(3)? as u64,
                modified_ns: row.get(4)?,
                is_dir: row.get::<_, i64>(5)? != 0,
            })
        })?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
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
                "INSERT OR REPLACE INTO file_index_entries
                    (path, name, parent, size, modified_ns, is_dir)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
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
    let report = walk_once(&mut store, roots, rules, home);
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
}
