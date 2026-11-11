//! The privacy rules that decide what is recorded at all.
//!
//! Upstream oracle: the capture rules upstream applies in
//! `Features/Clipboard/Service/ClipboardManager.swift` (`poll`, `sensitiveTypes`,
//! `maxCapturedFiles`, `isDurable`) and the default excluded applications in
//! `Features/Settings/AppSettings.swift`.
//!
//! The Windows clipboard watching itself lives in another crate; this module
//! holds only the *rules*, so they can be tested without a desktop.

/// The bundle IDs upstream excludes by default.
///
/// Upstream `AppSettings.clipboardDisabledApps`'s default: password managers
/// ship excluded, until the user first edits the list.
pub const DEFAULT_DISABLED_APPS: &[&str] = &["com.apple.keychainaccess", "com.apple.Passwords"];

/// The longest text the history holds; a longer copy is skipped, since
/// truncation would lose the tail.
///
/// Upstream `ClipboardStore.maxTextLength`.
pub const MAX_TEXT_LENGTH: usize = 32_000;

/// The most files one capture may bring in.
///
/// Upstream `ClipboardManager.maxCapturedFiles`: a finder's select-all must not
/// insert ten thousand rows on one poll tick.
pub const MAX_CAPTURED_FILES: usize = 32;

/// The rules that decide whether a captured thing is recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturePolicy {
    /// The bundle IDs whose copies are never recorded.
    disabled_apps: Vec<String>,
}

impl Default for CapturePolicy {
    fn default() -> CapturePolicy {
        CapturePolicy {
            disabled_apps: DEFAULT_DISABLED_APPS
                .iter()
                .map(|bundle_id| (*bundle_id).to_string())
                .collect(),
        }
    }
}

impl CapturePolicy {
    /// A policy that excludes nothing.
    pub fn permissive() -> CapturePolicy {
        CapturePolicy {
            disabled_apps: Vec::new(),
        }
    }

    /// A policy with the given bundle IDs excluded.
    pub fn with_disabled_apps(bundle_ids: &[String]) -> CapturePolicy {
        CapturePolicy {
            disabled_apps: bundle_ids.to_vec(),
        }
    }

    /// The bundle IDs this policy excludes.
    pub fn disabled_apps(&self) -> &[String] {
        &self.disabled_apps
    }

    /// Whether a copy from the given source is recorded.
    ///
    /// Upstream `ClipboardManager.poll`: the pasteboard carries no source, so
    /// the copy is attributed to the frontmost app, and a copy from an
    /// excluded app is dropped.
    pub fn allows_source(&self, source_bundle_id: Option<&str>) -> bool {
        match source_bundle_id {
            Some(bundle_id) => !self.disabled_apps.iter().any(|id| id == bundle_id),
            None => true,
        }
    }

    /// Whether text of this length is recorded.
    ///
    /// Upstream `ClipboardManager.poll` and `ClipboardStore.updateText(_:of:)`
    /// both hold the copy to `maxTextLength`.
    pub fn allows_text_length(&self, length: usize) -> bool {
        length <= MAX_TEXT_LENGTH
    }

    /// Whether a capture of this many files is recorded.
    ///
    /// Upstream `ClipboardManager.poll` holds the batch to `maxCapturedFiles`.
    pub fn allows_file_count(&self, count: usize) -> bool {
        count <= MAX_CAPTURED_FILES
    }
}
