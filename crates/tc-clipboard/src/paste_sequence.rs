//! The walk down the history that "Paste Sequentially" performs.
//!
//! Upstream oracle: `Features/Clipboard/Model/PasteSequence.swift`.

use crate::item::{ClipboardItem, EntryId, Timestamp};

/// How long the walk waits for the next press before it ends.
///
/// Upstream `PasteSequence.idleTimeout`: 60 seconds.
pub const IDLE_TIMEOUT_MILLIS: i64 = 60_000;

/// How long after a paste the next press is held back.
///
/// Upstream `PasteSequence.settleInterval`: 0.25 seconds. It outlasts the
/// post-delay of the paste chord and the target's read, so the next write
/// cannot beat that read.
pub const SETTLE_INTERVAL_MILLIS: i64 = 250;

/// The walk down history, newest first, frozen when the walk begins.
///
/// Upstream `PasteSequence`: ids rather than items, so an entry deleted
/// mid-walk is skipped instead of pasted.
#[derive(Debug, Clone)]
pub struct PasteSequence {
    entry_ids: Vec<EntryId>,
    next_index: usize,
    /// The pasteboard as our last write left it; any other count is a copy
    /// made since.
    change_count: u64,
    last_paste: Timestamp,
}

impl PasteSequence {
    /// Begin a walk over the history as it stands now.
    ///
    /// Upstream `PasteSequence.init(history:changeCount:now:)`.
    pub fn new(history: &[ClipboardItem], change_count: u64, now: Timestamp) -> PasteSequence {
        PasteSequence {
            entry_ids: history.iter().map(|item| item.id.clone()).collect(),
            next_index: 0,
            change_count,
            last_paste: now,
        }
    }

    /// Whether the walk continues: the pasteboard has not changed by a copy
    /// made since, and the idle timeout has not passed.
    ///
    /// Upstream `PasteSequence.continues(changeCount:at:)`.
    pub fn continues(&self, change_count: u64, now: Timestamp) -> bool {
        change_count == self.change_count && now - self.last_paste < IDLE_TIMEOUT_MILLIS
    }

    /// Whether a press is held back, because the last paste may not have
    /// landed yet.
    ///
    /// Upstream `PasteSequence.isSettling(at:)`.
    pub fn is_settling(&self, now: Timestamp) -> bool {
        now - self.last_paste < SETTLE_INTERVAL_MILLIS
    }

    /// The next entry to paste.
    ///
    /// Upstream `PasteSequence.next(in:)`: `None` at the end, where the walk
    /// stops rather than wrapping back to the newest entry. An entry deleted
    /// mid-walk is skipped.
    pub fn next(&mut self, history: &[ClipboardItem]) -> Option<ClipboardItem> {
        while self.next_index < self.entry_ids.len() {
            let id = self.entry_ids[self.next_index].clone();
            self.next_index += 1;
            // Newest first on both sides, so the scan stops about as deep as
            // the walk has gone.
            if let Some(item) = history.iter().find(|item| item.id == id) {
                return Some(item.clone());
            }
        }
        None
    }

    /// Record our own write.
    ///
    /// Upstream `PasteSequence.recordPaste(changeCount:at:)`: our own write
    /// moves the change count, so the walk adopts it rather than read it as a
    /// new copy.
    pub fn record_paste(&mut self, change_count: u64, now: Timestamp) {
        self.change_count = change_count;
        self.last_paste = now;
    }
}
