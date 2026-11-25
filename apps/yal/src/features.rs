//! Live stateful feature stores shared between the Win32 shell and the native
//! Direct2D palette: Clipboard History, Emoji Picker, Snippets, Calculator
//! History, and the background incremental File Indexer.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use chrono::Local;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde::{Deserialize, Serialize};
use tc_clipboard::{ClipboardFilter, ClipboardItem, ClipboardStore, PasteSequence};
use tc_commands::runner::ShellRunner;
use tc_commands::shell::Shell;
use tc_emoji::{EmojiIndex, FrequentEmojiStore, PinnedEmojiStore};
use tc_files::scope::IgnoreRules;
use tc_files::store::IndexStore;
use tc_platform::{Clipboard, Paths};
use tc_snippets::raycast::RaycastSnippetImport;
use tc_snippets::template::{ExpansionContext, SnippetTemplateEngine};
use tc_snippets::{Snippet, SnippetRepository, SnippetsStore, StoredSnippet};
use tc_ui::PaletteHandle;

use crate::launcher::{FileSearch, LauncherState};
use crate::shell::INDEX_FILE;

const EMOJI_PINS_FILE: &str = "emoji-pins.json";
const EMOJI_FREQ_FILE: &str = "emoji-frequency.json";
const CALC_HISTORY_FILE: &str = "calc-history.json";
const FAVORITES_FILE: &str = "favorites.json";
const QUICKLINKS_FILE: &str = "quicklinks.json";
const SNIPPETS_DIR: &str = "snippets";
const NOTES_DIR: &str = "notes";
const MAX_CALC_HISTORY: usize = 100;

fn lock_or_recover<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Filter tabs shown on the Clipboard History screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClipboardViewFilter {
    #[default]
    All,
    Text,
    Images,
    Files,
    Links,
    Colors,
    Pinned,
}

impl ClipboardViewFilter {
    pub const ALL: [ClipboardViewFilter; 7] = [
        ClipboardViewFilter::All,
        ClipboardViewFilter::Text,
        ClipboardViewFilter::Images,
        ClipboardViewFilter::Files,
        ClipboardViewFilter::Links,
        ClipboardViewFilter::Colors,
        ClipboardViewFilter::Pinned,
    ];

    pub fn next(self) -> Self {
        let index = Self::ALL.iter().position(|f| *f == self).unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }

    pub fn label(self) -> &'static str {
        match self {
            ClipboardViewFilter::All => "All",
            ClipboardViewFilter::Text => "Text",
            ClipboardViewFilter::Images => "Images",
            ClipboardViewFilter::Files => "Files",
            ClipboardViewFilter::Links => "Links",
            ClipboardViewFilter::Colors => "Colors",
            ClipboardViewFilter::Pinned => "Pinned",
        }
    }

    pub fn store_filter(self) -> ClipboardFilter {
        match self {
            ClipboardViewFilter::All | ClipboardViewFilter::Pinned => ClipboardFilter::All,
            ClipboardViewFilter::Text => ClipboardFilter::Text,
            ClipboardViewFilter::Images => ClipboardFilter::Image,
            ClipboardViewFilter::Files => ClipboardFilter::File,
            ClipboardViewFilter::Links => ClipboardFilter::Link,
            ClipboardViewFilter::Colors => ClipboardFilter::Color,
        }
    }
}

/// Result of advancing the sequential paste queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PasteNext {
    /// Ready to paste `item`, with `remaining` items left in the queue or walk.
    Item {
        item: ClipboardItem,
        remaining: usize,
    },
    /// The previous paste has not settled yet (`< 250ms`); ignore this rapid press.
    Settling,
    /// No queued or history items remain to paste.
    Empty,
}

/// Sequential paste queue supporting both an explicit FIFO queue and an
/// automatic `PasteSequence` walk over clipboard history.
#[derive(Debug, Default)]
pub struct PasteQueue {
    queued: Vec<ClipboardItem>,
    sequence: Option<PasteSequence>,
    sequence_consumed: usize,
    change_count: u64,
    last_paste_ms: Option<i64>,
}

impl PasteQueue {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record an external clipboard copy so any active history walk restarts
    /// from the newly copied item on the next Paste Sequentially invocation.
    pub fn note_external_change(&mut self) {
        self.change_count = self.change_count.wrapping_add(1);
    }

    /// Enqueue an item for explicit FIFO sequential pasting.
    pub fn add(&mut self, item: ClipboardItem) -> usize {
        self.queued.push(item);
        self.sequence = None;
        self.sequence_consumed = 0;
        self.queued.len()
    }

    /// Number of explicitly queued items waiting to be pasted.
    pub fn len(&self) -> usize {
        self.queued.len()
    }

    /// Clear the explicit queue and reset any active history walk.
    pub fn clear(&mut self) -> usize {
        let count = self.queued.len();
        self.queued.clear();
        self.sequence = None;
        self.sequence_consumed = 0;
        count
    }

    /// Pop the next item to paste, respecting the 250ms paste-settle window.
    ///
    /// When explicit items have been queued, pops in FIFO order. Otherwise
    /// walks `history` newest-first using `PasteSequence`, respecting idle
    /// timeout and external clipboard changes.
    pub fn pop_next(&mut self, history: &[ClipboardItem], now_ms: i64) -> PasteNext {
        if !self.queued.is_empty() {
            if self
                .last_paste_ms
                .is_some_and(|t| now_ms - t < tc_clipboard::paste_sequence::SETTLE_INTERVAL_MILLIS)
            {
                return PasteNext::Settling;
            }
            let item = self.queued.remove(0);
            self.change_count = self.change_count.wrapping_add(1);
            self.last_paste_ms = Some(now_ms);
            self.sequence = None;
            self.sequence_consumed = 0;
            return PasteNext::Item {
                item,
                remaining: self.queued.len(),
            };
        }

        if history.is_empty() {
            self.sequence = None;
            self.sequence_consumed = 0;
            return PasteNext::Empty;
        }

        if let Some(seq) = self.sequence.as_ref() {
            if seq.continues(self.change_count, now_ms) {
                if seq.is_settling(now_ms) {
                    return PasteNext::Settling;
                }
            } else {
                self.sequence = Some(PasteSequence::new(history, self.change_count, now_ms));
                self.sequence_consumed = 0;
            }
        } else {
            self.sequence = Some(PasteSequence::new(history, self.change_count, now_ms));
            self.sequence_consumed = 0;
        }

        let Some(seq) = self.sequence.as_mut() else {
            return PasteNext::Empty;
        };
        match seq.next(history) {
            Some(item) => {
                self.change_count = self.change_count.wrapping_add(1);
                seq.record_paste(self.change_count, now_ms);
                self.last_paste_ms = Some(now_ms);
                self.sequence_consumed += 1;
                let remaining = history.len().saturating_sub(self.sequence_consumed);
                PasteNext::Item { item, remaining }
            }
            None => {
                self.sequence = None;
                self.sequence_consumed = 0;
                PasteNext::Empty
            }
        }
    }
}

/// One remembered calculation entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CalcHistoryEntry {
    pub expression: String,
    pub display: String,
    pub copy_text: String,
    pub badge: String,
    pub created_at: i64,
}

/// Persistent JSON store for recent Calculator answers.
#[derive(Debug, Default)]
pub struct CalcHistoryStore {
    path: PathBuf,
    entries: Vec<CalcHistoryEntry>,
}

impl CalcHistoryStore {
    pub fn open(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        let entries = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Vec<CalcHistoryEntry>>(&bytes).ok())
            .unwrap_or_default();
        Self { path, entries }
    }

    pub fn entries(&self) -> &[CalcHistoryEntry] {
        &self.entries
    }

    pub fn record(&mut self, expression: &str, display: &str, copy_text: &str, badge: &str) {
        let expr = expression.trim();
        let disp = display.trim();
        if expr.is_empty() || disp.is_empty() {
            return;
        }
        // Deduplicate if the exact same expression/answer is already in history.
        self.entries
            .retain(|e| !(e.expression == expr && e.copy_text == copy_text));
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        self.entries.insert(
            0,
            CalcHistoryEntry {
                expression: expr.to_string(),
                display: disp.to_string(),
                copy_text: copy_text.to_string(),
                badge: badge.to_string(),
                created_at,
            },
        );
        self.entries.truncate(MAX_CALC_HISTORY);
        self.save();
    }

    pub fn remove_at(&mut self, index: usize) {
        if index < self.entries.len() {
            self.entries.remove(index);
            self.save();
        }
    }

    pub fn search(&self, query: &str) -> Vec<CalcHistoryEntry> {
        let q = query.trim().to_ascii_lowercase();
        if q.is_empty() {
            return self.entries.clone();
        }
        self.entries
            .iter()
            .filter(|e| {
                e.expression.to_ascii_lowercase().contains(&q)
                    || e.display.to_ascii_lowercase().contains(&q)
                    || e.badge.to_ascii_lowercase().contains(&q)
            })
            .cloned()
            .collect()
    }

    fn save(&self) {
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(&self.entries) {
            let _ = std::fs::write(&self.path, json);
        }
    }
}

/// One quick note stored as a Markdown file in `%APPDATA%\yal\notes\`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteItem {
    pub path: PathBuf,
    pub title: String,
    pub body: String,
    pub modified_ms: i64,
}

/// Persistent Markdown folder store for Quick Notes.
#[derive(Debug, Default)]
pub struct NotesStore {
    dir: PathBuf,
    notes: Vec<NoteItem>,
}

impl NotesStore {
    pub fn open(dir: impl AsRef<Path>) -> Self {
        let dir = dir.as_ref().to_path_buf();
        let _ = std::fs::create_dir_all(&dir);
        let mut store = Self {
            dir,
            notes: Vec::new(),
        };
        store.reload();
        store
    }

    pub fn reload(&mut self) {
        self.notes.clear();
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let is_md = path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("md"));
            if !is_md {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(&path) else {
                continue;
            };
            let modified_ms = entry
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);
            let (title, body) = split_note_title_and_body(&path, &content);
            self.notes.push(NoteItem {
                path,
                title,
                body,
                modified_ms,
            });
        }
        self.notes.sort_by_key(|a| std::cmp::Reverse(a.modified_ms));
    }

    pub fn search(&self, query: &str) -> Vec<NoteItem> {
        let q = query.trim().to_ascii_lowercase();
        if q.is_empty() {
            return self.notes.clone();
        }
        self.notes
            .iter()
            .filter(|n| {
                n.title.to_ascii_lowercase().contains(&q)
                    || n.body.to_ascii_lowercase().contains(&q)
            })
            .cloned()
            .collect()
    }

    pub fn create(&mut self, title: &str, body: &str) -> Result<NoteItem, String> {
        let _ = std::fs::create_dir_all(&self.dir);
        let clean_title = title.trim();
        if clean_title.is_empty() {
            return Err("note title cannot be empty".to_string());
        }
        let slug: String = clean_title
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect::<String>()
            .trim_matches('-')
            .chars()
            .take(40)
            .collect();
        let slug = if slug.is_empty() {
            "note".to_string()
        } else {
            slug
        };
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let path = self.dir.join(format!("{slug}-{stamp}.md"));
        let markdown = format!("# {clean_title}\n\n{}\n", body.trim());
        std::fs::write(&path, markdown).map_err(|e| e.to_string())?;
        self.reload();
        self.notes
            .iter()
            .find(|n| n.path == path)
            .cloned()
            .ok_or_else(|| "created note could not be reloaded".to_string())
    }

    pub fn delete(&mut self, path: &Path) {
        let _ = std::fs::remove_file(path);
        self.reload();
    }
}

fn split_note_title_and_body(path: &Path, content: &str) -> (String, String) {
    let mut lines = content.lines();
    if let Some(first) = lines.next() {
        let trimmed = first.trim();
        if let Some(heading) = trimmed.strip_prefix("# ") {
            let body = lines.collect::<Vec<_>>().join("\n").trim().to_string();
            return (heading.trim().to_string(), body);
        }
    }
    let fallback_title = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Untitled Note")
        .to_string();
    (fallback_title, content.trim().to_string())
}

/// One parameterized or direct web Quicklink (`{query}` is percent-encoded).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Quicklink {
    pub name: String,
    pub url_template: String,
    #[serde(default)]
    pub alias: Option<String>,
}

impl Quicklink {
    pub fn needs_query(&self) -> bool {
        self.url_template.contains("{query}") || self.url_template.contains("{argument}")
    }

    pub fn expand_url(&self, query: &str) -> String {
        if self.needs_query() {
            let encoded = utf8_percent_encode(query.trim(), NON_ALPHANUMERIC).to_string();
            self.url_template
                .replace("{query}", &encoded)
                .replace("{argument}", &encoded)
        } else {
            self.url_template.clone()
        }
    }
}

/// Persistent JSON store for Quicklinks (seeded with high-leverage defaults).
#[derive(Debug, Default)]
pub struct QuicklinksStore {
    path: PathBuf,
    links: Vec<Quicklink>,
}

impl QuicklinksStore {
    pub fn open(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        let links = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Vec<Quicklink>>(&bytes).ok())
            .unwrap_or_else(Self::defaults);
        Self { path, links }
    }

    pub fn reload(&mut self) {
        if let Ok(bytes) = std::fs::read(&self.path) {
            if let Ok(loaded) = serde_json::from_slice::<Vec<Quicklink>>(&bytes) {
                self.links = loaded;
            }
        }
    }

    pub fn defaults() -> Vec<Quicklink> {
        vec![
            Quicklink {
                name: "Google Search".to_string(),
                url_template: "https://www.google.com/search?q={query}".to_string(),
                alias: Some("g".to_string()),
            },
            Quicklink {
                name: "GitHub Search".to_string(),
                url_template: "https://github.com/search?q={query}".to_string(),
                alias: Some("gh".to_string()),
            },
            Quicklink {
                name: "YouTube Search".to_string(),
                url_template: "https://www.youtube.com/results?search_query={query}".to_string(),
                alias: Some("yt".to_string()),
            },
            Quicklink {
                name: "Crates.io Search".to_string(),
                url_template: "https://crates.io/search?q={query}".to_string(),
                alias: Some("crates".to_string()),
            },
            Quicklink {
                name: "MDN Web Docs".to_string(),
                url_template: "https://developer.mozilla.org/en-US/search?q={query}".to_string(),
                alias: Some("mdn".to_string()),
            },
            Quicklink {
                name: "Wikipedia".to_string(),
                url_template: "https://en.wikipedia.org/wiki/Special:Search?search={query}"
                    .to_string(),
                alias: Some("wiki".to_string()),
            },
        ]
    }

    pub fn search(&self, query: &str) -> Vec<Quicklink> {
        let q = query.trim().to_ascii_lowercase();
        if q.is_empty() {
            return self.links.clone();
        }
        self.links
            .iter()
            .filter(|l| {
                l.name.to_ascii_lowercase().contains(&q)
                    || l.url_template.to_ascii_lowercase().contains(&q)
                    || l.alias
                        .as_deref()
                        .is_some_and(|a| a.to_ascii_lowercase().contains(&q))
            })
            .cloned()
            .collect()
    }

    pub fn add(&mut self, name: String, url_template: String) {
        let clean_name = name.trim().to_string();
        let clean_url = normalize_quicklink_url(&url_template);
        if clean_name.is_empty() || clean_url.is_empty() {
            return;
        }
        self.links.insert(
            0,
            Quicklink {
                name: clean_name,
                url_template: clean_url,
                alias: None,
            },
        );
        self.save();
    }

    pub fn remove_by_name(&mut self, name: &str) {
        self.links.retain(|l| l.name != name);
        self.save();
    }

    fn save(&self) {
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(&self.links) {
            let _ = std::fs::write(&self.path, json);
        }
    }
}

fn normalize_quicklink_url(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    }
}

/// Persistent ordered list of pinned Favorite row IDs in the Launcher.
#[derive(Debug, Default)]
pub struct FavoritesStore {
    path: PathBuf,
    ids: Vec<String>,
}

impl FavoritesStore {
    pub fn open(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        let ids = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Vec<String>>(&bytes).ok())
            .unwrap_or_default();
        Self { path, ids }
    }

    pub fn reload(&mut self) {
        if let Ok(bytes) = std::fs::read(&self.path) {
            if let Ok(loaded) = serde_json::from_slice::<Vec<String>>(&bytes) {
                self.ids = loaded;
            }
        }
    }

    pub fn ids(&self) -> &[String] {
        &self.ids
    }

    pub fn contains(&self, id: &str) -> bool {
        self.ids.iter().any(|existing| existing == id)
    }

    pub fn toggle(&mut self, id: &str) -> bool {
        if let Some(pos) = self.ids.iter().position(|existing| existing == id) {
            self.ids.remove(pos);
            self.save();
            false
        } else {
            self.ids.insert(0, id.to_string());
            self.save();
            true
        }
    }

    fn save(&self) {
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(&self.ids) {
            let _ = std::fs::write(&self.path, json);
        }
    }
}

/// Every live feature store owned by the running YAL process.
pub struct FeatureStores {
    pub data_dir: PathBuf,
    pub home_dir: PathBuf,
    pub settings_file: PathBuf,
    pub clipboard: Mutex<Option<ClipboardStore>>,
    pub paste_queue: Mutex<PasteQueue>,
    pub emoji_index: Arc<EmojiIndex>,
    pub emoji_pins: Mutex<PinnedEmojiStore>,
    pub emoji_freq: Mutex<FrequentEmojiStore>,
    pub snippets: Mutex<SnippetsStore>,
    pub notes: Mutex<NotesStore>,
    pub quicklinks: Mutex<QuicklinksStore>,
    pub favorites: Mutex<FavoritesStore>,
    pub calc_history: Mutex<CalcHistoryStore>,
    pub file_indexing: Arc<AtomicBool>,
}

impl FeatureStores {
    /// Open or initialize every local store under `paths.data_dir`.
    pub fn open(paths: &Paths) -> Self {
        let _ = std::fs::create_dir_all(&paths.data_dir);

        let clipboard = match ClipboardStore::open(&paths.data_dir) {
            Ok(mut store) => {
                let _ = store.load();
                Some(store)
            }
            Err(error) => {
                eprintln!("yal: clipboard store unavailable: {error}");
                None
            }
        };

        let emoji_index = Arc::new(EmojiIndex::load());
        let emoji_pins = PinnedEmojiStore::open(paths.data_dir.join(EMOJI_PINS_FILE));
        let emoji_freq = FrequentEmojiStore::open(paths.data_dir.join(EMOJI_FREQ_FILE));

        let snippets_dir = paths.data_dir.join(SNIPPETS_DIR);
        let _ = std::fs::create_dir_all(&snippets_dir);
        let mut snippets = SnippetsStore::new(SnippetRepository::new(snippets_dir));
        let _ = snippets.start();

        let notes = NotesStore::open(paths.data_dir.join(NOTES_DIR));
        let quicklinks = QuicklinksStore::open(paths.data_dir.join(QUICKLINKS_FILE));
        let favorites = FavoritesStore::open(paths.data_dir.join(FAVORITES_FILE));
        let calc_history = CalcHistoryStore::open(paths.data_dir.join(CALC_HISTORY_FILE));

        Self {
            data_dir: paths.data_dir.clone(),
            home_dir: paths.home_dir.clone(),
            settings_file: paths.settings_file.clone(),
            clipboard: Mutex::new(clipboard),
            paste_queue: Mutex::new(PasteQueue::new()),
            emoji_index,
            emoji_pins: Mutex::new(emoji_pins),
            emoji_freq: Mutex::new(emoji_freq),
            snippets: Mutex::new(snippets),
            notes: Mutex::new(notes),
            quicklinks: Mutex::new(quicklinks),
            favorites: Mutex::new(favorites),
            calc_history: Mutex::new(calc_history),
            file_indexing: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Record a newly copied text string into SQLite clipboard history.
    pub fn record_clipboard_text(&self, text: &str, source: Option<&str>) {
        if text.is_empty() || text.chars().count() > tc_clipboard::store::MAX_TEXT_LENGTH {
            return;
        }
        {
            let mut guard = lock_or_recover(&self.clipboard);
            if let Some(store) = guard.as_mut() {
                let _ = store.add_text(text, source);
            }
        }
        lock_or_recover(&self.paste_queue).note_external_change();
    }

    /// Record copied file paths into SQLite clipboard history.
    pub fn record_clipboard_files(&self, paths: &[String], source: Option<&str>) {
        if paths.is_empty() {
            return;
        }
        {
            let mut guard = lock_or_recover(&self.clipboard);
            if let Some(store) = guard.as_mut() {
                let _ = store.add_files(paths, source);
            }
        }
        lock_or_recover(&self.paste_queue).note_external_change();
    }

    /// Record a copied RGBA image into SQLite clipboard history.
    pub fn record_clipboard_image(
        &self,
        width: u32,
        height: u32,
        rgba: &[u8],
        source: Option<&str>,
    ) {
        if width == 0 || height == 0 || rgba.is_empty() {
            return;
        }
        {
            let mut guard = lock_or_recover(&self.clipboard);
            if let Some(store) = guard.as_mut() {
                let _ = store.add_image_rgba(width, height, rgba, source);
            }
        }
        lock_or_recover(&self.paste_queue).note_external_change();
    }

    /// Decode an image clipboard item's PNG blob into `(width, height, rgba)`
    /// without holding any mutex across disk I/O.
    pub fn load_clipboard_image_rgba(&self, item: &ClipboardItem) -> Option<(u32, u32, Vec<u8>)> {
        if item.kind != tc_clipboard::ItemKind::Image {
            return None;
        }
        let path = item.image_path.as_deref()?;
        let bytes = std::fs::read(path).ok()?;
        tc_clipboard::decode_png_rgba(&bytes)
    }

    /// Search clipboard history under the given query and filter.
    pub fn search_clipboard(
        &self,
        query: &str,
        filter: tc_clipboard::ClipboardFilter,
    ) -> Vec<ClipboardItem> {
        lock_or_recover(&self.clipboard)
            .as_ref()
            .map(|s| s.search(query, filter))
            .unwrap_or_default()
    }

    /// Search clipboard history using the UI's `ClipboardViewFilter` tabs
    /// (including the Pinned-only view).
    pub fn search_clipboard_view(
        &self,
        query: &str,
        filter: ClipboardViewFilter,
    ) -> Vec<ClipboardItem> {
        let items = self.search_clipboard(query, filter.store_filter());
        if filter == ClipboardViewFilter::Pinned {
            items.into_iter().filter(ClipboardItem::is_pinned).collect()
        } else {
            items
        }
    }

    /// Enqueue a clipboard item for FIFO sequential pasting.
    pub fn paste_queue_add(&self, item: ClipboardItem) -> usize {
        lock_or_recover(&self.paste_queue).add(item)
    }

    /// Return the number of explicitly queued sequential-paste items.
    pub fn paste_queue_len(&self) -> usize {
        lock_or_recover(&self.paste_queue).len()
    }

    /// Clear the explicit sequential-paste queue and reset any active walk.
    pub fn paste_queue_clear(&self) -> usize {
        lock_or_recover(&self.paste_queue).clear()
    }

    /// Pop the next sequential-paste item at the current wall-clock timestamp.
    pub fn paste_queue_pop_next(&self) -> PasteNext {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
            .unwrap_or(0);
        self.paste_queue_pop_next_at(now_ms)
    }

    /// Deterministic helper for popping the next sequential-paste item at `now_ms`.
    ///
    /// Snapshots clipboard history first and releases the clipboard mutex before
    /// locking `paste_queue`, so both mutexes are never held simultaneously.
    pub fn paste_queue_pop_next_at(&self, now_ms: i64) -> PasteNext {
        let history: Vec<ClipboardItem> = {
            let guard = lock_or_recover(&self.clipboard);
            guard
                .as_ref()
                .map(|store| store.items().to_vec())
                .unwrap_or_default()
        };
        lock_or_recover(&self.paste_queue).pop_next(&history, now_ms)
    }

    /// Toggle pin state of a clipboard item.
    pub fn toggle_clipboard_pin(&self, item: &ClipboardItem) {
        if let Ok(mut guard) = self.clipboard.lock() {
            if let Some(store) = guard.as_mut() {
                let _ = store.toggle_pinned(item);
            }
        }
    }

    /// Remove a clipboard item permanently.
    pub fn remove_clipboard_item(&self, item: &ClipboardItem) {
        if let Ok(mut guard) = self.clipboard.lock() {
            if let Some(store) = guard.as_mut() {
                let _ = store.remove(item);
            }
        }
    }

    /// Remove all unpinned clipboard items, preserving pinned entries.
    pub fn clear_unpinned_clipboard(&self) -> usize {
        let Ok(mut guard) = self.clipboard.lock() else {
            return 0;
        };
        let Some(store) = guard.as_mut() else {
            return 0;
        };
        let unpinned: Vec<ClipboardItem> = store
            .items()
            .iter()
            .filter(|i| !i.is_pinned())
            .cloned()
            .collect();
        let mut removed = 0;
        for item in &unpinned {
            if store.remove(item).is_ok() {
                removed += 1;
            }
        }
        removed
    }

    /// Promote a chosen clipboard item to the top of recency.
    pub fn promote_clipboard_item(&self, item: &ClipboardItem) {
        if let Ok(mut guard) = self.clipboard.lock() {
            if let Some(store) = guard.as_mut() {
                let _ = store.promote(item);
            }
        }
    }

    /// Return ordered emoji cells for the 2D grid (pinned + frequent + catalog,
    /// or fuzzy-ranked matches when `query` is non-empty).
    pub fn emoji_cells(&self, query: &str) -> Vec<tc_ui::GridCell> {
        let pinned_glyphs: Vec<String> = self
            .emoji_pins
            .lock()
            .ok()
            .map(|p| p.glyphs().to_vec())
            .unwrap_or_default();
        let freq_guard = self.emoji_freq.lock().ok();

        let trimmed = query.trim();
        if !trimmed.is_empty() {
            let empty_freq = FrequentEmojiStore::in_memory();
            let freq = freq_guard.as_deref().unwrap_or(&empty_freq);
            return self
                .emoji_index
                .search(trimmed, freq, 160)
                .into_iter()
                .map(|entry| tc_ui::GridCell {
                    glyph: entry.glyph.clone(),
                    name: entry.name.clone(),
                    pinned: pinned_glyphs.iter().any(|g| g == &entry.glyph),
                })
                .collect();
        }

        let mut seen = std::collections::HashSet::<String>::new();
        let mut cells = Vec::with_capacity(200);

        // 1. Pinned emojis first.
        for glyph in &pinned_glyphs {
            if seen.insert(glyph.clone()) {
                let name = self
                    .emoji_index
                    .entry(glyph)
                    .map(|e| e.name.clone())
                    .unwrap_or_else(|| "Pinned".to_string());
                cells.push(tc_ui::GridCell {
                    glyph: glyph.clone(),
                    name,
                    pinned: true,
                });
            }
        }

        // 2. Frequently used emojis next.
        if let Some(freq) = freq_guard.as_deref() {
            for glyph in freq.top(32) {
                if seen.insert(glyph.clone()) {
                    if let Some(entry) = self.emoji_index.entry(&glyph) {
                        cells.push(tc_ui::GridCell {
                            glyph: entry.glyph.clone(),
                            name: entry.name.clone(),
                            pinned: false,
                        });
                    }
                }
            }
        }

        // 3. Popular catalog browse list (capped cleanly for instant rendering).
        for entry in self.emoji_index.entries().iter().take(240) {
            if seen.insert(entry.glyph.clone()) {
                cells.push(tc_ui::GridCell {
                    glyph: entry.glyph.clone(),
                    name: entry.name.clone(),
                    pinned: false,
                });
            }
        }

        cells
    }

    /// Record an emoji selection in frequency history.
    pub fn record_emoji_use(&self, glyph: &str) {
        if let Ok(mut freq) = self.emoji_freq.lock() {
            freq.record(glyph);
        }
    }

    /// Pin or unpin an emoji glyph.
    pub fn toggle_emoji_pin(&self, glyph: &str) {
        if let Ok(mut pins) = self.emoji_pins.lock() {
            pins.toggle(glyph);
        }
    }

    /// Search stored snippets by name, keyword, or body text.
    pub fn search_snippets(&self, query: &str) -> Vec<StoredSnippet> {
        let Ok(guard) = self.snippets.lock() else {
            return Vec::new();
        };
        let q = query.trim().to_ascii_lowercase();
        let q_no_bang = q.strip_prefix('!').unwrap_or(&q);
        guard
            .snippets()
            .iter()
            .filter(|record| {
                if q.is_empty() {
                    return true;
                }
                record.snippet.name.to_ascii_lowercase().contains(&q)
                    || record.snippet.text.to_ascii_lowercase().contains(&q)
                    || record.snippet.keyword.as_deref().is_some_and(|kw| {
                        let kl = kw.to_ascii_lowercase();
                        kl.contains(&q) || kl.contains(q_no_bang)
                    })
            })
            .cloned()
            .collect()
    }

    /// Exact keyword match for inline launcher expansion (e.g. typing `!sig` or `sig`).
    pub fn find_snippet_by_keyword(&self, query: &str) -> Option<StoredSnippet> {
        let trimmed = query.trim();
        if trimmed.len() < 2 {
            return None;
        }
        let stripped = trimmed.strip_prefix('!').unwrap_or(trimmed);
        let guard = lock_or_recover(&self.snippets);
        guard
            .snippets()
            .iter()
            .find(|record| {
                record.snippet.is_enabled
                    && record.snippet.keyword.as_deref().is_some_and(|kw| {
                        let kw_clean = kw.trim().strip_prefix('!').unwrap_or(kw.trim());
                        !kw_clean.is_empty() && kw_clean.eq_ignore_ascii_case(stripped)
                    })
            })
            .cloned()
    }

    /// Collect all enabled snippet keywords for global auto-expansion.
    ///
    /// Keywords starting with an ASCII alphanumeric character are registered both
    /// bare (`sig`) and `!`-prefixed (`!sig`), deduplicated.
    pub fn auto_expand_keywords(&self) -> Vec<String> {
        let guard = lock_or_recover(&self.snippets);
        let mut seen = HashSet::new();
        let mut keywords = Vec::new();
        for record in guard.snippets() {
            if !record.snippet.is_enabled {
                continue;
            }
            let Some(raw_kw) = record.snippet.keyword.as_deref() else {
                continue;
            };
            let kw = raw_kw.trim();
            if kw.is_empty() {
                continue;
            }
            if seen.insert(kw.to_string()) {
                keywords.push(kw.to_string());
            }
            if kw.chars().next().is_some_and(|c| c.is_ascii_alphanumeric()) {
                let prefixed = format!("!{kw}");
                if seen.insert(prefixed.clone()) {
                    keywords.push(prefixed);
                }
            }
        }
        keywords
    }

    /// Look up an enabled snippet by keyword and return its expanded text.
    pub fn expand_snippet_keyword(&self, keyword: &str) -> Option<String> {
        let record = self.find_snippet_by_keyword(keyword)?;
        Some(self.expand_snippet(&record))
    }

    /// Expand a snippet template using live local date/time, UUID, clipboard history,
    /// and nested `{snippet:Name}` references.
    pub fn expand_snippet(&self, record: &StoredSnippet) -> String {
        let all_snippets: Vec<StoredSnippet> = self
            .snippets
            .lock()
            .ok()
            .map(|g| g.snippets().to_vec())
            .unwrap_or_default();

        let mut clip_history: Vec<String> = self
            .clipboard
            .lock()
            .ok()
            .and_then(|g| {
                g.as_ref().map(|s| {
                    s.items()
                        .iter()
                        .filter_map(|i| i.plain_text().map(str::to_string))
                        .take(10)
                        .collect()
                })
            })
            .unwrap_or_default();
        if clip_history.is_empty() {
            if let Some(current) = tc_platform_win::clipboard().read_text() {
                clip_history.push(current);
            }
        }

        let local_now = Local::now();
        let offset = *local_now.offset();
        let fixed_now = local_now.fixed_offset();
        let ctx = ExpansionContext::new(clip_history, "", fixed_now, "en_US", offset);
        let result = SnippetTemplateEngine::expand(record, &all_snippets, &ctx, &BTreeMap::new());
        result.text
    }

    /// Create and persist a new snippet Markdown file.
    pub fn create_snippet(
        &self,
        name: String,
        keyword: Option<String>,
        text: String,
    ) -> Result<StoredSnippet, String> {
        let clean_kw = keyword.and_then(|k| {
            let trimmed = k.trim().trim_start_matches('!').trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        });
        let snippet = Snippet::with_options(name.trim(), text, clean_kw, true, false);
        let mut guard = self
            .snippets
            .lock()
            .map_err(|_| "snippets lock poisoned".to_string())?;
        guard.create(&snippet).map_err(|e| e.to_string())
    }

    /// Delete a stored snippet by id.
    pub fn delete_snippet(&self, id: &str) {
        if let Ok(mut guard) = self.snippets.lock() {
            let _ = guard.delete(id);
        }
    }

    /// Record a copied calculation into persistent calculator history.
    pub fn record_calc(&self, expression: &str, display: &str, copy_text: &str, badge: &str) {
        if let Ok(mut guard) = self.calc_history.lock() {
            guard.record(expression, display, copy_text, badge);
        }
    }

    /// Return the most recent numeric calculator result from persistent history, if any.
    pub fn latest_calc_numeric_answer(&self) -> Option<f64> {
        let guard = self.calc_history.lock().ok()?;
        guard
            .entries()
            .iter()
            .find_map(|entry| crate::launcher::parse_numeric_answer(&entry.copy_text))
    }

    /// Search calculator history.
    pub fn search_calc_history(&self, query: &str) -> Vec<CalcHistoryEntry> {
        self.calc_history
            .lock()
            .ok()
            .map(|g| g.search(query))
            .unwrap_or_default()
    }

    /// Delete one calculator history entry by index (in unfiltered list) or matching expression.
    pub fn remove_calc_entry(&self, entry: &CalcHistoryEntry) {
        if let Ok(mut guard) = self.calc_history.lock() {
            if let Some(pos) = guard
                .entries()
                .iter()
                .position(|e| e.expression == entry.expression && e.copy_text == entry.copy_text)
            {
                guard.remove_at(pos);
            }
        }
    }

    /// Whether a background file index pass is currently running.
    pub fn is_file_indexing(&self) -> bool {
        self.file_indexing.load(Ordering::Relaxed)
    }

    /// Search quick notes by title or body.
    pub fn search_notes(&self, query: &str) -> Vec<NoteItem> {
        self.notes
            .lock()
            .ok()
            .map(|g| g.search(query))
            .unwrap_or_default()
    }

    /// Create a new Markdown note.
    pub fn create_note(&self, title: &str, body: &str) -> Result<NoteItem, String> {
        let mut guard = self
            .notes
            .lock()
            .map_err(|_| "notes lock poisoned".to_string())?;
        guard.create(title, body)
    }

    /// Delete a quick note file.
    pub fn delete_note(&self, path: &Path) {
        if let Ok(mut guard) = self.notes.lock() {
            guard.delete(path);
        }
    }

    /// Search quicklinks by name, alias, or URL template.
    pub fn search_quicklinks(&self, query: &str) -> Vec<Quicklink> {
        self.quicklinks
            .lock()
            .ok()
            .map(|g| g.search(query))
            .unwrap_or_default()
    }

    /// Add a custom quicklink.
    pub fn add_quicklink(&self, name: String, url_template: String) {
        if let Ok(mut guard) = self.quicklinks.lock() {
            guard.add(name, url_template);
        }
    }

    /// Delete a quicklink by name.
    pub fn delete_quicklink(&self, name: &str) {
        if let Ok(mut guard) = self.quicklinks.lock() {
            guard.remove_by_name(name);
        }
    }

    /// Toggle whether `id` is pinned as a Launcher Favorite.
    pub fn toggle_favorite(&self, id: &str) -> bool {
        self.favorites
            .lock()
            .ok()
            .map(|mut g| g.toggle(id))
            .unwrap_or(false)
    }

    /// Ordered favorite row IDs.
    pub fn favorite_ids(&self) -> Vec<String> {
        self.favorites
            .lock()
            .ok()
            .map(|g| g.ids().to_vec())
            .unwrap_or_default()
    }

    /// Check if `id` is currently favorited.
    pub fn is_favorite(&self, id: &str) -> bool {
        self.favorites.lock().ok().is_some_and(|g| g.contains(id))
    }

    /// Export a verified `.yalbackup.json` snapshot to the user's Desktop (or home dir).
    pub fn export_backup_to_desktop(&self) -> Result<(PathBuf, usize), String> {
        // Mirror live `settings_file` into `data_dir/settings.json` before bundling so
        // `tc_backup` captures the exact active settings even when stored under `.config`.
        if self.settings_file.is_file() {
            let mirror = self.data_dir.join("settings.json");
            if mirror != self.settings_file {
                let _ = std::fs::copy(&self.settings_file, &mirror);
            }
        }
        let date = Local::now().format("%Y-%m-%d");
        let target_dir = {
            let desktop = self.home_dir.join("Desktop");
            if desktop.is_dir() {
                desktop
            } else {
                self.home_dir.clone()
            }
        };
        let out = target_dir.join(format!("yal-backup-{date}.yalbackup.json"));
        let summary = tc_backup::export_backup(&self.data_dir, &out).map_err(|e| e.to_string())?;
        Ok((out, summary.files_processed))
    }

    /// Verify and restore a `.yalbackup.json` file and hot-reload EVERY in-memory store
    /// so no subsequent action can overwrite restored files with stale memory state.
    pub fn import_backup_from(
        &self,
        path: &Path,
        settings: &tc_platform_win::WindowsSettings,
        launcher: &LauncherState,
    ) -> Result<usize, String> {
        let summary = tc_backup::import_backup(path, &self.data_dir).map_err(|e| e.to_string())?;

        // Sync restored `data_dir/settings.json` back to `paths.settings_file` and reload.
        let restored_settings = self.data_dir.join("settings.json");
        if restored_settings.is_file() && restored_settings != self.settings_file {
            if let Some(parent) = self.settings_file.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::copy(&restored_settings, &self.settings_file);
        }
        settings.reload();

        // Reload launcher ranking & every feature store from disk.
        launcher.reload_ranking(self.data_dir.join(crate::launcher::RANKING_FILE));
        if let Ok(mut s) = self.snippets.lock() {
            let _ = s.start();
        }
        if let Ok(mut n) = self.notes.lock() {
            n.reload();
        }
        if let Ok(mut q) = self.quicklinks.lock() {
            q.reload();
        }
        if let Ok(mut f) = self.favorites.lock() {
            f.reload();
        }
        if let Ok(mut pins) = self.emoji_pins.lock() {
            *pins = PinnedEmojiStore::open(self.data_dir.join(EMOJI_PINS_FILE));
        }
        if let Ok(mut freq) = self.emoji_freq.lock() {
            *freq = FrequentEmojiStore::open(self.data_dir.join(EMOJI_FREQ_FILE));
        }
        if let Ok(mut hist) = self.calc_history.lock() {
            *hist = CalcHistoryStore::open(self.data_dir.join(CALC_HISTORY_FILE));
        }
        Ok(summary.files_processed)
    }

    /// Import snippets from a Raycast JSON snippet export file.
    pub fn import_raycast_snippets_from(&self, path: &Path) -> Result<usize, String> {
        let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let val: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
        let parsed = RaycastSnippetImport::parse(&val);
        if parsed.is_empty() {
            return Err("no valid Raycast snippets found in file".to_string());
        }
        let mut guard = self
            .snippets
            .lock()
            .map_err(|_| "snippets lock poisoned".to_string())?;
        let created = guard.import_snippets(&parsed).map_err(|e| e.to_string())?;
        Ok(created.len())
    }

    /// Execute a quick one-off PowerShell command and return combined stdout/stderr.
    pub fn run_shell_command(command: &str) -> Result<String, String> {
        let plan = Shell::PowerShell
            .plan(command, &[], false, None)
            .map_err(|e| e.to_string())?;
        let mut running = ShellRunner::spawn(&plan).map_err(|e| e.to_string())?;
        let outcome = running.wait();
        if outcome.succeeded() {
            let out = outcome.stdout.trim();
            if out.is_empty() {
                Ok("Command completed (no output)".to_string())
            } else {
                Ok(out.to_string())
            }
        } else {
            let msg = outcome.failure_message();
            if msg.is_empty() {
                Err(format!("Exited with status {}", outcome.status))
            } else {
                Err(msg)
            }
        }
    }

    /// Kick off a non-blocking background incremental index walk of the user's
    /// personal and project folders under their home directory and attach the
    /// live WAL-backed SQLite store to `launcher`.
    pub fn trigger_file_index(
        &self,
        launcher: Arc<LauncherState>,
        handle_slot: Option<Arc<Mutex<Option<PaletteHandle>>>>,
    ) {
        if self
            .file_indexing
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }
        let flag = Arc::clone(&self.file_indexing);
        let db_path = self.data_dir.join(INDEX_FILE);
        let home_dir = self.home_dir.clone();

        let _ = std::thread::Builder::new()
            .name("yal-file-index".to_string())
            .spawn(move || {
                let roots = default_index_roots(&home_dir);
                if let Ok(mut store) = IndexStore::open(&db_path) {
                    // Attach a concurrent WAL reader before the walk starts if
                    // the launcher did not have a file store yet, so results
                    // become searchable live as each 250-dir batch commits.
                    if !launcher.has_file_search() {
                        if let Ok(reader) = IndexStore::open(&db_path) {
                            launcher.set_file_search(FileSearch::new(
                                reader,
                                home_dir.clone(),
                                Vec::new(),
                            ));
                        }
                    }
                    let rules = IgnoreRules::defaults();
                    let _ = store.walk_batched(&roots, &rules, &home_dir);
                    let now_ns = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_nanos() as i64)
                        .unwrap_or(0);
                    let _ = store.set_built_at(now_ns);
                    launcher.set_file_search(FileSearch::new(store, home_dir, Vec::new()));
                }
                flag.store(false, Ordering::Release);
                if let Some(slot) = handle_slot {
                    if let Ok(guard) = slot.lock() {
                        if let Some(handle) = *guard {
                            tc_ui::post_refresh(handle);
                        }
                    }
                }
            });
    }
}

/// Low-value Windows profile folders skipped from automatic home-root discovery.
const SKIPPED_PROFILE_DIRS: &[&str] = &[
    "contacts",
    "favorites",
    "links",
    "saved games",
    "searches",
    "3d objects",
    "intelgraphicsprofiles",
];

/// Priority Windows user folders ordered first in [`default_index_roots`].
const PRIORITY_HOME_DIRS: &[&str] = &[
    "Desktop",
    "Documents",
    "Downloads",
    "Pictures",
    "Videos",
    "Music",
];

/// Discover all non-hidden, non-junction, non-ignored user folders directly
/// under `home` (e.g. `Desktop`, `Documents`, `Downloads`, `development`,
/// `OneDrive`, `tools`, `AndroidStudioProjects`), placing standard personal
/// folders first and skipping `AppData`, dot-folders, and system junctions.
pub fn default_index_roots(home: &Path) -> Vec<PathBuf> {
    let rules = IgnoreRules::defaults();
    let mut roots: Vec<PathBuf> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    for name in PRIORITY_HOME_DIRS {
        let candidate = home.join(name);
        if is_indexable_home_subdir(&candidate, name, home, &rules)
            && seen.insert(name.to_ascii_lowercase())
        {
            roots.push(candidate);
        }
    }

    if let Ok(read_dir) = std::fs::read_dir(home) {
        let mut extras: Vec<(String, PathBuf)> = Vec::new();
        for entry in read_dir.flatten() {
            let path = entry.path();
            let Some(name) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };
            let key = name.to_ascii_lowercase();
            if seen.contains(&key) {
                continue;
            }
            if is_indexable_home_subdir(&path, &name, home, &rules) {
                seen.insert(key.clone());
                extras.push((key, path));
            }
        }
        extras.sort_by(|a, b| a.0.cmp(&b.0));
        roots.extend(extras.into_iter().map(|(_, p)| p));
    }

    if roots.is_empty() && home.is_dir() {
        roots.push(home.to_path_buf());
    }
    roots
}

fn is_indexable_home_subdir(path: &Path, name: &str, home: &Path, rules: &IgnoreRules) -> bool {
    if name.starts_with('.') {
        return false;
    }
    let lower = name.to_ascii_lowercase();
    if SKIPPED_PROFILE_DIRS.contains(&lower.as_str()) {
        return false;
    }
    if rules.is_ignored(name, path, home) {
        return false;
    }
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return false;
    };
    let ft = meta.file_type();
    if !ft.is_dir() || ft.is_symlink() {
        return false;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_HIDDEN: u32 = 0x0002;
        const FILE_ATTRIBUTE_SYSTEM: u32 = 0x0004;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        let attrs = meta.file_attributes();
        if attrs & (FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM | FILE_ATTRIBUTE_REPARSE_POINT)
            != 0
        {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calc_history_records_deduplicates_and_searches() {
        let dir = std::env::temp_dir().join(format!(
            "yal-calc-hist-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = dir.join("calc-history.json");
        let mut store = CalcHistoryStore::open(&path);
        store.record("100 USD to INR", "8,440.00 INR", "8440.00 INR", "INR");
        store.record("2+2", "4", "4", "Result");
        store.record("100 USD to INR", "8,440.00 INR", "8440.00 INR", "INR");
        assert_eq!(store.entries().len(), 2);
        assert_eq!(store.entries()[0].expression, "100 USD to INR");
        assert_eq!(store.search("inr").len(), 1);

        let reloaded = CalcHistoryStore::open(&path);
        assert_eq!(reloaded.entries().len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn quicklink_expands_both_query_and_argument_placeholders() {
        let q1 = Quicklink {
            name: "GitHub".to_string(),
            url_template: "https://github.com/search?q={query}".to_string(),
            alias: Some("gh".to_string()),
        };
        assert!(q1.needs_query());
        assert_eq!(
            q1.expand_url("rust win32"),
            "https://github.com/search?q=rust%20win32"
        );

        let q2 = Quicklink {
            name: "Docs".to_string(),
            url_template: "https://docs.rs/{argument}".to_string(),
            alias: None,
        };
        assert!(q2.needs_query());
        assert_eq!(q2.expand_url("serde json"), "https://docs.rs/serde%20json");

        let q3 = Quicklink {
            name: "Direct".to_string(),
            url_template: "https://example.com".to_string(),
            alias: None,
        };
        assert!(!q3.needs_query());
        assert_eq!(q3.expand_url("ignored"), "https://example.com");
    }

    #[test]
    fn clipboard_view_filter_cycles_in_order() {
        let mut current = ClipboardViewFilter::All;
        let mut visited = Vec::new();
        for _ in 0..ClipboardViewFilter::ALL.len() {
            visited.push((current, current.label()));
            current = current.next();
        }
        assert_eq!(current, ClipboardViewFilter::All);
        assert_eq!(
            visited,
            vec![
                (ClipboardViewFilter::All, "All"),
                (ClipboardViewFilter::Text, "Text"),
                (ClipboardViewFilter::Images, "Images"),
                (ClipboardViewFilter::Files, "Files"),
                (ClipboardViewFilter::Links, "Links"),
                (ClipboardViewFilter::Colors, "Colors"),
                (ClipboardViewFilter::Pinned, "Pinned"),
            ]
        );
        assert_eq!(
            ClipboardViewFilter::Pinned.store_filter(),
            ClipboardFilter::All
        );
        assert_eq!(
            ClipboardViewFilter::Images.store_filter(),
            ClipboardFilter::Image
        );
    }

    #[test]
    fn paste_queue_supports_fifo_explicit_items_and_history_sequence_walk() {
        let mut first = ClipboardItem::text("first", None);
        first.id = "id-1".to_string();
        let mut second = ClipboardItem::text("second", None);
        second.id = "id-2".to_string();
        let mut third = ClipboardItem::text("third", None);
        third.id = "id-3".to_string();

        let mut queue = PasteQueue::new();
        assert_eq!(queue.len(), 0);
        assert_eq!(queue.add(first.clone()), 1);
        assert_eq!(queue.add(second.clone()), 2);
        assert_eq!(queue.len(), 2);

        match queue.pop_next(&[], 1_000) {
            PasteNext::Item { item, remaining } => {
                assert_eq!(item.plain_text(), Some("first"));
                assert_eq!(remaining, 1);
            }
            other => panic!("expected first fifo item, got {other:?}"),
        }
        // Rapid repeat inside the 250ms settle window is held back.
        assert_eq!(queue.pop_next(&[], 1_100), PasteNext::Settling);
        assert_eq!(queue.clear(), 1);
        assert_eq!(queue.len(), 0);

        // When explicit queue is empty, walks history newest-first via PasteSequence.
        let history = vec![third.clone(), second.clone(), first.clone()];
        match queue.pop_next(&history, 10_000) {
            PasteNext::Item { item, remaining } => {
                assert_eq!(item.plain_text(), Some("third"));
                assert_eq!(remaining, 2);
            }
            other => panic!("expected history 1, got {other:?}"),
        }

        // Rapid double-tap inside settle interval does not overwrite clipboard.
        assert_eq!(queue.pop_next(&history, 10_100), PasteNext::Settling);

        match queue.pop_next(&history, 10_500) {
            PasteNext::Item { item, remaining } => {
                assert_eq!(item.plain_text(), Some("second"));
                assert_eq!(remaining, 1);
            }
            other => panic!("expected history 2, got {other:?}"),
        }

        // An external clipboard copy invalidates the active walk and restarts from newest.
        queue.note_external_change();
        match queue.pop_next(&history, 10_900) {
            PasteNext::Item { item, remaining } => {
                assert_eq!(item.plain_text(), Some("third"));
                assert_eq!(remaining, 2);
            }
            other => panic!("expected restarted walk after external copy, got {other:?}"),
        }

        assert!(matches!(
            queue.pop_next(&history, 11_300),
            PasteNext::Item { remaining: 1, .. }
        ));
        assert!(matches!(
            queue.pop_next(&history, 11_700),
            PasteNext::Item { remaining: 0, .. }
        ));

        // Exhausted walk stops rather than wrapping immediately.
        assert_eq!(queue.pop_next(&history, 12_100), PasteNext::Empty);

        // After ending, a subsequent call starts a fresh walk.
        match queue.pop_next(&history, 12_500) {
            PasteNext::Item { item, remaining } => {
                assert_eq!(item.plain_text(), Some("third"));
                assert_eq!(remaining, 2);
            }
            other => panic!("expected restarted walk, got {other:?}"),
        }
    }

    #[test]
    fn snippets_auto_expand_keywords_and_template_placeholders() {
        let dir = std::env::temp_dir().join(format!(
            "yal-features-snippets-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let paths = Paths {
            data_dir: dir.join("data"),
            cache_dir: dir.join("cache"),
            home_dir: dir.join("home"),
            settings_file: dir.join("settings.json"),
        };
        let stores = FeatureStores::open(&paths);
        stores.record_clipboard_text("clip-payload", None);
        stores
            .create_snippet(
                "Meta".to_string(),
                Some("meta".to_string()),
                "d={date}|t={time}|dt={datetime}|u={uuid}|c={clipboard}".to_string(),
            )
            .expect("create snippet");

        let keywords = stores.auto_expand_keywords();
        assert!(keywords.contains(&"meta".to_string()));
        assert!(keywords.contains(&"!meta".to_string()));

        let expanded_bare = stores
            .expand_snippet_keyword("meta")
            .expect("expand bare keyword");
        let expanded_bang = stores
            .expand_snippet_keyword("!meta")
            .expect("expand bang keyword");
        for expanded in [&expanded_bare, &expanded_bang] {
            assert!(!expanded.contains("{date}"));
            assert!(!expanded.contains("{time}"));
            assert!(!expanded.contains("{datetime}"));
            assert!(!expanded.contains("{uuid}"));
            assert!(!expanded.contains("{clipboard}"));
            assert!(expanded.ends_with("|c=clip-payload"));
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn default_index_roots_discovers_user_project_dirs_and_skips_noise() {
        let dir = std::env::temp_dir().join(format!(
            "yal-index-roots-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        for sub in [
            "Desktop",
            "Documents",
            "development",
            "tools",
            "AppData",
            "node_modules",
            ".vscode",
            ".cache",
            "Contacts",
        ] {
            std::fs::create_dir_all(dir.join(sub)).unwrap();
        }

        let roots = default_index_roots(&dir);
        let names: Vec<String> = roots
            .iter()
            .filter_map(|p| p.file_name().and_then(|n| n.to_str()).map(str::to_string))
            .collect();
        assert_eq!(names, vec!["Desktop", "Documents", "development", "tools"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
