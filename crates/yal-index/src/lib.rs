//! Discovery of installed applications and files.
//!
//! The index answers "what can the user launch?" It reads the host system
//! only; it never writes to it and never launches anything.

pub mod lnk;
pub mod windows;

pub use windows::{Discovered, Source, discover};

use std::path::{Path, PathBuf};

/// A file found by the file search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundFile {
    pub name: String,
    pub path: PathBuf,
    /// The directory containing the file, shown as the item subtitle.
    pub directory: PathBuf,
}

/// The roots that file search covers by default.
pub fn default_search_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for variable in ["USERPROFILE", "HOME"] {
        if let Some(home) = std::env::var_os(variable) {
            let home = PathBuf::from(home);
            for relative in ["Desktop", "Documents", "Downloads"] {
                let candidate = home.join(relative);
                if candidate.is_dir() {
                    roots.push(candidate);
                }
            }
            break;
        }
    }
    roots
}

/// Search directories for files matching a name.
///
/// The walk is bounded: at most `limit` results are collected and at most
/// `max_entries` directory entries are visited *per root*, so a
/// pathological tree cannot stall the palette. Each root is walked in
/// passes: first its immediate files, then its whole subtree — that way a
/// deep directory that eats the whole budget cannot starve the files
/// sitting next to it, which the depth-first order would otherwise leave
/// unvisited after a busy sibling is done.
pub fn search_files(
    roots: &[PathBuf],
    query: &str,
    limit: usize,
    max_entries: usize,
) -> Vec<FoundFile> {
    if query.is_empty() {
        return Vec::new();
    }
    let needle = query.to_lowercase();
    let mut found = Vec::new();

    for root in roots {
        if !root.is_dir() {
            continue;
        }
        let mut seen: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
        // The first pass is the root's own files; the second is everything
        // below it, bounded by the same walk depth as before.
        for depth in [1, 8] {
            let walker = ignore::WalkBuilder::new(root)
                .hidden(true)
                .ignore(true)
                .git_ignore(true)
                .max_depth(Some(depth))
                .build();
            let mut visited = 0usize;
            for entry in walker.flatten() {
                visited += 1;
                if visited > max_entries {
                    break;
                }
                if !entry.file_type().is_some_and(|kind| kind.is_file()) {
                    continue;
                }
                let path = entry.path();
                if !seen.insert(path.to_path_buf()) {
                    continue;
                }
                let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };
                if !name.to_lowercase().contains(&needle) {
                    continue;
                }
                found.push(FoundFile {
                    name: name.to_string(),
                    path: path.to_path_buf(),
                    directory: path
                        .parent()
                        .map(Path::to_path_buf)
                        .unwrap_or_default(),
                });
                if found.len() >= limit {
                    return found;
                }
            }
            // The root's own files are done walking in depth 1; whenever
            // the limit filled that pass the search is over.
            if found.len() >= limit {
                return found;
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_search_finds_matching_names() {
        let dir = std::env::temp_dir().join(format!(
            "yal-index-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("needle.txt"), b"x").unwrap();
        std::fs::write(dir.join("haystack.txt"), b"x").unwrap();

        let found = search_files(std::slice::from_ref(&dir), "needle", 10, 1000);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "needle.txt");

        std::fs::remove_dir_all(&dir).ok();
    }

    /// A root whose first subdirectory eats the whole walk budget must not
    /// starve the files sitting next to it: the root's own files are walked
    /// in their own pass.
    #[test]
    fn a_busy_subtree_cannot_starve_the_root_files() {
        let dir = std::env::temp_dir().join(format!(
            "yal-index-starve-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(dir.join("big")).unwrap();
        std::fs::write(dir.join("wanted.txt"), b"x").unwrap();
        for index in 0..50 {
            std::fs::create_dir_all(dir.join("big").join(format!("leaf{index}"))).unwrap();
        }

        // A budget far smaller than the busy subtree forces the old walk to
        // give up before the wanted file; the two-pass walk gets there.
        let found = search_files(
            std::slice::from_ref(&dir),
            "wanted",
            10,
            30,
        );
        assert_eq!(
            found.len(),
            1,
            "the root's own file is found within a tiny budget"
        );
        assert_eq!(found[0].name, "wanted.txt");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn file_search_respects_the_limit() {
        let dir = std::env::temp_dir().join(format!(
            "yal-index-limit-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        for index in 0..5 {
            std::fs::write(dir.join(format!("match{index}.txt")), b"x").unwrap();
        }
        let found = search_files(std::slice::from_ref(&dir), "match", 2, 1000);
        assert_eq!(found.len(), 2);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn empty_query_returns_nothing() {
        assert!(search_files(&[PathBuf::from(".")], "", 10, 100).is_empty());
    }
}
