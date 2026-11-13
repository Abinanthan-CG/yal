//! The custom commands, kept in a JSON file.
//!
//! Upstream keeps them in `UserDefaults` under the key `customCommands`. macOS
//! has no other store a backup can read, so the port writes a JSON file
//! instead; the shape of the records is upstream's, so a backup written by
//! either app can be read by the other.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::model::{CustomCommand, ValidationError};

/// Why the commands could not be read or written.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("The commands could not be read: {0}")]
    Read(String),
    #[error("The commands could not be written: {0}")]
    Write(String),
}

/// The user's own shell commands.
#[derive(Debug, Clone)]
pub struct CustomCommandStore {
    path: PathBuf,
    commands: Vec<CustomCommand>,
}

impl CustomCommandStore {
    /// Opens the store, reading the commands it holds.
    ///
    /// A file that cannot be read is an empty store, not a failure: the app
    /// must still start.
    pub fn open(path: impl Into<PathBuf>) -> Result<CustomCommandStore, StoreError> {
        let path = path.into();
        let commands = match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice::<Vec<CustomCommand>>(&bytes)
                .map(|values| Self::sanitized(&values))
                .map_err(|error| StoreError::Read(error.to_string()))?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(error) => return Err(StoreError::Read(error.to_string())),
        };
        Ok(CustomCommandStore { path, commands })
    }

    /// A store that is not backed by a file, for the tests and for a preview.
    pub fn in_memory() -> CustomCommandStore {
        CustomCommandStore {
            path: PathBuf::new(),
            commands: Vec::new(),
        }
    }

    /// Where the commands are kept.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Every command, in the order they were added.
    pub fn commands(&self) -> &[CustomCommand] {
        &self.commands
    }

    /// The command with this identity.
    pub fn command(&self, id: Uuid) -> Option<&CustomCommand> {
        self.commands.iter().find(|command| command.id == id)
    }

    /// The command a row id names.
    pub fn command_of_entry_id(&self, entry_id: &str) -> Option<&CustomCommand> {
        let id = CustomCommand::id_from_entry_id(entry_id)?;
        self.command(id)
    }

    /// The commands whose name the query finds.
    ///
    /// "Only the command's name is searchable, not the command itself, so you
    /// can't run something by half-remembering its flags." A command that is
    /// not enabled is never offered.
    pub fn matching(&self, query: &str) -> Vec<&CustomCommand> {
        let wanted = query.trim();
        self.commands
            .iter()
            .filter(|command| {
                command.is_enabled
                    && (wanted.is_empty()
                        || command.name.to_lowercase().contains(&wanted.to_lowercase()))
            })
            .collect()
    }

    /// Adds a command, after checking it against the ones already there.
    pub fn add(&mut self, draft: CustomCommand) -> Result<CustomCommand, ValidationError> {
        let value = draft.validated(&self.commands)?;
        self.commands.push(value.clone());
        self.persist();
        Ok(value)
    }

    /// Adds a whole batch, skipping the ones that do not fit.
    ///
    /// "Tinycast warns you before importing and skips any name you already
    /// have, so importing the same folder again only adds new scripts."
    ///
    /// Returns how many of the drafts were new.
    pub fn add_all(&mut self, drafts: &[CustomCommand]) -> usize {
        let mut added = 0usize;
        for draft in drafts {
            let cleaned = draft.cleaned();
            if cleaned.name.is_empty() || cleaned.command.is_empty() {
                continue;
            }
            let taken = self
                .commands
                .iter()
                .any(|other| other.name.eq_ignore_ascii_case(&cleaned.name));
            if taken {
                continue;
            }
            self.commands.push(cleaned);
            added += 1;
        }
        if added > 0 {
            self.persist();
        }
        added
    }

    /// Replaces a command, keeping its identity.
    pub fn update(&mut self, draft: CustomCommand) -> Result<(), ValidationError> {
        let Some(index) = self
            .commands
            .iter()
            .position(|command| command.id == draft.id)
        else {
            return Ok(());
        };
        let value = draft.validated(&self.commands)?;
        self.commands[index] = value;
        self.persist();
        Ok(())
    }

    /// Turns a command on or off.
    ///
    /// "Each command has an Enabled checkbox on its row. Clearing it keeps the
    /// command, its shortcut and its settings, but the command can't run until
    /// you select the checkbox again."
    pub fn set_enabled(&mut self, id: Uuid, enabled: bool) {
        let Some(command) = self.commands.iter_mut().find(|command| command.id == id) else {
            return;
        };
        if command.is_enabled == enabled {
            return;
        }
        command.is_enabled = enabled;
        self.persist();
    }

    /// Takes a command out of the main search, or puts it back.
    ///
    /// "To take a command out of the main search but keep its shortcut, select
    /// it in the launcher and press Hide from Root Search."
    pub fn set_shows_in_root_search(&mut self, id: Uuid, shows: bool) {
        let Some(command) = self.commands.iter_mut().find(|command| command.id == id) else {
            return;
        };
        if command.shows_in_root_search == shows {
            return;
        }
        command.shows_in_root_search = shows;
        self.persist();
    }

    /// Removes a command.
    ///
    /// "Deleting it removes all three" - the command, its favourite and its
    /// shortcut. The favourite and the shortcut live elsewhere, so the caller
    /// removes those; this returns what was removed.
    pub fn remove(&mut self, id: Uuid) -> Option<CustomCommand> {
        let index = self.commands.iter().position(|command| command.id == id)?;
        let removed = self.commands.remove(index);
        self.persist();
        Some(removed)
    }

    /// Replaces the whole set, dropping the records that do not fit.
    ///
    /// "Replaces the whole set on backup import, dropping invalid and duplicate
    /// records."
    pub fn replace_with(&mut self, commands: Vec<CustomCommand>) -> usize {
        self.commands = Self::sanitized(&commands);
        self.persist();
        self.commands.len()
    }

    /// Writes the commands out.
    pub fn save(&self) -> Result<(), StoreError> {
        self.persist();
        Ok(())
    }

    /// The commands that survive a load: valid, with no repeated identity and
    /// no repeated name.
    fn sanitized(values: &[CustomCommand]) -> Vec<CustomCommand> {
        let mut ids: HashSet<Uuid> = HashSet::new();
        let mut names: HashSet<String> = HashSet::new();
        let mut result: Vec<CustomCommand> = Vec::new();
        for value in values {
            let cleaned = value.cleaned();
            if cleaned.name.is_empty()
                || cleaned.command.is_empty()
                || cleaned.name.contains('\0')
                || cleaned.command.contains('\0')
            {
                continue;
            }
            let folded = cleaned.name.to_lowercase();
            if !ids.insert(cleaned.id) || !names.insert(folded) {
                continue;
            }
            result.push(cleaned);
        }
        result
    }

    /// Writes the commands out, once, after a change.
    fn persist(&self) {
        if self.path.as_os_str().is_empty() {
            return;
        }
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let Ok(bytes) = serde_json::to_vec_pretty(&self.commands) else {
            return;
        };
        let _ = std::fs::write(&self.path, bytes);
    }
}
