//! Watching the clipboard for new entries.
//!
//! The watcher polls the clipboard on an interval. Polling is used rather
//! than a clipboard-change notification because the notification requires a
//! window handle to be owned, which would tie the watcher to the palette
//! window's lifetime.
//!
//! The watcher owns its own database connection. SQLite connections are not
//! shared across threads, so the watcher opens a second connection to the
//! same file rather than borrowing the palette's.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::clipboard::Clipboard;
use yal_store::{ClipboardEntry, ClipboardKind, Store};

/// How often the clipboard is polled.
pub const DEFAULT_INTERVAL: Duration = Duration::from_millis(400);

/// How many clipboard entries are kept.
pub const DEFAULT_HISTORY: usize = 500;

/// A running clipboard watcher.
pub struct ClipboardWatcher {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl ClipboardWatcher {
    /// Start watching the clipboard.
    ///
    /// `database` is the store's path; the watcher opens its own connection
    /// to it. `on_entry` is called for each new entry, on the watcher's
    /// thread.
    pub fn start<F>(
        database: Option<PathBuf>,
        interval: Duration,
        history: usize,
        on_entry: F,
    ) -> Option<Self>
    where
        F: Fn(ClipboardEntry) + Send + 'static,
    {
        let path = database?;
        let store = Store::open(&path).ok()?;
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();

        let handle = std::thread::Builder::new()
            .name("clipboard-watcher".into())
            .spawn(move || {
                let mut clipboard = match Clipboard::new() {
                    Ok(clipboard) => clipboard,
                    Err(_) => return,
                };

                // The first reading is the baseline, not a new entry.
                let mut last = clipboard.text();

                while !flag.load(Ordering::Relaxed) {
                    std::thread::sleep(interval);

                    let current = clipboard.text();
                    if current == last {
                        continue;
                    }
                    last = current.clone();

                    let Some(text) = current else { continue };
                    let trimmed = text.trim();
                    if trimmed.is_empty() || trimmed.len() > 100_000 {
                        continue;
                    }

                    let entry = ClipboardEntry {
                        id: 0,
                        kind: classify(&text),
                        text,
                        source_app: None,
                        created_at: chrono::Utc::now(),
                    };

                    if store.push_clipboard(&entry, history).is_ok() {
                        on_entry(entry);
                    }
                }
            })
            .ok()?;

        Some(Self {
            stop,
            handle: Some(handle),
        })
    }

    /// Stop watching.
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for ClipboardWatcher {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Classify clipboard content.
fn classify(text: &str) -> ClipboardKind {
    let trimmed = text.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return ClipboardKind::Url;
    }
    if trimmed.contains('\n') && trimmed.lines().count() > 1 {
        return ClipboardKind::Text;
    }
    ClipboardKind::Text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_are_classified() {
        assert_eq!(classify("https://example.com"), ClipboardKind::Url);
        assert_eq!(classify("  http://example.com  "), ClipboardKind::Url);
    }

    #[test]
    fn plain_text_is_classified() {
        assert_eq!(classify("hello"), ClipboardKind::Text);
        assert_eq!(classify("two\nlines"), ClipboardKind::Text);
    }
}
