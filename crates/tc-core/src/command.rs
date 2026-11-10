//! The built-in command registry.
//!
//! Every command the palette can run has a stable id here. The ids are the ones
//! upstream ships (`command:ai-chat` and friends), because a deeplink, a global
//! shortcut and a backup all name a command by that id and the ids must survive
//! the port.

/// A built-in command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CommandId {
    QuickAi,
    AiChat,
    FixGrammar,
    Rewrite,
    Translate,
    Summarize,
    CalculatorHistory,
    ClipboardHistory,
    PasteSequentially,
    SearchEmoji,
    SearchFiles,
    SearchMenuItems,
    SwitchWindows,
    SwitchWindowsBackwards,
    OpenCamera,
    OpenInBrowser,
    RunShellCommand,
    CreateCustomCommand,
    SearchCustomCommands,
    Define,
    JoinNextMeeting,
    MySchedule,
    CreateEvent,
    CopyMeetingLink,
    OpenInCalendar,
    ShowNotes,
    CreateNote,
    SearchNotes,
    CreateWindowLayout,
    CaptureWindowLayout,
    SwitchRoom,
    CreateRoom,
    CreateQuicklink,
    SearchQuicklinks,
    ImportQuicklinks,
    ExportQuicklinks,
    SearchSnippets,
    CreateSnippet,
    ExportSettings,
    ImportSettings,
    ImportFromRaycast,
    CheckForUpdates,
    Settings,
    About,
    Support,
    Quit,
}

impl CommandId {
    /// Every built-in command, in the order upstream declares them.
    pub const ALL: [CommandId; 46] = [
        CommandId::QuickAi,
        CommandId::AiChat,
        CommandId::FixGrammar,
        CommandId::Rewrite,
        CommandId::Translate,
        CommandId::Summarize,
        CommandId::CalculatorHistory,
        CommandId::ClipboardHistory,
        CommandId::PasteSequentially,
        CommandId::SearchEmoji,
        CommandId::SearchFiles,
        CommandId::SearchMenuItems,
        CommandId::SwitchWindows,
        CommandId::SwitchWindowsBackwards,
        CommandId::OpenCamera,
        CommandId::OpenInBrowser,
        CommandId::RunShellCommand,
        CommandId::CreateCustomCommand,
        CommandId::SearchCustomCommands,
        CommandId::Define,
        CommandId::JoinNextMeeting,
        CommandId::MySchedule,
        CommandId::CreateEvent,
        CommandId::CopyMeetingLink,
        CommandId::OpenInCalendar,
        CommandId::ShowNotes,
        CommandId::CreateNote,
        CommandId::SearchNotes,
        CommandId::CreateWindowLayout,
        CommandId::CaptureWindowLayout,
        CommandId::SwitchRoom,
        CommandId::CreateRoom,
        CommandId::CreateQuicklink,
        CommandId::SearchQuicklinks,
        CommandId::ImportQuicklinks,
        CommandId::ExportQuicklinks,
        CommandId::SearchSnippets,
        CommandId::CreateSnippet,
        CommandId::ExportSettings,
        CommandId::ImportSettings,
        CommandId::ImportFromRaycast,
        CommandId::CheckForUpdates,
        CommandId::Settings,
        CommandId::About,
        CommandId::Support,
        CommandId::Quit,
    ];

    /// The stable id, exactly as upstream ships it.
    pub fn entry_id(self) -> &'static str {
        match self {
            CommandId::QuickAi => "command:ai-chat",
            CommandId::AiChat => "command:ai-chat-window",
            CommandId::FixGrammar => "command:fix-grammar",
            CommandId::Rewrite => "command:rewrite",
            CommandId::Translate => "command:translate",
            CommandId::Summarize => "command:summarize",
            CommandId::CalculatorHistory => "command:calculator-history",
            CommandId::ClipboardHistory => "command:clipboard-history",
            CommandId::PasteSequentially => "command:paste-sequentially",
            CommandId::SearchEmoji => "command:search-emoji",
            CommandId::SearchFiles => "command:search-files",
            CommandId::SearchMenuItems => "command:search-menu-items",
            CommandId::SwitchWindows => "command:switch-windows",
            CommandId::SwitchWindowsBackwards => "command:switch-windows-backwards",
            CommandId::OpenCamera => "command:open-camera",
            CommandId::OpenInBrowser => "command:open-in-browser",
            CommandId::RunShellCommand => "command:run-shell-command",
            CommandId::CreateCustomCommand => "command:create-custom-command",
            CommandId::SearchCustomCommands => "command:search-custom-commands",
            CommandId::Define => "command:define",
            CommandId::JoinNextMeeting => "command:join-next-meeting",
            CommandId::MySchedule => "command:my-schedule",
            CommandId::CreateEvent => "command:create-event",
            CommandId::CopyMeetingLink => "command:copy-meeting-link",
            CommandId::OpenInCalendar => "command:open-in-calendar",
            CommandId::ShowNotes => "command:show-notes",
            CommandId::CreateNote => "command:create-note",
            CommandId::SearchNotes => "command:search-notes",
            CommandId::CreateWindowLayout => "command:create-window-layout",
            CommandId::CaptureWindowLayout => "command:capture-window-layout",
            CommandId::SwitchRoom => "command:switch-room",
            CommandId::CreateRoom => "command:create-room",
            CommandId::CreateQuicklink => "command:create-quicklink",
            CommandId::SearchQuicklinks => "command:search-quicklinks",
            CommandId::ImportQuicklinks => "command:import-quicklinks",
            CommandId::ExportQuicklinks => "command:export-quicklinks",
            CommandId::SearchSnippets => "command:search-snippets",
            CommandId::CreateSnippet => "command:create-snippet",
            CommandId::ExportSettings => "command:export-settings",
            CommandId::ImportSettings => "command:import-settings",
            CommandId::ImportFromRaycast => "command:import-from-raycast",
            CommandId::CheckForUpdates => "command:check-for-updates",
            CommandId::Settings => "command:settings",
            CommandId::About => "command:about",
            CommandId::Support => "command:support",
            CommandId::Quit => "command:quit",
        }
    }

    /// Look a command up by its stable id.
    pub fn from_entry_id(entry_id: &str) -> Option<CommandId> {
        CommandId::ALL
            .into_iter()
            .find(|c| c.entry_id() == entry_id)
    }

    /// The English name, which is also the localization key upstream uses.
    ///
    /// The four Quick Action commands borrow their names from the Quick Action
    /// catalog, so they are spelled out here rather than derived.
    pub fn english_name(self) -> &'static str {
        match self {
            CommandId::QuickAi => "Quick AI",
            CommandId::AiChat => "AI Chat",
            CommandId::FixGrammar => "Fix Grammar",
            CommandId::Rewrite => "Rewrite",
            CommandId::Translate => "Translate",
            CommandId::Summarize => "Summarize",
            CommandId::CalculatorHistory => "Calculator History",
            CommandId::ClipboardHistory => "Clipboard History",
            CommandId::PasteSequentially => "Paste Sequentially",
            CommandId::SearchEmoji => "Search Emoji & Symbols",
            CommandId::SearchFiles => "Search Files",
            CommandId::SearchMenuItems => "Search Menu Bar Items",
            CommandId::SwitchWindows => "Switch Windows",
            CommandId::SwitchWindowsBackwards => "Switch Windows Backwards",
            CommandId::OpenCamera => "Open Camera",
            CommandId::OpenInBrowser => "Open in Browser",
            CommandId::RunShellCommand => "Run Shell Command",
            CommandId::CreateCustomCommand => "Create Custom Command",
            CommandId::SearchCustomCommands => "Search Custom Commands",
            CommandId::Define => "Define Word",
            CommandId::JoinNextMeeting => "Join Next Meeting",
            CommandId::MySchedule => "My Schedule",
            CommandId::CreateEvent => "Create Event",
            CommandId::CopyMeetingLink => "Copy Meeting Link",
            CommandId::OpenInCalendar => "Open in Calendar",
            CommandId::ShowNotes => "Show Notes",
            CommandId::CreateNote => "Create Note",
            CommandId::SearchNotes => "Search Notes",
            CommandId::CreateWindowLayout => "Create Window Layout",
            CommandId::CaptureWindowLayout => "Create Layout from Current Windows",
            CommandId::SwitchRoom => "Switch Room",
            CommandId::CreateRoom => "Create Room",
            CommandId::CreateQuicklink => "Create Quicklink",
            CommandId::SearchQuicklinks => "Search Quicklinks",
            CommandId::ImportQuicklinks => "Import Quicklinks",
            CommandId::ExportQuicklinks => "Export Quicklinks",
            CommandId::SearchSnippets => "Search Snippets",
            CommandId::CreateSnippet => "Create Snippet",
            CommandId::ExportSettings => "Export Backup",
            CommandId::ImportSettings => "Import Backup",
            CommandId::ImportFromRaycast => "Import from Raycast",
            CommandId::CheckForUpdates => "Check for Updates",
            CommandId::Settings => "Tinycast Settings",
            CommandId::About => "About Tinycast",
            CommandId::Support => "Support Tinycast",
            CommandId::Quit => "Quit Tinycast",
        }
    }

    /// Query-driven commands take the typed text as their input, so they are
    /// built where the query is offered and never listed as a result.
    pub fn is_query_driven(self) -> bool {
        matches!(self, CommandId::OpenInBrowser | CommandId::RunShellCommand)
    }

    /// Whether a global shortcut can be bound to this command.
    ///
    /// A query-driven command carries no query in a chord, and no chord should
    /// be able to terminate the app outright.
    pub fn can_have_global_shortcut(self) -> bool {
        !self.is_query_driven() && self != CommandId::Quit
    }

    /// Queries this command wins until the user opens a rival more.
    pub fn boosted_terms(self) -> &'static [&'static str] {
        match self {
            CommandId::QuickAi => &["ai"],
            CommandId::AiChat => &["chat"],
            _ => &[],
        }
    }

    /// How strongly the command is suggested in the empty state, highest first.
    pub fn suggestion_priority(self) -> Option<u8> {
        match self {
            CommandId::ClipboardHistory => Some(80),
            CommandId::SearchFiles => Some(70),
            CommandId::MySchedule => Some(60),
            CommandId::SearchEmoji => Some(50),
            CommandId::CreateQuicklink | CommandId::CreateSnippet => Some(30),
            _ => None,
        }
    }

    /// The built-in Quick Action this command runs, if it is one.
    pub fn built_in_quick_action(self) -> Option<BuiltInQuickAction> {
        match self {
            CommandId::FixGrammar => Some(BuiltInQuickAction::FixGrammar),
            CommandId::Rewrite => Some(BuiltInQuickAction::Rewrite),
            CommandId::Translate => Some(BuiltInQuickAction::Translate),
            CommandId::Summarize => Some(BuiltInQuickAction::Summarize),
            _ => None,
        }
    }
}

/// The four Quick Actions that ship built in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuiltInQuickAction {
    FixGrammar,
    Rewrite,
    Translate,
    Summarize,
}

impl BuiltInQuickAction {
    /// All four, in the order upstream lists them.
    pub const ALL: [BuiltInQuickAction; 4] = [
        BuiltInQuickAction::FixGrammar,
        BuiltInQuickAction::Rewrite,
        BuiltInQuickAction::Translate,
        BuiltInQuickAction::Summarize,
    ];

    /// The title, which is also the localization key.
    pub fn title(self) -> &'static str {
        match self {
            BuiltInQuickAction::FixGrammar => "Fix Grammar",
            BuiltInQuickAction::Rewrite => "Rewrite",
            BuiltInQuickAction::Translate => "Translate",
            BuiltInQuickAction::Summarize => "Summarize",
        }
    }

    /// The command that runs this action.
    pub fn command(self) -> CommandId {
        match self {
            BuiltInQuickAction::FixGrammar => CommandId::FixGrammar,
            BuiltInQuickAction::Rewrite => CommandId::Rewrite,
            BuiltInQuickAction::Translate => CommandId::Translate,
            BuiltInQuickAction::Summarize => CommandId::Summarize,
        }
    }
}

/// The commands the compact bar offers.
pub const DEFAULT_COMPACT_BAR_COMMANDS: [CommandId; 4] = [
    CommandId::ClipboardHistory,
    CommandId::SearchEmoji,
    CommandId::SearchFiles,
    CommandId::SearchQuicklinks,
];

/// The full set the compact bar may be configured with.
pub const COMPACT_BAR_OPTIONS: [CommandId; 8] = [
    CommandId::ClipboardHistory,
    CommandId::SearchEmoji,
    CommandId::SearchFiles,
    CommandId::SearchQuicklinks,
    CommandId::SearchSnippets,
    CommandId::ShowNotes,
    CommandId::MySchedule,
    CommandId::QuickAi,
];

/// How many commands the compact bar holds.
pub const COMPACT_BAR_LIMIT: usize = 4;

/// Whether a selection of compact bar commands is acceptable.
pub fn valid_compact_bar_commands(commands: &[CommandId]) -> bool {
    commands.len() <= COMPACT_BAR_LIMIT
        && commands
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            == commands.len()
        && commands.iter().all(|c| COMPACT_BAR_OPTIONS.contains(c))
}

/// Parse compact bar commands from their stored identifiers.
///
/// A list that does not resolve completely, or that is not a valid selection,
/// is rejected rather than repaired.
pub fn compact_bar_commands_from(identifiers: &[String]) -> Option<Vec<CommandId>> {
    let commands: Vec<CommandId> = identifiers
        .iter()
        .filter_map(|id| CommandId::from_entry_id(id))
        .collect();
    if commands.len() != identifiers.len() || !valid_compact_bar_commands(&commands) {
        return None;
    }
    Some(commands)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn there_are_exactly_forty_six_built_in_commands() {
        assert_eq!(CommandId::ALL.len(), 46);
    }

    #[test]
    fn command_ids_are_unique() {
        let ids: BTreeSet<&str> = CommandId::ALL.iter().map(|c| c.entry_id()).collect();
        assert_eq!(ids.len(), CommandId::ALL.len());
    }

    #[test]
    fn every_command_id_round_trips() {
        for command in CommandId::ALL {
            assert_eq!(CommandId::from_entry_id(command.entry_id()), Some(command));
        }
    }

    #[test]
    fn unknown_command_ids_do_not_resolve() {
        assert_eq!(CommandId::from_entry_id("command:not-a-command"), None);
        assert_eq!(CommandId::from_entry_id(""), None);
    }

    #[test]
    fn three_commands_can_never_have_a_global_shortcut() {
        let blocked: Vec<CommandId> = CommandId::ALL
            .into_iter()
            .filter(|c| !c.can_have_global_shortcut())
            .collect();
        assert_eq!(
            blocked,
            vec![
                CommandId::OpenInBrowser,
                CommandId::RunShellCommand,
                CommandId::Quit
            ]
        );
    }

    #[test]
    fn query_driven_commands_are_open_in_browser_and_run_shell_command() {
        let query_driven: Vec<CommandId> = CommandId::ALL
            .into_iter()
            .filter(|c| c.is_query_driven())
            .collect();
        assert_eq!(
            query_driven,
            vec![CommandId::OpenInBrowser, CommandId::RunShellCommand]
        );
    }

    #[test]
    fn compact_bar_holds_at_most_four_distinct_options() {
        assert!(valid_compact_bar_commands(&DEFAULT_COMPACT_BAR_COMMANDS));
        assert!(!valid_compact_bar_commands(&COMPACT_BAR_OPTIONS));
        assert!(!valid_compact_bar_commands(&[
            CommandId::ClipboardHistory,
            CommandId::ClipboardHistory
        ]));
        assert!(!valid_compact_bar_commands(&[CommandId::Settings]));
    }

    #[test]
    fn compact_bar_commands_from_rejects_a_partial_list() {
        let good = vec!["command:clipboard-history".to_string()];
        assert_eq!(
            compact_bar_commands_from(&good),
            Some(vec![CommandId::ClipboardHistory])
        );

        let bad = vec![
            "command:clipboard-history".to_string(),
            "command:nope".to_string(),
        ];
        assert_eq!(compact_bar_commands_from(&bad), None);

        let too_many = vec![
            "command:clipboard-history".to_string(),
            "command:search-emoji".to_string(),
            "command:search-files".to_string(),
            "command:search-quicklinks".to_string(),
            "command:search-snippets".to_string(),
        ];
        assert_eq!(compact_bar_commands_from(&too_many), None);
    }

    #[test]
    fn the_four_built_in_quick_actions_map_to_commands() {
        assert_eq!(BuiltInQuickAction::ALL.len(), 4);
        for action in BuiltInQuickAction::ALL {
            assert_eq!(action.command().built_in_quick_action(), Some(action));
        }
    }

    #[test]
    fn suggestion_priority_is_ordered_highest_first() {
        let mut priorities: Vec<(u8, CommandId)> = CommandId::ALL
            .into_iter()
            .filter_map(|c| c.suggestion_priority().map(|p| (p, c)))
            .collect();
        priorities.sort_by_key(|entry| std::cmp::Reverse(entry.0));
        assert_eq!(
            priorities,
            vec![
                (80, CommandId::ClipboardHistory),
                (70, CommandId::SearchFiles),
                (60, CommandId::MySchedule),
                (50, CommandId::SearchEmoji),
                (30, CommandId::CreateQuicklink),
                (30, CommandId::CreateSnippet),
            ]
        );
    }
}
