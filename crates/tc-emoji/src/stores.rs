//! The pinned characters and the ones used most.
//!
//! Mirrors upstream `Tinycast/Features/Emoji/Service/PinnedEmojiStore.swift` and
//! `FrequentEmojiStore.swift`.
//!
//! The pins are the user's own order and the frequency is what the user did; the
//! two are kept apart so neither can rewrite the other.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// How many pins the store keeps.
///
/// The catalog is smaller than this; the cap only bounds a malformed or a
/// hand-edited file.
const PIN_CAP: usize = 3_000;

/// How many tallies the history keeps.
const FREQUENCY_CAP: usize = 300;

/// The user's own favourites, in the order they pinned them.
#[derive(Debug, Clone)]
pub struct PinnedEmojiStore {
    path: PathBuf,
    glyphs: Vec<String>,
    revision: u64,
    persistence_failure: bool,
}

impl PinnedEmojiStore {
    /// Opens the store, reading the pins it holds.
    pub fn open(path: impl AsRef<Path>) -> PinnedEmojiStore {
        let path = path.as_ref().to_path_buf();
        let glyphs = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Vec<String>>(&bytes).ok())
            .map(|glyphs| normalized(&glyphs))
            .unwrap_or_default();
        PinnedEmojiStore {
            path,
            glyphs,
            revision: 0,
            persistence_failure: false,
        }
    }

    /// The pins, in order.
    pub fn glyphs(&self) -> &[String] {
        &self.glyphs
    }

    /// The revision, which changes with every write.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Whether the last write failed.
    pub fn persistence_failure(&self) -> bool {
        self.persistence_failure
    }

    /// Pins a character, or unpins one that is already pinned.
    pub fn toggle(&mut self, glyph: &str) {
        if glyph.is_empty() {
            return;
        }
        match self.glyphs.iter().position(|pinned| pinned == glyph) {
            Some(index) => {
                self.glyphs.remove(index);
            }
            None => self.glyphs.push(glyph.to_string()),
        }
        self.did_change();
    }

    /// Restores authored pins from a backup, keeping the order and dropping the
    /// invalid duplicates.
    pub fn replace(&mut self, imported: Vec<String>) {
        self.glyphs = normalized(&imported);
        self.did_change();
    }

    /// Swaps two pins.
    ///
    /// The caller names the neighbour: a stored pin the catalog cannot show is
    /// not one.
    pub fn swap(&mut self, glyph: &str, other: &str) {
        let (Some(source), Some(destination)) = (
            self.glyphs.iter().position(|pinned| pinned == glyph),
            self.glyphs.iter().position(|pinned| pinned == other),
        ) else {
            return;
        };
        self.glyphs.swap(source, destination);
        self.did_change();
    }

    /// Writes the pins, and remembers that the write failed.
    fn did_change(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        let Ok(data) = serde_json::to_vec(&self.glyphs) else {
            self.persistence_failure = true;
            return;
        };
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        self.persistence_failure = std::fs::write(&self.path, data).is_err();
    }
}

/// The pins with the blanks and the duplicates dropped.
fn normalized(glyphs: &[String]) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    let mut result: Vec<String> = Vec::with_capacity(glyphs.len().min(PIN_CAP));
    for glyph in glyphs {
        if glyph.is_empty() || seen.contains(glyph) {
            continue;
        }
        seen.push(glyph.clone());
        result.push(glyph.clone());
        if result.len() == PIN_CAP {
            break;
        }
    }
    result
}

/// One character's usage tally, keyed on the base, untoned glyph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrequentEmoji {
    /// The character.
    pub glyph: String,
    /// How many times it was used.
    pub count: u32,
    /// When it was last used, in milliseconds since the epoch.
    pub last_used: i64,
}

/// The capped history and the usage counts, kept together.
#[derive(Debug, Clone)]
pub struct FrequentEmojiStore {
    path: PathBuf,
    records: Vec<FrequentEmoji>,
    revision: u64,
}

impl FrequentEmojiStore {
    /// Opens the store, reading the history it holds.
    pub fn open(path: impl AsRef<Path>) -> FrequentEmojiStore {
        let path = path.as_ref().to_path_buf();
        let records = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Vec<FrequentEmoji>>(&bytes).ok())
            .map(|records| history(&records))
            .unwrap_or_default();
        FrequentEmojiStore {
            path,
            records,
            revision: 0,
        }
    }

    /// The history, newest first.
    pub fn records(&self) -> &[FrequentEmoji] {
        &self.records
    }

    /// The revision, which changes with every write.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// A store that keeps its history in memory only.
    pub fn in_memory() -> FrequentEmojiStore {
        FrequentEmojiStore {
            path: PathBuf::new(),
            records: Vec::new(),
            revision: 0,
        }
    }

    /// Records a use, moving the character to the front.
    pub fn record(&mut self, glyph: &str) {
        self.record_at(glyph, now_millis());
    }

    /// Records a use at a given moment, for a test that needs the clock.
    pub fn record_at(&mut self, glyph: &str, at: i64) {
        self.revision = self.revision.wrapping_add(1);
        match self.records.iter().position(|record| record.glyph == glyph) {
            Some(index) => {
                let mut entry = self.records.remove(index);
                entry.count = entry.count.saturating_add(1);
                entry.last_used = at;
                self.records.insert(0, entry);
            }
            None => self.records.insert(
                0,
                FrequentEmoji {
                    glyph: glyph.to_string(),
                    count: 1,
                    last_used: at,
                },
            ),
        }
        if self.records.len() > FREQUENCY_CAP {
            let extra = self.records.len() - FREQUENCY_CAP;
            self.records.truncate(self.records.len() - extra);
        }
        self.persist();
    }

    /// Restores a history from a backup.
    pub fn replace(&mut self, imported: Vec<FrequentEmoji>) {
        self.revision = self.revision.wrapping_add(1);
        self.records = history(&imported);
        self.persist();
    }

    /// The most-used characters, with the recency breaking a tie.
    pub fn top(&self, n: usize) -> Vec<String> {
        let mut sorted = self.records.clone();
        sorted.sort_by(|left, right| {
            right
                .count
                .cmp(&left.count)
                .then_with(|| right.last_used.cmp(&left.last_used))
        });
        sorted
            .into_iter()
            .take(n)
            .map(|record| record.glyph)
            .collect()
    }

    /// Writes the history.
    fn persist(&self) {
        let Ok(data) = serde_json::to_vec(&self.records) else {
            return;
        };
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&self.path, data);
    }
}

/// A history with the repeated glyphs dropped and the newest tally kept.
fn history(tallies: &[FrequentEmoji]) -> Vec<FrequentEmoji> {
    let mut sorted: Vec<FrequentEmoji> = tallies
        .iter()
        .filter(|tally| !tally.glyph.is_empty() && tally.count > 0)
        .cloned()
        .collect();
    sorted.sort_by_key(|tally| std::cmp::Reverse(tally.last_used));
    let mut seen: Vec<String> = Vec::new();
    let mut result: Vec<FrequentEmoji> = Vec::new();
    for tally in sorted {
        if seen.contains(&tally.glyph) {
            continue;
        }
        seen.push(tally.glyph.clone());
        result.push(tally);
        if result.len() == FREQUENCY_CAP {
            break;
        }
    }
    result
}

/// The clock the history reads, in milliseconds since the epoch.
pub fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_millis() as i64)
        .unwrap_or(0)
}
