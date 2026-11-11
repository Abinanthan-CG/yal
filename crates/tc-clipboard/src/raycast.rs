//! The Raycast clipboard export, mapped onto this port's entries.
//!
//! Upstream oracle: `Features/Clipboard/Model/RaycastClipboardImport.swift`,
//! and the expectations in `Tests/raycast-test.swift` (`clipboardMapping`,
//! `clipboardPinMetadata`).

use crate::item::{ClipboardItem, Timestamp};
use serde_json::Value;

/// What a Raycast clipboard export maps onto.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RaycastImport {
    /// The entries that mapped cleanly.
    pub items: Vec<ClipboardItem>,
    /// How many image entries pointed at a file that is no longer there.
    pub missing: usize,
}

/// Parse a Raycast clipboard export.
///
/// Upstream `RaycastClipboardImport.parse(_:now:fileExists:)`: the export is a
/// `clipboardEntries` array; each entry carries a `createdAt`, an optional
/// Boolean `pinned` flag, and a list of items whose representations are
/// flattened. A `text/plain` representation wins over an image one, and an
/// image whose file is gone is counted as missing rather than imported.
pub fn parse(
    value: &Value,
    now: impl Fn() -> Timestamp,
    file_exists: impl Fn(&str) -> bool,
) -> RaycastImport {
    let entries = match value.get("clipboardEntries").and_then(Value::as_array) {
        Some(entries) => entries,
        None => return RaycastImport::default(),
    };

    let mut items: Vec<ClipboardItem> = Vec::new();
    let mut missing = 0usize;

    for entry in entries {
        let created_at = entry
            .get("createdAt")
            .and_then(Value::as_str)
            .and_then(parse_iso8601)
            .unwrap_or_else(&now);
        // A Boolean pin uses the original creation date; anything else is no
        // pin at all.
        let pinned_at = match entry.get("pinned") {
            Some(Value::Bool(true)) => Some(created_at),
            _ => None,
        };

        let representations = flatten_representations(entry);

        if let Some(text) = first_text(&representations) {
            items.push(ClipboardItem {
                id: crate::store::new_entry_id(),
                kind: crate::item::ItemKind::Text,
                text: Some(text),
                image_path: None,
                created_at,
                source_bundle_id: None,
                pinned_at,
            });
            continue;
        }

        if let Some(path) = first_image_path(&representations) {
            if !file_exists(&path) {
                missing += 1;
                continue;
            }
            items.push(ClipboardItem {
                id: crate::store::new_entry_id(),
                kind: crate::item::ItemKind::Image,
                text: None,
                image_path: Some(path),
                created_at,
                source_bundle_id: None,
                pinned_at,
            });
        }
    }

    RaycastImport { items, missing }
}

/// The representations of an entry, flattened across its items.
fn flatten_representations(entry: &Value) -> Vec<&Value> {
    let items = match entry.get("items").and_then(Value::as_array) {
        Some(items) => items,
        None => return Vec::new(),
    };
    let mut out = Vec::new();
    for item in items {
        if let Some(representations) = item.get("representations").and_then(Value::as_array) {
            out.extend(representations.iter());
        }
    }
    out
}

/// The first `text/plain` representation's content, when it is there and not
/// empty.
///
/// Upstream takes the *first* `text/plain` representation and uses it, so an
/// empty one falls through to the image branch rather than to a later
/// representation.
fn first_text(representations: &[&Value]) -> Option<String> {
    let representation = representations.iter().find(|representation| {
        representation
            .get("mimeType")
            .and_then(Value::as_str)
            .map(|mime| mime.starts_with("text/plain"))
            .unwrap_or(false)
    })?;
    let content = representation.get("content").and_then(Value::as_str)?;
    if content.is_empty() {
        None
    } else {
        Some(content.to_string())
    }
}

/// The first `image/*` representation that names a URL, as its path.
fn first_image_path(representations: &[&Value]) -> Option<String> {
    let representation = representations.iter().find(|representation| {
        representation
            .get("mimeType")
            .and_then(Value::as_str)
            .map(|mime| mime.starts_with("image/"))
            .unwrap_or(false)
            && representation.get("contentType").and_then(Value::as_str) == Some("url")
    })?;
    representation
        .get("content")
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// Parse an ISO 8601 timestamp into milliseconds since the epoch.
///
/// Upstream tries the fractional-seconds formatter first and the plain one
/// second; both spellings of a valid timestamp are accepted here.
fn parse_iso8601(text: &str) -> Option<Timestamp> {
    match chrono::DateTime::parse_from_rfc3339(text) {
        Ok(parsed) => Some(parsed.timestamp_millis()),
        Err(_) => None,
    }
}
