//! The emoji and symbol picker: the catalog, the search and the stores.
//!
//! Mirrors upstream `Tinycast/Features/Emoji/`. The catalog is upstream's whole
//! table, the search is upstream's ranking, and the pins and the frequency are
//! upstream's two stores.
//!
//! # Example
//!
//! ```
//! use tc_emoji::EmojiIndex;
//!
//! let index = EmojiIndex::load();
//! let results = index.search("pray", &tc_emoji::FrequentEmojiStore::in_memory(), 5);
//! assert!(results.iter().any(|entry| entry.glyph == "🙏"));
//! ```

pub mod catalog;
pub mod data;
pub mod geometry;
pub mod index;
pub mod stores;

#[cfg(test)]
mod tests;

pub use catalog::{
    apply_tone, EmojiCatalog, EmojiCategory, EmojiCategoryFilter, EmojiEntry, EmojiGridColumns,
    EmojiGridZoom, EmojiSkinTone,
};
pub use geometry::EmojiGridGeometry;
pub use index::EmojiIndex;
pub use stores::{FrequentEmoji, FrequentEmojiStore, PinnedEmojiStore};
