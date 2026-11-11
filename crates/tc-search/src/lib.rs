//! `tc-search` - how YAL finds things.
//!
//! The launcher's search is the heart of the product: one query has to find
//! the application, the file, the setting, the snippet or the window the user
//! meant, and order them the way the user expects.
//!
//! The parts, each ported from upstream:
//!
//! - [`fuzzy`] - the fold and the tiered match every search shares.
//! - [`matcher`] - the scorer, where a word start outscores any other letter.
//! - [`relevance`] - which alias matched, and how well.
//! - [`ranking`] - what the user opens, and by which query.
//! - [`romanization`] - how a name in another script is read.
//! - [`entry_naming`] - which names an entry is found by at all.
//! - [`order`] - which entry comes first.
//!
//! Nothing here touches Windows, so every rule is testable without a desktop.

pub mod entry_naming;
pub mod fuzzy;
pub mod matcher;
pub mod order;
pub mod ranking;
pub mod relevance;
pub mod romanization;

pub use entry_naming::{EntryNaming, SearchProfile, Sources};
pub use fuzzy::{Candidate, Match, Query, Tier};
pub use matcher::{Outcome, SearchSensitivity, SearchText};
pub use order::{CategoryQuery, LauncherOrder, Query as LauncherQuery, Rankable, Signals};
pub use ranking::{LauncherRankingStore, LauncherUsage, LauncherVisit};
pub use relevance::{Role, SearchAlias, SearchFields, SearchRelevance};
pub use romanization::Script;
