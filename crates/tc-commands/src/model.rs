//! The custom command: what it is, what it holds, and what it refuses.
//!
//! Upstream's model is `Features/CustomCommands/Model/CustomCommand.swift`.
//! The fields, their defaults and the validation rules are the same here; the
//! one addition is the shell a command runs under, because Windows has more
//! than one.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::shell::Shell;

/// One value the command asks for before it runs.
///
/// "Values are passed to the shell as `$1`, `$2` …, and are never inserted into
/// the command text."
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomCommandArgument {
    /// The name shown beside the search field.
    pub name: String,
    /// An optional argument may be submitted empty; a required one holds the
    /// return key until it has a value.
    #[serde(default)]
    pub is_optional: bool,
}

impl CustomCommandArgument {
    /// Raycast's own cap, and what keeps the inline fields on screen.
    pub const LIMIT: usize = 3;

    /// A new argument.
    pub fn new(name: impl Into<String>, is_optional: bool) -> CustomCommandArgument {
        CustomCommandArgument {
            name: name.into(),
            is_optional,
        }
    }

    /// The inline field holding `$n`, keyed by position, since two arguments
    /// may share a name.
    pub fn field_id_at(index: usize) -> String {
        format!("${}", index + 1)
    }

    /// A blank name is dropped rather than rejected, so an import cannot lose
    /// the whole command.
    pub fn sanitized(arguments: &[CustomCommandArgument]) -> Vec<CustomCommandArgument> {
        arguments
            .iter()
            .filter_map(|argument| {
                let name = argument.name.trim();
                if name.is_empty() || name.contains('\0') {
                    None
                } else {
                    Some(CustomCommandArgument {
                        name: name.to_string(),
                        is_optional: argument.is_optional,
                    })
                }
            })
            .take(Self::LIMIT)
            .collect()
    }
}

/// Why a command was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ValidationError {
    #[error("Enter a name for the command.")]
    EmptyName,
    #[error("Enter a command to run.")]
    EmptyCommand,
    #[error("A custom command with this name already exists.")]
    DuplicateName,
    #[error("Names and commands cannot contain null characters.")]
    InvalidCharacter,
}

/// A shell command the user wrote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomCommand {
    /// The command's identity, stable across renames.
    pub id: Uuid,
    /// What the user searches for.
    pub name: String,
    /// The shell command itself.
    pub command: String,
    /// Off keeps the command and everything attached to it, but nothing may
    /// offer or run it.
    #[serde(default = "yes")]
    pub is_enabled: bool,
    /// Off keeps the command out of the launcher's results, while its
    /// shortcut keeps working.
    #[serde(default = "yes")]
    pub shows_in_root_search: bool,
    /// Sources the shell configuration, so aliases and functions resolve.
    #[serde(default)]
    pub loads_shell_environment: bool,
    /// Asks before running.
    #[serde(default = "yes")]
    pub requires_confirmation: bool,
    /// Shows a short message when the command succeeds.
    #[serde(default)]
    pub shows_confirmation: bool,
    /// The values the command asks for first.
    #[serde(default)]
    pub arguments: Vec<CustomCommandArgument>,
    /// Captures what the command prints and opens the output window.
    #[serde(default)]
    pub shows_output: bool,
    /// The folder the command starts in. Empty means the home folder.
    #[serde(default)]
    pub working_directory: Option<String>,
    /// The row's glyph. `None` falls back to the shared terminal symbol.
    #[serde(default)]
    pub icon_symbol: Option<String>,
    /// The shell the command runs under.
    ///
    /// A Windows addition: upstream has one shell, zsh. The stored value
    /// defaults to PowerShell, which is what the port runs when the field is
    /// left alone.
    #[serde(default)]
    pub shell: Shell,
}

/// The default for a switch that is on unless it is turned off.
fn yes() -> bool {
    true
}

impl CustomCommand {
    /// The prefix a custom command's row id carries.
    pub const ENTRY_ID_PREFIX: &str = "custom-command:";
    /// One glyph for every custom command, so every surface reads alike.
    pub const SYMBOL: &str = "terminal";

    /// A new command, with the fields the documentation gives their defaults.
    ///
    /// "Needs confirmation - Asks before running. On by default."
    #[allow(clippy::too_many_arguments)]
    pub fn new(name: impl Into<String>, command: impl Into<String>) -> CustomCommand {
        CustomCommand {
            id: Uuid::new_v4(),
            name: name.into(),
            command: command.into(),
            is_enabled: true,
            shows_in_root_search: true,
            loads_shell_environment: false,
            requires_confirmation: true,
            shows_confirmation: false,
            arguments: Vec::new(),
            shows_output: false,
            working_directory: None,
            icon_symbol: None,
            shell: Shell::default(),
        }
    }

    /// The row's glyph.
    pub fn symbol(&self) -> &str {
        self.icon_symbol
            .as_deref()
            .filter(|symbol| !symbol.trim().is_empty())
            .unwrap_or(Self::SYMBOL)
    }

    /// The key the launcher opens the command by.
    pub fn entry_id(&self) -> String {
        format!("{}{}", Self::ENTRY_ID_PREFIX, self.id.hyphenated())
    }

    /// The command a row id names, if it names one.
    pub fn id_from_entry_id(entry_id: &str) -> Option<Uuid> {
        let rest = entry_id.strip_prefix(Self::ENTRY_ID_PREFIX)?;
        Uuid::parse_str(rest).ok()
    }

    /// The values in `$n` order.
    ///
    /// `None` while a required value is still empty: "A required argument
    /// can't be left empty."
    pub fn positional_values(&self, values: &[String]) -> Option<Vec<String>> {
        if values.len() < self.arguments.len() {
            return None;
        }
        let complete = self
            .arguments
            .iter()
            .zip(values.iter())
            .all(|(argument, value)| argument.is_optional || !value.is_empty());
        if !complete {
            return None;
        }
        Some(values[..self.arguments.len()].to_vec())
    }

    /// The first required value that is still missing.
    ///
    /// "return runs the command once every required value is present.
    /// Otherwise it focuses the first missing field."
    pub fn first_missing(&self, values: &[String]) -> Option<usize> {
        self.arguments
            .iter()
            .enumerate()
            .find_map(|(index, argument)| {
                if argument.is_optional {
                    return None;
                }
                let value = values.get(index).map(String::as_str).unwrap_or("");
                if value.is_empty() {
                    Some(index)
                } else {
                    None
                }
            })
    }

    /// The command as it is stored: trimmed, with its arguments cleaned.
    pub fn cleaned(&self) -> CustomCommand {
        let mut value = self.clone();
        value.name = self.name.trim().to_string();
        value.command = self.command.trim().to_string();
        value.arguments = CustomCommandArgument::sanitized(&self.arguments);
        value.working_directory = self.working_directory.as_deref().and_then(cleaned_path);
        value.icon_symbol = self.icon_symbol.as_deref().and_then(cleaned_path);
        value
    }

    /// The command, checked against the commands it must not clash with.
    ///
    /// `existing` is what the name has to be unique against: the library, or a
    /// batch in progress.
    pub fn validated(&self, existing: &[CustomCommand]) -> Result<CustomCommand, ValidationError> {
        let value = self.cleaned();
        if value.name.is_empty() {
            return Err(ValidationError::EmptyName);
        }
        if value.command.is_empty() {
            return Err(ValidationError::EmptyCommand);
        }
        if value.name.contains('\0') || value.command.contains('\0') {
            return Err(ValidationError::InvalidCharacter);
        }
        let clash = existing
            .iter()
            .any(|other| other.id != value.id && other.name.eq_ignore_ascii_case(&value.name));
        if clash {
            return Err(ValidationError::DuplicateName);
        }
        Ok(value)
    }
}

/// A trimmed path component, or nothing when it is empty.
///
/// An empty optional field means "unset", not "".
fn cleaned_path(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.contains('\0') {
        None
    } else {
        Some(trimmed.to_string())
    }
}
