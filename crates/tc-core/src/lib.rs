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
pub mod quicklink;
pub mod url;

pub use command::{BuiltInQuickAction, CommandId};
pub use deeplink::{Arguments, Context, DeepLink, DeepLinkInput, QueryItem};
pub use entry::{
    action_for_entry_id, HotKeyAction, SystemActionCategory, SystemActionConfirmation,
    SystemActionId,
};
pub use ids::Uuid;
pub use quicklink::QuicklinkDestination;

/// The GitHub repository YAL is published from.
///
/// The slug is written down once, here. Everything that has to name the
/// repository - the updater, the support link, the packaging manifests -
/// reads it from this constant, so there is only one place to change when the
/// repository is created.
pub const REPOSITORY: &str = "OWNER/REPO";

/// Where a user is sent when they ask for support.
///
/// Support belongs to the original project, because the original project is
/// what people who search for a launcher will find.
pub const SUPPORT_URL: &str = "https://tinycast.dev/support";

/// The latest release of YAL, used by the updater.
pub fn latest_release_url() -> String {
    format!("https://github.com/{REPOSITORY}/releases/latest")
}
