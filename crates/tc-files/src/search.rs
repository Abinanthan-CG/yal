//! The search over the index.
//!
//! Upstream's rules:
//!
//! - **Every word you type must appear in the file name, in any order.**
//! - **The best matches come first**: an exact name, then names that start
//!   with what you typed.
//! - **Files in the home folder rank above hidden files** and the contents of
//!   the program folders.
//! - **At most 200 results are shown.**
//! - **With nothing typed**, the list is the files in the home folder that
//!   changed most recently.

use std::path::Path;

use unicode_normalization::UnicodeNormalization;

use crate::index::Entry;

/// The most results the palette shows.
pub const RESULT_LIMIT: usize = 200;

/// How many recent files the empty list shows.
pub const RECENT_LIMIT: usize = 20;

/// Fold a name for matching: lower case, and without the accents.
///
/// Upstream's matching ignores case and accents, so `cafe` finds `café` and
/// the other way round.
pub fn fold(text: &str) -> String {
    text.to_lowercase()
        .nfkd()
        .filter(|character| !unicode_normalization::char::is_combining_mark(*character))
        .collect()
}

/// How well a name or path matched. Lower is better.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MatchTier {
    /// The name is exactly what was typed.
    Exact,
    /// The name starts with what was typed.
    Prefix,
    /// The name contains every word, somewhere.
    Contains,
    /// Every word appears in the path relative to the user's home folder
    /// (e.g., files inside a matching folder like `AIStudio2API\runtime\...`).
    PathContains,
}

/// The words of a query.
pub fn query_words(query: &str) -> Vec<String> {
    query.split_whitespace().map(fold).collect()
}

/// Whether every word of the query appears in the name.
pub fn matches_words(name: &str, words: &[String]) -> bool {
    if words.is_empty() {
        return false;
    }
    let lowered = fold(name);
    words.iter().all(|word| lowered.contains(word))
}

/// Relative path text used for fallback path-segment matching.
///
/// Strips `home` first so queries matching the user's profile folder name
/// (e.g. `users` or the username) never spuriously match every file on disk.
fn relative_path_for_search<'a>(path: &'a str, home: &Path) -> &'a str {
    let home_str = home.to_string_lossy();
    let home_trimmed = home_str.trim_end_matches(['\\', '/']);
    if !home_trimmed.is_empty() && path.len() > home_trimmed.len() {
        let prefix = &path[..home_trimmed.len()];
        if prefix.eq_ignore_ascii_case(home_trimmed) {
            let rest = &path[home_trimmed.len()..];
            if let Some(stripped) = rest.strip_prefix('\\').or_else(|| rest.strip_prefix('/')) {
                return stripped;
            }
        }
    }
    path
}

/// How well a name matched.
pub fn tier_of(name: &str, query: &str) -> MatchTier {
    let lowered = fold(name);
    let query = fold(query.trim());
    if !query.is_empty() && lowered == query {
        return MatchTier::Exact;
    }
    if !query.is_empty() && lowered.starts_with(&query) {
        return MatchTier::Prefix;
    }
    MatchTier::Contains
}

/// Whether a path is hidden: a dot-file, or something inside one.
pub fn is_hidden(path: &Path) -> bool {
    path.components()
        .any(|component| component.as_os_str().to_string_lossy().starts_with('.'))
}

/// Whether a path is inside one of the program folders.
///
/// Upstream leaves the contents of application bundles and the busiest parts
/// of the system folders out of the ranking; on Windows those are the program
/// folders and the app data.
pub fn is_program_area(path: &Path) -> bool {
    let text = path.to_string_lossy().to_lowercase();
    text.contains("\\program files")
        || text.contains("\\appdata")
        || text.contains("\\programdata")
        || text.contains("\\windows\\")
        || text.contains("$recycle.bin")
}

/// Whether an entry belongs in the user's own files: under the home folder,
/// not hidden, and not in a program folder.
pub fn is_own_file(entry: &Entry, home: &Path) -> bool {
    let path = Path::new(&entry.path);
    path.starts_with(home) && !is_hidden(path) && !is_program_area(path)
}

/// The order two entries come in: the better match first, then the user's own
/// files, then the name.
fn compare(
    a: &Entry,
    b: &Entry,
    tier_a: MatchTier,
    tier_b: MatchTier,
    home: &Path,
) -> std::cmp::Ordering {
    tier_a
        .cmp(&tier_b)
        .then_with(|| is_own_file(b, home).cmp(&is_own_file(a, home)))
        .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        .then_with(|| a.path.cmp(&b.path))
}

/// The files and folders that match a query.
///
/// Filename matches (`Exact` → `Prefix` → `Contains`) always rank strictly
/// above path-segment matches (`PathContains`), which activate for queries of
/// 3+ characters so searching a project/folder name also surfaces files inside it.
pub fn search(entries: &[Entry], query: &str, home: &Path, limit: usize) -> Vec<Entry> {
    let words = query_words(query);
    if words.is_empty() {
        return Vec::new();
    }
    let allow_path_match = query.trim().chars().count() >= 3;
    let mut matches: Vec<(Entry, MatchTier)> = entries
        .iter()
        .filter_map(|entry| {
            if matches_words(&entry.name, &words) {
                Some((entry.clone(), tier_of(&entry.name, query)))
            } else if allow_path_match
                && matches_words(relative_path_for_search(&entry.path, home), &words)
            {
                Some((entry.clone(), MatchTier::PathContains))
            } else {
                None
            }
        })
        .collect();
    matches.sort_by(|a, b| compare(&a.0, &b.0, a.1, b.1, home));
    matches
        .into_iter()
        .take(limit)
        .map(|(entry, _)| entry)
        .collect()
}

/// The files that changed most recently in the home folder.
///
/// Upstream: with nothing typed the list shows the twenty files in the home
/// folder that changed most recently, and hidden files and the contents of
/// application bundles are left out.
pub fn recent(entries: &[Entry], home: &Path, limit: usize) -> Vec<Entry> {
    let mut recent: Vec<Entry> = entries
        .iter()
        .filter(|entry| !entry.is_dir && is_own_file(entry, home))
        .cloned()
        .collect();
    recent.sort_by(|a, b| {
        b.modified_ns
            .cmp(&a.modified_ns)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    recent.truncate(limit);
    recent
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, name: &str, modified_ns: i64, is_dir: bool) -> Entry {
        Entry {
            path: path.to_string(),
            name: name.to_string(),
            parent: path
                .rsplit_once('\\')
                .map(|(parent, _)| parent)
                .unwrap_or("")
                .to_string(),
            size: 10,
            modified_ns,
            is_dir,
        }
    }

    fn sample() -> (Vec<Entry>, std::path::PathBuf) {
        let home = std::path::PathBuf::from("C:\\Users\\dabin");
        let entries = vec![
            entry(
                "C:\\Users\\dabin\\Documents\\annual report.pdf",
                "annual report.pdf",
                300,
                false,
            ),
            entry(
                "C:\\Users\\dabin\\Documents\\annual-report.txt",
                "annual-report.txt",
                200,
                false,
            ),
            entry(
                "C:\\Users\\dabin\\Documents\\report.md",
                "report.md",
                100,
                false,
            ),
            entry(
                "C:\\Users\\dabin\\Documents\\notes.txt",
                "notes.txt",
                400,
                false,
            ),
            entry(
                "C:\\Users\\dabin\\.hidden\\secret.pdf",
                "secret.pdf",
                500,
                false,
            ),
            entry(
                "C:\\Program Files\\App\\readme.txt",
                "readme.txt",
                600,
                false,
            ),
        ];
        (entries, home)
    }

    #[test]
    fn every_word_must_appear_in_the_name() {
        let words = query_words("annual report");
        assert_eq!(words, vec!["annual".to_string(), "report".to_string()]);
        assert!(matches_words("annual report.pdf", &words));
        assert!(matches_words("report annual.pdf", &words), "any order");
        assert!(!matches_words("annual.pdf", &words));
    }

    #[test]
    fn an_empty_query_matches_nothing() {
        assert!(query_words("   ").is_empty());
        assert!(!matches_words("anything", &query_words("")));
    }

    #[test]
    fn the_best_matches_come_first() {
        let (entries, home) = sample();
        let results = search(&entries, "annual report", &home, RESULT_LIMIT);
        assert_eq!(results[0].name, "annual report.pdf", "the exact name first");
        assert_eq!(results[1].name, "annual-report.txt");
    }

    #[test]
    fn a_name_that_starts_with_the_query_comes_before_one_that_only_contains_it() {
        let (entries, home) = sample();
        let results = search(&entries, "annual", &home, RESULT_LIMIT);
        assert_eq!(results[0].name, "annual report.pdf");
        assert_eq!(results[1].name, "annual-report.txt");
    }

    #[test]
    fn a_name_that_is_exactly_the_query_beats_a_longer_one() {
        let (entries, home) = sample();
        let results = search(&entries, "report.md", &home, RESULT_LIMIT);
        assert_eq!(results[0].name, "report.md");
    }

    #[test]
    fn the_users_own_files_come_before_the_rest() {
        let home = std::path::PathBuf::from("C:\\Users\\dabin");
        let entries = vec![
            entry("C:\\Program Files\\App\\notes.txt", "notes.txt", 100, false),
            entry(
                "C:\\Users\\dabin\\Documents\\notes.txt",
                "notes.txt",
                100,
                false,
            ),
        ];
        let results = search(&entries, "notes", &home, RESULT_LIMIT);
        assert_eq!(results[0].path, "C:\\Users\\dabin\\Documents\\notes.txt");
    }

    #[test]
    fn a_hidden_file_ranks_below_the_users_own() {
        let home = std::path::PathBuf::from("C:\\Users\\dabin");
        let entries = vec![
            entry(
                "C:\\Users\\dabin\\.hidden\\secret.pdf",
                "secret.pdf",
                100,
                false,
            ),
            entry(
                "C:\\Users\\dabin\\Documents\\secret.pdf",
                "secret.pdf",
                100,
                false,
            ),
        ];
        let results = search(&entries, "secret", &home, RESULT_LIMIT);
        assert_eq!(results[0].path, "C:\\Users\\dabin\\Documents\\secret.pdf");
    }

    #[test]
    fn a_file_in_a_program_folder_is_never_in_the_users_own_files() {
        let home = std::path::PathBuf::from("C:\\Users\\dabin");
        let program = entry(
            "C:\\Program Files\\App\\readme.txt",
            "readme.txt",
            100,
            false,
        );
        assert!(!is_own_file(&program, &home));
        let own = entry(
            "C:\\Users\\dabin\\Documents\\readme.txt",
            "readme.txt",
            100,
            false,
        );
        assert!(is_own_file(&own, &home));
    }

    #[test]
    fn the_limit_is_honoured() {
        let (entries, home) = sample();
        let results = search(&entries, "txt", &home, 1);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn nothing_matches_the_empty_query() {
        let (entries, home) = sample();
        assert!(search(&entries, "", &home, RESULT_LIMIT).is_empty());
    }

    #[test]
    fn a_folder_is_found_as_well_as_a_file() {
        let home = std::path::PathBuf::from("C:\\Users\\dabin");
        let entries = vec![entry(
            "C:\\Users\\dabin\\Documents\\Projects",
            "Projects",
            100,
            true,
        )];
        let results = search(&entries, "projects", &home, RESULT_LIMIT);
        assert_eq!(results.len(), 1);
        assert!(results[0].is_dir);
    }

    #[test]
    fn the_recent_list_is_the_home_folders_newest_files() {
        let (entries, home) = sample();
        let results = recent(&entries, &home, RECENT_LIMIT);
        assert_eq!(results[0].name, "notes.txt", "the newest own file");
        assert_eq!(results[1].name, "annual report.pdf");
    }

    #[test]
    fn the_recent_list_leaves_out_hidden_and_program_files() {
        let (entries, home) = sample();
        let results = recent(&entries, &home, RECENT_LIMIT);
        let names: Vec<&str> = results.iter().map(|entry| entry.name.as_str()).collect();
        assert!(!names.contains(&"secret.pdf"), "hidden files are left out");
        assert!(!names.contains(&"readme.txt"), "program files are left out");
    }

    #[test]
    fn the_recent_list_honours_its_limit() {
        let (entries, home) = sample();
        assert_eq!(recent(&entries, &home, 1).len(), 1);
    }

    #[test]
    fn the_matching_ignores_the_accents() {
        let home = std::path::PathBuf::from("C:\\Users\\dabin");
        let entries = vec![entry(
            "C:\\Users\\dabin\\Documents\\café.txt",
            "café.txt",
            100,
            false,
        )];
        assert_eq!(search(&entries, "cafe", &home, RESULT_LIMIT).len(), 1);
        assert_eq!(search(&entries, "café", &home, RESULT_LIMIT).len(), 1);
    }

    #[test]
    fn path_matches_rank_below_filename_matches_and_ignore_home_prefix() {
        let home = std::path::PathBuf::from("C:\\Users\\dabin");
        let entries = vec![
            entry(
                "C:\\Users\\dabin\\development\\opencode-stack\\AIStudio2API\\runtime\\2x2.png",
                "2x2.png",
                300,
                false,
            ),
            entry(
                "C:\\Users\\dabin\\development\\opencode-stack\\AIStudio2API\\aistudio2api.exe",
                "aistudio2api.exe",
                200,
                false,
            ),
            entry(
                "C:\\Users\\dabin\\development\\opencode-stack\\AIStudio2API",
                "AIStudio2API",
                100,
                true,
            ),
        ];

        let results = search(&entries, "aistudio2api", &home, RESULT_LIMIT);
        let names: Vec<&str> = results.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["AIStudio2API", "aistudio2api.exe", "2x2.png"]);

        // Searching the username or "users" must NOT spuriously match files under C:\Users\dabin.
        assert!(search(&entries, "dabin", &home, RESULT_LIMIT).is_empty());
        assert!(search(&entries, "users", &home, RESULT_LIMIT).is_empty());
    }
}
