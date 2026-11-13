//! The custom command: its fields, its defaults and what it refuses.

use crate::model::{CustomCommand, CustomCommandArgument, ValidationError};

#[test]
fn a_new_command_carries_the_documented_defaults() {
    let command = CustomCommand::new("Sleep", "Start-Sleep 5");
    assert_eq!(command.name, "Sleep");
    assert!(command.is_enabled, "enabled");
    assert!(command.shows_in_root_search, "show in root search");
    assert!(
        !command.loads_shell_environment,
        "load shell environment is off"
    );
    assert!(command.requires_confirmation, "needs confirmation is on");
    assert!(!command.shows_confirmation, "show confirmation is off");
    assert!(!command.shows_output, "show output is off");
    assert!(command.arguments.is_empty());
    assert!(command.working_directory.is_none());
}

#[test]
fn a_command_needs_a_name() {
    let command = CustomCommand::new("   ", "echo hello");
    assert_eq!(
        command.validated(&[]).unwrap_err(),
        ValidationError::EmptyName
    );
}

#[test]
fn a_command_needs_something_to_run() {
    let command = CustomCommand::new("Sleep", "  ");
    assert_eq!(
        command.validated(&[]).unwrap_err(),
        ValidationError::EmptyCommand
    );
}

#[test]
fn two_commands_cannot_share_a_name() {
    let first = CustomCommand::new("Sleep", "Start-Sleep 5");
    let second = CustomCommand::new("sleep", "Start-Sleep 10");
    assert_eq!(
        second.validated(&[first]).unwrap_err(),
        ValidationError::DuplicateName
    );
}

#[test]
fn a_name_cannot_hold_a_null_character() {
    let mut command = CustomCommand::new("Sleep", "Start-Sleep 5");
    command.name = "Sle\0ep".to_string();
    assert_eq!(
        command.validated(&[]).unwrap_err(),
        ValidationError::InvalidCharacter
    );
}

#[test]
fn a_blank_argument_is_dropped_and_there_are_at_most_three() {
    let arguments = vec![
        CustomCommandArgument::new("First", false),
        CustomCommandArgument::new("   ", false),
        CustomCommandArgument::new("Second", true),
        CustomCommandArgument::new("Third", false),
        CustomCommandArgument::new("Fourth", false),
    ];
    let cleaned = CustomCommandArgument::sanitized(&arguments);
    assert_eq!(cleaned.len(), 3);
    assert_eq!(cleaned[0].name, "First");
    assert_eq!(cleaned[1].name, "Second");
    assert!(cleaned[1].is_optional);
    assert_eq!(cleaned[2].name, "Third");
}

#[test]
fn an_argument_is_addressed_by_its_position() {
    assert_eq!(CustomCommandArgument::field_id_at(0), "$1");
    assert_eq!(CustomCommandArgument::field_id_at(2), "$3");
}

#[test]
fn a_required_argument_cannot_be_left_empty() {
    let mut command = CustomCommand::new("Sleep", "Start-Sleep $1");
    command.arguments = vec![CustomCommandArgument::new("Seconds", false)];
    assert_eq!(
        command.positional_values(&[String::new()]),
        None,
        "a required value that is empty stops the command"
    );
    assert_eq!(command.first_missing(&[String::new()]), Some(0));
    assert_eq!(command.first_missing(&["5".to_string()]), None);
    assert_eq!(
        command.positional_values(&["5".to_string()]),
        Some(vec!["5".to_string()])
    );
}

#[test]
fn an_empty_optional_argument_keeps_its_position() {
    let mut command = CustomCommand::new("Move", "Move-Item $1 $2");
    command.arguments = vec![
        CustomCommandArgument::new("From", false),
        CustomCommandArgument::new("To", true),
    ];
    let values = command
        .positional_values(&["a.txt".to_string(), String::new()])
        .expect("the optional value may be empty");
    assert_eq!(values.len(), 2, "the second value keeps its place");
    assert_eq!(values[1], "");
}

#[test]
fn a_command_has_a_row_id_that_finds_it_again() {
    let command = CustomCommand::new("Sleep", "Start-Sleep 5");
    let entry_id = command.entry_id();
    assert!(entry_id.starts_with("custom-command:"));
    assert_eq!(CustomCommand::id_from_entry_id(&entry_id), Some(command.id));
    assert_eq!(
        CustomCommand::id_from_entry_id("command:search-files"),
        None
    );
}

#[test]
fn a_command_without_an_icon_uses_the_shared_symbol() {
    let mut command = CustomCommand::new("Sleep", "Start-Sleep 5");
    assert_eq!(command.symbol(), "terminal");
    command.icon_symbol = Some("  ".to_string());
    assert_eq!(command.symbol(), "terminal");
    command.icon_symbol = Some("moon".to_string());
    assert_eq!(command.symbol(), "moon");
}

#[test]
fn a_command_is_stored_trimmed() {
    let mut command = CustomCommand::new("  Sleep  ", "  Start-Sleep 5  ");
    command.working_directory = Some("   ".to_string());
    let cleaned = command.cleaned();
    assert_eq!(cleaned.name, "Sleep");
    assert_eq!(cleaned.command, "Start-Sleep 5");
    assert_eq!(cleaned.working_directory, None);
}
