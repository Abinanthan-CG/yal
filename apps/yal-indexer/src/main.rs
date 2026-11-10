//! The file indexer helper.
//!
//! The launcher keeps its own index of names, folders and dates, built by a
//! helper process so a large scan never blocks the launcher and a crash in the
//! scanner cannot take the app down.
//!
//! The helper is a plain command-line tool: it scans a folder, applies the
//! ignore patterns and prints a JSON summary. The launcher starts it and reads
//! the summary; nothing else runs it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The folders that are never indexed.
///
/// These are the Windows equivalents of the folders upstream skips: package
/// caches, build output and the system's own bookkeeping.
const IGNORED_FOLDER_NAMES: &[&str] = &[
    "node_modules",
    ".git",
    ".svn",
    ".hg",
    "target",
    "build",
    "dist",
    "out",
    "obj",
    "bin",
    "AppData",
    "$Recycle.Bin",
    "System Volume Information",
    ".cache",
    ".cargo",
    ".rustup",
];

/// The file extensions that are never indexed.
const IGNORED_EXTENSIONS: &[&str] = &[
    "tmp", "temp", "log", "lock", "dmp", "etl", "lnk", "bak", "old", "swp",
];

/// The result of one scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanSummary {
    pub root: PathBuf,
    pub folders: usize,
    pub files: usize,
    pub skipped: usize,
    pub bytes: u64,
    pub by_extension: BTreeMap<String, usize>,
}

/// Scan a folder and count what a name-and-date index would hold.
pub fn scan(root: &Path) -> std::io::Result<ScanSummary> {
    let mut summary = ScanSummary {
        root: root.to_path_buf(),
        folders: 0,
        files: 0,
        skipped: 0,
        bytes: 0,
        by_extension: BTreeMap::new(),
    };

    let mut stack = vec![root.to_path_buf()];
    while let Some(folder) = stack.pop() {
        let entries = match std::fs::read_dir(&folder) {
            Ok(entries) => entries,
            Err(_) => {
                summary.skipped += 1;
                continue;
            }
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(_) => {
                    summary.skipped += 1;
                    continue;
                }
            };
            if file_type.is_dir() {
                if is_ignored_folder(&name) {
                    summary.skipped += 1;
                    continue;
                }
                summary.folders += 1;
                stack.push(path);
                continue;
            }
            if !file_type.is_file() {
                summary.skipped += 1;
                continue;
            }
            if is_ignored_file(&name) {
                summary.skipped += 1;
                continue;
            }
            summary.files += 1;
            if let Ok(metadata) = entry.metadata() {
                summary.bytes += metadata.len();
            }
            let extension = path
                .extension()
                .map(|e| e.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            *summary
                .by_extension
                .entry(extension)
                .or_insert(0) += 1;
        }
    }
    Ok(summary)
}

/// Whether a folder is skipped by name.
pub fn is_ignored_folder(name: &str) -> bool {
    IGNORED_FOLDER_NAMES
        .iter()
        .any(|ignored| ignored.eq_ignore_ascii_case(name))
}

/// Whether a file is skipped by its extension.
pub fn is_ignored_file(name: &str) -> bool {
    match name.rsplit_once('.') {
        Some((_, extension)) => IGNORED_EXTENSIONS
            .iter()
            .any(|ignored| ignored.eq_ignore_ascii_case(extension)),
        None => false,
    }
}

/// The summary as JSON.
pub fn summary_json(summary: &ScanSummary) -> String {
    let extensions: Vec<String> = summary
        .by_extension
        .iter()
        .map(|(extension, count)| format!("\"{extension}\":{count}"))
        .collect();
    format!(
        "{{\"root\":\"{}\",\"folders\":{},\"files\":{},\"skipped\":{},\"bytes\":{},\"by_extension\":{{{}}}}}",
        summary
            .root
            .display()
            .to_string()
            .replace('\\', "\\\\"),
        summary.folders,
        summary.files,
        summary.skipped,
        summary.bytes,
        extensions.join(",")
    )
}

fn main() -> std::process::ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let folder = arguments
        .iter()
        .position(|a| a == "--scan")
        .and_then(|index| arguments.get(index + 1))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::env::args_os()
                .nth(1)
                .map(std::path::PathBuf::from)
                .unwrap_or_else(std::env::temp_dir)
        });

    match scan(&folder) {
        Ok(summary) => {
            println!("{}", summary_json(&summary));
            std::process::ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("yal-indexer: {}: {error}", folder.display());
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ignored_folders_are_the_ones_upstream_skips() {
        assert!(is_ignored_folder("node_modules"));
        assert!(is_ignored_folder(".git"));
        assert!(is_ignored_folder("AppData"));
        assert!(is_ignored_folder("$Recycle.Bin"));
        assert!(!is_ignored_folder("Documents"));
    }

    #[test]
    fn the_ignored_files_are_the_temporary_ones() {
        assert!(is_ignored_file("build.log"));
        assert!(is_ignored_file("notes.TMP"));
        assert!(!is_ignored_file("notes.md"));
        assert!(!is_ignored_file("README"));
    }

    #[test]
    fn a_scan_of_an_empty_folder_finds_nothing() {
        let folder = std::env::temp_dir().join("yal-indexer-test-empty");
        std::fs::create_dir_all(&folder).unwrap();
        let summary = scan(&folder).unwrap();
        assert_eq!(summary.files, 0);
        assert_eq!(summary.folders, 0);
    }
}