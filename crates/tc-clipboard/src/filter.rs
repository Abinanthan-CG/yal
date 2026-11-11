//! The type filter that splits the history into its kinds.
//!
//! Upstream oracle: `Features/Clipboard/Model/ClipboardFilter.swift` (the
//! `ClipboardFilter` enum) and the store's `search(_:filter:)` which applies
//! it.

use crate::item::{ClipboardItem, TextForm};

/// Which kind of entry the history shows.
///
/// Upstream `ClipboardFilter`: `all`, `text`, `image`, `file`, `color`, `link`
/// and `email`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ClipboardFilter {
    /// Every entry, whatever it holds.
    #[default]
    All,
    /// Prose only: a link, an address and a colour are their own types.
    Text,
    /// Captured images only.
    Image,
    /// Referenced files only.
    File,
    /// Colours only, in whichever notation they were written.
    Color,
    /// Links only.
    Link,
    /// Email addresses only.
    Email,
}

impl ClipboardFilter {
    /// Every filter, in the order upstream's `CaseIterable` gives them.
    pub const ALL: [ClipboardFilter; 7] = [
        ClipboardFilter::All,
        ClipboardFilter::Text,
        ClipboardFilter::Image,
        ClipboardFilter::File,
        ClipboardFilter::Color,
        ClipboardFilter::Link,
        ClipboardFilter::Email,
    ];

    /// Whether the entry passes the filter.
    ///
    /// Upstream `ClipboardFilter.matches(_:)`: the kinds are exclusive, so a
    /// copied URL is a link rather than a narrower kind of text.
    pub fn matches(self, item: &ClipboardItem) -> bool {
        match self {
            ClipboardFilter::All => true,
            ClipboardFilter::Image => item.kind == crate::item::ItemKind::Image,
            ClipboardFilter::File => item.kind == crate::item::ItemKind::File,
            ClipboardFilter::Text => item.text_form() == Some(TextForm::Plain),
            ClipboardFilter::Color => item.text_form() == Some(TextForm::Color),
            ClipboardFilter::Link => item.text_form() == Some(TextForm::Link),
            ClipboardFilter::Email => item.text_form() == Some(TextForm::Email),
        }
    }

    /// The entries that pass the filter.
    ///
    /// Upstream `ClipboardFilter.apply(to:)`: the filter is applied *after* the
    /// pinned/unpinned split, so a matching pin stays in the Pinned section in
    /// pin order.
    pub fn apply(self, items: &[ClipboardItem]) -> Vec<ClipboardItem> {
        if self == ClipboardFilter::All {
            return items.to_vec();
        }
        items
            .iter()
            .filter(|item| self.matches(item))
            .cloned()
            .collect()
    }
}
