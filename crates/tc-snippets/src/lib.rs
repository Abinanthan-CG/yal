//! `tc-snippets` — the snippet model layer of the YAL port.
//!
//! This crate holds the parts of the snippet feature that are pure: what a
//! snippet is, the Markdown file it is stored in, the keyword that triggers it,
//! and the locale table the date tokens format with.
//!
//! Nothing here talks to Windows. The snippets live as Markdown files under a
//! root folder that the caller passes in, so every rule in this crate is
//! testable without a desktop and without touching the user's real
//! `%APPDATA%`.
//!
//! The upstream authority for everything here is the macOS app Tinycast
//! (<https://github.com/abue-ammar/tinycast>, AGPL-3.0): the Swift files in
//! `Features/Snippets/Model` and the tests in `Tests/snippets-test.swift`.

pub mod keyword;
#[cfg(test)]
mod library_tests;
pub mod locale;
pub mod markdown;
pub mod raycast;
pub mod repository;
pub mod snippet;
pub mod store;
pub mod template;
#[cfg(test)]
mod template_tests;
#[cfg(test)]
mod tests;

pub use keyword::SnippetKeywordPolicy;
pub use locale::{for_identifier, LocaleData};
pub use markdown::{ParseError, SnippetMarkdownSerializer};
pub use raycast::RaycastSnippetImport;
pub use repository::{Issue, RepositoryError, SnippetRepository};
pub use snippet::{Snippet, SnippetSourceRevision, StoredSnippet};
pub use store::SnippetsStore;
pub use template::{ExpansionContext, MissingArgument};
