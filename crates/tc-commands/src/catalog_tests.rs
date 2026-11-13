//! The catalogue: which pane owns a command, and which switches decide that
//! it exists.

use tc_core::CommandId;

use crate::catalog::{
    commands_in, commands_in_the_commands_pane, display_name, is_custom_command_pair, pane_of,
    screen_of, CommandPane, CommandScreen, CommandSettings, CommandSwitches,
};

/// The commands each pane lists, as the documentation table shows them.
const THE_TABLE: [(CommandPane, &[CommandId]); 12] = [
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

#[test]
fn each_built_in_command_belongs_to_one_settings_pane() {
    for (pane, commands) in THE_TABLE {
        for command in commands {
            assert_eq!(
                pane_of(*command),
                Some(pane),
                "{} belongs to the {} pane",
                command.english_name(),
                pane.label()
            );
        }
    }
}

#[test]
fn a_pane_lists_the_commands_it_owns() {
    for (pane, commands) in THE_TABLE {
        assert_eq!(
            commands_in(pane),
            commands.to_vec(),
            "the {} pane",
            pane.label()
        );
    }
}

#[test]
fn the_commands_pane_holds_the_commands_no_feature_pane_owns() {
    let owned = commands_in_the_commands_pane();
    for command in [
        CommandId::CalculatorHistory,
        CommandId::Define,
        CommandId::OpenCamera,
        CommandId::ExportSettings,
        CommandId::ImportSettings,
        CommandId::ImportFromRaycast,
        CommandId::CheckForUpdates,
        CommandId::Settings,
        CommandId::About,
        CommandId::Support,
        CommandId::Quit,
    ] {
        assert!(
            owned.contains(&command),
            "{} is in the Commands pane",
            command.english_name()
        );
        assert_eq!(pane_of(command), None);
    }
}

#[test]
fn every_command_is_in_exactly_one_place() {
    let mut seen: Vec<CommandId> = Vec::new();
    for pane in CommandPane::ALL {
        seen.extend(commands_in(pane));
    }
    seen.extend(commands_in_the_commands_pane());
    seen.extend([CommandId::OpenInBrowser, CommandId::RunShellCommand]);
    seen.sort();
    seen.dedup();
    let mut all = CommandId::ALL.to_vec();
    all.sort();
    assert_eq!(seen, all, "every built-in command is placed once");
}

#[test]
fn a_feature_pane_only_shows_its_commands_while_the_feature_is_on() {
    let mut switches = CommandSwitches::new();
    assert!(switches.is_listed(CommandId::ClipboardHistory));
    assert!(switches.shortcut_works(CommandId::ClipboardHistory));

    switches.hidden.insert(CommandId::ClipboardHistory);
    assert!(!switches.is_listed(CommandId::ClipboardHistory));
    assert!(
        !switches.shortcut_works(CommandId::ClipboardHistory),
        "a feature that is off takes its shortcut with it"
    );
}

#[test]
fn enable_commands_turns_off_the_commands_in_that_pane_and_their_shortcuts() {
    let mut switches = CommandSwitches::new();
    switches.commands_enabled = false;
    assert!(!switches.is_listed(CommandId::CalculatorHistory));
    assert!(!switches.shortcut_works(CommandId::CalculatorHistory));
    assert!(
        switches.shortcut_works(CommandId::ClipboardHistory),
        "another pane's commands keep working"
    );
}

#[test]
fn clearing_a_row_checkbox_hides_the_command_from_search_but_keeps_its_shortcut() {
    let mut settings = CommandSettings::new();
    settings.set_listed(CommandId::SearchFiles, false);
    assert!(!settings.switches.is_listed(CommandId::SearchFiles));
    assert!(
        settings.switches.shortcut_works(CommandId::SearchFiles),
        "the shortcut keeps working"
    );
    settings.set_listed(CommandId::SearchFiles, true);
    assert!(settings.switches.is_listed(CommandId::SearchFiles));
}

#[test]
fn the_custom_commands_follow_their_own_switch() {
    let mut switches = CommandSwitches::new();
    assert!(
        !switches.custom_commands_enabled,
        "custom commands are off by default"
    );
    assert!(!switches.is_listed(CommandId::CreateCustomCommand));
    assert!(!switches.shortcut_works(CommandId::CreateCustomCommand));

    switches.custom_commands_enabled = true;
    assert!(switches.is_listed(CommandId::CreateCustomCommand));
    assert!(switches.is_listed(CommandId::SearchCustomCommands));
    assert!(switches.shortcut_works(CommandId::CreateCustomCommand));
    assert!(is_custom_command_pair(CommandId::SearchCustomCommands));
}

#[test]
fn turning_off_show_in_launcher_hides_the_section_but_keeps_the_shortcuts() {
    let mut switches = CommandSwitches::new();
    switches.custom_commands_enabled = true;
    assert!(switches.custom_commands_in_launcher, "on by default");
    switches.custom_commands_in_launcher = false;
    assert!(
        switches.shortcut_works(CommandId::CreateCustomCommand),
        "every shortcut keeps working"
    );
}

#[test]
fn most_commands_that_open_a_screen_work_as_toggles() {
    for command in [
        CommandId::ClipboardHistory,
        CommandId::SearchFiles,
        CommandId::SearchEmoji,
        CommandId::SearchQuicklinks,
        CommandId::CalculatorHistory,
        CommandId::Define,
        CommandId::MySchedule,
        CommandId::ShowNotes,
        CommandId::SearchNotes,
        CommandId::SearchSnippets,
        CommandId::SearchCustomCommands,
        CommandId::SwitchRoom,
    ] {
        assert_eq!(
            screen_of(command),
            CommandScreen::Toggle,
            "{} opens a screen, so it toggles",
            command.english_name()
        );
    }
}

#[test]
fn switch_windows_cycles_the_selection_on_repeated_presses() {
    assert_eq!(screen_of(CommandId::SwitchWindows), CommandScreen::Cycle);
    assert_eq!(
        screen_of(CommandId::SwitchWindowsBackwards),
        CommandScreen::Cycle
    );
}

#[test]
fn a_command_that_runs_and_returns_is_not_a_toggle() {
    for command in [
        CommandId::FixGrammar,
        CommandId::Rewrite,
        CommandId::Translate,
        CommandId::Summarize,
        CommandId::PasteSequentially,
        CommandId::JoinNextMeeting,
        CommandId::CopyMeetingLink,
        CommandId::OpenInCalendar,
        CommandId::CreateSnippet,
        CommandId::CaptureWindowLayout,
        CommandId::Quit,
    ] {
        assert_eq!(
            screen_of(command),
            CommandScreen::Action,
            "{} runs and returns",
            command.english_name()
        );
    }
}

#[test]
fn two_more_commands_only_appear_for_what_you_type() {
    let switches = CommandSwitches::new();
    for command in [CommandId::OpenInBrowser, CommandId::RunShellCommand] {
        assert!(command.is_query_driven());
        assert!(!switches.is_listed(command));
        assert!(!switches.shortcut_works(command));
    }
}

#[test]
fn a_command_can_have_a_global_shortcut_except_the_three() {
    for command in CommandId::ALL {
        let expected = !matches!(
            command,
            CommandId::OpenInBrowser | CommandId::RunShellCommand | CommandId::Quit
        );
        assert_eq!(
            command.can_have_global_shortcut(),
            expected,
            "{} and a shortcut",
            command.english_name()
        );
    }
}

#[test]
fn a_command_that_cannot_have_a_shortcut_is_refused_one() {
    let mut settings = CommandSettings::new();
    assert!(settings.set_shortcut(CommandId::SearchFiles, Some("Ctrl+Alt+F".to_string())));
    assert_eq!(
        settings.row(CommandId::SearchFiles).shortcut.as_deref(),
        Some("Ctrl+Alt+F")
    );
    assert!(!settings.set_shortcut(CommandId::Quit, Some("Ctrl+Alt+Q".to_string())));
    assert_eq!(settings.row(CommandId::Quit).shortcut, None);
}

#[test]
fn a_command_can_be_given_an_alias() {
    let mut settings = CommandSettings::new();
    assert_eq!(settings.name(CommandId::SearchFiles), "Search Files");
    settings.set_alias(CommandId::SearchFiles, Some("Find a file".to_string()));
    assert_eq!(settings.name(CommandId::SearchFiles), "Find a file");
    settings.set_alias(CommandId::SearchFiles, Some("   ".to_string()));
    assert_eq!(settings.name(CommandId::SearchFiles), "Search Files");
}

#[test]
fn the_commands_carry_this_port_s_own_name() {
    assert_eq!(display_name(CommandId::Settings), "YAL Settings");
    assert_eq!(display_name(CommandId::About), "About YAL");
    assert_eq!(display_name(CommandId::Support), "Support YAL");
    assert_eq!(display_name(CommandId::Quit), "Quit YAL");
    assert_eq!(display_name(CommandId::SearchFiles), "Search Files");
}
