//! The incremental index: the pass that keeps it fresh.
//!
//! The design is the donor project's, from
//! `C:\Users\dabin\development\yal\crates\yal-index\src\walk.rs`
//! (GPL-3.0-only, <https://github.com/Abinanthan-CG/yal>). The idea is two
//! tables: every listed directory with the time it was listed at, and every
//! listed file keyed by its path. A pass re-lists only the directories whose
//! modification time moved - which is every directory whose listing changed,
//! because a listing change is exactly what moves a directory's time.
//!
//! What changed in this port: the roots are the user's search scopes rather
//! than the machine's fixed drives, and what is skipped is decided by the
//! ignore rules rather than by a hard-coded list of names.
//!
//! Nothing here knows the store: the writes go through [`Backing`], which the
//! store implements.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::scope::IgnoreRules;

/// One indexed thing: a file or a folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The full path.
    pub path: String,
    /// The name, without the folders around it.
    pub name: String,
    /// The folder the thing is in.
    pub parent: String,
    /// The size in bytes. Zero for a folder.
    pub size: u64,
    /// When the thing last changed, in nanoseconds since the epoch.
    pub modified_ns: i64,
    /// Whether the thing is a folder.
    pub is_dir: bool,
}

/// The writes a pass needs.
///
/// The store implements this against SQLite; the tests implement it over a
/// map.
pub trait Backing {
    /// Every recorded directory and the time it was listed at, in nanoseconds
    /// since the epoch.
    fn recorded_dirs(&self) -> Vec<(String, i64)>;

    /// Record directories with the times they were listed at.
    fn remember_dirs(&mut self, rows: &[(String, i64)]);

    /// Rewrite one directory's listing: the listing is the whole truth.
    fn rewrite_entries(&mut self, dir: &str, rows: &[Entry]);

    /// Forget directories that are gone, together with what was in them.
    fn forget_dirs(&mut self, dirs: &[String]);
}

/// What one pass did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WalkReport {
    /// Directories re-listed: changed, new, or queued because an ancestor's
    /// listing changed.
    pub listed: usize,
    /// Recorded directories that vanished since the last pass.
    pub removed: usize,
    /// Whether the index changed at all, and a snapshot is worth reloading.
    pub changed: bool,
}

/// A directory's modification time in nanoseconds since the epoch.
///
/// `None` when the directory is gone, unreadable, no longer a directory, or
/// a link - a link's target could loop the walk, so links are treated as
/// absent.
///
/// Nanoseconds, not milliseconds: NTFS ticks at 100 nanoseconds, and a
/// millisecond floor can bury a write that lands in the same millisecond the
/// pass recorded the directory, so the change sits invisible until the
/// directory happens to change again.
///
/// The read is `symlink_metadata`, not `metadata`: the metadata of the
/// directory itself, without following what it points at. A junction's own
/// attributes carry the reparse point; followed, the target's attributes
/// would hide it and the walk would loop.
pub fn dir_mtime_ns(path: &Path) -> Option<i64> {
    let metadata = std::fs::symlink_metadata(path).ok()?;
    let kind = metadata.file_type();
    if !kind.is_dir() || kind.is_symlink() {
        return None;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // Reparse points besides plain symlinks - junctions, mount points -
        // carry the attribute; the file type check above might not set
        // `is_symlink` for them on every toolchain, so the attribute is the
        // surer stop.
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return None;
        }
    }
    let modified = metadata.modified().ok()?;
    let since = modified.duration_since(UNIX_EPOCH).ok()?;
    Some(i64::try_from(since.as_nanos()).unwrap_or(i64::MAX))
}

/// The time a file last changed, in nanoseconds since the epoch.
fn file_mtime_ns(metadata: &std::fs::Metadata) -> Option<i64> {
    let modified = metadata.modified().ok()?;
    let since = modified.duration_since(UNIX_EPOCH).ok()?;
    Some(i64::try_from(since.as_nanos()).unwrap_or(i64::MAX))
}

/// Run one refresh pass over the roots, through the backing.
///
/// The pass stats every recorded directory first - a burst of stat calls, no
/// listing - then re-lists only what changed, and the directories discovered
/// on the way. Deletion is exact: every recorded directory is stat'd on every
/// pass, so a subtree that vanished is reported missing one directory at a
/// time, and no prefix matching is needed, which `LIKE` would get wrong with
/// an underscore in a path. A directory listed here records the time read
/// *after* its listing, so a change that lands inside the instant between the
/// stat and the read is caught by the next pass rather than buried.
pub fn walk_once<B: Backing>(
    backing: &mut B,
    roots: &[PathBuf],
    rules: &IgnoreRules,
    home: &Path,
) -> WalkReport {
    // Phase one: stat everything recorded. Unchanged directories keep their
    // rows; changed and missing ones are queued.
    let mut kept: HashMap<String, i64> = HashMap::new();
    let mut queued: HashMap<String, i64> = HashMap::new();
    let mut removed: Vec<String> = Vec::new();
    for (path, mtime) in backing.recorded_dirs() {
        match dir_mtime_ns(Path::new(&path)) {
            Some(current) if current == mtime => {
                kept.insert(path, mtime);
            }
            Some(current) => {
                queued.insert(path, current);
            }
            None => removed.push(path),
        }
    }

    // An unrecorded root is new: its listing is due. A recorded root was just
    // handled by the stats above.
    for root in roots {
        let Some(text) = root.to_str() else { continue };
        if !kept.contains_key(text) && !queued.contains_key(text) {
            if let Some(mtime) = dir_mtime_ns(root) {
                queued.insert(text.to_string(), mtime);
            }
        }
    }

    // Phase two: list the queued directories, the deepest last; the listing
    // queues new directories behind their parents, so a parent is never
    // listed twice and a child is listed only once.
    let mut updates: Vec<(String, i64)> = Vec::new();
    let mut listed = 0usize;
    let mut order: Vec<String> = queued.keys().cloned().collect();
    order.sort_by_key(|path| path.matches('\\').count());
    let mut position = 0usize;
    while position < order.len() {
        let dir = order[position].clone();
        position += 1;

        // The listing. A directory that vanished between the stat and this
        // read joins the removals; its recorded rows are swept by the forget
        // below.
        let entries = match std::fs::read_dir(Path::new(&dir)) {
            Ok(read) => read,
            Err(_) => {
                if !removed.contains(&dir) {
                    removed.push(dir);
                }
                continue;
            }
        };

        // A directory recorded under one name and renamed into an ignored one
        // is evicted here: the record goes away the pass after the time moved.
        let skip_self = Path::new(&dir)
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| rules.is_ignored(name, Path::new(&dir), home))
            .unwrap_or(false);
        if skip_self {
            if !removed.contains(&dir) {
                removed.push(dir);
            }
            continue;
        }

        let mut rows: Vec<Entry> = Vec::new();
        for entry in entries.flatten() {
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            let kind = metadata.file_type();
            if kind.is_symlink() {
                // Links and junction points: the target could loop.
                continue;
            }
            let name = entry.file_name();
            let Some(name_text) = name.to_str() else {
                continue;
            };
            let full_path = entry.path();
            let Some(full) = full_path.to_str() else {
                continue;
            };
            if rules.is_ignored(name_text, &full_path, home) {
                continue;
            }
            if kind.is_dir() {
                if !kept.contains_key(full) && !queued.contains_key(full) {
                    let Some(mtime) = file_mtime_ns(&metadata) else {
                        continue;
                    };
                    queued.insert(full.to_string(), mtime);
                    order.push(full.to_string());
                }
                rows.push(Entry {
                    path: full.to_string(),
                    name: name_text.to_string(),
                    parent: dir.clone(),
                    size: 0,
                    modified_ns: file_mtime_ns(&metadata).unwrap_or(0),
                    is_dir: true,
                });
            } else if kind.is_file() {
                rows.push(Entry {
                    path: full.to_string(),
                    name: name_text.to_string(),
                    parent: dir.clone(),
                    size: metadata.len(),
                    modified_ns: file_mtime_ns(&metadata).unwrap_or(0),
                    is_dir: false,
                });
            }
        }
        backing.rewrite_entries(&dir, &rows);
        // The time recorded is the one *after* the listing: a change that
        // lands inside the instant between the stat and the read is caught by
        // the next pass. A directory that vanished while being listed is
        // swept here too, so its rewritten rows do not linger past this pass.
        if let Some(mtime) = dir_mtime_ns(Path::new(&dir)) {
            updates.push((dir, mtime));
            if updates.len() >= 250 {
                backing.remember_dirs(&updates);
                updates.clear();
            }
        } else if !removed.contains(&dir) {
            removed.push(dir);
        }
        listed += 1;
    }

    let report = WalkReport {
        listed,
        removed: removed.len(),
        changed: listed > 0 || !removed.is_empty(),
    };

    if !updates.is_empty() {
        backing.remember_dirs(&updates);
    }
    backing.forget_dirs(&removed);
    report
}

/// List the immediate children of a single directory without touching the index.
///
/// Directories are sorted first, then entries are sorted case-insensitively by
/// name with a deterministic case-sensitive tie-breaker. Unreadable directories
/// return an empty list.
pub fn list_directory(dir: &Path, include_hidden: bool) -> Vec<Entry> {
    let Ok(read_dir) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let parent = dir.to_string_lossy().into_owned();
    let mut entries = Vec::new();
    for item in read_dir {
        let Ok(entry) = item else {
            continue;
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        let full_path = entry.path();
        let Ok(metadata) = std::fs::symlink_metadata(&full_path) else {
            continue;
        };
        if !include_hidden {
            if name.starts_with('.') {
                continue;
            }
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
                const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;
                if metadata.file_attributes() & (FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM) != 0
                {
                    continue;
                }
            }
        }
        let is_dir = metadata.is_dir();
        let size = if is_dir { 0 } else { metadata.len() };
        let modified_ns = file_mtime_ns(&metadata).unwrap_or(0);
        entries.push(Entry {
            path: full_path.to_string_lossy().into_owned(),
            name,
            parent: parent.clone(),
            size,
            modified_ns,
            is_dir,
        });
    }
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.name.cmp(&b.name))
    });
    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scope::IgnorePattern;

    /// A backing that keeps its rows in maps, for the walker's tests.
    #[derive(Default)]
    struct MemoryBacking {
        dirs: HashMap<String, i64>,
        entries: HashMap<String, Entry>,
    }

    impl MemoryBacking {
        fn new() -> MemoryBacking {
            MemoryBacking::default()
        }

        /// The names listed under a directory, from the walker's own record.
        fn names_in(&self, dir: &Path) -> Vec<String> {
            let dir_text = dir.to_str().unwrap();
            let mut names: Vec<String> = self
                .entries
                .values()
                .filter(|entry| entry.parent == dir_text)
                .map(|entry| entry.name.clone())
                .collect();
            names.sort();
            names
        }
    }

    impl Backing for MemoryBacking {
        fn recorded_dirs(&self) -> Vec<(String, i64)> {
            self.dirs
                .iter()
                .map(|(path, mtime)| (path.clone(), *mtime))
                .collect()
        }

        fn remember_dirs(&mut self, rows: &[(String, i64)]) {
            for (path, mtime) in rows {
                self.dirs.insert(path.clone(), *mtime);
            }
        }

        fn rewrite_entries(&mut self, dir: &str, rows: &[Entry]) {
            self.entries.retain(|_, entry| entry.parent != dir);
            for entry in rows {
                self.entries.insert(entry.path.clone(), entry.clone());
            }
        }

        fn forget_dirs(&mut self, dirs: &[String]) {
            self.dirs.retain(|path, _| !dirs.contains(path));
            self.entries
                .retain(|_, entry| !dirs.contains(&entry.parent));
        }
    }

    /// The home folder the rules are matched against.
    fn home() -> PathBuf {
        PathBuf::from("C:\\Users\\dabin")
    }

    fn unique_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "yal-files-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_fresh_tree_is_listed_whole() {
        let root = unique_dir("fresh");
        std::fs::write(root.join("alpha.txt"), b"x").unwrap();
        std::fs::write(root.join("beta.txt"), b"x").unwrap();
        std::fs::create_dir_all(root.join("scratch")).unwrap();
        std::fs::write(root.join("scratch").join("gamma.txt"), b"x").unwrap();

        let mut backing = MemoryBacking::new();
        let report = walk_once(
            &mut backing,
            std::slice::from_ref(&root),
            &IgnoreRules::empty(),
            &home(),
        );
        assert!(report.changed);
        assert!(
            report.listed >= 2,
            "the root and its subdir were listed: {}",
            report.listed
        );
        assert_eq!(
            backing.names_in(&root),
            vec!["alpha.txt", "beta.txt", "scratch"]
        );
        assert_eq!(backing.names_in(&root.join("scratch")), vec!["gamma.txt"]);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn an_unchanged_tree_is_not_listed_twice() {
        let root = unique_dir("steady");
        std::fs::write(root.join("keep.txt"), b"x").unwrap();

        let mut backing = MemoryBacking::new();
        walk_once(
            &mut backing,
            std::slice::from_ref(&root),
            &IgnoreRules::empty(),
            &home(),
        );
        let first_dirs = backing.dirs.len();
        assert!(first_dirs >= 1);

        let report = walk_once(
            &mut backing,
            std::slice::from_ref(&root),
            &IgnoreRules::empty(),
            &home(),
        );
        assert!(!report.changed, "nothing moved, the pass changed nothing");
        assert_eq!(report.listed, 0);
        assert_eq!(backing.names_in(&root), vec!["keep.txt"]);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_creation_and_a_removal_are_both_caught_without_a_full_walk() {
        let root = unique_dir("catch");
        std::fs::create_dir_all(root.join("branch")).unwrap();
        std::fs::write(root.join("branch").join("old.txt"), b"x").unwrap();

        let mut backing = MemoryBacking::new();
        walk_once(
            &mut backing,
            std::slice::from_ref(&root),
            &IgnoreRules::empty(),
            &home(),
        );
        let listed_dirs_at_cold = backing.dirs.len();

        // The writes that follow must land *after* the tick the pass recorded
        // `branch` at, so a few milliseconds make the race impossible.
        std::thread::sleep(std::time::Duration::from_millis(5));

        std::fs::write(root.join("branch").join("removed.txt"), b"x").unwrap();
        std::fs::create_dir_all(root.join("branch").join("new-leaf")).unwrap();
        std::fs::write(root.join("branch").join("new-leaf").join("added.txt"), b"x").unwrap();

        let report = walk_once(
            &mut backing,
            std::slice::from_ref(&root),
            &IgnoreRules::empty(),
            &home(),
        );
        assert!(report.changed);
        assert_eq!(
            report.listed, 2,
            "only the changed dir and the newly created dir"
        );
        assert_eq!(
            backing.names_in(&root.join("branch")),
            vec!["new-leaf", "old.txt", "removed.txt"]
        );
        assert_eq!(
            backing.names_in(&root.join("branch").join("new-leaf")),
            vec!["added.txt"]
        );
        assert_eq!(
            backing.dirs.len(),
            listed_dirs_at_cold + 1,
            "the new leaf is recorded"
        );

        // Removed files disappear without anything else moving.
        std::thread::sleep(std::time::Duration::from_millis(5));
        std::fs::remove_file(root.join("branch").join("old.txt")).unwrap();
        let report = walk_once(
            &mut backing,
            std::slice::from_ref(&root),
            &IgnoreRules::empty(),
            &home(),
        );
        assert_eq!(
            report.listed, 1,
            "only the dir whose listing shrank is listed"
        );
        assert_eq!(
            backing.names_in(&root.join("branch")),
            vec!["new-leaf", "removed.txt"]
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_removed_directory_forgets_itself_and_its_contents() {
        let root = unique_dir("gone");
        std::fs::create_dir_all(root.join("dying").join("deep")).unwrap();
        std::fs::write(root.join("dying").join("deep").join("file.txt"), b"x").unwrap();

        let mut backing = MemoryBacking::new();
        walk_once(
            &mut backing,
            std::slice::from_ref(&root),
            &IgnoreRules::empty(),
            &home(),
        );

        std::fs::remove_dir_all(root.join("dying")).unwrap();
        let report = walk_once(
            &mut backing,
            std::slice::from_ref(&root),
            &IgnoreRules::empty(),
            &home(),
        );
        assert!(report.changed);
        assert_eq!(
            report.removed, 2,
            "the dying dir and its deep one are both reported missing"
        );
        assert!(!backing
            .dirs
            .contains_key(root.join("dying").to_str().unwrap()));
        assert_eq!(backing.names_in(&root.join("dying")), Vec::<String>::new());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn an_ignored_folder_is_never_entered_or_recorded() {
        let root = unique_dir("ignored");
        std::fs::create_dir_all(root.join("node_modules")).unwrap();
        std::fs::write(root.join("node_modules").join("junk.txt"), b"x").unwrap();
        std::fs::write(root.join("keep.txt"), b"x").unwrap();

        let rules = IgnoreRules::with_user_patterns(vec![IgnorePattern::parse("scratch.tmp")]);
        let mut backing = MemoryBacking::new();
        walk_once(&mut backing, std::slice::from_ref(&root), &rules, &home());
        assert_eq!(backing.names_in(&root), vec!["keep.txt"]);
        assert!(!backing
            .dirs
            .contains_key(root.join("node_modules").to_str().unwrap()));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn an_ignored_file_is_not_recorded_but_its_folder_is() {
        let root = unique_dir("ignored-file");
        std::fs::write(root.join("keep.txt"), b"x").unwrap();
        std::fs::write(root.join("scratch.tmp"), b"x").unwrap();

        let rules = IgnoreRules::with_user_patterns(vec![IgnorePattern::parse("scratch.tmp")]);
        let mut backing = MemoryBacking::new();
        walk_once(&mut backing, std::slice::from_ref(&root), &rules, &home());
        assert_eq!(backing.names_in(&root), vec!["keep.txt"]);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_folder_renamed_into_an_ignored_name_is_evicted() {
        let root = unique_dir("renamed");
        std::fs::create_dir_all(root.join("work")).unwrap();
        std::fs::write(root.join("work").join("file.txt"), b"x").unwrap();

        let mut backing = MemoryBacking::new();
        walk_once(
            &mut backing,
            std::slice::from_ref(&root),
            &IgnoreRules::empty(),
            &home(),
        );
        assert!(backing
            .dirs
            .contains_key(root.join("work").to_str().unwrap()));

        std::thread::sleep(std::time::Duration::from_millis(5));
        std::fs::rename(root.join("work"), root.join("node_modules")).unwrap();

        walk_once(
            &mut backing,
            std::slice::from_ref(&root),
            &IgnoreRules::defaults(),
            &home(),
        );
        assert!(
            !backing
                .dirs
                .contains_key(root.join("node_modules").to_str().unwrap()),
            "the renamed folder is evicted"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_file_carries_its_size_and_the_time_it_changed() {
        let root = unique_dir("facts");
        std::fs::write(root.join("note.txt"), b"hello").unwrap();

        let mut backing = MemoryBacking::new();
        walk_once(
            &mut backing,
            std::slice::from_ref(&root),
            &IgnoreRules::empty(),
            &home(),
        );
        let entry = backing
            .entries
            .values()
            .find(|entry| entry.name == "note.txt")
            .expect("the entry");
        assert_eq!(entry.size, 5);
        assert!(entry.modified_ns > 0);
        assert!(!entry.is_dir);
        assert_eq!(entry.parent, root.to_str().unwrap());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_folder_is_recorded_as_a_folder() {
        let root = unique_dir("folder");
        std::fs::create_dir_all(root.join("inner")).unwrap();

        let mut backing = MemoryBacking::new();
        walk_once(
            &mut backing,
            std::slice::from_ref(&root),
            &IgnoreRules::empty(),
            &home(),
        );
        let entry = backing
            .entries
            .values()
            .find(|entry| entry.name == "inner")
            .expect("the entry");
        assert!(entry.is_dir);
        assert_eq!(entry.size, 0);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn no_roots_means_nothing_is_listed() {
        let mut backing = MemoryBacking::new();
        let report = walk_once(&mut backing, &[], &IgnoreRules::empty(), &home());
        assert_eq!(report.listed, 0);
        assert!(!report.changed);
    }

    #[test]
    fn a_root_that_is_not_a_directory_is_not_listed() {
        let root = unique_dir("not-a-dir");
        let file = root.join("plain.txt");
        std::fs::write(&file, b"x").unwrap();

        let mut backing = MemoryBacking::new();
        let report = walk_once(&mut backing, &[file], &IgnoreRules::empty(), &home());
        assert_eq!(report.listed, 0);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn list_directory_sorts_dirs_first_case_insensitively_and_filters_hidden() {
        let root = unique_dir("list-dir");
        std::fs::create_dir_all(root.join("zebra_dir")).unwrap();
        std::fs::create_dir_all(root.join("Alpha_dir")).unwrap();
        std::fs::create_dir_all(root.join(".hidden_dir")).unwrap();
        std::fs::write(root.join("beta.txt"), b"hello").unwrap();
        std::fs::write(root.join("Apple.txt"), b"1234567").unwrap();
        std::fs::write(root.join(".secret"), b"shh").unwrap();

        let visible = list_directory(&root, false);
        let visible_names: Vec<&str> = visible.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(
            visible_names,
            vec!["Alpha_dir", "zebra_dir", "Apple.txt", "beta.txt"]
        );
        assert!(visible[0].is_dir);
        assert_eq!(visible[0].size, 0);
        assert!(visible[0].modified_ns > 0);
        assert_eq!(visible[0].parent, root.to_string_lossy());

        assert!(!visible[2].is_dir);
        assert_eq!(visible[2].size, 7);
        assert!(visible[2].modified_ns > 0);
        assert_eq!(visible[3].size, 5);

        let all = list_directory(&root, true);
        let all_names: Vec<&str> = all.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(
            all_names,
            vec![
                ".hidden_dir",
                "Alpha_dir",
                "zebra_dir",
                ".secret",
                "Apple.txt",
                "beta.txt"
            ]
        );
        assert_eq!(all[3].size, 3);

        let missing = list_directory(&root.join("does-not-exist"), true);
        assert!(missing.is_empty());

        std::fs::remove_dir_all(&root).ok();
    }
}
