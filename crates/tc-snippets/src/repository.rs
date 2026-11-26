//! The snippet library on disk: one Markdown file per snippet.
//!
//! Mirrors upstream `Features/Snippets/Model/SnippetRepository.swift`, with
//! the channel folder replaced by an injected root folder, so the crate never
//! reaches outside the folder it was given.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::markdown::SnippetMarkdownSerializer;
use crate::snippet::{Snippet, SnippetSourceRevision, StoredSnippet};

/// What went wrong while reading or writing the library.
#[derive(Debug)]
pub enum RepositoryError {
    /// The file changed on disk since it was read.
    Conflict {
        /// The file that changed.
        file: PathBuf,
        /// The revision the caller expected.
        expected: SnippetSourceRevision,
        /// The revision the file actually has, if it could be read.
        actual: Option<SnippetSourceRevision>,
    },
    /// The file is gone.
    FileNotFound(PathBuf),
    /// The file is not inside the snippets folder.
    InvalidFileLocation(PathBuf),
    /// The file could not be read or written.
    Io {
        /// The file that could not be accessed.
        file: PathBuf,
        /// What went wrong.
        message: String,
    },
}

impl std::fmt::Display for RepositoryError {
    /// Mirrors upstream's `RepositoryError.errorDescription`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Conflict { file, .. } => write!(
                f,
                "The snippet changed on disk. Reload it before saving or deleting. ({})",
                file.file_name().unwrap_or_default().to_string_lossy()
            ),
            Self::FileNotFound(file) => write!(
                f,
                "The snippet file no longer exists. ({})",
                file.file_name().unwrap_or_default().to_string_lossy()
            ),
            Self::InvalidFileLocation(file) => write!(
                f,
                "The snippet file is outside this YAL channel. ({})",
                file.display()
            ),
            Self::Io { file, message } => {
                write!(f, "Could not access {}: {}", file.display(), message)
            }
        }
    }
}

impl std::error::Error for RepositoryError {}

/// A file in the library that could not be read as a snippet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    /// The file that failed.
    pub file: PathBuf,
    /// What was wrong with it.
    pub message: String,
}

impl Issue {
    /// The issue's identity: the standardized path of the file.
    pub fn id(&self) -> String {
        crate::snippet::standardized_path(&self.file)
    }
}

/// Everything the library holds.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Snapshot {
    /// The snippets that parsed.
    pub records: Vec<StoredSnippet>,
    /// The files that did not.
    pub issues: Vec<Issue>,
}

impl Snapshot {
    /// Every file still on disk: one that fails to parse is mid-edit, not deleted.
    pub fn file_ids(&self) -> std::collections::HashSet<String> {
        let mut ids: std::collections::HashSet<String> =
            self.records.iter().map(|record| record.id()).collect();
        ids.extend(self.issues.iter().map(|issue| issue.id()));
        ids
    }
}

/// The snippet library, stored as Markdown files in one folder.
///
/// Mirrors upstream's `SnippetRepository`.
#[derive(Debug, Clone)]
pub struct SnippetRepository {
    root: PathBuf,
}

impl SnippetRepository {
    /// A library whose files live in `root`.
    ///
    /// The folder is created when it does not exist yet, and nothing outside
    /// it is ever read or written.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The folder the library lives in.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Read every snippet in the library.
    ///
    /// Mirrors upstream's `load()`: a file that fails to parse becomes an
    /// issue rather than hiding the files that did parse.
    pub fn load(&self) -> Result<Snapshot, RepositoryError> {
        self.ensure_root()?;
        let mut records: Vec<StoredSnippet> = Vec::new();
        let mut issues: Vec<Issue> = Vec::new();

        for file in self.markdown_files()? {
            let content = match fs::read_to_string(&file) {
                Ok(content) => content,
                Err(error) => {
                    issues.push(Issue {
                        file,
                        message: error.to_string(),
                    });
                    continue;
                }
            };
            match SnippetMarkdownSerializer::parse(&content, &file) {
                Ok(snippet) => records.push(StoredSnippet::for_content(file, snippet, &content)),
                Err(error) => issues.push(Issue {
                    file: error.file.clone(),
                    message: error.to_string(),
                }),
            }
        }

        records.sort_by(record_order);
        issues.sort_by_key(|issue| issue.id());
        Ok(Snapshot { records, issues })
    }

    /// Write a new snippet to a file of its own.
    ///
    /// Mirrors upstream's `create(_:)`: the file name comes from the
    /// snippet's name, and a name that is taken gets a numbered suffix.
    pub fn create(&self, snippet: &Snippet) -> Result<StoredSnippet, RepositoryError> {
        self.ensure_root()?;
        let content = SnippetMarkdownSerializer::serialize(snippet);
        let mut suffix = 1usize;

        loop {
            let file = self.unique_file_url(&snippet.name, suffix);
            match write_new_file(&file, &content) {
                Ok(()) => {
                    return Ok(StoredSnippet::for_content(file, snippet.clone(), &content));
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    suffix += 1;
                }
                Err(error) => {
                    return Err(RepositoryError::Io {
                        file,
                        message: error.to_string(),
                    })
                }
            }
        }
    }

    /// Write several new snippets, rolling back if one of them fails.
    ///
    /// Mirrors upstream's `create(_:)` for an array.
    pub fn create_many(&self, snippets: &[Snippet]) -> Result<Vec<StoredSnippet>, RepositoryError> {
        let mut created: Vec<StoredSnippet> = Vec::new();
        for snippet in snippets {
            match self.create(snippet) {
                Ok(record) => created.push(record),
                Err(error) => {
                    for record in created.iter().rev() {
                        // A file this call created is its own to remove.
                        let _ = fs::remove_file(&record.file_url);
                    }
                    return Err(error);
                }
            }
        }
        Ok(created)
    }

    /// Write a snippet back to the file it was read from.
    ///
    /// Mirrors upstream's `save(_:fileURL:expectedRevision:)`: the file has to
    /// still hold the revision the caller read, or the save is refused.
    pub fn save(
        &self,
        snippet: &Snippet,
        file: &Path,
        expected_revision: &SnippetSourceRevision,
    ) -> Result<StoredSnippet, RepositoryError> {
        let file = self.validated_file_url(file)?;
        let content = SnippetMarkdownSerializer::serialize(snippet);
        let actual = self.revision_at(&file)?;
        if &actual != expected_revision {
            return Err(RepositoryError::Conflict {
                file: file.clone(),
                expected: expected_revision.clone(),
                actual: Some(actual),
            });
        }
        fs::write(&file, content.as_bytes()).map_err(|error| RepositoryError::Io {
            file: file.clone(),
            message: error.to_string(),
        })?;
        Ok(StoredSnippet::for_content(file, snippet.clone(), &content))
    }

    /// Remove a snippet's file.
    ///
    /// Mirrors upstream's `delete(fileURL:expectedRevision:)`: the file has to
    /// still hold the revision the caller read, or the delete is refused.
    pub fn delete(
        &self,
        file: &Path,
        expected_revision: &SnippetSourceRevision,
    ) -> Result<(), RepositoryError> {
        let file = self.validated_file_url(file)?;
        let actual = self.revision_at(&file)?;
        if &actual != expected_revision {
            return Err(RepositoryError::Conflict {
                file: file.clone(),
                expected: expected_revision.clone(),
                actual: Some(actual),
            });
        }
        fs::remove_file(&file).map_err(|error| RepositoryError::Io {
            file: file.clone(),
            message: error.to_string(),
        })
    }

    /// Create the library folder if it is not there yet.
    fn ensure_root(&self) -> Result<(), RepositoryError> {
        fs::create_dir_all(&self.root).map_err(|error| RepositoryError::Io {
            file: self.root.clone(),
            message: error.to_string(),
        })
    }

    /// The Markdown files in the library, in file-name order.
    fn markdown_files(&self) -> Result<Vec<PathBuf>, RepositoryError> {
        let entries = fs::read_dir(&self.root).map_err(|error| RepositoryError::Io {
            file: self.root.clone(),
            message: error.to_string(),
        })?;

        let mut files: Vec<PathBuf> = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| RepositoryError::Io {
                file: self.root.clone(),
                message: error.to_string(),
            })?;
            let path = entry.path();
            let is_markdown = path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("md"));
            if !is_markdown {
                continue;
            }
            let metadata = match fs::metadata(&path) {
                Ok(metadata) => metadata,
                Err(_) => continue,
            };
            if !metadata.is_file() {
                continue;
            }
            files.push(path);
        }
        files.sort_by_key(|path| path.file_name().map(|name| name.to_os_string()));
        Ok(files)
    }

    /// The file a new snippet goes into.
    fn unique_file_url(&self, name: &str, suffix: usize) -> PathBuf {
        let base = SnippetMarkdownSerializer::slug(name);
        let file_name = if suffix == 1 {
            format!("{base}.md")
        } else {
            format!("{base}-{suffix}.md")
        };
        self.root.join(file_name)
    }

    /// The file a snippet may be written to.
    ///
    /// Mirrors upstream's `validatedFileURL(_:)`: the file has to sit
    /// directly in the snippets folder and carry the `.md` extension.
    fn validated_file_url(&self, file: &Path) -> Result<PathBuf, RepositoryError> {
        let standardized = crate::snippet::standardized_path(file);
        let parent = file
            .parent()
            .map(crate::snippet::standardized_path)
            .unwrap_or_default();
        let root = crate::snippet::standardized_path(&self.root);
        let has_markdown_extension = file
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("md"));

        if parent != root || !has_markdown_extension {
            return Err(RepositoryError::InvalidFileLocation(PathBuf::from(
                standardized,
            )));
        }
        Ok(PathBuf::from(standardized))
    }

    /// The revision of a file's bytes.
    fn revision_at(&self, file: &Path) -> Result<SnippetSourceRevision, RepositoryError> {
        if !file.exists() {
            return Err(RepositoryError::FileNotFound(file.to_path_buf()));
        }
        let content = fs::read_to_string(file).map_err(|error| RepositoryError::Io {
            file: file.to_path_buf(),
            message: error.to_string(),
        })?;
        Ok(SnippetSourceRevision::new(&content))
    }
}

/// Write a file that must not exist yet.
///
/// A plain write would overwrite whatever was there, so the file is opened with
/// `create_new`: a name that is taken fails instead of losing a snippet.
fn write_new_file(file: &Path, content: &str) -> io::Result<()> {
    use std::io::Write;
    let mut handle = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(file)?;
    handle.write_all(content.as_bytes())
}

/// The order snippets are listed in.
///
/// Mirrors upstream's `recordOrder(_:_:)`: by name, case-insensitively, and
/// then by identity so the order is stable.
fn record_order(lhs: &StoredSnippet, rhs: &StoredSnippet) -> std::cmp::Ordering {
    let comparison = lhs
        .snippet
        .name
        .to_lowercase()
        .cmp(&rhs.snippet.name.to_lowercase());
    if comparison == std::cmp::Ordering::Equal {
        lhs.id().cmp(&rhs.id())
    } else {
        comparison
    }
}
