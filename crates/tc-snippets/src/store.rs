//! The collection of snippets the app shows and edits.
//!
//! Mirrors upstream `Features/Snippets/Model/SnippetsStore.swift`: the store
//! owns the repository, holds the current list of snippets, and reports the
//! files that could not be read.

use crate::repository::{Issue, RepositoryError, Snapshot, SnippetRepository};
use crate::snippet::{Snippet, StoredSnippet};

/// What the store is doing.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum State {
    /// The store has not been loaded yet.
    #[default]
    Idle,
    /// The store is reading the library.
    Loading,
    /// The store holds the library.
    Ready,
    /// The store could not read the library.
    Failed(String),
}

/// The snippet library, as the app sees it.
///
/// Mirrors upstream's `SnippetsStore`.
#[derive(Debug)]
pub struct SnippetsStore {
    repository: SnippetRepository,
    state: State,
    snippets: Vec<StoredSnippet>,
    issues: Vec<Issue>,
    operation_error: Option<String>,
}

impl SnippetsStore {
    /// A store over a repository.
    ///
    /// Mirrors upstream's `init(repository:)`.
    pub fn new(repository: SnippetRepository) -> Self {
        Self {
            repository,
            state: State::Idle,
            snippets: Vec::new(),
            issues: Vec::new(),
            operation_error: None,
        }
    }

    /// The repository the store reads and writes.
    pub fn repository(&self) -> &SnippetRepository {
        &self.repository
    }

    /// What the store is doing.
    pub fn state(&self) -> &State {
        &self.state
    }

    /// The snippets the store holds.
    pub fn snippets(&self) -> &[StoredSnippet] {
        &self.snippets
    }

    /// The files the store could not read.
    pub fn issues(&self) -> &[Issue] {
        &self.issues
    }

    /// The message of the last failed operation.
    pub fn operation_error(&self) -> Option<&str> {
        self.operation_error.as_deref()
    }

    /// Read the library.
    ///
    /// Mirrors upstream's `start()`: the list is replaced by what is on disk.
    pub fn start(&mut self) -> Result<(), RepositoryError> {
        self.state = State::Loading;
        match self.repository.load() {
            Ok(snapshot) => {
                self.apply(snapshot);
                self.operation_error = None;
                Ok(())
            }
            Err(error) => {
                self.state = State::Failed(error.to_string());
                Err(error)
            }
        }
    }

    /// Read the library again.
    ///
    /// Mirrors upstream's `retry()`.
    pub fn retry(&mut self) -> Result<(), RepositoryError> {
        self.start()
    }

    /// Write a new snippet to the library.
    ///
    /// Mirrors upstream's `create(_:)`.
    pub fn create(&mut self, snippet: &Snippet) -> Result<StoredSnippet, RepositoryError> {
        let record = self.repository.create(snippet)?;
        self.operation_error = None;
        self.publish_local(record.clone());
        self.start()?;
        Ok(record)
    }

    /// Write several new snippets to the library.
    ///
    /// Mirrors upstream's `importSnippets(_:)`.
    pub fn import_snippets(
        &mut self,
        snippets: &[Snippet],
    ) -> Result<Vec<StoredSnippet>, RepositoryError> {
        if snippets.is_empty() {
            return Ok(Vec::new());
        }
        let created = self.repository.create_many(snippets)?;
        self.operation_error = None;
        for record in &created {
            self.publish_local(record.clone());
        }
        self.start()?;
        Ok(created)
    }

    /// Write a snippet back to the library.
    ///
    /// Mirrors upstream's `save(_:)`.
    pub fn save(&mut self, record: &StoredSnippet) -> Result<StoredSnippet, RepositoryError> {
        let saved =
            self.repository
                .save(&record.snippet, &record.file_url, &record.source_revision)?;
        self.operation_error = None;
        self.publish_local(saved.clone());
        self.start()?;
        Ok(saved)
    }

    /// Remove a snippet from the library.
    ///
    /// Mirrors upstream's `delete(id:)`.
    pub fn delete(&mut self, id: &str) -> Result<(), RepositoryError> {
        let record = self
            .snippets
            .iter()
            .find(|record| record.id() == id)
            .cloned()
            .ok_or_else(|| RepositoryError::FileNotFound(std::path::PathBuf::from(id)))?;
        self.repository
            .delete(&record.file_url, &record.source_revision)?;
        self.operation_error = None;
        self.publish_local(record);
        self.start()?;
        Ok(())
    }

    /// The snippet with an identity.
    ///
    /// Mirrors upstream's `record(id:)`.
    pub fn record(&self, id: &str) -> Option<&StoredSnippet> {
        self.snippets.iter().find(|record| record.id() == id)
    }

    /// Put a snapshot's contents into the store.
    fn apply(&mut self, snapshot: Snapshot) {
        self.snippets = snapshot.records;
        self.issues = snapshot.issues;
        self.state = State::Ready;
    }

    /// Put one record into the store without reading the disk.
    ///
    /// Mirrors upstream's `publishLocal(records:)`.
    fn publish_local(&mut self, record: StoredSnippet) {
        let mut records: Vec<StoredSnippet> = self
            .snippets
            .iter()
            .filter(|existing| existing.id() != record.id())
            .cloned()
            .collect();
        records.push(record);
        records.sort_by(|lhs, rhs| {
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
        });
        self.snippets = records;
    }
}
