//! The whole-drive file index tables.
//!
//! The schema for these lives in migration 2 (see `schema.rs`). They are
//! kept in a separate module from the user-facing records because they are
//! bulk-written by the indexer's own thread, not by anything a user is
//! editing: the operations here are batch upserts, per-directory
//! rewrites and removals, not named deletes.
//!
//! The invariant the indexer keeps is that every directory it has ever
//! listed has a row in `index_dirs`, and every file row's `dir` names a
//! recorded directory — which is exactly what makes a removal cheap and
//! exact: delete the files whose `dir` is one of the removed directories.

use crate::{params, Store, StoreResult};

impl Store {
    /// Every indexed path, in name order.
    ///
    /// This is the query snapshot's raw material: `name` and `dir` are
    /// recomputed at query time for the few hits, so the snapshot loads
    /// paths only, which keeps the reload cheap for a few hundred
    /// thousand entries.
    pub fn indexed_paths(&self) -> StoreResult<Vec<String>> {
        let mut statement = self.conn.prepare(
            "SELECT path FROM file_index",
        )?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        let mut paths = Vec::new();
        for path in rows {
            paths.push(path?);
        }
        Ok(paths)
    }

    /// How many files and directories the index holds.
    ///
    /// For the status line and for honest measurement after a pass.
    pub fn indexed_counts(&self) -> StoreResult<(usize, usize)> {
        let files: i64 = self.conn.query_row("SELECT COUNT(*) FROM file_index", [], |row| row.get(0))?;
        let dirs: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM index_dirs", [], |row| row.get(0))?;
        Ok((files as usize, dirs as usize))
    }

    /// Every recorded directory and its modification time, in
    /// nanoseconds since the epoch.
    ///
    /// The refresh stats all of these to decide what to re-list.
    pub fn indexed_dirs(&self) -> StoreResult<Vec<(String, i64)>> {
        let mut statement = self.conn.prepare("SELECT path, mtime_ns FROM index_dirs")?;
        let rows = statement.query_map([], |row| {
            let path: String = row.get(0)?;
            let mtime: i64 = row.get(1)?;
            Ok((path, mtime))
        })?;
        let mut dirs = Vec::new();
        for dir in rows {
            dirs.push(dir?);
        }
        Ok(dirs)
    }

    /// Record directories with their modification times (nanoseconds
    /// since the epoch), in one write.
    pub fn record_dirs(&self, rows: &[(String, i64)]) -> StoreResult<()> {
        if rows.is_empty() {
            return Ok(());
        }
        self.conn.execute_batch("BEGIN IMMEDIATE")?;
        let result = self.batch_dirs(rows);
        match result {
            Ok(()) => self.conn.execute_batch("COMMIT")?,
            Err(error) => {
                self.conn.execute_batch("ROLLBACK").ok();
                return Err(error);
            }
        }
        Ok(())
    }

    fn batch_dirs(&self, rows: &[(String, i64)]) -> StoreResult<()> {
        let mut statement = self.conn.prepare(
            "INSERT INTO index_dirs (path, mtime_ns) VALUES (?1, ?2)
             ON CONFLICT(path) DO UPDATE SET mtime_ns = excluded.mtime_ns",
        )?;
        for (path, mtime) in rows {
            statement.execute(params![path, mtime])?;
        }
        Ok(())
    }

    /// Rewrite one directory's files: anything recorded under `dir` is
    /// gone, then the listing goes in, in one write.
    ///
    /// The listing of a directory is its whole truth, so delete-then-
    /// write is what catches removals and renames.
    pub fn rewrite_dir_files(&self, dir: &str, rows: &[(String, String)]) -> StoreResult<()> {
        self.conn.execute_batch("BEGIN IMMEDIATE")?;
        let result = (|| -> StoreResult<()> {
            self.conn
                .execute("DELETE FROM file_index WHERE dir = ?1", params![dir])?;
            let mut statement = self
                .conn
                .prepare("INSERT OR IGNORE INTO file_index (path, dir, name) VALUES (?1, ?2, ?3)")?;
            for (path, name) in rows {
                statement.execute(params![path, dir, name])?;
            }
            Ok(())
        })();
        match result {
            Ok(()) => self.conn.execute_batch("COMMIT")?,
            Err(error) => {
                self.conn.execute_batch("ROLLBACK").ok();
                return Err(error);
            }
        }
        Ok(())
    }

    /// Forget directories: their rows in both tables go, in one write.
    ///
    /// Every recorded directory under a removed one is reported missing
    /// by the refresh on its own stat, so an exact `IN` delete is all it
    /// takes — no prefix matching, which `LIKE` would get wrong with an
    /// underscore in a path anyway.
    pub fn forget_dirs(&self, paths: &[String]) -> StoreResult<()> {
        if paths.is_empty() {
            return Ok(());
        }
        self.conn.execute_batch("BEGIN IMMEDIATE")?;
        let result = (|| -> StoreResult<()> {
            let mut statement = self
                .conn
                .prepare("DELETE FROM index_dirs WHERE path = ?1")?;
            for path in paths {
                statement.execute(params![path])?;
            }
            let mut statement = self
                .conn
                .prepare("DELETE FROM file_index WHERE dir = ?1")?;
            for path in paths {
                statement.execute(params![path])?;
            }
            Ok(())
        })();
        match result {
            Ok(()) => self.conn.execute_batch("COMMIT")?,
            Err(error) => {
                self.conn.execute_batch("ROLLBACK").ok();
                return Err(error);
            }
        }
        Ok(())
    }

    /// Empty the whole index.
    ///
    /// For a turn-off that means it, and for tests.
    pub fn clear_file_index(&self) -> StoreResult<()> {
        self.conn.execute("DELETE FROM file_index", [])?;
        self.conn.execute("DELETE FROM index_dirs", [])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory_store() -> Store {
        Store::open_in_memory().unwrap()
    }

    #[test]
    fn rows_round_trip_through_the_store() {
        let store = memory_store();
        store.record_dirs(&[("C:\\tree".to_string(), 1_720_573_440_000)]).unwrap();
        store
            .rewrite_dir_files(
                "C:\\tree",
                &[
                    ("C:\\tree\\alpha.txt".to_string(), "alpha.txt".to_string()),
                    ("C:\\tree\\beta.md".to_string(), "beta.md".to_string()),
                ],
            )
            .unwrap();

        let (files, dirs) = store.indexed_counts().unwrap();
        assert_eq!((files, dirs), (2, 1));

        let paths = store.indexed_paths().unwrap();
        assert_eq!(
            paths,
            vec!["C:\\tree\\alpha.txt".to_string(), "C:\\tree\\beta.md".to_string()]
        );

        let dirs = store.indexed_dirs().unwrap();
        assert_eq!(dirs[0], ("C:\\tree".to_string(), 1_720_573_440_000));
    }

    #[test]
    fn a_rewritten_directory_keeps_its_listing_as_the_whole_truth() {
        let store = memory_store();
        store.record_dirs(&[("C:\\tree".to_string(), 10)]).unwrap();
        store
            .rewrite_dir_files(
                "C:\\tree",
                &[
                    ("C:\\tree\\old.txt".to_string(), "old.txt".to_string()),
                    ("C:\\tree\\kept.txt".to_string(), "kept.txt".to_string()),
                ],
            )
            .unwrap();

        // One file is renamed away and one joins: the rewrite is the
        // whole listing, so `old` is gone and `new` is there.
        store
            .rewrite_dir_files(
                "C:\\tree",
                &[
                    ("C:\\tree\\kept.txt".to_string(), "kept.txt".to_string()),
                    ("C:\\tree\\new.txt".to_string(), "new.txt".to_string()),
                ],
            )
            .unwrap();

        let paths = store.indexed_paths().unwrap();
        assert_eq!(
            paths,
            vec!["C:\\tree\\kept.txt".to_string(), "C:\\tree\\new.txt".to_string()]
        );
    }

    #[test]
    fn forgotten_directories_take_their_files_along() {
        let store = memory_store();
        store.record_dirs(&[
            ("C:\\tree\\go".to_string(), 10),
            ("C:\\tree\\stay".to_string(), 10),
        ]).unwrap();
        store.rewrite_dir_files("C:\\tree\\go", &[("C:\\tree\\go\\file.txt".to_string(), "file.txt".to_string())]).unwrap();
        store.rewrite_dir_files("C:\\tree\\stay", &[("C:\\tree\\stay\\file.txt".to_string(), "file.txt".to_string())]).unwrap();

        store.forget_dirs(&["C:\\tree\\go".to_string()]).unwrap();

        assert_eq!(store.indexed_paths().unwrap(), vec!["C:\\tree\\stay\\file.txt"]);
        let dirs = store.indexed_dirs().unwrap();
        assert_eq!(dirs.len(), 1);
        assert_eq!(dirs[0].0, "C:\\tree\\stay");
    }
}
