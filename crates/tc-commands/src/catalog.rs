//! The catalogue of built-in commands.
//!
//! Upstream keeps the list in `Features/Launcher/Model/CommandCatalog.swift`,
//! the names and shortcut rules in `CommandID.swift`, and the pane that owns
//! each command in `SettingsTab.ownedCommands`. This module is the same table,
//! with the Windows wording already applied.
//!
//! Three things are answered here:
//!
//! * which Settings pane lists a command's controls,
//! * which switches decide that the command exists at all,
//! * what its global shortcut does when it is pressed again.

use std::collections::{BTreeMap, BTreeSet};

use tc_core::CommandId;

/// The Settings pane that lists a command's controls.
///
/// Upstream keeps one pane per feature. "Each built-in command belongs to one
/// Settings pane. A feature's commands are in that feature's pane and only
/// appear while the feature is on. The rest are in Settings -> Commands."
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CommandPane {
    Ai,
    QuickActions,
    Clipboard,
    Emoji,
    FileSearch,
    Navigation,
    Calendar,
    Notes,
    WindowManagement,
    Quicklinks,
    Snippets,
    Commands,
}

impl CommandPane {
    /// The pane's name, as the Settings window shows it.
    pub fn label(self) -> &'static str {
        match self {
            CommandPane::Ai => "AI",
            CommandPane::QuickActions => "Quick Actions",
            CommandPane::Clipboard => "Clipboard",
            CommandPane::Emoji => "Emoji & Symbols",
            CommandPane::FileSearch => "File Search",
            CommandPane::Navigation => "Navigation",
            CommandPane::Calendar => "Calendar",
            CommandPane::Notes => "Notes",
            CommandPane::WindowManagement => "Window Management",
            CommandPane::Quicklinks => "Quicklinks",
            CommandPane::Snippets => "Snippets",
            CommandPane::Commands => "Commands",
        }
    }

    /// Every pane that owns commands, in the order the documentation table
    /// lists them.
    pub const ALL: [CommandPane; 12] = [
        CommandPane::Ai,
        CommandPane::QuickActions,
        CommandPane::Clipboard,
        CommandPane::Emoji,
        CommandPane::FileSearch,
        CommandPane::Navigation,
        CommandPane::Calendar,
        CommandPane::Notes,
        CommandPane::WindowManagement,
        CommandPane::Quicklinks,
        CommandPane::Snippets,
        CommandPane::Commands,
    ];
}

/// What a command's global shortcut does when it is pressed again.
///
/// "Most commands that open a screen work as toggles: press the shortcut again
/// to close. Switch Windows cycles the selection on repeated presses."
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandScreen {
    /// The command opens a screen; a second press closes it.
    Toggle,
    /// The command opens a screen and moves the selection on a second press.
    Cycle,
    /// The command runs and returns; there is no screen to close.
    Action,
}

/// The name the launcher shows for a command.
///
/// The list is upstream's, "unchanged apart from OS nouns"; the four commands
/// that carry the app's own name carry this port's name instead.
pub fn display_name(command: CommandId) -> String {
    match command {
        CommandId::Settings => "YAL Settings".to_string(),
        CommandId::About => "About YAL".to_string(),
        CommandId::Support => "Support YAL".to_string(),
        CommandId::Quit => "Quit YAL".to_string(),
        other => other.english_name().to_string(),
    }
}

/// The key a command's settings are stored under.
///
/// Upstream keeps the settings in one file; the port keeps them in the app's
/// settings file, under a key that names the command.
pub fn settings_key(command: CommandId) -> String {
    command
        .entry_id()
        .strip_prefix("command:")
        .unwrap_or(command.entry_id())
        .to_string()
}

/// The pane that lists a command's controls.
///
/// `None` means the command belongs to no feature pane, so it is listed in
/// Settings -> Commands instead.
pub fn pane_of(command: CommandId) -> Option<CommandPane> {
    use CommandId::*;
    use CommandPane::*;
    Some(match command {
        QuickAi | AiChat => Ai,
        FixGrammar | Rewrite | Translate | Summarize => QuickActions,
        ClipboardHistory | PasteSequentially => Clipboard,
        SearchEmoji => Emoji,
        SearchFiles => FileSearch,
        SwitchWindows | SwitchWindowsBackwards | SearchMenuItems => Navigation,
        JoinNextMeeting | MySchedule | CreateEvent | CopyMeetingLink | OpenInCalendar => Calendar,
        ShowNotes | CreateNote | SearchNotes => Notes,
        CreateWindowLayout | CaptureWindowLayout | SwitchRoom | CreateRoom => WindowManagement,
        CreateQuicklink | SearchQuicklinks | ImportQuicklinks | ExportQuicklinks => Quicklinks,
        SearchSnippets | CreateSnippet => Snippets,
        CreateCustomCommand | SearchCustomCommands => Commands,
        _ => return None,
    })
}

/// The commands a pane lists itself, in the order the documentation table
/// shows them.
///
/// Upstream: "The commands a pane lists itself; its own switch, not
/// `Enable Commands`, decides they exist."
const PANE_COMMANDS: [(CommandPane, &[CommandId]); 12] = [
    (CommandPane::Ai, &[CommandId::QuickAi, CommandId::AiChat]),
    (
        CommandPane::QuickActions,
        &[
            CommandId::FixGrammar,
            CommandId::Rewrite,
            CommandId::Translate,
            CommandId::Summarize,
        ],
    ),
    (
        CommandPane::Clipboard,
        &[CommandId::ClipboardHistory, CommandId::PasteSequentially],
    ),
    (CommandPane::Emoji, &[CommandId::SearchEmoji]),
    (CommandPane::FileSearch, &[CommandId::SearchFiles]),
    (
        CommandPane::Navigation,
        &[
            CommandId::SwitchWindows,
            CommandId::SwitchWindowsBackwards,
            CommandId::SearchMenuItems,
        ],
    ),
    (
        CommandPane::Calendar,
        &[
            CommandId::JoinNextMeeting,
            CommandId::MySchedule,
            CommandId::CreateEvent,
            CommandId::CopyMeetingLink,
            CommandId::OpenInCalendar,
        ],
    ),
    (
        CommandPane::Notes,
        &[
            CommandId::ShowNotes,
            CommandId::CreateNote,
            CommandId::SearchNotes,
        ],
    ),
    (
        CommandPane::WindowManagement,
        &[
            CommandId::CreateWindowLayout,
            CommandId::CaptureWindowLayout,
            CommandId::SwitchRoom,
            CommandId::CreateRoom,
        ],
    ),
    (
        CommandPane::Quicklinks,
        &[
            CommandId::CreateQuicklink,
            CommandId::SearchQuicklinks,
            CommandId::ImportQuicklinks,
            CommandId::ExportQuicklinks,
        ],
    ),
    (
        CommandPane::Snippets,
        &[CommandId::SearchSnippets, CommandId::CreateSnippet],
    ),
    (
        CommandPane::Commands,
        &[
            CommandId::CreateCustomCommand,
            CommandId::SearchCustomCommands,
        ],
    ),
];

/// The commands a pane lists itself.
///
/// Upstream: "The commands a pane lists itself; its own switch, not
/// `Enable Commands`, decides they exist."
pub fn commands_in(pane: CommandPane) -> Vec<CommandId> {
    PANE_COMMANDS
        .iter()
        .find(|(candidate, _)| *candidate == pane)
        .map(|(_, commands)| commands.to_vec())
        .unwrap_or_default()
}

/// The commands the Commands pane lists that no feature pane owns.
///
/// These are the ones in the "Commands" row of the documentation table, minus
/// the two custom-command commands, which follow their own switch.
pub fn commands_in_the_commands_pane() -> Vec<CommandId> {
    CommandId::ALL
        .into_iter()
        .filter(|command| pane_of(*command).is_none())
        .collect()
}

/// Whether a command is one of the two custom-command commands.
///
/// They live in Settings -> Commands under Custom Commands, and they follow
/// "Enable custom commands" rather than "Enable Commands".
pub fn is_custom_command_pair(command: CommandId) -> bool {
    matches!(
        command,
        CommandId::CreateCustomCommand | CommandId::SearchCustomCommands
    )
}

/// What the command's shortcut does when it is pressed again.
pub fn screen_of(command: CommandId) -> CommandScreen {
    use CommandId::*;
    match command {
        // The two window-switching commands move the selection instead.
        SwitchWindows | SwitchWindowsBackwards => CommandScreen::Cycle,
        // The commands that run and return, with no screen of their own.
        FixGrammar | Rewrite | Translate | Summarize | PasteSequentially | JoinNextMeeting
        | CopyMeetingLink | OpenInCalendar | CreateSnippet | CaptureWindowLayout
        | ImportQuicklinks | ExportQuicklinks | ExportSettings | ImportSettings
        | ImportFromRaycast | CheckForUpdates | OpenCamera | Settings | About | Support | Quit => {
            CommandScreen::Action
        }
        _ => CommandScreen::Toggle,
    }
}

/// The switches that decide which commands exist.
///
/// The rules, as upstream states them:
///
/// * "A feature's commands are in that feature's pane and only appear while
///   the feature is on."
/// * "Enable Commands at the top of Settings -> Commands turns off every
///   command in that pane, along with their shortcuts."
/// * "Clearing one row's checkbox only hides that command from search; its
///   shortcut keeps working."
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSwitches {
    /// The "Enable Commands" switch at the top of Settings -> Commands.
    pub commands_enabled: bool,
    /// The "Enable custom commands" switch. Off by default.
    pub custom_commands_enabled: bool,
    /// The "Show in launcher" switch for the custom commands. On by default.
    pub custom_commands_in_launcher: bool,
    /// The commands a disabled feature hides: out of the launcher, and their
    /// shortcuts go too.
    pub hidden: BTreeSet<CommandId>,
    /// The commands whose row checkbox is cleared: kept out of search, while
    /// the shortcut keeps working.
    pub unlisted: BTreeSet<CommandId>,
}

impl Default for CommandSwitches {
    fn default() -> Self {
        CommandSwitches {
            commands_enabled: true,
            custom_commands_enabled: false,
            custom_commands_in_launcher: true,
            hidden: BTreeSet::new(),
            unlisted: BTreeSet::new(),
        }
    }
}

impl CommandSwitches {
    /// Switches with nothing turned off.
    pub fn new() -> CommandSwitches {
        CommandSwitches::default()
    }

    /// Whether the command is offered in the launcher's results.
    ///
    /// A query-driven command is never offered: it answers the text that is
    /// already typed.
    pub fn is_listed(&self, command: CommandId) -> bool {
        if command.is_query_driven() {
            return false;
        }
        if self.hidden.contains(&command) {
            return false;
        }
        if self.unlisted.contains(&command) {
            return false;
        }
        if is_custom_command_pair(command) {
            return self.custom_commands_enabled;
        }
        if pane_of(command) == Some(CommandPane::Commands) || pane_of(command).is_none() {
            return self.commands_enabled;
        }
        true
    }

    /// Whether the command's global shortcut works.
    ///
    /// Clearing a row's checkbox only hides the command from search; its
    /// shortcut keeps working. Turning a feature off, or "Enable Commands" off,
    /// takes the shortcut with it.
    pub fn shortcut_works(&self, command: CommandId) -> bool {
        if command.is_query_driven() {
            // A chord carries no query, so there is nothing for it to act on.
            return false;
        }
        if self.hidden.contains(&command) {
            return false;
        }
        if is_custom_command_pair(command) {
            return self.custom_commands_enabled;
        }
        if pane_of(command) == Some(CommandPane::Commands) || pane_of(command).is_none() {
            return self.commands_enabled;
        }
        true
    }

    /// The commands the launcher offers, in the order upstream lists them.
    pub fn listed_commands(&self) -> Vec<CommandId> {
        CommandId::ALL
            .into_iter()
            .filter(|command| self.is_listed(*command))
            .collect()
    }
}

/// One row of a Settings pane.
///
/// "Each listed command has a launcher checkbox and an alias field. Commands
/// that support a global shortcut also have a recorder."
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CommandRow {
    /// The name the launcher shows instead of the command's own.
    pub alias: Option<String>,
    /// The global shortcut that runs the command.
    pub shortcut: Option<String>,
}

impl CommandRow {
    /// A row with nothing set on it.
    pub fn new() -> CommandRow {
        CommandRow::default()
    }
}

/// The settings of the built-in commands.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CommandSettings {
    /// The switches that decide which commands exist.
    pub switches: CommandSwitches,
    rows: BTreeMap<CommandId, CommandRow>,
}

impl CommandSettings {
    /// Settings with nothing changed.
    pub fn new() -> CommandSettings {
        CommandSettings::default()
    }

    /// The row of a command.
    pub fn row(&self, command: CommandId) -> CommandRow {
        self.rows.get(&command).cloned().unwrap_or_default()
    }

    /// Sets the name a command is searched for.
    pub fn set_alias(&mut self, command: CommandId, alias: Option<String>) {
        let alias = alias
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        self.rows.entry(command).or_default().alias = alias;
    }

    /// Sets the global shortcut that runs a command.
    ///
    /// The three commands that cannot have one are refused: the two that need
    /// text to act on, and the one that would quit the app.
    pub fn set_shortcut(&mut self, command: CommandId, shortcut: Option<String>) -> bool {
        if shortcut.is_some() && !command.can_have_global_shortcut() {
            return false;
        }
        self.rows.entry(command).or_default().shortcut = shortcut;
        true
    }

    /// Turns a row's checkbox on or off.
    ///
    /// Off keeps the command out of the search results; its shortcut keeps
    /// working.
    pub fn set_listed(&mut self, command: CommandId, listed: bool) {
        if listed {
            self.switches.unlisted.remove(&command);
        } else {
            self.switches.unlisted.insert(command);
        }
    }

    /// The name the launcher shows for a command.
    pub fn name(&self, command: CommandId) -> String {
        self.row(command)
            .alias
            .unwrap_or_else(|| display_name(command))
    }

    /// The rows a pane lists.
    pub fn pane_rows(&self, pane: CommandPane) -> Vec<(CommandId, CommandRow)> {
        commands_in(pane)
            .into_iter()
            .map(|command| (command, self.row(command)))
            .collect()
    }
}
