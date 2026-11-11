//! The snippet record: what a snippet is, and how it is identified on disk.
//!
//! Mirrors upstream `Features/Snippets/Model/Snippet.swift`.

use std::path::{Path, PathBuf};

/// A snippet's editable content.
///
/// Mirrors upstream `Snippet`: a display name, the text that is pasted, an
/// optional keyword that triggers it, whether the snippet may be used at all,
/// and whether it asks for confirmation before it runs.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Snippet {
    /// The label shown in the snippet list.
    pub name: String,
    /// The template that is expanded and pasted.
    pub text: String,
    /// The optional trigger keyword, without its leading punctuation.
    pub keyword: Option<String>,
    /// Whether the snippet may be expanded at all.
    pub is_enabled: bool,
    /// Whether the snippet asks before it is pasted.
    pub shows_confirmation: bool,
}

impl Default for Snippet {
    /// A new snippet is enabled and does not ask for confirmation.
    ///
    /// Mirrors upstream's initializer defaults (`keyword: nil`,
    /// `isEnabled: true`, `showsConfirmation: false`).
    fn default() -> Self {
        Self {
            name: String::new(),
            text: String::new(),
            keyword: None,
            is_enabled: true,
            shows_confirmation: false,
        }
    }
}

impl Snippet {
    /// A snippet with upstream's default options.
    pub fn new(name: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            text: text.into(),
            keyword: None,
            is_enabled: true,
            shows_confirmation: false,
        }
    }

    /// A snippet with every option spelled out.
    pub fn with_options(
        name: impl Into<String>,
        text: impl Into<String>,
        keyword: Option<String>,
        is_enabled: bool,
        shows_confirmation: bool,
    ) -> Self {
        Self {
            name: name.into(),
            text: text.into(),
            keyword,
            is_enabled,
            shows_confirmation,
        }
    }
}

/// A fingerprint of a snippet file's bytes.
///
/// Mirrors upstream `SnippetSourceRevision`: it detects an external edit
/// before a save or a delete, so a stale write is refused instead of
/// silently overwriting somebody else's change.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SnippetSourceRevision {
    value: String,
}

impl SnippetSourceRevision {
    /// Fingerprint the UTF-8 bytes of a file's content.
    ///
    /// Mirrors upstream's FNV-1a fingerprint, formatted as
    /// `<byte count>:<hash in lowercase hex>`.
    pub fn new(content: &str) -> Self {
        let mut hash: u64 = 14_695_981_039_346_656_037;
        let mut byte_count: u64 = 0;
        for byte in content.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(1_099_511_628_211);
            byte_count += 1;
        }
        Self {
            value: format!("{byte_count}:{hash:x}"),
        }
    }

    /// The fingerprint as upstream writes it.
    pub fn as_str(&self) -> &str {
        &self.value
    }
}

/// A snippet as it exists on disk.
///
/// Mirrors upstream `StoredSnippet`: the file it lives in, the snippet that
/// was parsed out of it, and the revision of the bytes it was parsed from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredSnippet {
    /// The file the snippet is stored in.
    pub file_url: PathBuf,
    /// The parsed snippet.
    pub snippet: Snippet,
    /// The revision of the file's bytes at parse time.
    pub source_revision: SnippetSourceRevision,
}

impl StoredSnippet {
    /// The entry-id prefix a snippet row carries.
    pub const ENTRY_ID_PREFIX: &'static str = "snippet:";

    /// A stored snippet for a file that has not been written yet.
    ///
    /// The revision is the fingerprint of `content`, so a caller can save the
    /// file and still hand the matching revision back to the repository.
    pub fn for_content(file_url: impl Into<PathBuf>, snippet: Snippet, content: &str) -> Self {
        Self {
            file_url: file_url.into(),
            snippet,
            source_revision: SnippetSourceRevision::new(content),
        }
    }

    /// The stable identity of the stored snippet: its standardized path.
    pub fn id(&self) -> String {
        standardized_path(&self.file_url)
    }

    /// The launcher entry id of the stored snippet.
    pub fn entry_id(&self) -> String {
        format!("{}{}", Self::ENTRY_ID_PREFIX, self.id())
    }

    /// The snippet id an entry id names, or `None` for another kind's id.
    pub fn id_from_entry_id(entry_id: &str) -> Option<String> {
        entry_id
            .strip_prefix(Self::ENTRY_ID_PREFIX)
            .map(str::to_string)
    }
}

/// A path in its standardized form: no `.` segments and no trailing separator.
///
/// Mirrors upstream's `standardizedFileURL`, which is what a stored snippet's
/// identity is built from.
pub(crate) fn standardized_path(path: &Path) -> String {
    let mut parts: Vec<String> = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                parts.pop();
            }
            std::path::Component::RootDir => parts.push(String::new()),
            std::path::Component::Prefix(prefix) => {
                parts.push(prefix.as_os_str().to_string_lossy().into_owned())
            }
            std::path::Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
        }
    }
    parts.join(std::path::MAIN_SEPARATOR_STR)
}
