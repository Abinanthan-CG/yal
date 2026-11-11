//! The clipboard entry record and the rules that classify one.
//!
//! Upstream oracle: `Features/Clipboard/Model/ClipboardStore.swift` (the
//! `ClipboardItem` struct) and `Features/Clipboard/Model/ClipboardFilter.swift`
//! (the `ClipboardItem.TextForm` extension).

use crate::color::ColorValue;
use crate::file_kind::ClipboardFileKind;

/// The identifier of a stored entry, in the canonical UUID form upstream uses.
///
/// Upstream stores `UUID.uuidString` in the database's `id` column and parses
/// it back with `UUID(uuidString:)`, which is strict about the 8-4-4-4-12
/// shape and tolerant about case.
pub type EntryId = String;

/// Milliseconds since the Unix epoch.
///
/// Upstream stores `Date` values as `timeIntervalSince1970` seconds; this port
/// keeps the same ordering semantics in milliseconds so sub-second timestamps
/// (the Raycast import's fractional `createdAt`) survive a round trip.
pub type Timestamp = i64;

/// What kind of thing an entry holds.
///
/// Upstream `ClipboardItem.Kind` has exactly three cases: `text`, `image` and
/// `file`. A colour is *not* a kind upstream: a colour is a text entry whose
/// contents happen to parse as one, and it is found through
/// [`ClipboardItem::text_form`] rather than through the kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemKind {
    /// Copied text. A colour, a link and an address are all text entries.
    Text,
    /// A captured image, held as a blob under the store's images directory.
    Image,
    /// A referenced file, whose absolute path is held in `text`.
    File,
}

impl ItemKind {
    /// The spelling upstream writes into the database's `kind` column.
    pub fn as_str(self) -> &'static str {
        match self {
            ItemKind::Text => "text",
            ItemKind::Image => "image",
            ItemKind::File => "file",
        }
    }

    /// Read the stored spelling back, rejecting anything upstream's
    /// `ClipboardItem.Kind(rawValue:)` would reject.
    pub fn parse(value: &str) -> Option<ItemKind> {
        match value {
            "text" => Some(ItemKind::Text),
            "image" => Some(ItemKind::Image),
            "file" => Some(ItemKind::File),
            _ => None,
        }
    }
}

/// What a text entry holds.
///
/// Upstream `ClipboardItem.TextForm`: `Kind` says how an entry is stored,
/// this says what it is. Derived and never persisted, so reclassifying is a
/// code change rather than a migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextForm {
    /// Prose, or anything that is not one of the narrower forms.
    Plain,
    /// A colour literal, in any notation the parser accepts.
    Color,
    /// A URL or a bare domain.
    Link,
    /// An email address.
    Email,
}

/// One entry in the clipboard history.
///
/// Mirrors upstream `ClipboardItem`: the id, the kind, the text (or, for a
/// `.file` entry, the absolute path), the image blob path, the creation time,
/// the bundle ID of the app that was frontmost when the copy was captured, and
/// the pin stamp.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardItem {
    /// The entry's stable identifier.
    pub id: EntryId,
    /// How the entry is stored.
    pub kind: ItemKind,
    /// The copied text, or for a `File` entry the absolute path.
    pub text: Option<String>,
    /// Absolute path on disk; only files under the store's images directory
    /// are ours to delete.
    pub image_path: Option<String>,
    /// When the copy was captured.
    pub created_at: Timestamp,
    /// Bundle ID of the app frontmost when the copy was captured.
    pub source_bundle_id: Option<String>,
    /// When the entry was pinned. Pins lead the list and are exempt from pruning.
    pub pinned_at: Option<Timestamp>,
}

impl ClipboardItem {
    /// A new text entry, stamped now.
    pub fn text(text: impl Into<String>, source_bundle_id: Option<String>) -> ClipboardItem {
        ClipboardItem {
            id: crate::store::new_entry_id(),
            kind: ItemKind::Text,
            text: Some(text.into()),
            image_path: None,
            created_at: crate::store::now_millis(),
            source_bundle_id,
            pinned_at: None,
        }
    }

    /// A new image entry pointing at a blob under the store's images directory.
    pub fn image(image_path: impl Into<String>, source_bundle_id: Option<String>) -> ClipboardItem {
        ClipboardItem {
            id: crate::store::new_entry_id(),
            kind: ItemKind::Image,
            text: None,
            image_path: Some(image_path.into()),
            created_at: crate::store::now_millis(),
            source_bundle_id,
            pinned_at: None,
        }
    }

    /// A new file entry, referenced where it lies: `image_path` stays `None`,
    /// keeping an unowned file from ever being deleted.
    pub fn file(file_path: impl Into<String>, source_bundle_id: Option<String>) -> ClipboardItem {
        ClipboardItem {
            id: crate::store::new_entry_id(),
            kind: ItemKind::File,
            text: Some(file_path.into()),
            image_path: None,
            created_at: crate::store::now_millis(),
            source_bundle_id,
            pinned_at: None,
        }
    }

    /// Whether the entry is pinned. A pin is a deliberate keep, so it outlives
    /// the bulk clear and the retention cut.
    pub fn is_pinned(&self) -> bool {
        self.pinned_at.is_some()
    }

    /// The referenced path, so no call site re-derives a file entry's meaning
    /// from `text`.
    pub fn file_path(&self) -> Option<&str> {
        match self.kind {
            ItemKind::File => self.text.as_deref(),
            _ => None,
        }
    }

    /// What "Paste as Plain Text" writes: the text, or a file's path in place
    /// of the file. An image has no plain text.
    pub fn plain_text(&self) -> Option<&str> {
        match self.kind {
            ItemKind::Image => None,
            _ => self.text.as_deref(),
        }
    }

    /// Whether "Copy Text" applies: a captured image, or an image file copied
    /// in the file manager. Never text, never a PDF.
    pub fn offers_text_extraction(&self) -> bool {
        match self.kind {
            ItemKind::Image => self.image_path.is_some(),
            ItemKind::File => self
                .file_path()
                .map(|path| ClipboardFileKind::of(path, false) == ClipboardFileKind::Image)
                .unwrap_or(false),
            ItemKind::Text => false,
        }
    }

    /// The colour the entry holds, when it holds one.
    ///
    /// Upstream `ClipboardItem.colorValue`: only a text entry can be a colour,
    /// and the text must parse as one.
    pub fn color_value(&self) -> Option<ColorValue> {
        if self.kind != ItemKind::Text {
            return None;
        }
        ColorValue::parse(self.text.as_deref()?)
    }

    /// What the entry is, when it is text at all.
    ///
    /// Upstream `ClipboardItem.textForm`: derived, never persisted.
    pub fn text_form(&self) -> Option<TextForm> {
        if self.kind != ItemKind::Text {
            return None;
        }
        Some(classify(self.text.as_deref()?))
    }

    /// Case-insensitive substring match: how the store filters without FTS.
    ///
    /// Upstream `ClipboardItem.matches(_:)` uses
    /// `localizedCaseInsensitiveContains`, so the match folds case but not
    /// accents.
    pub fn matches(&self, query: &str) -> bool {
        match self.text.as_deref() {
            Some(text) => contains_case_insensitive(text, query),
            None => false,
        }
    }

    /// Copy with the two fields the store rewrites; the pin is always stated
    /// outright. Upstream `ClipboardItem.with(createdAt:pinnedAt:)`.
    pub fn with(
        &self,
        created_at: Option<Timestamp>,
        pinned_at: Option<Timestamp>,
    ) -> ClipboardItem {
        ClipboardItem {
            created_at: created_at.unwrap_or(self.created_at),
            pinned_at,
            ..self.clone()
        }
    }
}

/// Case-insensitive substring match, folding case but not accents.
///
/// This is the Rust counterpart of Swift's
/// `String.localizedCaseInsensitiveContains`, which upstream uses both for the
/// in-memory filter and as the expectation the FTS search is compared against.
pub fn contains_case_insensitive(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    haystack.to_lowercase().contains(&needle.to_lowercase())
}

/// A link or an address is one short token, so anything longer is prose by
/// definition. Upstream `ClipboardItem.detectionLimit`.
const DETECTION_LIMIT: usize = 2048;

/// A scheme-less link needs a TLD people actually copy, or every `report.pdf`
/// reads as one. Upstream `ClipboardItem.commonTopLevelDomains`.
const COMMON_TOP_LEVEL_DOMAINS: &[&str] = &[
    "com", "org", "net", "edu", "gov", "io", "co", "ai", "app", "dev", "me", "info", "biz", "xyz",
    "tv", "ly", "gg", "to", "uk", "us", "eu", "de", "fr", "es", "it", "nl", "se", "no", "fi", "dk",
    "ch", "at", "be", "ie", "cz", "ru", "ua", "tr", "cn", "jp", "kr", "hk", "sg", "au", "nz", "ca",
    "mx", "br", "ar", "za",
];

/// Classify a piece of text into the form it holds.
///
/// Upstream `ClipboardItem.classify(_:)`: a colour is its own form and beats
/// the prose branch even when written with spaces; a link or an address is one
/// short token.
pub fn classify(text: &str) -> TextForm {
    // The byte count is O(1); a grapheme walk across a multi-MB copy is not.
    if text.len() > DETECTION_LIMIT {
        return TextForm::Plain;
    }
    let token = text.trim();
    if token.is_empty() {
        return TextForm::Plain;
    }
    // Before the whitespace reject: `rgb(255, 87, 51)` is one value written
    // with spaces.
    if ColorValue::parse(token).is_some() {
        return TextForm::Color;
    }
    if token.chars().any(char::is_whitespace) {
        return TextForm::Plain;
    }
    if let Some(form) = scheme_form(token) {
        return form;
    }
    if is_address(token) {
        return TextForm::Email;
    }
    if is_bare_domain(token) {
        TextForm::Link
    } else {
        TextForm::Plain
    }
}

/// `mailto:` is an address; any other `scheme://` is a link, so `vscode://`
/// needs no allowlist. Upstream `ClipboardItem.schemeForm(of:)`.
fn scheme_form(token: &str) -> Option<TextForm> {
    if token.len() >= 7 && token[..7].eq_ignore_ascii_case("mailto:") {
        return Some(TextForm::Email);
    }
    let separator = token.find("://")?;
    let scheme = &token[..separator];
    if scheme.is_empty() {
        return None;
    }
    let valid = scheme
        .chars()
        .all(|c| c.is_ascii() && (c.is_ascii_alphabetic() || c == '+' || c == '-' || c == '.'));
    if !valid {
        return None;
    }
    Some(TextForm::Link)
}

/// `local@domain.tld`: one `@`, a non-empty local part, and a host-shaped
/// domain. Upstream `ClipboardItem.isAddress(_:)`.
fn is_address(token: &str) -> bool {
    let mut parts = token.split('@');
    match parts.next() {
        Some(local) if !local.is_empty() => {}
        _ => return false,
    }
    let domain = match parts.next() {
        Some(domain) => domain,
        None => return false,
    };
    if parts.next().is_some() {
        return false;
    }
    top_level_label(domain).is_some()
}

/// A heuristic, and deliberately so: its worst case files a row under the
/// wrong type. Upstream `ClipboardItem.isBareDomain(_:)`.
fn is_bare_domain(token: &str) -> bool {
    let host: &str = token.split(|c| "/?#:".contains(c)).next().unwrap_or("");
    if host.len() >= 4 && host[..4].eq_ignore_ascii_case("www.") {
        return true;
    }
    // Hosts are canonically lower case, which is what keeps `Safari.app` out
    // of the links.
    if host.chars().any(char::is_uppercase) {
        return false;
    }
    match top_level_label(host) {
        Some(label) => COMMON_TOP_LEVEL_DOMAINS.contains(&label),
        None => false,
    }
}

/// `None` unless every label is one, which rejects `@apple.com` before it
/// reads as a domain. Upstream `ClipboardItem.topLevelLabel(of:)`.
fn top_level_label(host: &str) -> Option<&str> {
    let labels: Vec<&str> = host.split('.').collect();
    if labels.len() < 2 {
        return None;
    }
    if !labels.iter().all(|label| is_host_label(label)) {
        return None;
    }
    let tld = labels[labels.len() - 1];
    if tld.len() < 2 || !tld.chars().all(|c| c.is_ascii() && c.is_ascii_alphabetic()) {
        return None;
    }
    Some(tld)
}

/// Upstream `ClipboardItem.isHostLabel(_:)`.
fn is_host_label(label: &str) -> bool {
    !label.is_empty()
        && label
            .chars()
            .all(|c| c.is_ascii() && (c.is_ascii_alphanumeric() || c == '-'))
}
