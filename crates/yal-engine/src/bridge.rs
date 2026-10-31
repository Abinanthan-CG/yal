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
