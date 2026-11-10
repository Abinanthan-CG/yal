//! `tc-core` - domain types shared by every part of the YAL port.
//!
//! This crate holds the parts of the product that are pure: the command
//! registry, the launcher-row identity scheme, the deeplink grammar and the
//! stable id rules. Nothing here touches Windows, so every rule in it is
//! testable without a desktop.
//!
//! See `docs/ARCHITECTURE.md` for the crate map.

pub mod command;
pub mod deeplink;
pub mod entry;
pub mod ids;
pub mod url;

pub use command::{BuiltInQuickAction, CommandId};
pub use deeplink::{Arguments, Context, DeepLink, DeepLinkInput, QueryItem};
pub use entry::{
    action_for_entry_id, HotKeyAction, SystemActionCategory, SystemActionConfirmation,
    SystemActionId,
};
pub use ids::Uuid;
