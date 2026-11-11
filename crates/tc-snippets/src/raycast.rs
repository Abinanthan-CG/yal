//! Importing Raycast's snippet export.
//!
//! Mirrors upstream `Features/Snippets/Model/RaycastSnippetImport.swift`:
//! Raycast writes each snippet as an object with a `title`, a `text` and an
//! optional `keyword`, and Tinycast reads the same three fields.

use serde_json::Value;

use crate::snippet::Snippet;

/// Reads Raycast's snippet export.
///
/// Mirrors upstream's `RaycastSnippetImport`.
pub struct RaycastSnippetImport;

impl RaycastSnippetImport {
    /// Read the snippets out of a Raycast export.
    ///
    /// Mirrors upstream's `parse(_:)`: only an array of objects is a
    /// container, and an entry without a title or a text is skipped.
    pub fn parse(value: &Value) -> Vec<Snippet> {
        let Some(entries) = value.as_array() else {
            return Vec::new();
        };

        entries
            .iter()
            .filter_map(|entry| Self::parse_entry(entry))
            .collect()
    }

    /// Read one entry of a Raycast export.
    fn parse_entry(entry: &Value) -> Option<Snippet> {
        let object = entry.as_object()?;
        let name = object.get("title")?.as_str()?;
        if name.trim_matches(|c: char| c.is_whitespace()).is_empty() {
            return None;
        }
        let text = object.get("text")?.as_str()?;

        let keyword = object
            .get("keyword")
            .and_then(|keyword| keyword.as_str())
            .map(|keyword| keyword.trim_matches(|c: char| c.is_whitespace()).to_string())
            .filter(|keyword| !keyword.is_empty());

        Some(Snippet::with_options(
            name.to_string(),
            text.to_string(),
            keyword,
            true,
            false,
        ))
    }
}
