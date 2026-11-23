//! Live stateful feature stores shared between the Win32 shell and the native
//! Direct2D palette: Clipboard History, Emoji Picker, Snippets, Calculator
//! History, and the background incremental File Indexer.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use chrono::Local;
use serde::{Deserialize, Serialize};
use tc_clipboard::{ClipboardItem, ClipboardStore};
use tc_emoji::{EmojiIndex, FrequentEmojiStore, PinnedEmojiStore};
use tc_files::index::walk_once;
use tc_files::scope::IgnoreRules;
use tc_files::store::IndexStore;
use tc_platform::{Clipboard, Paths};
use tc_snippets::template::{ExpansionContext, SnippetTemplateEngine};
use tc_snippets::{Snippet, SnippetRepository, SnippetsStore, StoredSnippet};
use tc_ui::PaletteHandle;

use crate::launcher::{FileSearch, LauncherState};
use crate::shell::INDEX_FILE;

const EMOJI_PINS_FILE: &str = "emoji-pins.json";
const EMOJI_FREQ_FILE: &str = "emoji-frequency.json";
const CALC_HISTORY_FILE: &str = "calc-history.json";
const SNIPPETS_DIR: &str = "snippets";
const MAX_CALC_HISTORY: usize = 100;

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

/// Every live feature store owned by the running YAL process.
pub struct FeatureStores {
    pub data_dir: PathBuf,
    pub home_dir: PathBuf,
    pub clipboard: Mutex<Option<ClipboardStore>>,
    pub emoji_index: Arc<EmojiIndex>,
    pub emoji_pins: Mutex<PinnedEmojiStore>,
    pub emoji_freq: Mutex<FrequentEmojiStore>,
    pub snippets: Mutex<SnippetsStore>,
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

        let calc_history = CalcHistoryStore::open(paths.data_dir.join(CALC_HISTORY_FILE));

        Self {
            data_dir: paths.data_dir.clone(),
            home_dir: paths.home_dir.clone(),
            clipboard: Mutex::new(clipboard),
            emoji_index,
            emoji_pins: Mutex::new(emoji_pins),
            emoji_freq: Mutex::new(emoji_freq),
            snippets: Mutex::new(snippets),
            calc_history: Mutex::new(calc_history),
            file_indexing: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Record a newly copied text string into SQLite clipboard history.
    pub fn record_clipboard_text(&self, text: &str, source: Option<&str>) {
        if text.is_empty() || text.chars().count() > tc_clipboard::store::MAX_TEXT_LENGTH {
            return;
        }
        if let Ok(mut guard) = self.clipboard.lock() {
            if let Some(store) = guard.as_mut() {
                let _ = store.add_text(text, source);
            }
        }
    }

    /// Search clipboard history under the given query and filter.
    pub fn search_clipboard(
        &self,
        query: &str,
        filter: tc_clipboard::ClipboardFilter,
    ) -> Vec<ClipboardItem> {
        self.clipboard
            .lock()
            .ok()
            .and_then(|guard| guard.as_ref().map(|s| s.search(query, filter)))
            .unwrap_or_default()
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
        let guard = self.snippets.lock().ok()?;
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

    /// Kick off a non-blocking background incremental index walk of the user's
    /// default Windows personal folders (Desktop, Documents, Downloads, Pictures,
    /// Videos, Music) and attach the live SQLite store to `launcher`.
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
                    let rules = IgnoreRules::defaults();
                    let _ = walk_once(&mut store, &roots, &rules, &home_dir);
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

/// Default Windows user folders for incremental file search.
fn default_index_roots(home: &Path) -> Vec<PathBuf> {
    let candidates = [
        home.join("Desktop"),
        home.join("Documents"),
        home.join("Downloads"),
        home.join("Pictures"),
        home.join("Videos"),
        home.join("Music"),
    ];
    let mut roots: Vec<PathBuf> = candidates.into_iter().filter(|p| p.is_dir()).collect();
    if roots.is_empty() && home.is_dir() {
        roots.push(home.to_path_buf());
    }
    roots
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
}
