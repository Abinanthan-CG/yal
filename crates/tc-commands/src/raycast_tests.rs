//! Reading a folder of Raycast script commands.

use std::path::Path;

use crate::raycast::{command_from_source, RaycastScriptImport};

/// A script that names a program Windows has.
const SCRIPT: &str = r#"#!/usr/bin/env cmd
# @raycast.schemaVersion 1
# @raycast.title Disk usage
# @raycast.description Shows the disk usage
# @raycast.mode compact
# @raycast.needsConfirmation true
# @raycast.argument1 {"type":"text","placeholder":"Drive","required":true}
# @raycast.argument2 {"type":"text","placeholder":"Label","optional":true}
# @raycast.currentDirectoryPath C:\Windows
echo %1
"#;

fn write(folder: &Path, name: &str, body: &str) -> std::path::PathBuf {
    let path = folder.join(name);
    std::fs::write(&path, body).expect("the script is written");
    path
}

#[test]
fn a_file_counts_as_a_script_command_when_it_has_a_shebang_and_a_title() {
    let directory = tempfile::tempdir().expect("a folder");
    let path = write(directory.path(), "disk.cmd", SCRIPT);
    let command = command_from_source(&path, SCRIPT).expect("the script is read");
    assert_eq!(command.name, "Disk usage");
}

#[test]
fn a_file_without_a_shebang_is_skipped() {
    let directory = tempfile::tempdir().expect("a folder");
    let body = "# @raycast.title Helper\n# not a script\n";
    let path = write(directory.path(), "helper.txt", body);
    let error = command_from_source(&path, body).expect_err("the file is skipped");
    assert!(error.contains("shebang"), "{error}");
}

#[test]
fn a_file_without_a_title_is_skipped() {
    let directory = tempfile::tempdir().expect("a folder");
    let body = "#!/usr/bin/env cmd\n# a helper script\n";
    let path = write(directory.path(), "helper.cmd", body);
    let error = command_from_source(&path, body).expect_err("the file is skipped");
    assert!(error.contains("@raycast.title"), "{error}");
}

#[test]
fn the_title_is_the_name() {
    let directory = tempfile::tempdir().expect("a folder");
    let path = write(directory.path(), "disk.cmd", SCRIPT);
    let command = command_from_source(&path, SCRIPT).expect("the script is read");
    assert_eq!(command.name, "Disk usage");
}

#[test]
fn compact_and_full_output_turn_show_output_on() {
    let directory = tempfile::tempdir().expect("a folder");
    let path = write(directory.path(), "disk.cmd", SCRIPT);
    let command = command_from_source(&path, SCRIPT).expect("the script is read");
    assert!(command.shows_output);

    let silent = SCRIPT.replace("compact", "silent");
    let command = command_from_source(&path, &silent).expect("the script is read");
    assert!(!command.shows_output);
}

#[test]
fn needs_confirmation_is_read() {
    let directory = tempfile::tempdir().expect("a folder");
    let path = write(directory.path(), "disk.cmd", SCRIPT);
    let command = command_from_source(&path, SCRIPT).expect("the script is read");
    assert!(command.requires_confirmation);

    let without = SCRIPT.replace("true", "false");
    let command = command_from_source(&path, &without).expect("the script is read");
    assert!(!command.requires_confirmation);
}

#[test]
fn the_arguments_are_named_after_their_placeholders() {
    let directory = tempfile::tempdir().expect("a folder");
    let path = write(directory.path(), "disk.cmd", SCRIPT);
    let command = command_from_source(&path, SCRIPT).expect("the script is read");
    assert_eq!(command.arguments.len(), 2);
    assert_eq!(command.arguments[0].name, "Drive");
    assert!(!command.arguments[0].is_optional);
    assert_eq!(command.arguments[1].name, "Label");
    assert!(command.arguments[1].is_optional);
}

#[test]
fn the_run_in_folder_is_the_declared_one_or_the_scripts_own_folder() {
    let directory = tempfile::tempdir().expect("a folder");
    let path = write(directory.path(), "disk.cmd", SCRIPT);
    let command = command_from_source(&path, SCRIPT).expect("the script is read");
    assert_eq!(
        command.working_directory.as_deref(),
        Some("C:\\Windows"),
        "the declared folder is used"
    );

    let body = SCRIPT.replace("# @raycast.currentDirectoryPath C:\\Windows\n", "");
    let command = command_from_source(&path, &body).expect("the script is read");
    assert_eq!(
        command.working_directory,
        Some(directory.path().to_string_lossy().into_owned()),
        "otherwise the script's own folder"
    );
}

#[test]
fn a_script_with_an_interpreter_windows_does_not_have_is_skipped_with_a_message() {
    let directory = tempfile::tempdir().expect("a folder");
    let body = "#!/bin/zsh\n# @raycast.title Zsh only\necho hi\n";
    let path = write(directory.path(), "zsh.cmd", body);
    let error = command_from_source(&path, body).expect_err("the script is skipped");
    assert!(error.contains("Zsh"), "{error}");
    assert!(error.contains("Windows"), "{error}");
}

#[test]
fn a_folder_is_read_and_the_helper_scripts_are_left_alone() {
    let directory = tempfile::tempdir().expect("a folder");
    write(directory.path(), "disk.cmd", SCRIPT);
    write(directory.path(), "helper.txt", "not a script\n");
    let result = RaycastScriptImport::scan(directory.path());
    assert_eq!(result.commands.len(), 1);
    assert_eq!(result.commands[0].name, "Disk usage");
    assert_eq!(result.skipped.len(), 1);
    assert_eq!(result.skipped[0].name, "helper.txt");
}

#[test]
fn the_imported_command_uses_the_shared_symbol() {
    let directory = tempfile::tempdir().expect("a folder");
    let path = write(directory.path(), "disk.cmd", SCRIPT);
    let command = command_from_source(&path, SCRIPT).expect("the script is read");
    assert_eq!(command.symbol(), "terminal");
    assert_eq!(command.icon_symbol, None);
}
