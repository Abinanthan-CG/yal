//! Records stored in the local database.

use chrono::{DateTime, Utc};
use yal_search::Frecency;

/// A saved text snippet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snippet {
    pub id: String,
    pub name: String,
    /// The word that expands this snippet.
    pub keyword: String,
    /// The text the snippet expands to.
    pub expansion: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A saved link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quicklink {
    pub id: String,
    pub name: String,
    pub url: String,
    pub created_at: DateTime<Utc>,
}

/// A saved bookmark.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bookmark {
    pub id: String,
    pub name: String,
    pub url: String,
    pub created_at: DateTime<Utc>,
}

/// A user-defined search alias, mapping a term onto a command or application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alias {
    pub id: String,
    pub term: String,
    pub target: String,
    pub created_at: DateTime<Utc>,
}

/// The kind of clipboard content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardKind {
    Text,
    Url,
    Image,
    File,
}

impl ClipboardKind {
    /// The stable name this kind is stored under.
    pub fn as_str(self) -> &'static str {
        match self {
            ClipboardKind::Text => "text",
            ClipboardKind::Url => "url",
            ClipboardKind::Image => "image",
            ClipboardKind::File => "file",
        }
    }
}

impl std::str::FromStr for ClipboardKind {
    type Err = std::convert::Infallible;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(match value {
            "url" => ClipboardKind::Url,
            "image" => ClipboardKind::Image,
            "file" => ClipboardKind::File,
            _ => ClipboardKind::Text,
        })
    }
}

/// A clipboard history entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardEntry {
    pub id: i64,
    pub text: String,
    pub kind: ClipboardKind,
    pub source_app: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// An indexed application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedApplication {
    pub id: String,
    pub name: String,
    pub path: String,
    /// Where the entry was discovered: `start_menu`, `msix`, `registry`, `path`.
    pub source: String,
    pub keywords: String,
    pub last_indexed: DateTime<Utc>,
}

/// A usage record, used to derive frecency.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageRecord {
    pub key: String,
    pub count: u32,
    pub last_used: DateTime<Utc>,
}

impl UsageRecord {
    /// Convert to the frecency representation used by the search layer.
    pub fn frecency(&self) -> Frecency {
        Frecency::new(self.count, Some(self.last_used))
    }
}
