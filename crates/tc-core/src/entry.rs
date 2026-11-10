//! Launcher rows and the actions they can be bound to.
//!
//! A row is named by an *entry id*: a stable string of the form `<kind>:<id>`.
//! The same string names a deeplink, a stored shortcut and a backup entry, so
//! the scheme is part of the port's contract with the user's data.

use crate::command::CommandId;
use crate::ids::Uuid;

/// The entry-id prefixes for the kinds that carry a stored item.
pub const PREFIX_CUSTOM_WINDOW_SIZE: &str = "window-size:";
pub const PREFIX_WINDOW_LAYOUT: &str = "window-layout:";
pub const PREFIX_WINDOW_ROOM: &str = "window-room:";
pub const PREFIX_QUICK_ACTION: &str = "quick-action:";
pub const PREFIX_CUSTOM_COMMAND: &str = "custom-command:";
pub const PREFIX_QUICKLINK: &str = "quicklink:";
pub const PREFIX_APPLE_SHORTCUT: &str = "apple-shortcut:";
pub const PREFIX_SNIPPET: &str = "snippet:";
pub const PREFIX_SYSTEM_ACTION: &str = "system-action:";
pub const PREFIX_WINDOW_COMMAND: &str = "window-command:";

/// An action a global shortcut or a deeplink can run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HotKeyAction {
    /// Show or hide the palette.
    TogglePalette,
    /// Start or stop dictation.
    Dictation,
    /// One of the built-in commands.
    Command(CommandId),
    /// Bring an installed app forward.
    App { bundle_id: String },
    /// Open a Settings pane.
    SettingsPane { bundle_id: String },
    CustomCommand { id: Uuid },
    SystemAction { id: SystemActionId },
    WindowCommand { id: String },
    WindowLayout { id: Uuid },
    WindowRoom { id: Uuid },
    CustomWindowSize { id: Uuid },
    Quicklink { id: Uuid },
    QuickAction { id: Uuid },
    /// Upstream's Apple Shortcuts; on Windows, an Automation.
    AppleShortcut { id: Uuid },
    Snippet { id: String },
    /// Keyed by the extension's own entry id, which survives a reinstall.
    ExtensionCommand { entry_id: String },
}

impl HotKeyAction {
    /// The entry id this action is stored under, or `None` for the actions no
    /// row id names.
    pub fn entry_id(&self) -> Option<String> {
        match self {
            HotKeyAction::TogglePalette => None,
            HotKeyAction::Dictation => None,
            HotKeyAction::Command(command) => Some(command.entry_id().to_string()),
            HotKeyAction::App { bundle_id } => Some(format!("app:{bundle_id}")),
            HotKeyAction::SettingsPane { bundle_id } => Some(format!("pane:{bundle_id}")),
            HotKeyAction::CustomCommand { id } => Some(format!("{PREFIX_CUSTOM_COMMAND}{id}")),
            HotKeyAction::SystemAction { id } => Some(format!("{PREFIX_SYSTEM_ACTION}{}", id.as_str())),
            HotKeyAction::WindowCommand { id } => Some(format!("{PREFIX_WINDOW_COMMAND}{id}")),
            HotKeyAction::WindowLayout { id } => Some(format!("{PREFIX_WINDOW_LAYOUT}{id}")),
            HotKeyAction::WindowRoom { id } => Some(format!("{PREFIX_WINDOW_ROOM}{id}")),
            HotKeyAction::CustomWindowSize { id } => Some(format!("{PREFIX_CUSTOM_WINDOW_SIZE}{id}")),
            HotKeyAction::Quicklink { id } => Some(format!("{PREFIX_QUICKLINK}{id}")),
            HotKeyAction::QuickAction { id } => Some(format!("{PREFIX_QUICK_ACTION}{id}")),
            HotKeyAction::AppleShortcut { id } => Some(format!("{PREFIX_APPLE_SHORTCUT}{id}")),
            HotKeyAction::Snippet { id } => Some(format!("{PREFIX_SNIPPET}{id}")),
            HotKeyAction::ExtensionCommand { entry_id } => Some(entry_id.clone()),
        }
    }

    /// The key this action's global shortcut is stored under.
    pub fn defaults_key(&self) -> String {
        match self {
            HotKeyAction::TogglePalette => "hotkey.togglePalette".to_string(),
            HotKeyAction::Dictation => "hotkey.dictation".to_string(),
            HotKeyAction::Command(command) => format!("hotkey.{}", command.entry_id()),
            HotKeyAction::App { bundle_id } => format!("hotkey.app.{bundle_id}"),
            HotKeyAction::SettingsPane { bundle_id } => format!("hotkey.pane.{bundle_id}"),
            HotKeyAction::CustomCommand { id } => format!("hotkey.customCommand.{id}"),
            HotKeyAction::SystemAction { id } => format!("hotkey.systemAction.{}", id.as_str()),
            HotKeyAction::WindowCommand { id } => format!("hotkey.windowCommand.{id}"),
            HotKeyAction::WindowLayout { id } => format!("hotkey.windowLayout.{id}"),
            HotKeyAction::WindowRoom { id } => format!("hotkey.windowRoom.{id}"),
            HotKeyAction::CustomWindowSize { id } => format!("hotkey.customWindowSize.{id}"),
            HotKeyAction::Quicklink { id } => format!("hotkey.quicklink.{id}"),
            HotKeyAction::QuickAction { id } => format!("hotkey.quickAction.{id}"),
            HotKeyAction::AppleShortcut { id } => format!("hotkey.appleShortcut.{id}"),
            HotKeyAction::Snippet { id } => format!("hotkey.snippet.{id}"),
            HotKeyAction::ExtensionCommand { entry_id } => format!("hotkey.extensionCommand.{entry_id}"),
        }
    }
}

/// Resolve an entry id to the action it names.
///
/// `None` for a row no link can run, so a copied link always resolves.
pub fn action_for_entry_id(entry_id: &str) -> Option<HotKeyAction> {
    if let Some(command) = CommandId::from_entry_id(entry_id) {
        return Some(HotKeyAction::Command(command));
    }
    if let Some(rest) = entry_id.strip_prefix(PREFIX_SYSTEM_ACTION) {
        return SystemActionId::from_str(rest).map(|id| HotKeyAction::SystemAction { id });
    }
    if let Some(rest) = entry_id.strip_prefix(PREFIX_WINDOW_COMMAND) {
        if rest.is_empty() {
            return None;
        }
        return Some(HotKeyAction::WindowCommand {
            id: rest.to_string(),
        });
    }
    if let Some(rest) = entry_id.strip_prefix(PREFIX_CUSTOM_WINDOW_SIZE) {
        return Uuid::parse(rest).map(|id| HotKeyAction::CustomWindowSize { id });
    }
    if let Some(rest) = entry_id.strip_prefix(PREFIX_WINDOW_LAYOUT) {
        return Uuid::parse(rest).map(|id| HotKeyAction::WindowLayout { id });
    }
    if let Some(rest) = entry_id.strip_prefix(PREFIX_WINDOW_ROOM) {
        return Uuid::parse(rest).map(|id| HotKeyAction::WindowRoom { id });
    }
    if let Some(rest) = entry_id.strip_prefix(PREFIX_QUICK_ACTION) {
        return Uuid::parse(rest).map(|id| HotKeyAction::QuickAction { id });
    }
    if let Some(rest) = entry_id.strip_prefix(PREFIX_CUSTOM_COMMAND) {
        return Uuid::parse(rest).map(|id| HotKeyAction::CustomCommand { id });
    }
    if let Some(rest) = entry_id.strip_prefix(PREFIX_QUICKLINK) {
        return Uuid::parse(rest).map(|id| HotKeyAction::Quicklink { id });
    }
    if let Some(rest) = entry_id.strip_prefix(PREFIX_APPLE_SHORTCUT) {
        return Uuid::parse(rest).map(|id| HotKeyAction::AppleShortcut { id });
    }
    if let Some(rest) = entry_id.strip_prefix(PREFIX_SNIPPET) {
        if rest.is_empty() {
            return None;
        }
        return Some(HotKeyAction::Snippet {
            id: rest.to_string(),
        });
    }
    None
}

/// One of the 32 built-in system actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SystemActionId {
    LockScreen,
    Sleep,
    SleepDisplays,
    Restart,
    ShutDown,
    LogOut,
    ShowScreenSaver,
    PlayPause,
    NextTrack,
    PreviousTrack,
    ToggleMute,
    ToggleMicrophoneMute,
    VolumeUp,
    VolumeDown,
    SetVolume,
    Volume0,
    Volume25,
    Volume50,
    Volume75,
    Volume100,
    ShowDesktop,
    ToggleAppearance,
    ToggleStageManager,
    OpenTrash,
    EmptyTrash,
    EjectAllDisks,
    ToggleHiddenFiles,
    HideOtherApps,
    UnhideAllApps,
    QuitAllApps,
    DismissNotifications,
    ToggleBluetooth,
}

/// The group a system action belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemActionCategory {
    Session,
    Media,
    Volume,
    Desktop,
    Files,
    Hardware,
    Microphone,
}

impl SystemActionId {
    /// Every action, in the order upstream declares them.
    pub const ALL: [SystemActionId; 32] = [
        SystemActionId::LockScreen,
        SystemActionId::Sleep,
        SystemActionId::SleepDisplays,
        SystemActionId::Restart,
        SystemActionId::ShutDown,
        SystemActionId::LogOut,
        SystemActionId::ShowScreenSaver,
        SystemActionId::PlayPause,
        SystemActionId::NextTrack,
        SystemActionId::PreviousTrack,
        SystemActionId::ToggleMute,
        SystemActionId::ToggleMicrophoneMute,
        SystemActionId::VolumeUp,
        SystemActionId::VolumeDown,
        SystemActionId::SetVolume,
        SystemActionId::Volume0,
        SystemActionId::Volume25,
        SystemActionId::Volume50,
        SystemActionId::Volume75,
        SystemActionId::Volume100,
        SystemActionId::ShowDesktop,
        SystemActionId::ToggleAppearance,
        SystemActionId::ToggleStageManager,
        SystemActionId::OpenTrash,
        SystemActionId::EmptyTrash,
        SystemActionId::EjectAllDisks,
        SystemActionId::ToggleHiddenFiles,
        SystemActionId::HideOtherApps,
        SystemActionId::UnhideAllApps,
        SystemActionId::QuitAllApps,
        SystemActionId::DismissNotifications,
        SystemActionId::ToggleBluetooth,
    ];

    /// The stored id.
    pub fn as_str(self) -> &'static str {
        match self {
            SystemActionId::LockScreen => "lock-screen",
            SystemActionId::Sleep => "sleep",
            SystemActionId::SleepDisplays => "sleep-displays",
            SystemActionId::Restart => "restart",
            SystemActionId::ShutDown => "shut-down",
            SystemActionId::LogOut => "log-out",
            SystemActionId::ShowScreenSaver => "show-screen-saver",
            SystemActionId::PlayPause => "play-pause",
            SystemActionId::NextTrack => "next-track",
            SystemActionId::PreviousTrack => "previous-track",
            SystemActionId::ToggleMute => "toggle-mute",
            SystemActionId::ToggleMicrophoneMute => "toggle-microphone-mute",
            SystemActionId::VolumeUp => "volume-up",
            SystemActionId::VolumeDown => "volume-down",
            SystemActionId::SetVolume => "set-volume",
            SystemActionId::Volume0 => "volume-0",
            SystemActionId::Volume25 => "volume-25",
            SystemActionId::Volume50 => "volume-50",
            SystemActionId::Volume75 => "volume-75",
            SystemActionId::Volume100 => "volume-100",
            SystemActionId::ShowDesktop => "show-desktop",
            SystemActionId::ToggleAppearance => "toggle-system-appearance",
            SystemActionId::ToggleStageManager => "toggle-stage-manager",
            SystemActionId::OpenTrash => "open-trash",
            SystemActionId::EmptyTrash => "empty-trash",
            SystemActionId::EjectAllDisks => "eject-all-disks",
            SystemActionId::ToggleHiddenFiles => "toggle-hidden-files",
            SystemActionId::HideOtherApps => "hide-all-apps-except-frontmost",
            SystemActionId::UnhideAllApps => "unhide-all-hidden-apps",
            SystemActionId::QuitAllApps => "quit-all-apps",
            SystemActionId::DismissNotifications => "dismiss-notifications",
            SystemActionId::ToggleBluetooth => "toggle-bluetooth",
        }
    }

    /// Look an action up by its stored id.
    pub fn from_str(value: &str) -> Option<SystemActionId> {
        SystemActionId::ALL
            .into_iter()
            .find(|action| action.as_str() == value)
    }

    /// The English name, which is also the localization key.
    pub fn english_name(self) -> &'static str {
        match self {
            SystemActionId::LockScreen => "Lock Screen",
            SystemActionId::Sleep => "Sleep",
            SystemActionId::SleepDisplays => "Sleep Displays",
            SystemActionId::Restart => "Restart",
            SystemActionId::ShutDown => "Shut Down",
            SystemActionId::LogOut => "Log Out",
            SystemActionId::ShowScreenSaver => "Show Screen Saver",
            SystemActionId::PlayPause => "Play / Pause",
            SystemActionId::NextTrack => "Next Track",
            SystemActionId::PreviousTrack => "Previous Track",
            SystemActionId::ToggleMute => "Toggle Mute",
            SystemActionId::ToggleMicrophoneMute => "Toggle Microphone Mute",
            SystemActionId::VolumeUp => "Turn Volume Up",
            SystemActionId::VolumeDown => "Turn Volume Down",
            SystemActionId::SetVolume => "Set Volume…",
            SystemActionId::Volume0 => "Set Volume to 0%",
            SystemActionId::Volume25 => "Set Volume to 25%",
            SystemActionId::Volume50 => "Set Volume to 50%",
            SystemActionId::Volume75 => "Set Volume to 75%",
            SystemActionId::Volume100 => "Set Volume to 100%",
            SystemActionId::ShowDesktop => "Show Desktop",
            SystemActionId::ToggleAppearance => "Toggle System Appearance",
            SystemActionId::ToggleStageManager => "Toggle Stage Manager",
            SystemActionId::OpenTrash => "Open Trash",
            SystemActionId::EmptyTrash => "Empty Trash",
            SystemActionId::EjectAllDisks => "Eject All Disks",
            SystemActionId::ToggleHiddenFiles => "Toggle Hidden Files",
            SystemActionId::HideOtherApps => "Hide All Apps Except Frontmost",
            SystemActionId::UnhideAllApps => "Unhide All Hidden Apps",
            SystemActionId::QuitAllApps => "Quit All Applications",
            SystemActionId::DismissNotifications => "Dismiss Notifications",
            SystemActionId::ToggleBluetooth => "Toggle Bluetooth",
        }
    }

    /// The group the action is listed under.
    pub fn category(self) -> SystemActionCategory {
        match self {
            SystemActionId::LockScreen
            | SystemActionId::Sleep
            | SystemActionId::SleepDisplays
            | SystemActionId::Restart
            | SystemActionId::ShutDown
            | SystemActionId::LogOut
            | SystemActionId::ShowScreenSaver => SystemActionCategory::Session,
            SystemActionId::PlayPause | SystemActionId::NextTrack | SystemActionId::PreviousTrack => {
                SystemActionCategory::Media
            }
            SystemActionId::ToggleMute
            | SystemActionId::VolumeUp
            | SystemActionId::VolumeDown
            | SystemActionId::SetVolume
            | SystemActionId::Volume0
            | SystemActionId::Volume25
            | SystemActionId::Volume50
            | SystemActionId::Volume75
            | SystemActionId::Volume100 => SystemActionCategory::Volume,
            SystemActionId::ShowDesktop
            | SystemActionId::ToggleAppearance
            | SystemActionId::ToggleStageManager
            | SystemActionId::OpenTrash
            | SystemActionId::EmptyTrash
            | SystemActionId::EjectAllDisks
            | SystemActionId::ToggleHiddenFiles => SystemActionCategory::Desktop,
            SystemActionId::HideOtherApps
            | SystemActionId::UnhideAllApps
            | SystemActionId::QuitAllApps
            | SystemActionId::DismissNotifications => SystemActionCategory::Files,
            SystemActionId::ToggleBluetooth => SystemActionCategory::Hardware,
            SystemActionId::ToggleMicrophoneMute => SystemActionCategory::Microphone,
        }
    }

    /// Whether the action confirms first, and how.
    pub fn confirmation(self) -> SystemActionConfirmation {
        match self {
            SystemActionId::Restart => SystemActionConfirmation::Required {
                title: "Restart your Mac?".to_string(),
                message: "Applications with unsaved changes may ask you to save.".to_string(),
            },
            SystemActionId::ShutDown => SystemActionConfirmation::Required {
                title: "Shut down your Mac?".to_string(),
                message: "Applications with unsaved changes may ask you to save.".to_string(),
            },
            SystemActionId::LogOut => SystemActionConfirmation::Required {
                title: "Log out now?".to_string(),
                message: "Applications with unsaved changes may ask you to save.".to_string(),
            },
            SystemActionId::EmptyTrash => SystemActionConfirmation::FollowsFinder {
                title: "Empty Trash?".to_string(),
                message: "The items in the Trash will be permanently deleted.".to_string(),
            },
            SystemActionId::QuitAllApps => SystemActionConfirmation::Computed,
            _ => SystemActionConfirmation::None,
        }
    }
}

/// How an action asks before it runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SystemActionConfirmation {
    /// It runs without asking.
    None,
    /// It always asks.
    Required { title: String, message: String },
    /// It asks only while the Recycle Bin's own delete confirmation is on.
    FollowsFinder { title: String, message: String },
    /// It counts its targets before asking, so the copy is built at call time.
    Computed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn there_are_exactly_thirty_two_system_actions() {
        assert_eq!(SystemActionId::ALL.len(), 32);
    }

    #[test]
    fn the_categories_count_up_to_thirty_two() {
        let session = SystemActionId::ALL
            .iter()
            .filter(|a| a.category() == SystemActionCategory::Session)
            .count();
        let media = SystemActionId::ALL
            .iter()
            .filter(|a| a.category() == SystemActionCategory::Media)
            .count();
        let volume = SystemActionId::ALL
            .iter()
            .filter(|a| a.category() == SystemActionCategory::Volume)
            .count();
        let desktop = SystemActionId::ALL
            .iter()
            .filter(|a| a.category() == SystemActionCategory::Desktop)
            .count();
        let files = SystemActionId::ALL
            .iter()
            .filter(|a| a.category() == SystemActionCategory::Files)
            .count();
        let hardware = SystemActionId::ALL
            .iter()
            .filter(|a| a.category() == SystemActionCategory::Hardware)
            .count();
        let microphone = SystemActionId::ALL
            .iter()
            .filter(|a| a.category() == SystemActionCategory::Microphone)
            .count();
        assert_eq!(
            (session, media, volume, desktop, files, hardware, microphone),
            (7, 3, 9, 7, 4, 1, 1)
        );
    }

    #[test]
    fn five_actions_confirm_first() {
        let confirming: Vec<SystemActionId> = SystemActionId::ALL
            .into_iter()
            .filter(|a| a.confirmation() != SystemActionConfirmation::None)
            .collect();
        assert_eq!(
            confirming,
            vec![
                SystemActionId::Restart,
                SystemActionId::ShutDown,
                SystemActionId::LogOut,
                SystemActionId::EmptyTrash,
                SystemActionId::QuitAllApps,
            ]
        );
    }

    #[test]
    fn system_action_ids_round_trip() {
        for action in SystemActionId::ALL {
            assert_eq!(SystemActionId::from_str(action.as_str()), Some(action));
        }
    }

    #[test]
    fn a_built_in_command_resolves_to_its_action() {
        let action = action_for_entry_id("command:clipboard-history").unwrap();
        assert_eq!(action, HotKeyAction::Command(CommandId::ClipboardHistory));
    }

    #[test]
    fn a_system_action_resolves_by_its_prefixed_id() {
        let action = action_for_entry_id("system-action:lock-screen").unwrap();
        assert_eq!(
            action,
            HotKeyAction::SystemAction {
                id: SystemActionId::LockScreen
            }
        );
        assert!(action_for_entry_id("system-action:not-an-action").is_none());
    }

    #[test]
    fn a_stored_item_resolves_when_its_id_is_a_uuid() {
        let id = "3f2504e0-4f89-11d3-9a0c-0305e82c3301";
        assert_eq!(
            action_for_entry_id(&format!("quicklink:{id}")),
            Some(HotKeyAction::Quicklink {
                id: Uuid::parse(id).unwrap()
            })
        );
        assert!(action_for_entry_id("quicklink:not-a-uuid").is_none());
    }

    #[test]
    fn an_unknown_entry_id_resolves_to_nothing() {
        assert!(action_for_entry_id("").is_none());
        assert!(action_for_entry_id("command:nope").is_none());
        assert!(action_for_entry_id("mystery:thing").is_none());
    }

    #[test]
    fn the_entry_id_round_trips() {
        for action in [
            HotKeyAction::Command(CommandId::About),
            HotKeyAction::SystemAction {
                id: SystemActionId::Sleep,
            },
            HotKeyAction::Snippet {
                id: "greeting".to_string(),
            },
        ] {
            let entry_id = action.entry_id().unwrap();
            assert_eq!(action_for_entry_id(&entry_id), Some(action));
        }
    }
}
