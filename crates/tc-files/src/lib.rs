//! `tc-files` - YAL port of Tinycast's file search.
//!
//! The private index behind the Search Files command: the folders the user
//! picks, the rules that keep things out of them, the pass that keeps the
//! index current, the store it lives in, and the search over it.
//!
//! Upstream's macOS build uses Spotlight's neighbour - a crawl of the chosen
//! folders, kept current by the file system's own change notifications. On
//! Windows there is nothing like that, so this port reads the folders
//! directly and keeps the index fresh the way the donor project in
//! `C:\Users\dabin\development\yal` does (GPL-3.0-only): a pass re-lists only
//! the directories whose modification time moved.
//!
//! **No file's contents are ever read.** The index holds the name, where the
//! thing is, when it changed, how big it is and whether it is a folder - and
//! nothing else.
//!
//! See `docs/ARCHITECTURE.md` for the crate map.

pub mod index;
pub mod scope;
pub mod search;
pub mod store;

pub use index::{dir_mtime_ns, list_directory, walk_once, Backing, Entry, WalkReport};
pub use scope::{
    expanded_path, stored_path, IgnorePattern, IgnorePatternKind, IgnoreRules, Scope, ScopeList,
};
pub use search::{
    is_hidden, is_own_file, is_program_area, matches_words, query_words, recent, search, MatchTier,
    RECENT_LIMIT, RESULT_LIMIT,
};
pub use store::{IndexStore, StoreError};
