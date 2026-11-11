//! The history store: how entries are recorded, deduplicated, ordered, capped
//! and pruned.
//!
//! Upstream oracle: `Features/Clipboard/Model/ClipboardStore.swift` — the
//! `ClipboardStore` class, its `items` array, and the `ClipboardItem`,
//! `ClipboardRetention`, `ClipboardDefaultAction` and `ClipboardChord` types
//! declared alongside it.
//!
//! The store keeps its history in a SQLite database whose path is injected by
//! the caller, so a test can point it at a temporary directory and the app can
//! point it at `%APPDATA%\yal`. The rules are upstream's; the file layout is
//! this port's own.

use std::path::{Path, PathBuf};

use rusqlite::{params, Connection};

use crate::filter::ClipboardFilter;
use crate::item::{ClipboardItem, EntryId, ItemKind, Timestamp};

/// The longest text history holds; a longer copy is skipped, since truncation
/// would lose the tail.
///
/// Upstream `ClipboardStore.maxTextLength`.
pub const MAX_TEXT_LENGTH: usize = 32_000;

/// The most entries held in memory at once.
///
/// Upstream `ClipboardStore.memoryWindow`: a cap on the resident window, not
/// on the history itself — the rows past it stay in the database and remain
/// searchable.
pub const MEMORY_WINDOW: usize = 1000;

/// The most unpinned rows any one query answers with.
///
/// Upstream `ClipboardStore.searchLimit`.
pub const SEARCH_LIMIT: usize = 200;

/// How long the history keeps an entry, in days.
///
/// Upstream `ClipboardRetention`: `forever` is -1, so an unset key (0) falls
/// through to the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardRetention {
    /// One day.
    Day,
    /// One week.
    Week,
    /// One month.
    Month,
    /// Three months.
    ThreeMonths,
    /// Six months.
    SixMonths,
    /// One year.
    Year,
    /// Forever: nothing is pruned.
    Forever,
}

impl ClipboardRetention {
    /// The retention's own setting, in days. `Forever` is -1.
    pub fn days(self) -> i64 {
        match self {
            ClipboardRetention::Day => 1,
            ClipboardRetention::Week => 7,
            ClipboardRetention::Month => 30,
            ClipboardRetention::ThreeMonths => 90,
            ClipboardRetention::SixMonths => 180,
            ClipboardRetention::Year => 365,
            ClipboardRetention::Forever => -1,
        }
    }

    /// The oldest an entry may be and still be kept, in milliseconds.
    ///
    /// Upstream `ClipboardRetention.maxAge`: `forever` is the greatest finite
    /// magnitude, so nothing is ever old enough to prune.
    pub fn max_age_millis(self) -> i64 {
        if self == ClipboardRetention::Forever {
            i64::MAX
        } else {
            self.days() * 86_400 * 1000
        }
    }

    /// The retention a settings value names, defaulting to three months.
    ///
    /// Upstream `AppSettings.clipboardRetention` reads its stored day count
    /// and falls back to `threeMonths` for anything it does not know.
    pub fn from_days(days: i64) -> ClipboardRetention {
        match days {
            1 => ClipboardRetention::Day,
            7 => ClipboardRetention::Week,
            30 => ClipboardRetention::Month,
            90 => ClipboardRetention::ThreeMonths,
            180 => ClipboardRetention::SixMonths,
            365 => ClipboardRetention::Year,
            -1 => ClipboardRetention::Forever,
            _ => ClipboardRetention::ThreeMonths,
        }
    }
}

/// Why a write to the store failed.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// The database refused the write.
    #[error("the clipboard database refused the write")]
    WriteFailed,
    /// The database could not be opened or read.
    #[error("the clipboard database could not be read")]
    Database(#[from] rusqlite::Error),
    /// A file on disk could not be read or written.
    #[error("a clipboard file could not be read")]
    Io(#[from] std::io::Error),
}

/// Why an edit of an entry was refused.
///
/// Upstream `ClipboardStore.UpdateError`.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum UpdateError {
    /// The text was longer than the capture limit.
    #[error("the text is longer than the capture limit")]
    TooLong,
    /// The database refused the write.
    #[error("the clipboard database refused the write")]
    WriteFailed,
}

/// What ↵ does on a clipboard entry; Paste takes the chord the chosen action
/// leaves free.
///
/// Upstream `ClipboardDefaultAction`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardDefaultAction {
    /// Paste the entry.
    Paste,
    /// Copy the entry to the clipboard.
    Copy,
    /// Paste the entry as plain text.
    PastePlainText,
}

impl ClipboardDefaultAction {
    /// The chord an action answers while Paste is the default.
    ///
    /// Upstream `ClipboardDefaultAction.ownChord`.
    fn own_chord(self) -> ClipboardChord {
        match self {
            ClipboardDefaultAction::Paste => ClipboardChord::Return,
            ClipboardDefaultAction::Copy => ClipboardChord::Command,
            ClipboardDefaultAction::PastePlainText => ClipboardChord::ControlCommand,
        }
    }

    /// What the chord runs on the entry, given this default.
    ///
    /// Upstream `ClipboardDefaultAction.action(for:on:)`: `None` when the
    /// action has no text to work on.
    pub fn action(
        self,
        chord: ClipboardChord,
        item: &ClipboardItem,
    ) -> Option<ClipboardDefaultAction> {
        let has_plain_text = item.plain_text().is_some();
        // An image has no text, so a plain-text default pastes it as it is.
        let resolved = if self == ClipboardDefaultAction::PastePlainText && !has_plain_text {
            ClipboardDefaultAction::Paste
        } else {
            self
        };
        let action = match chord {
            ClipboardChord::Return => resolved,
            chord if chord == resolved.own_chord() => ClipboardDefaultAction::Paste,
            ClipboardChord::Command => ClipboardDefaultAction::Copy,
            ClipboardChord::ControlCommand => ClipboardDefaultAction::PastePlainText,
        };
        if action == ClipboardDefaultAction::PastePlainText && !has_plain_text {
            None
        } else {
            Some(action)
        }
    }
}

/// The ↵ chords a default reorders; ⌥↵ always pastes, so it is not one of them.
///
/// Upstream `ClipboardChord`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardChord {
    /// Return.
    Return,
    /// Command-Return.
    Command,
    /// Control-Command-Return.
    ControlCommand,
}

impl ClipboardChord {
    /// Every chord, in the order upstream's `CaseIterable` gives them.
    pub const ALL: [ClipboardChord; 3] = [
        ClipboardChord::Return,
        ClipboardChord::Command,
        ClipboardChord::ControlCommand,
    ];
}

/// The clipboard history.
///
/// Upstream `ClipboardStore`: newest-first with pins in place, every pin
/// resident, and a bounded window of entries held in memory.
pub struct ClipboardStore {
    connection: Connection,
    /// Where the database file lives.
    db_path: PathBuf,
    /// Where this store's own image blobs live.
    images_dir: PathBuf,
    /// The resident window: newest-first, every pin held however old it is.
    items: Vec<ClipboardItem>,
    /// The oldest an entry may be and still be kept, in milliseconds.
    max_age: i64,
}

impl ClipboardStore {
    /// Open the store in a directory, creating it if it is not there.
    ///
    /// Upstream `ClipboardStore.init(directory:)`: the database is
    /// `clipboard.sqlite3` in the directory, and the store's own images live
    /// in `images` beside it. The directory is the caller's choice, so the app
    /// passes `%APPDATA%\yal` and a test passes a temporary one.
    pub fn open(directory: &Path) -> Result<ClipboardStore, StoreError> {
        std::fs::create_dir_all(directory)?;
        let images_dir = directory.join("images");
        std::fs::create_dir_all(&images_dir)?;
        let db_path = directory.join("clipboard.sqlite3");
        let connection = Connection::open(&db_path)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "NORMAL")?;
        let store = ClipboardStore {
            connection,
            db_path,
            images_dir,
            items: Vec::new(),
            max_age: ClipboardRetention::ThreeMonths.max_age_millis(),
        };
        store.create_schema()?;
        Ok(store)
    }

    /// The database file's path.
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    /// The directory this store's own image blobs live in.
    pub fn images_dir(&self) -> &Path {
        &self.images_dir
    }

    /// The resident window: newest-first, with every pin held.
    pub fn items(&self) -> &[ClipboardItem] {
        &self.items
    }

    /// The oldest an entry may be and still be kept, in milliseconds.
    pub fn max_age(&self) -> i64 {
        self.max_age
    }

    /// Hold the history for a given time.
    pub fn set_max_age(&mut self, millis: i64) {
        self.max_age = millis;
    }

    /// Hold the history for a given retention.
    pub fn set_retention(&mut self, retention: ClipboardRetention) {
        self.max_age = retention.max_age_millis();
    }

    /// Read the history from disk into the resident window, then prune.
    ///
    /// Upstream `ClipboardStore.load()`: the window holds the newest entries
    /// plus every pin, and the retention cut runs afterwards, because age
    /// passes while the app is not running.
    pub fn load(&mut self) -> Result<(), StoreError> {
        let rows = self.all_rows()?;
        let mut resident: Vec<ClipboardItem> = Vec::new();
        let mut unpinned_seen = 0usize;
        // Newest first, so the window fills with the newest unpinned rows and
        // every pin is kept whatever its age.
        for row in rows {
            if row.is_pinned() {
                resident.push(row);
            } else {
                if unpinned_seen == MEMORY_WINDOW {
                    continue;
                }
                unpinned_seen += 1;
                resident.push(row);
            }
        }
        self.items = resident;
        self.prune()?;
        Ok(())
    }

    /// Record a copied text.
    ///
    /// Upstream `ClipboardStore.addText(_:sourceBundleID:)`: a copy of the
    /// text that is already the newest entry is a repeat of the same ⌘C, so
    /// it is not recorded twice.
    pub fn add_text(
        &mut self,
        text: &str,
        source_bundle_id: Option<&str>,
    ) -> Result<(), StoreError> {
        if let Some(newest) = self.items.first() {
            if newest.kind == ItemKind::Text && newest.text.as_deref() == Some(text) {
                return Ok(());
            }
        }
        let mut item = ClipboardItem::text(text, source_bundle_id.map(str::to_string));
        item.id = new_entry_id();
        self.insert(&item)?;
        Ok(())
    }

    /// Record copied files, one row each.
    ///
    /// Upstream `ClipboardStore.addFiles(_:sourceBundleID:)`: only a single
    /// file can be a ⌘C repeat, which is the case `addText` also guards. The
    /// batch prunes once rather than once per file.
    pub fn add_files(
        &mut self,
        paths: &[String],
        source_bundle_id: Option<&str>,
    ) -> Result<(), StoreError> {
        if paths.len() == 1 {
            if let Some(newest) = self.items.first() {
                if newest.kind == ItemKind::File
                    && newest.text.as_deref() == Some(paths[0].as_str())
                {
                    return Ok(());
                }
            }
        }
        for path in paths {
            let mut item = ClipboardItem::file(path.clone(), source_bundle_id.map(str::to_string));
            item.id = new_entry_id();
            self.insert_row(&item)?;
            self.items.insert(0, item);
            self.trim_window();
        }
        self.prune()?;
        Ok(())
    }

    /// Record a captured image.
    ///
    /// Upstream `ClipboardStore.addImage(_:sourceBundleID:capture:)` writes
    /// the blob itself; the blob write belongs to the capture layer, so this
    /// records the entry that points at an already-written blob.
    pub fn add_image(
        &mut self,
        image_path: &str,
        source_bundle_id: Option<&str>,
    ) -> Result<ClipboardItem, StoreError> {
        let mut item = ClipboardItem::image(image_path, source_bundle_id.map(str::to_string));
        item.id = new_entry_id();
        self.insert(&item)?;
        Ok(item)
    }

    /// Remove an entry.
    ///
    /// Upstream `ClipboardStore.remove(_:)`: a pin is a deliberate keep, so
    /// `remove` is the way to drop one.
    pub fn remove(&mut self, item: &ClipboardItem) -> Result<(), StoreError> {
        self.connection
            .execute("DELETE FROM items WHERE id = ?1", params![item.id])?;
        self.items.retain(|resident| resident.id != item.id);
        self.delete_blob(item);
        Ok(())
    }

    /// Clear the history, sparing every pin.
    ///
    /// Upstream `ClipboardStore.clearAll()`: a pin is a deliberate keep, so it
    /// outlives the bulk clear.
    pub fn clear_all(&mut self) -> Result<(), StoreError> {
        let stale: Vec<String> = {
            let mut statement = self
                .connection
                .prepare("SELECT image_path FROM items WHERE pinned_at IS NULL")?;
            let rows = statement.query_map([], |row| row.get::<_, Option<String>>(0))?;
            let mut paths = Vec::new();
            for row in rows {
                if let Some(path) = row? {
                    paths.push(path);
                }
            }
            paths
        };
        self.connection
            .execute("DELETE FROM items WHERE pinned_at IS NULL", [])?;
        for path in stale {
            self.delete_owned_blob(&path);
        }
        // Every pinned row is resident however old, so the window stays whole
        // without a reload.
        self.items.retain(|item| item.is_pinned());
        Ok(())
    }

    /// Pin an entry, or unpin it when it is already pinned.
    ///
    /// Upstream `ClipboardStore.togglePinned(_:)`: the row keeps its place and
    /// gains a stamp, which heads the Pinned section.
    pub fn toggle_pinned(&mut self, item: &ClipboardItem) -> Result<(), StoreError> {
        if item.is_pinned() {
            self.unpin(item)
        } else {
            self.pin(item)
        }
    }

    /// Pin an entry.
    fn pin(&mut self, item: &ClipboardItem) -> Result<(), StoreError> {
        let stamp = now_millis();
        self.connection.execute(
            "UPDATE items SET pinned_at = ?1 WHERE id = ?2",
            params![stamp, item.id],
        )?;
        match self
            .items
            .iter_mut()
            .find(|resident| resident.id == item.id)
        {
            Some(resident) => resident.pinned_at = Some(stamp),
            None => {
                // Pinned from a hit outside the window, so splice it in by
                // recency.
                let mut pinned = item.clone();
                pinned.pinned_at = Some(stamp);
                let index = self
                    .items
                    .iter()
                    .position(|resident| resident.created_at < pinned.created_at)
                    .unwrap_or(self.items.len());
                self.items.insert(index, pinned);
            }
        }
        Ok(())
    }

    /// Unpin an entry, which rejoins as the newest entry.
    ///
    /// Upstream `ClipboardStore.unpin(_:)`: the row is rewritten so it leads
    /// the history again.
    fn unpin(&mut self, item: &ClipboardItem) -> Result<(), StoreError> {
        self.reinsert(&item.with(Some(now_millis()), None))
    }

    /// Promote an entry to the head of the history.
    ///
    /// Upstream `ClipboardStore.promote(_:)`: a pinned row holds its place, so
    /// re-recencying it would rewrite for no change.
    pub fn promote(&mut self, item: &ClipboardItem) -> Result<(), StoreError> {
        if item.is_pinned() {
            return Ok(());
        }
        if self.items.first().map(|newest| newest.id.as_str()) == Some(item.id.as_str()) {
            return Ok(());
        }
        self.reinsert(&item.with(Some(now_millis()), None))
    }

    /// Rewrite an entry's text, keeping its id, its pin and its place.
    ///
    /// Upstream `ClipboardStore.updateText(_:of:)`: the edit is held to the
    /// same length capture records, and an entry that is not editable — an
    /// image, or a row that has since been deleted — yields `None`.
    pub fn update_text(
        &mut self,
        text: &str,
        item: &ClipboardItem,
    ) -> Result<Option<ClipboardItem>, UpdateError> {
        if item.kind != ItemKind::Text {
            return Ok(None);
        }
        if text.chars().count() > MAX_TEXT_LENGTH {
            return Err(UpdateError::TooLong);
        }
        let changed = self
            .connection
            .execute(
                "UPDATE items SET text = ?1 WHERE id = ?2",
                params![text, item.id],
            )
            .map_err(|_| UpdateError::WriteFailed)?;
        if changed == 0 {
            return Ok(None);
        }
        let current = self
            .items
            .iter()
            .find(|resident| resident.id == item.id)
            .cloned()
            .unwrap_or_else(|| item.clone());
        let edited = ClipboardItem {
            id: current.id,
            kind: ItemKind::Text,
            text: Some(text.to_string()),
            image_path: None,
            created_at: current.created_at,
            source_bundle_id: current.source_bundle_id,
            pinned_at: current.pinned_at,
        };
        match self
            .items
            .iter_mut()
            .find(|resident| resident.id == item.id)
        {
            Some(resident) => *resident = edited.clone(),
            None => {
                // A hit past the resident window: only the search sees the
                // change, so nothing is spliced into the window.
            }
        }
        Ok(Some(edited))
    }

    /// The entries that answer a query, under a filter.
    ///
    /// Upstream `ClipboardStore.search(_:filter:)`: display order is pinned
    /// entries first, each block newest-first, and the filter is applied
    /// after the split, so a matching pin stays in the Pinned section in pin
    /// order.
    pub fn search(&self, query: &str, filter: ClipboardFilter) -> Vec<ClipboardItem> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return filter.apply(&self.ordered_items());
        }
        let pinned = self.pinned_items();
        let matching_pins: Vec<ClipboardItem> = pinned
            .into_iter()
            .filter(|item| item.matches(trimmed))
            .collect();
        let ordinary = self.matching_unpinned(trimmed);
        let mut result = matching_pins;
        result.extend(ordinary);
        filter.apply(&result)
    }

    /// The row index of an entry as currently listed.
    ///
    /// Upstream `ClipboardStore.rowIndex(of:in:filter:)`.
    pub fn row_index(
        &self,
        item: &ClipboardItem,
        query: &str,
        filter: ClipboardFilter,
    ) -> Option<usize> {
        self.search(query, filter)
            .iter()
            .position(|listed| listed.id == item.id)
    }

    /// The nth visible pinned entry under a query and a filter, where 0 is the
    /// first pinned row.
    ///
    /// Upstream `ClipboardStore.pinnedItem(at:in:filter:)`.
    pub fn pinned_item(
        &self,
        index: usize,
        query: &str,
        filter: ClipboardFilter,
    ) -> Option<ClipboardItem> {
        self.search(query, filter)
            .iter()
            .take_while(|item| item.is_pinned())
            .nth(index)
            .cloned()
    }

    /// Where a reset lands: past the pins to the newest clip, or on the first
    /// match once typed.
    ///
    /// Upstream `ClipboardStore.landingIndex(in:filter:)`.
    pub fn landing_index(&self, query: &str, filter: ClipboardFilter) -> usize {
        if !query.trim().is_empty() {
            return 0;
        }
        self.search(query, filter)
            .iter()
            .position(|item| !item.is_pinned())
            .unwrap_or(0)
    }

    /// Import entries in bulk, with their original timestamps, deduplicated.
    ///
    /// Upstream `ClipboardStore.importEntries(_:)`: the oldest is inserted
    /// first, so the newest ends up with the highest row id and leads.
    pub fn import_entries(&mut self, entries: &[ClipboardItem]) -> Result<usize, StoreError> {
        let mut sorted: Vec<&ClipboardItem> = entries.iter().collect();
        sorted.sort_by_key(|item| item.created_at);
        let mut known = self.stored_keys()?;
        let mut inserted = 0usize;
        for entry in sorted {
            if !known.insert(entry_key(entry)) {
                continue;
            }
            self.insert_row(entry)?;
            inserted += 1;
        }
        self.load()?;
        Ok(inserted)
    }

    /// Every row in the database, oldest first.
    ///
    /// Upstream `ClipboardStore.forEachStoredItem(inDatabaseAt:_:)`: a backup
    /// reads the whole table, not the resident window, which stops at the
    /// memory limit.
    pub fn for_each_stored_item(
        &self,
        mut body: impl FnMut(&ClipboardItem),
    ) -> Result<(), StoreError> {
        for item in self.stored_rows_oldest_first()? {
            body(&item);
        }
        Ok(())
    }

    /// Run the retention cut now.
    ///
    /// Upstream `ClipboardStore.enforceLimits()`.
    pub fn enforce_limits(&mut self) -> Result<(), StoreError> {
        self.prune()
    }

    /// The entries in display order: pins in pin order, then the newest
    /// unpinned entries.
    ///
    /// Upstream `ClipboardStore.orderedItems`.
    fn ordered_items(&self) -> Vec<ClipboardItem> {
        let mut result = self.pinned_items();
        result.extend(self.items.iter().filter(|item| !item.is_pinned()).cloned());
        result
    }

    /// The Pinned section in pin order, so a new pin joins the end rather than
    /// the head.
    ///
    /// Upstream `ClipboardStore.pinnedItems`.
    fn pinned_items(&self) -> Vec<ClipboardItem> {
        let mut pinned: Vec<ClipboardItem> = self
            .items
            .iter()
            .filter(|item| item.is_pinned())
            .cloned()
            .collect();
        pinned.sort_by_key(|item| item.pinned_at.unwrap_or(i64::MAX));
        pinned
    }

    /// The unpinned entries that match a query, newest first.
    ///
    /// Upstream's FTS statement answers with the newest rows and stops at
    /// `searchLimit`; a query shorter than a trigram filters the resident
    /// window instead, because the index cannot answer it.
    fn matching_unpinned(&self, query: &str) -> Vec<ClipboardItem> {
        if query.chars().count() < 3 {
            return self
                .items
                .iter()
                .filter(|item| !item.is_pinned() && item.matches(query))
                .cloned()
                .collect();
        }
        let rows = match self.all_rows() {
            Ok(rows) => rows,
            Err(_) => return Vec::new(),
        };
        rows.into_iter()
            .filter(|item| item.matches(query))
            .take(SEARCH_LIMIT)
            .filter(|item| !item.is_pinned())
            .collect()
    }

    /// Insert an entry and put it at the head of the window.
    fn insert(&mut self, item: &ClipboardItem) -> Result<(), StoreError> {
        self.insert_row(item)?;
        self.items.insert(0, item.clone());
        self.trim_window();
        self.prune()?;
        Ok(())
    }

    /// Write an entry to the database.
    fn insert_row(&self, item: &ClipboardItem) -> Result<(), StoreError> {
        self.connection.execute(
            "INSERT INTO items(id, kind, text, image_path, created_at, source_app, pinned_at) \
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                item.id,
                item.kind.as_str(),
                item.text,
                item.image_path,
                item.created_at,
                item.source_bundle_id,
                item.pinned_at
            ],
        )?;
        Ok(())
    }

    /// Rewrite a row under the same id so it leads.
    ///
    /// Upstream `ClipboardStore.reinsert(_:)`: a delete would take the row's
    /// derived text with it, so the row is rewritten instead.
    fn reinsert(&mut self, updated: &ClipboardItem) -> Result<(), StoreError> {
        self.connection.execute(
            "UPDATE items SET rowid = (SELECT COALESCE(MAX(rowid), 0) + 1 FROM items), \
             created_at = ?1, pinned_at = ?2 WHERE id = ?3",
            params![updated.created_at, updated.pinned_at, updated.id],
        )?;
        self.items.retain(|resident| resident.id != updated.id);
        self.items.insert(0, updated.clone());
        self.trim_window();
        Ok(())
    }

    /// Cap the resident window, but never drop a pinned row.
    ///
    /// Upstream `ClipboardStore.trimWindow()`.
    fn trim_window(&mut self) {
        if self.items.len() <= MEMORY_WINDOW {
            return;
        }
        if let Some(index) = self.items.iter().rposition(|item| !item.is_pinned()) {
            self.items.remove(index);
        }
    }

    /// The retention cut.
    ///
    /// Upstream `ClipboardStore.prune()`: pinned rows are exempt, and only
    /// blobs this store owns are deleted.
    fn prune(&mut self) -> Result<(), StoreError> {
        let cutoff = now_millis().saturating_sub(self.max_age);
        let stale: Vec<String> = {
            let mut statement = self.connection.prepare(
                "SELECT image_path FROM items WHERE created_at < ?1 AND pinned_at IS NULL \
                 AND image_path IS NOT NULL",
            )?;
            let rows =
                statement.query_map(params![cutoff], |row| row.get::<_, Option<String>>(0))?;
            let mut paths = Vec::new();
            for row in rows {
                if let Some(path) = row? {
                    paths.push(path);
                }
            }
            paths
        };
        self.connection.execute(
            "DELETE FROM items WHERE created_at < ?1 AND pinned_at IS NULL",
            params![cutoff],
        )?;
        for path in stale {
            self.delete_owned_blob(&path);
        }
        self.items
            .retain(|item| item.is_pinned() || item.created_at >= cutoff);
        Ok(())
    }

    /// Whether a blob belongs to this store.
    ///
    /// Upstream `ClipboardStore.owns(_:)`: only files under the store's own
    /// images directory are ours to delete.
    pub fn owns_image(&self, item: &ClipboardItem) -> bool {
        match &item.image_path {
            Some(path) => self.owns(path),
            None => false,
        }
    }

    /// Whether a path is inside the store's images directory.
    fn owns(&self, path: &str) -> bool {
        match Path::new(path).strip_prefix(&self.images_dir) {
            Ok(rest) => !rest.as_os_str().is_empty(),
            Err(_) => false,
        }
    }

    /// Delete an entry's blob, if this store owns it.
    fn delete_blob(&self, item: &ClipboardItem) {
        if let Some(path) = &item.image_path {
            self.delete_owned_blob(path);
        }
    }

    /// Delete a blob this store owns.
    fn delete_owned_blob(&self, path: &str) {
        if !self.owns(path) {
            return;
        }
        let _ = std::fs::remove_file(path);
    }

    /// Create the schema.
    fn create_schema(&self) -> Result<(), StoreError> {
        self.connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS items(
               id TEXT NOT NULL UNIQUE,
               kind TEXT NOT NULL,
               text TEXT,
               image_path TEXT,
               created_at INTEGER NOT NULL,
               source_app TEXT,
               pinned_at INTEGER
             );
             CREATE INDEX IF NOT EXISTS items_created_at ON items(created_at);
             CREATE INDEX IF NOT EXISTS items_pinned_at ON items(pinned_at)
               WHERE pinned_at IS NOT NULL;",
        )?;
        Ok(())
    }

    /// Every row, newest first.
    fn all_rows(&self) -> Result<Vec<ClipboardItem>, StoreError> {
        let mut statement = self.connection.prepare(
            "SELECT id, kind, text, image_path, created_at, source_app, pinned_at \
             FROM items ORDER BY rowid DESC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<i64>>(6)?,
            ))
        })?;
        let mut items = Vec::new();
        for row in rows {
            let (id, kind, text, image_path, created_at, source_app, pinned_at) = row?;
            let kind = ItemKind::parse(&kind).ok_or(StoreError::WriteFailed)?;
            items.push(ClipboardItem {
                id,
                kind,
                text,
                image_path,
                created_at,
                source_bundle_id: source_app,
                pinned_at,
            });
        }
        Ok(items)
    }

    /// Every row, oldest first.
    fn stored_rows_oldest_first(&self) -> Result<Vec<ClipboardItem>, StoreError> {
        let mut statement = self.connection.prepare(
            "SELECT id, kind, text, image_path, created_at, source_app, pinned_at \
             FROM items ORDER BY rowid",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<i64>>(6)?,
            ))
        })?;
        let mut items = Vec::new();
        for row in rows {
            let (id, kind, text, image_path, created_at, source_app, pinned_at) = row?;
            let kind = ItemKind::parse(&kind).ok_or(StoreError::WriteFailed)?;
            items.push(ClipboardItem {
                id,
                kind,
                text,
                image_path,
                created_at,
                source_bundle_id: source_app,
                pinned_at,
            });
        }
        Ok(items)
    }

    /// The keys of the rows already stored, for deduplicating an import.
    ///
    /// Upstream `ClipboardStore.importKey(_:)`: keyed off the text for
    /// everything but an image, or every file entry hashes alike.
    fn stored_keys(&self) -> Result<std::collections::HashSet<String>, StoreError> {
        let mut keys = std::collections::HashSet::new();
        for item in self.stored_rows_oldest_first()? {
            keys.insert(entry_key(&item));
        }
        Ok(keys)
    }
}

/// The key an entry deduplicates on.
///
/// Upstream `ClipboardStore.importKey(_:)`: an image is keyed off its path,
/// everything else off its text.
fn entry_key(item: &ClipboardItem) -> String {
    let subject = if item.kind == ItemKind::Image {
        item.image_path.as_deref().unwrap_or("")
    } else {
        item.text.as_deref().unwrap_or("")
    };
    format!("{}|{}", item.kind.as_str(), subject)
}

/// A fresh identifier for an entry.
///
/// Upstream gives every entry a `UUID()`; this port's identifier is the same
/// shape, built from the clock and a counter so a test never collides.
pub fn new_entry_id() -> EntryId {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos() as u64)
        .unwrap_or(0);
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    let high = nanos.rotate_left(17) ^ count.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    let low = nanos ^ high.rotate_left(31);
    format!(
        "{:08x}-{:04x}-4{:03x}-8{:03x}-{:012x}",
        (high >> 32) as u32,
        (high >> 16) as u16,
        (high & 0x0fff) as u16,
        (low >> 48) as u16,
        low & 0xffff_ffff_ffff
    )
}

/// The current time, in milliseconds since the epoch.
///
/// The clock never answers the same millisecond twice: upstream's `Date()`
/// has a finer resolution than a millisecond, so two pins in the same
/// millisecond would otherwise be indistinguishable and their order — which
/// the Pinned section is built on — would be arbitrary.
pub fn now_millis() -> Timestamp {
    use std::sync::atomic::{AtomicI64, Ordering};
    static LAST: AtomicI64 = AtomicI64::new(0);
    let wall = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or(0);
    let mut previous = LAST.load(Ordering::Relaxed);
    loop {
        let next = wall.max(previous + 1);
        match LAST.compare_exchange_weak(previous, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return next,
            Err(current) => previous = current,
        }
    }
}
