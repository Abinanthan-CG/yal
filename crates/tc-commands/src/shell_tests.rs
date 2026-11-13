//! The process a command turns into.

use std::path::PathBuf;

use crate::shell::{resolve_working_directory, LaunchError, Shell};

/// The home folder, as the port sees it.
fn home() -> PathBuf {
    crate::shell::home_directory()
}

#[test]
fn the_shell_is_powershell_by_default() {
    assert_eq!(Shell::default(), Shell::PowerShell);
    assert_eq!(Shell::parse("PowerShell"), Some(Shell::PowerShell));
    assert_eq!(Shell::parse("cmd"), Some(Shell::Cmd));
    assert_eq!(Shell::parse("git bash"), Some(Shell::GitBash));
    assert_eq!(Shell::parse("wsl"), Some(Shell::Wsl));
    assert_eq!(Shell::parse("fish"), None);
}

#[test]
fn a_command_runs_in_its_run_in_folder_or_the_home_folder() {
    let plan = Shell::PowerShell
        .plan("Write-Output hi", &[], false, None)
        .expect("the plan is made");
    assert_eq!(plan.working_directory, home());

    let directory = tempfile::tempdir().expect("a folder");
    let plan = Shell::PowerShell
        .plan(
            "Write-Output hi",
            &[],
            false,
            Some(&directory.path().to_string_lossy()),
        )
        .expect("the plan is made");
    assert_eq!(plan.working_directory, directory.path());
}

#[test]
fn a_run_in_folder_that_is_gone_stops_the_command_and_says_which_one() {
    let missing = home().join("a-folder-that-is-not-there");
    let error = resolve_working_directory(Some(&missing.to_string_lossy()))
        .expect_err("the folder is missing");
    match error {
        LaunchError::WorkingDirectoryMissing(name) => {
            assert!(name.contains("a-folder-that-is-not-there"), "{name}");
        }
        other => panic!("{other}"),
    }
    assert!(Shell::PowerShell
        .plan(
            "Write-Output hi",
            &[],
            false,
            Some("C:\\no-such-folder-here")
        )
        .is_err());
}

#[test]
fn powershell_binds_the_values_to_dollar_one_and_dollar_two() {
    let values = vec!["first".to_string(), "second".to_string()];
    let plan = Shell::PowerShell
        .plan("Write-Output $1 $2", &values, false, None)
        .expect("the plan is made");
    let script = plan.script.expect("the command is written to a file");
    let body = std::fs::read_to_string(&script).expect("the script is there");
    assert!(body.contains("$1 = $args[0]"), "{body}");
    assert!(body.contains("$2 = $args[1]"), "{body}");
    assert!(body.contains("Write-Output $1 $2"), "{body}");
    let _ = std::fs::remove_file(script);
}

#[test]
fn the_values_are_never_written_into_the_command_text() {
    let values = vec!["; rm -rf ~".to_string()];
    let plan = Shell::PowerShell
        .plan("Write-Output $1", &values, false, None)
        .expect("the plan is made");
    let script = plan.script.expect("the command is written to a file");
    let body = std::fs::read_to_string(&script).expect("the script is there");
    assert!(
        !body.contains("rm -rf"),
        "the value is a string the command reads, not part of the text: {body}"
    );
    let _ = std::fs::remove_file(script);
}

#[test]
fn cmd_passes_the_values_as_its_own_parameters() {
    let values = vec!["first".to_string()];
    let plan = Shell::Cmd
        .plan("echo %1", &values, false, None)
        .expect("the plan is made");
    let script = plan.script.expect("the command is written to a file");
    let body = std::fs::read_to_string(&script).expect("the script is there");
    assert!(body.contains("echo %1"), "{body}");
    assert!(
        plan.arguments.contains(&"first".to_string()),
        "the value is handed over, not written into the text"
    );
    let _ = std::fs::remove_file(script);
}

#[test]
fn bash_passes_the_values_after_the_command() {
    let values = vec!["first".to_string()];
    let plan = Shell::GitBash
        .plan("echo $1", &values, false, None)
        .expect("the plan is made");
    assert_eq!(plan.arguments[0], "-lc");
    assert_eq!(plan.arguments[1], "echo $1");
    assert_eq!(plan.arguments[2], "yal", "$0 is the launcher");
    assert_eq!(plan.arguments[3], "first", "$1 is the first value");
}

#[test]
fn load_shell_environment_loads_the_powershell_profile() {
    let plan = Shell::PowerShell
        .plan("Write-Output hi", &[], false, None)
        .expect("the plan is made");
    assert!(
        plan.arguments.contains(&"-NoProfile".to_string()),
        "the profile is not read by default"
    );

    let plan = Shell::PowerShell
        .plan("Write-Output hi", &[], true, None)
        .expect("the plan is made");
    assert!(
        !plan.arguments.contains(&"-NoProfile".to_string()),
        "the profile is read when the setting is on"
    );
}

#[test]
fn load_shell_environment_makes_bash_interactive() {
    let plan = Shell::GitBash
        .plan("echo $1", &[], true, None)
        .expect("the plan is made");
    assert_eq!(plan.arguments[0], "-ilc");
}

#[test]
fn the_command_runs_with_no_console_window() {
    let plan = Shell::PowerShell
        .plan("Write-Output hi", &[], false, None)
        .expect("the plan is made");
    assert_eq!(plan.program.to_string_lossy(), "powershell.exe");
    assert!(
        !plan
            .arguments
            .iter()
            .any(|argument| argument == "-WindowStyle"),
        "no window is asked for"
    );
}

#[test]
fn wsl_runs_the_command_in_its_linux_shell() {
    let plan = Shell::Wsl
        .plan("echo $1", &["first".to_string()], false, None)
        .expect("the plan is made");
    assert_eq!(plan.program.to_string_lossy(), "wsl.exe");
    assert!(plan.arguments.contains(&"bash".to_string()));
    assert!(plan.arguments.contains(&"first".to_string()));
}
