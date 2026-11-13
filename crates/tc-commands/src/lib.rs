//! `tc-commands` - the command layer of the YAL port.
//!
//! Upstream splits the feature in two: the catalogue of built-in commands
//! (`Features/Launcher/Model/CommandCatalog.swift` and `CommandID.swift`) and
//! the user's own shell commands (`Features/CustomCommands`). This crate is
//! the same two halves.
//!
//! * [`catalog`] says which Settings pane owns a built-in command, which
//!   feature has to be on for it to exist, and how its shortcut behaves.
//! * [`model`] is the custom command: its fields, its validation and how its
//!   argument values reach the shell.
//! * [`store`] keeps the custom commands in a JSON file.
//! * [`shell`] turns a command plus its values into the process to run.
//! * [`runner`] runs it, captures what it printed and can stop the whole tree.
//! * [`raycast`] imports a folder of Raycast script commands.
//!
//! The upstream authority is the macOS app Tinycast
//! (<https://github.com/abue-ammar/tinycast>, AGPL-3.0): the Swift files in
//! `Features/CustomCommands` and `Features/Launcher/Model`, and the page
//! `docs/launcher/commands.md`.

pub mod arguments;
#[cfg(test)]
mod arguments_tests;
pub mod catalog;
#[cfg(test)]
mod catalog_tests;
pub mod model;
#[cfg(test)]
mod model_tests;
pub mod raycast;
#[cfg(test)]
mod raycast_tests;
pub mod runner;
pub mod shell;
#[cfg(test)]
mod shell_tests;
pub mod store;
#[cfg(test)]
mod store_tests;

pub use arguments::{ArgumentSheet, Submission};
pub use catalog::{CommandPane, CommandRow, CommandScreen, CommandSettings, CommandSwitches};
pub use model::{CustomCommand, CustomCommandArgument, ValidationError};
pub use raycast::RaycastScriptImport;
pub use runner::{RunOutcome, ShellRunner};
pub use shell::{LaunchError, LaunchPlan, Shell};
pub use store::CustomCommandStore;

/// The name of the file the custom commands live in.
///
/// Upstream keeps them in `UserDefaults`; Windows has no equivalent store that
/// a backup can read, so they are a JSON file beside the app's other data.
pub const COMMANDS_FILE: &str = "commands.json";

/// The prefix a custom command's row id carries.
pub const CUSTOM_COMMAND_PREFIX: &str = "custom-command:";
