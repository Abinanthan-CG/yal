//! The whole-drive file index: the pass that keeps it fresh.
//!
//! The design lives in `docs/architecture.md`. In short: the index is two
//! tables — every listed directory with its modification time, and every
//! listed file keyed by its path — and a pass re-lists only the
//! directories whose modification time moved, which is every directory
//! whose listing changed, because a listing change is exactly what moves
//! a directory's mtime.
//!
//! Nothing here knows the store: the writes go through the [`Backing`]
//! interface, which `yal-engine` implements for the store — the
//! application layer is the one crate allowed to know both adapters.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// Whether a directory name is never entered and never recorded.
///
/// Build trees, system junk and caches: not launcher material. Recorded
/// honestly in the design: skipping is by name, so a project that
/// deliberately names a build folder one of these will not have its files
/// indexed, and the list is the price of a walk that stays in the
/// hundreds of thousands of files instead of the millions.
pub fn is_skipped(name: &str) -> bool {
    let lower = name.to_lowercase();
    if lower.starts_with('.') {
        return true;
    }
    matches!(
        lower.as_str(),
        "$recycle.bin"
            | "system volume information"
            | "$winreagent"
            | "recovery"
            | "windows"
            | "programdata"
            | "appdata"
            | "program files"
            | "program files (x86)"
            | "node_modules"
            | "target"
            | "venv"
            | "__pycache__"
            | "cache"
    )
}

/// The roots the index covers: the machine's fixed drives.
///
/// Removable and network drives are the ones that can disappear or hang
/// under a walk, so the index covers what stays put. On other platforms
/// there are no fixed drives in this sense, and the list is empty; the
/// combined `search_files` walk keeps working regardless.
#[cfg(windows)]
pub fn fixed_drive_roots() -> Vec<PathBuf> {
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{GetDriveTypeW, GetLogicalDrives};
    use windows::Win32::System::WindowsProgramming::DRIVE_FIXED;

    let mask = unsafe { GetLogicalDrives() };
    let mut roots = Vec::new();
    for index in 0..26u32 {
        if mask & (1 << index) == 0 {
            continue;
        }
        let letter = (b'A' + index as u8) as char;
        let text = format!("{letter}:\\");
        let root = {
            let wide = text.encode_utf16().chain([0]).collect::<Vec<u16>>();
            unsafe { GetDriveTypeW(PCWSTR::from_raw(wide.as_ptr())) }
        };
        if root == DRIVE_FIXED {
            roots.push(PathBuf::from(text));
        }
    }
    roots
}

/// The roots on any platform outside Windows: none, by this definition.
#[cfg(not(windows))]
pub fn fixed_drive_roots() -> Vec<PathBuf> {
    Vec::new()
}

/// A directory's modification time in nanoseconds since the epoch, or
/// `None` when it is gone, unreadable, no longer a directory, or a
/// link — a link's target could loop the walk, so links are treated as
/// absent.
///
/// Nanoseconds, not milliseconds: NTFS ticks at 100 nanoseconds, and a
/// millisecond floor can bury a write that lands in the same
/// millisecond the pass recorded the directory — the change then sits
/// invisible until the directory happens to change again.
///
/// The read is `symlink_metadata`, not `metadata`: the metadata of the
/// directory itself, without following what it points at. A junction's
/// own attributes carry the reparse point; followed, the target's
/// attributes would hide it, and the walk would loop.
fn dir_mtime_ns(path: &Path) -> Option<i64> {
    let metadata = std::fs::symlink_metadata(path).ok()?;
    let kind = metadata.file_type();
    if !kind.is_dir() || kind.is_symlink() {
        return None;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // Reparse points besides plain symlinks (junctions, mount points)
        // carry the attribute; the file type check above might not set
        // `is_symlink` for them on every toolchain, so the attribute is
        // the surer stop.
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return None;
        }
    }
    let modified = metadata.modified().ok()?;
    let since = modified.duration_since(UNIX_EPOCH).ok()?;
    Some(i64::try_from(since.as_nanos()).unwrap_or(i64::MAX))
}

/// The writes a pass needs. `yal-engine` implements this for the store;
/// tests implement it over a `HashMap`.
pub trait Backing {
    /// Every recorded directory and the modification time it was listed
    /// at, in nanoseconds since the epoch.
    fn recorded_dirs(&self) -> Vec<(String, i64)>;
    /// Record directories with their modification times.
    fn remember_dirs(&mut self, rows: &[(String, i64)]);
    /// Rewrite one directory's files: the listing is the whole truth.
    fn rewrite_files(&mut self, dir: &str, rows: &[(String, String)]);
    /// Forget directories that are gone, together with their files.
    fn forget_dirs(&mut self, dirs: &[String]);
}

/// What one pass did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WalkReport {
    /// Directories re-listed (changed, new, or queued because an
    /// ancestor's listing changed).
    pub listed: usize,
    /// Recorded directories that vanished since the last pass.
    pub removed: usize,
    /// Whether the snapshot over the index is out of date and worth
    /// reloading.
    pub changed: bool,
}

/// Run one refresh pass over the roots, through the backing.
///
/// The pass stats every recorded directory first — a burst of stat calls,
/// no listing — then re-lists only what changed, and the directories
/// discovered on the way. Deletion is exact: every recorded directory is
/// stat'd on every pass, so a subtree that vanished is reported missing
/// one directory at a time, and no prefix matching is needed, which
/// `LIKE` would get wrong with an underscore in a path. A directory
/// listed here records the mtime read *after* its listing, so a change
/// that lands inside the instant between the stat and the read is caught
/// by the next pass rather than buried.
pub fn walk_once<B: Backing>(backing: &mut B, roots: &[PathBuf]) -> WalkReport {
    // Phase one: stat everything recorded. Unchanged dirs keep their
    // rows; changed daemon and missing dirs are queued.
    let mut kept: HashMap<String, i64> = HashMap::new();
    let mut queued: HashMap<String, i64> = HashMap::new();
    let mut removed: Vec<String> = Vec::new();
    for (path, mtime) in backing.recorded_dirs() {
        match dir_mtime_ns(Path::new(&path)) {
            Some(t) if t == mtime => {
                kept.insert(path, mtime);
            }
            Some(t) => {
                queued.insert(path, t);
            }
            None => removed.push(path),
        }
    }

    // An unrecorded root is new: its listing is due. A recorded roots
    // was just handled by the stats above.
    for root in roots {
        let Some(text) = root.to_str() else { continue };
        if !kept.contains_key(text) && !queued.contains_key(text) {
            if let Some(mtime) = dir_mtime_ns(root) {
                queued.insert(text.to_string(), mtime);
            }
        }
    }

    // Phase two: list the queued dirs, newest queued last; the listing
    // queues new dirs behind their parents, so a parent is never listed
    // twice and a child is listed only once.
    let mut updates: Vec<(String, i64)> = Vec::new();
    let mut listed = 0usize;
    let mut order: Vec<String> = queued.keys().cloned().collect();
    order.sort_by_key(|path| path.matches('\\').count());
    let mut position = 0usize;
    while position < order.len() {
        let dir = order[position].clone();
        position += 1;

        // The listing. A directory that vanished between the stat and
        // this read joins the removals; its recorded rows are swept by
        // the forget below.
        let entries = match std::fs::read_dir(Path::new(&dir)) {
            Ok(read) => read,
            Err(_) => {
                if !removed.contains(&dir) {
                    removed.push(dir);
                }
                continue;
            }
        };

        // A directory recorded under one name and renamed into a
        // skip-listed one is evicted here, as old as a renaming scheme
        // makes it: the record goes away the pass after the mtime moved.
        let skip_self = Path::new(&dir)
            .file_name()
            .and_then(|name| name.to_str())
            .map(is_skipped)
            .unwrap_or(false);
        if skip_self {
            if !removed.contains(&dir) {
                removed.push(dir);
            }
            continue;
        }
        let mut files: Vec<(String, String)> = Vec::new();
        for entry in entries.flatten() {
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            let kind = meta.file_type();
            if kind.is_symlink() {
                // Links and junction points: the target could loop.
                continue;
            }
            let name = entry.file_name();
            let Some(name_text) = name.to_str() else {
                continue;
            };
            if kind.is_dir() {
                if is_skipped(name_text) {
                    continue;
                }
                let child_path = entry.path();
                let Some(child_text) = child_path.to_str() else {
                    continue;
                };
                if !kept.contains_key(child_text) && !queued.contains_key(child_text) {
                    let Some(mtime) = {
                        meta.modified().ok().and_then(|at| at.duration_since(UNIX_EPOCH).ok())
                    }.map(|since| i64::try_from(since.as_nanos()).unwrap_or(i64::MAX)) else {
                        continue;
                    };
                    queued.insert(child_text.to_string(), mtime);
                    order.push(child_text.to_string());
                }
            } else if kind.is_file() {
                let full_path = entry.path();
                let Some(full) = full_path.to_str() else {
                    continue;
                };
                files.push((full.to_string(), name_text.to_string()));
            }
        }
        backing.rewrite_files(&dir, &files);
        // The mtime recorded is the one *after* the listing: a change
        // that lands inside the instant between the stat and the read
        // is caught by the next pass. A directory that vanished while
        // being listed is swept here too, so its rewritten rows do not
        // linger past this pass.
        if let Some(mtime) = dir_mtime_ns(Path::new(&dir)) {
            updates.push((dir, mtime));
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

    backing.remember_dirs(&updates);
    backing.forget_dirs(&removed);
    report
}

/// The indexed paths whose file name contains the needle.
///
/// This is the in-memory scan the palette pays for a whole-drive query:
/// contains over file names, case-folded, over a snapshot of paths the
/// caller loaded from its own copy. `exclude` carries the hits the
/// combined live walk already produced, so the index can only add rows,
/// never shadow fresher ones — borrowed, nothing reowned.
pub fn match_paths<'a>(
    paths: &'a [PathBuf],
    needle: &str,
    exclude: &std::collections::HashSet<&PathBuf>,
    limit: usize,
) -> Vec<&'a PathBuf> {
    if needle.is_empty() || limit == 0 {
        return Vec::new();
    }
    let lower = needle.to_lowercase();
    let mut found = Vec::new();
    for path in paths {
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.to_lowercase().contains(&lower) {
            continue;
        }
        if exclude.contains(path) {
            continue;
        }
        found.push(path);
        if found.len() >= limit {
            return found;
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A backing that keeps its rows in maps, for the walker's tests.
    struct MemoryBacking {
        dirs: HashMap<String, i64>,
        files: HashMap<String, (String, String)>,
    }

    impl MemoryBacking {
        fn new() -> Self {
            Self {
                dirs: HashMap::new(),
                files: HashMap::new(),
            }
        }
    }

    impl Backing for MemoryBacking {
        fn recorded_dirs(&self) -> Vec<(String, i64)> {
            self.dirs.iter().map(|(path, mtime)| (path.clone(), *mtime)).collect()
        }

        fn remember_dirs(&mut self, rows: &[(String, i64)]) {
            for (path, mtime) in rows {
                self.dirs.insert(path.clone(), *mtime);
            }
        }

        fn rewrite_files(&mut self, dir: &str, rows: &[(String, String)]) {
            self.files.retain(|_, (file_dir, _)| file_dir != dir);
            for (path, name) in rows {
                self.files.insert(path.clone(), (dir.to_string(), name.clone()));
            }
        }

        fn forget_dirs(&mut self, dirs: &[String]) {
            self.dirs.retain(|path, _| !dirs.contains(path));
            self.files.retain(|_, (file_dir, _)| !dirs.contains(file_dir));
        }
    }

    fn unique_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("yal-walk-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// The names listed under `dir`, from walker's own record.
    fn names_in(backing: &MemoryBacking, dir: &Path) -> Vec<String> {
        let dir_text = dir.to_str().unwrap();
        let mut names: Vec<String> = backing
            .files
            .values()
            .filter(|(file_dir, _)| file_dir == dir_text)
            .map(|(_, name)| name.clone())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn a_fresh_tree_is_listed_whole() {
        let root = unique_dir("fresh");
        std::fs::write(root.join("alpha.txt"), b"x").unwrap();
        std::fs::write(root.join("beta.txt"), b"x").unwrap();
        std::fs::create_dir_all(root.join("scratch")).unwrap();
        std::fs::write(root.join("scratch").join("gamma.txt"), b"x").unwrap();

        let mut backing = MemoryBacking::new();
        let report = walk_once(&mut backing, std::slice::from_ref(&root));
        assert!(report.changed);
        assert!(report.listed >= 2, "the root and its subdir were listed: {}", report.listed);
        assert_eq!(names_in(&backing, &root), vec!["alpha.txt", "beta.txt"]);
        assert_eq!(
            names_in(&backing, &root.join("scratch")),
            vec!["gamma.txt"]
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn an_unchanged_tree_is_not_listed_twice() {
        let root = unique_dir("steady");
        std::fs::write(root.join("keep.txt"), b"x").unwrap();

        let mut backing = MemoryBacking::new();
        walk_once(&mut backing, std::slice::from_ref(&root));
        let first_dirs = backing.dirs.len();
        assert!(first_dirs >= 1);

        let report = walk_once(&mut backing, std::slice::from_ref(&root));
        assert!(!report.changed, "nothing moved, the pass changed nothing");
        assert_eq!(report.listed, 0);
        assert_eq!(names_in(&backing, &root), vec!["keep.txt"]);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_creation_and_a_removal_are_both_caught_without_a_full_walk() {
        let root = unique_dir("catch");
        std::fs::create_dir_all(root.join("branch")).unwrap();
        std::fs::write(root.join("branch").join("old.txt"), b"x").unwrap();

        let mut backing = MemoryBacking::new();
        walk_once(&mut backing, std::slice::from_ref(&root));
        let listed_dirs_at_cold = backing.dirs.len();

        // The writes that follow must land *after* the tick the pass
        // recorded `branch` at — the clock moves by whole ticks, so a
        // change finding no new tick is invisible by design's own
        // arithmetic. A few milliseconds make the race impossible.
        std::thread::sleep(std::time::Duration::from_millis(5));

        // A new file in a deeper-new dir, and an old file: only the two
        // changed dirs should be listed again.
        std::fs::write(root.join("branch").join("removed.txt"), b"x").unwrap();
        std::fs::create_dir_all(root.join("branch").join("new-leaf")).unwrap();
        std::fs::write(root.join("branch").join("new-leaf").join("added.txt"), b"x").unwrap();

        let report = walk_once(&mut backing, std::slice::from_ref(&root));
        assert!(report.changed);
        let listed = report.listed;
        assert!(
            listed == 2,
            "only the changed dir and a newly created dir: listed {listed} of {listed_dirs_at_cold}"
        );
        let names = names_in(&backing, &root.join("branch"));
        assert_eq!(
            names,
            vec!["old.txt", "removed.txt"],
            "the branch's file rows; the new-leaf dir is a dir row, not a file row"
        );
        assert_eq!(
            names_in(&backing, &root.join("branch").join("new-leaf")),
            vec!["added.txt"]
        );
        assert!(backing.dirs.contains_key(
            root.join("branch").join("new-leaf").to_str().unwrap()
        ));
        assert_eq!(backing.dirs.len(), listed_dirs_at_cold + 1, "the new leaf is recorded");

        // Removed files disappear without anything else moving.
        std::thread::sleep(std::time::Duration::from_millis(5));
        std::fs::remove_file(root.join("branch").join("old.txt")).unwrap();
        let report = walk_once(&mut backing, std::slice::from_ref(&root));
        assert_eq!(
            report.listed, 1,
            "only the dir whose listing shrunk is listed"
        );
        let names = names_in(&backing, &root.join("branch"));
        assert_eq!(names, vec!["removed.txt"], "only the file removal is visible here");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_removed_dir_forgets_itself_and_its_files() {
        let root = unique_dir("gone");
        std::fs::create_dir_all(root.join("dying").join("deep")).unwrap();
        std::fs::write(root.join("dying").join("deep").join("file.txt"), b"x").unwrap();

        let mut backing = MemoryBacking::new();
        walk_once(&mut backing, std::slice::from_ref(&root));

        std::fs::remove_dir_all(root.join("dying")).unwrap();
        let report = walk_once(&mut backing, std::slice::from_ref(&root));
        assert!(report.changed);
        assert_eq!(report.removed, 2, "the dying dir and its deep one are both reported missing");
        assert!(!backing.dirs.contains_key(
            root.join("dying").to_str().unwrap()
        ));
        assert_eq!(names_in(&backing, &root.join("dying")), Vec::<String>::new());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn skipped_names_and_dotfiles_are_never_recorded() {
        let root = unique_dir("skips");
        std::fs::create_dir_all(root.join("node_modules")).unwrap();
        std::fs::write(root.join("node_modules").join("junk.txt"), b"x").unwrap();
        std::fs::create_dir_all(root.join(".github")).unwrap();
        std::fs::write(root.join("seen.txt"), b"x").unwrap();

        let mut backing = MemoryBacking::new();
        walk_once(&mut backing, std::slice::from_ref(&root));
        let dir_texts: Vec<&String> = backing.dirs.keys().collect();
        assert!(
            dir_texts.iter().all(|path| !path.contains("node_modules") && !path.contains(".github")),
            "no skip-listed dir is recorded: {dir_texts:?}"
        );
        assert_eq!(names_in(&backing, &root), vec!["seen.txt"]);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn matching_names_is_a_contains_scan_that_respects_the_exclude() {
        let paths: Vec<PathBuf> = ["C:\\work\\needle.txt", "C:\\work\\haystack.txt",
            "C:\\works\\needle.md", "C:\\other\\readme"]
            .iter()
            .map(PathBuf::from)
            .collect();

        // The exclude holds borrowed paths — the walker never takes
        // ownership of what it skips.
        let exclude: std::collections::HashSet<&PathBuf> =
            paths[..1].iter().collect();
        let found = match_paths(&paths, "needle", &exclude, 10);
        assert_eq!(found, vec![&paths[2]], "excluded hits are skipped");

        let case_fold = match_paths(&paths, "NEEDLE", &std::collections::HashSet::new(), 10);
        assert_eq!(case_fold.len(), 2);

        let capped = match_paths(&paths, "e", &std::collections::HashSet::new(), 2);
        assert_eq!(capped.len(), 2, "the limit holds");

        let empty = match_paths(&paths, "", &std::collections::HashSet::new(), 10);
        assert!(empty.is_empty(), "an empty needle finds nothing");
    }

    #[test]
    fn a_dir_renamed_into_a_skip_listed_name_is_evicted() {
        let root = unique_dir("renamed");
        std::fs::create_dir_all(root.join("keepdir")).unwrap();
        std::fs::write(root.join("keepdir").join("junk.txt"), b"x").unwrap();

        let mut backing = MemoryBacking::new();
        walk_once(&mut backing, std::slice::from_ref(&root));
        let recorded = root.join("keepdir").to_str().unwrap().to_string();
        assert!(backing.dirs.contains_key(&recorded));

        // Renamed into a skip-listed name; the next pass's stat sees the
        // mtime move, the listing refuses it, and its rows go.
        let renamed = root.join("node_modules");
        std::fs::rename(root.join("keepdir"), &renamed).unwrap();
        let report = walk_once(&mut backing, std::slice::from_ref(&root));
        assert!(report.changed);
        let forgotten = report.removed;
        assert!(
            forgotten >= 1,
            "the renamed dir is among the removed: {forgotten}"
        );
        assert!(!backing.dirs.contains_key(&recorded));
    }

    #[cfg(windows)]
    #[test]
    fn fixed_drives_look_like_drive_roots() {
        let roots = fixed_drive_roots();
        assert!(!roots.is_empty(), "a machine with a system drive has one");
        for root in &roots {
            assert!(root.is_dir(), "{root:?} names a directory");
            assert!(root.to_str().map(|t| t.ends_with(":\\")).unwrap_or(false));
        }
    }

    #[cfg(windows)]
    #[test]
    fn a_junction_is_treated_as_absent() {
        // POSIX-style demonstration junction created with mklink; skip
        // when the shell lacks it. The mount point attribute is what the
        // mtime reader refuses.
        let root = unique_dir("repase");
        std::fs::create_dir_all(root.join("real")).unwrap();
        let created = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(root.join("loop"))
            .arg(root.join("real"))
            .output();
        if let Ok(output) = created {
            if output.status.success() {
                assert_eq!(dir_mtime_ns(&root.join("loop")), None, "a junction reads as absent");
            }
        }
        std::fs::remove_dir_all(&root).ok();
    }
}
