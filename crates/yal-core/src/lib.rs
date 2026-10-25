//! Domain model for the launcher.
//!
//! This crate is the innermost layer of the rebuild. It owns the vocabulary
//! that every other crate speaks: what a *command* is, what an *item* is, what
//! an *action* does, and how a *match* against a query is scored.
//!
//! Nothing here depends on a UI toolkit, a database, or the operating system.
//! The layer boundary mirrors the observed architecture in
//! `docs/observations.md` §1: domain semantics sit below provider adapters
//! (indexers, store, system integration) and below the presentation layer
//! (egui palette, CLI).

pub mod action;
pub mod command;
pub mod context;
pub mod error;
pub mod icon;
pub mod item;
pub mod ranking;

pub use action::{Action, ActionKind, OpenTarget};
pub use command::{Command, CommandId, CommandKind, CommandSource};
pub use context::Context;
pub use error::{CoreError, CoreResult};
pub use icon::Icon;
pub use item::{Accessory, Item, ItemId};
pub use ranking::{Match, MatchKind};
