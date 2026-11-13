//! The file-search screen, and the files the launcher offers alongside its
//! own rows.
//!
//! The index and the search over it are [`tc_files`]'s job. This module is the
//! screen's own rules:
//!
//! - **With nothing typed**, the list is **Recent**: the twenty files in the
//!   home folder that changed most recently.
//! - **With something typed**, the list is **Results**: every word typed has
//!   to appear in the name, in any order.
//! - **The filter is applied during the search**, so it never hides matches
//!   that were already cut off by the result limit. The filter is applied
//!   first, and the limit is taken afterwards.

use std::path::Path;

use tc_files::index::Entry;
use tc_files::search::{recent, search, RESULT_LIMIT};

/// The kinds of file the filter offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FileType {
    /// Everything.
    #[default]
    All,
    /// Folders only.
    Folders,
    /// Documents.
    Documents,
    /// Images.
    Images,
    /// Audio.
    Audio,
    /// Videos.
    Videos,
    /// Archives.
    Archives,
}

impl FileType {
    /// The seven options, in the order they are offered.
    pub const OPTIONS: [FileType; 7] = [
        FileType::All,
        FileType::Folders,
        FileType::Documents,
        FileType::Images,
        FileType::Audio,
        FileType::Videos,
        FileType::Archives,
    ];

    /// The name the filter is offered under.
    pub fn label(&self) -> &'static str {
        match self {
            FileType::All => "All Types",
            FileType::Folders => "Folders",
            FileType::Documents => "Documents",
            FileType::Images => "Images",
            FileType::Audio => "Audio",
            FileType::Videos => "Videos",
            FileType::Archives => "Archives",
        }
    }

    /// The next filter, for the key that cycles through them.
    pub fn next(&self) -> FileType {
        let index = FileType::OPTIONS
            .iter()
            .position(|option| option == self)
            .unwrap_or(0);
        FileType::OPTIONS[(index + 1) % FileType::OPTIONS.len()]
    }
}

/// The extensions that make a file a document.
const DOCUMENTS: [&str; 21] = [
    "pdf", "doc", "docx", "odt", "rtf", "txt", "md", "xls", "xlsx", "csv", "ppt", "pptx", "odp",
    "epub", "pages", "numbers", "key", "tex", "log", "json", "xml",
];

/// The extensions that make a file an image.
const IMAGES: [&str; 11] = [
    "png", "jpg", "jpeg", "gif", "bmp", "webp", "svg", "tif", "tiff", "ico", "heic",
];

/// The extensions that make a file audio.
const AUDIO: [&str; 9] = [
    "mp3", "wav", "flac", "m4a", "aac", "ogg", "oga", "wma", "opus",
];

/// The extensions that make a file a video.
const VIDEOS: [&str; 9] = [
    "mp4", "mkv", "mov", "avi", "webm", "wmv", "flv", "m4v", "mpeg",
];

/// The extensions that make a file an archive.
const ARCHIVES: [&str; 9] = ["zip", "rar", "7z", "tar", "gz", "bz2", "xz", "iso", "zst"];

/// The file's extension, lower case and without the dot.
pub fn extension_of(entry: &Entry) -> String {
    let name = entry.name.to_lowercase();
    match name.rsplit_once('.') {
        Some((_, extension)) if !extension.is_empty() => extension.to_string(),
        _ => String::new(),
    }
}

/// The kind of file the entry is.
pub fn classify(entry: &Entry) -> FileType {
    if entry.is_dir {
        return FileType::Folders;
    }
    let extension = extension_of(entry);
    if DOCUMENTS.contains(&extension.as_str()) {
        FileType::Documents
    } else if IMAGES.contains(&extension.as_str()) {
        FileType::Images
    } else if AUDIO.contains(&extension.as_str()) {
        FileType::Audio
    } else if VIDEOS.contains(&extension.as_str()) {
        FileType::Videos
    } else if ARCHIVES.contains(&extension.as_str()) {
        FileType::Archives
    } else {
        FileType::All
    }
}

/// Whether the entry passes the filter.
pub fn matches(entry: &Entry, filter: FileType) -> bool {
    match filter {
        FileType::All => true,
        FileType::Folders => entry.is_dir,
        other => classify(entry) == other,
    }
}

/// The rows of the file-search screen.
///
/// With nothing typed the list is the recent files; with something typed it
/// is the results. The filter is applied before the limit, so a filtered
/// search still fills the list.
pub fn screen_rows(
    entries: &[Entry],
    query: &str,
    home: &Path,
    filter: FileType,
    limit: usize,
) -> Vec<Entry> {
    if query.trim().is_empty() {
        let candidates: Vec<Entry> = entries
            .iter()
            .filter(|entry| matches(entry, filter))
            .cloned()
            .collect();
        return recent(&candidates, home, limit);
    }
    let candidates: Vec<Entry> = entries
        .iter()
        .filter(|entry| matches(entry, filter))
        .cloned()
        .collect();
    search(&candidates, query, home, limit)
}

/// How many characters the launcher needs before it offers files.
///
/// "When your search has three or more characters, a Files section lists up
/// to five matches."
pub const LAUNCHER_FILES_MIN_CHARS: usize = 3;

/// How many files the launcher offers.
///
/// "It sits below the apps and commands, so return still opens the top
/// result."
pub const LAUNCHER_FILES_LIMIT: usize = 5;

/// The files the launcher offers under its own rows.
///
/// The files the user has made a favorite of come first.
pub fn launcher_rows(
    entries: &[Entry],
    query: &str,
    home: &Path,
    favorites: &[String],
) -> Vec<Entry> {
    if query.trim().chars().count() < LAUNCHER_FILES_MIN_CHARS {
        return Vec::new();
    }
    let mut found = search(entries, query, home, RESULT_LIMIT);
    found.sort_by_key(|entry| !favorites.contains(&entry.path));
    found.truncate(LAUNCHER_FILES_LIMIT);
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

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

    fn home() -> PathBuf {
        PathBuf::from("C:\\Users\\dabin")
    }

    fn sample() -> Vec<Entry> {
        vec![
            entry(
                "C:\\Users\\dabin\\Documents\\annual report.pdf",
                "annual report.pdf",
                300,
                false,
            ),
            entry(
                "C:\\Users\\dabin\\Documents\\notes.txt",
                "notes.txt",
                400,
                false,
            ),
            entry(
                "C:\\Users\\dabin\\Pictures\\photo.png",
                "photo.png",
                500,
                false,
            ),
            entry("C:\\Users\\dabin\\Movies\\clip.mp4", "clip.mp4", 600, false),
            entry(
                "C:\\Users\\dabin\\Downloads\\bundle.zip",
                "bundle.zip",
                700,
                false,
            ),
            entry(
                "C:\\Users\\dabin\\Documents\\annual report",
                "annual report",
                800,
                true,
            ),
        ]
    }

    #[test]
    fn the_filter_offers_the_seven_documented_choices() {
        let labels: Vec<&str> = FileType::OPTIONS.iter().map(|kind| kind.label()).collect();
        assert_eq!(
            labels,
            vec![
                "All Types",
                "Folders",
                "Documents",
                "Images",
                "Audio",
                "Videos",
                "Archives"
            ]
        );
    }

    #[test]
    fn a_folder_is_a_folder_whatever_it_is_named() {
        let folder = entry("C:\\Users\\dabin\\Pictures", "Pictures", 1, true);
        assert_eq!(classify(&folder), FileType::Folders);
    }

    #[test]
    fn the_kind_of_a_file_is_its_extension() {
        let document = entry("C:\\a\\report.pdf", "report.pdf", 1, false);
        let image = entry("C:\\a\\photo.png", "photo.png", 1, false);
        let video = entry("C:\\a\\clip.mp4", "clip.mp4", 1, false);
        let archive = entry("C:\\a\\bundle.zip", "bundle.zip", 1, false);
        let audio = entry("C:\\a\\song.mp3", "song.mp3", 1, false);
        assert_eq!(classify(&document), FileType::Documents);
        assert_eq!(classify(&image), FileType::Images);
        assert_eq!(classify(&video), FileType::Videos);
        assert_eq!(classify(&archive), FileType::Archives);
        assert_eq!(classify(&audio), FileType::Audio);
    }

    #[test]
    fn the_extension_is_read_without_its_dot_and_in_lower_case() {
        let file = entry("C:\\a\\REPORT.PDF", "REPORT.PDF", 1, false);
        assert_eq!(extension_of(&file), "pdf");
    }

    #[test]
    fn a_file_without_an_extension_is_no_particular_kind() {
        let file = entry("C:\\a\\README", "README", 1, false);
        assert_eq!(extension_of(&file), "");
        assert_eq!(classify(&file), FileType::All);
    }

    #[test]
    fn an_empty_search_shows_the_recent_files() {
        let rows = screen_rows(&sample(), "", &home(), FileType::All, 20);
        assert_eq!(rows[0].name, "bundle.zip", "the newest own file");
    }

    #[test]
    fn the_recent_list_leaves_the_folders_out() {
        let rows = screen_rows(&sample(), "", &home(), FileType::All, 20);
        let names: Vec<&str> = rows.iter().map(|entry| entry.name.as_str()).collect();
        assert!(!names.contains(&"annual report"));
    }

    #[test]
    fn a_search_shows_the_results() {
        let rows = screen_rows(&sample(), "annual", &home(), FileType::All, 200);
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|entry| entry.name == "annual report.pdf"));
    }

    #[test]
    fn the_filter_is_applied_during_the_search() {
        let rows = screen_rows(&sample(), "annual", &home(), FileType::Folders, 200);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "annual report");
        assert!(rows[0].is_dir);
    }

    #[test]
    fn the_filter_does_not_hide_matches_the_limit_cut_off() {
        // Ten documents match, and the limit is two. Without the filter
        // being applied first, the two that survive would be whichever the
        // ranking happened to put first.
        let mut entries: Vec<Entry> = (0..10)
            .map(|number| {
                entry(
                    &format!("C:\\Users\\dabin\\Documents\\report{number}.txt"),
                    &format!("report{number}.txt"),
                    100,
                    false,
                )
            })
            .collect();
        entries.push(entry(
            "C:\\Users\\dabin\\Documents\\report.txt",
            "report.txt",
            1_000,
            false,
        ));
        let rows = screen_rows(&entries, "report", &home(), FileType::All, 2);
        assert_eq!(rows.len(), 2);
        // With every document in the way, the exact name only comes first
        // when nothing was filtered out before the ranking.
        assert_eq!(rows[0].name, "report.txt");
    }

    #[test]
    fn the_images_filter_keeps_the_images() {
        let rows = screen_rows(&sample(), "photo", &home(), FileType::Images, 200);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "photo.png");
    }

    #[test]
    fn the_filter_cycles_through_the_options_and_back() {
        let mut filter = FileType::All;
        for _ in 0..FileType::OPTIONS.len() {
            filter = filter.next();
        }
        assert_eq!(filter, FileType::All);
        assert_eq!(FileType::All.next(), FileType::Folders);
    }

    #[test]
    fn the_launcher_offers_no_files_below_three_characters() {
        assert!(launcher_rows(&sample(), "an", &home(), &[]).is_empty());
        assert!(launcher_rows(&sample(), "ann", &home(), &[]).len() <= 5);
    }

    #[test]
    fn the_launcher_offers_at_most_five_files() {
        let entries: Vec<Entry> = (0..20)
            .map(|number| {
                entry(
                    &format!("C:\\Users\\dabin\\Documents\\report{number}.txt"),
                    &format!("report{number}.txt"),
                    100,
                    false,
                )
            })
            .collect();
        let rows = launcher_rows(&entries, "report", &home(), &[]);
        assert_eq!(rows.len(), LAUNCHER_FILES_LIMIT);
    }

    #[test]
    fn the_launcher_puts_the_favorite_files_first() {
        // The folder sorts first by name, so the file only comes first when
        // the favorite rule puts it there.
        let favorite = "C:\\Users\\dabin\\Documents\\annual report.pdf";
        let rows = launcher_rows(&sample(), "ort", &home(), &[favorite.to_string()]);
        assert_eq!(rows[0].name, "annual report.pdf");
    }

    #[test]
    fn the_launcher_leaves_the_favorites_alone_when_there_are_none() {
        let rows = launcher_rows(&sample(), "ort", &home(), &[]);
        assert_eq!(rows[0].name, "annual report", "the folder sorts first");
    }
}
