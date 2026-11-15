//! Running a command for real, and stopping it.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::runner::{is_command_not_found, last_line, tail_of_file, RunOutcome, ShellRunner};
use crate::shell::Shell;

/// Runs a PowerShell command and waits for it.
fn run(command: &str) -> RunOutcome {
    let plan = Shell::PowerShell
        .plan(command, &[], false, None)
        .expect("the plan is made");
    let mut running = ShellRunner::spawn(&plan).expect("the command starts");
    running.wait()
}

#[test]
fn a_command_prints_what_it_prints() {
    let outcome = run("Write-Output 'hello from yal'");
    assert_eq!(outcome.status, 0);
    assert!(
        outcome.stdout.contains("hello from yal"),
        "{}",
        outcome.stdout
    );
    assert!(outcome.succeeded());
}

#[test]
fn a_command_that_fails_reports_its_code() {
    let outcome = run("exit 3");
    assert_eq!(outcome.status, 3);
    assert!(!outcome.succeeded());
}

#[test]
fn the_environment_carries_the_marker() {
    let outcome = run("Write-Output $env:TINYCAST");
    assert_eq!(outcome.stdout.trim(), "1");
}

#[test]
fn the_input_is_closed() {
    let outcome = run("Write-Output ([Console]::In.ReadToEnd())");
    assert_eq!(
        outcome.stdout.trim(),
        "",
        "anything that reads input gets an end of file"
    );
}

#[test]
fn a_value_is_a_string_the_command_reads_and_not_a_second_command() {
    let plan = Shell::PowerShell
        .plan("Write-Output $1", &["; rm -rf ~".to_string()], false, None)
        .expect("the plan is made");
    let mut running = ShellRunner::spawn(&plan).expect("the command starts");
    let outcome = running.wait();
    assert_eq!(outcome.stdout.trim(), "; rm -rf ~");
}

#[test]
fn the_command_runs_in_its_run_in_folder() {
    let directory = tempfile::tempdir().expect("a folder");
    let plan = Shell::PowerShell
        .plan(
            "Write-Output (Get-Location).Path",
            &[],
            false,
            Some(&directory.path().to_string_lossy()),
        )
        .expect("the plan is made");
    let mut running = ShellRunner::spawn(&plan).expect("the command starts");
    let outcome = running.wait();
    let printed = outcome.stdout.trim();
    // The two are the same folder, but they need not be spelled the same way:
    // Windows may hand the temporary folder over in its short form
    // (`RUNNER~1`) while the command reports the long one. Resolving both
    // sides is the only honest way to compare them.
    let printed_path = std::fs::canonicalize(printed).unwrap_or_else(|_| PathBuf::from(printed));
    let expected_path =
        std::fs::canonicalize(directory.path()).unwrap_or_else(|_| directory.path().to_path_buf());
    assert_eq!(
        printed_path, expected_path,
        "the command ran in {printed} rather than in {}",
        directory.path().display()
    );
}

#[test]
fn the_command_keeps_running_after_the_launcher_lets_go() {
    let directory = tempfile::tempdir().expect("a folder");
    let first = directory.path().join("first.txt");
    let second = directory.path().join("second.txt");
    let command = format!(
        "Set-Content -LiteralPath '{}' -Value 'x'; Start-Sleep -Seconds 4; Set-Content -LiteralPath '{}' -Value 'x'",
        first.display(),
        second.display()
    );
    let plan = Shell::PowerShell
        .plan(&command, &[], false, None)
        .expect("the plan is made");
    {
        let running = ShellRunner::spawn(&plan).expect("the command starts");
        wait_for(&first, Duration::from_secs(10));
        assert!(first.exists(), "the command has started");
        drop(running);
    }
    // The launcher has let go of the command; it keeps running.
    wait_for(&second, Duration::from_secs(10));
    assert!(
        second.exists(),
        "the command kept running after the launcher let it go"
    );
}

#[test]
fn stop_ends_the_command_and_everything_it_started() {
    let directory = tempfile::tempdir().expect("a folder");
    let marker = directory.path().join("marks.txt");
    let child = directory.path().join("child.ps1");
    std::fs::write(
        &child,
        format!(
            "while ($true) {{ Add-Content -LiteralPath '{}' -Value 'x'; Start-Sleep -Milliseconds 200 }}",
            marker.display()
        ),
    )
    .expect("the child script is written");
    let command = format!(
        "Start-Process powershell.exe -ArgumentList '-NoProfile','-File','{}'; Start-Sleep -Seconds 60",
        child.display()
    );
    let plan = Shell::PowerShell
        .plan(&command, &[], false, None)
        .expect("the plan is made");
    let mut running = ShellRunner::spawn(&plan).expect("the command starts");
    wait_for(&marker, Duration::from_secs(15));
    let before = std::fs::metadata(&marker)
        .map(|meta| meta.len())
        .unwrap_or(0);
    assert!(before > 0, "the child process is writing");
    running.stop();
    std::thread::sleep(Duration::from_millis(800));
    let after = std::fs::metadata(&marker)
        .map(|meta| meta.len())
        .unwrap_or(0);
    std::thread::sleep(Duration::from_millis(2000));
    let later = std::fs::metadata(&marker)
        .map(|meta| meta.len())
        .unwrap_or(0);
    assert_eq!(
        after, later,
        "nothing is writing any more: the whole tree was stopped"
    );
}

/// Waits for a file to show up, or gives up.
fn wait_for(path: &Path, limit: Duration) {
    let start = std::time::Instant::now();
    while start.elapsed() < limit {
        if path.exists() {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn a_full_path_runs_without_the_shell_configuration() {
    // "You can also avoid relying on the shell configuration by using full
    // paths."
    let plan = Shell::PowerShell
        .plan(
            "& \"$env:SystemRoot\\System32\\where.exe\" calc",
            &[],
            false,
            None,
        )
        .expect("the plan is made");
    let mut running = ShellRunner::spawn(&plan).expect("the command starts");
    let outcome = running.wait();
    assert!(outcome.succeeded(), "{}", outcome.stderr);
    assert!(
        outcome.stdout.to_lowercase().contains("calc.exe"),
        "{}",
        outcome.stdout
    );
}

#[test]
fn the_output_window_keeps_the_last_256_kb() {
    let directory = tempfile::tempdir().expect("a folder");
    let path = directory.path().join("big.log");
    let block = "x".repeat(1024);
    let mut body = String::new();
    for _ in 0..300 {
        body.push_str(&block);
        body.push('\n');
    }
    std::fs::write(&path, &body).expect("the file is written");
    let tail = tail_of_file(&path, crate::runner::OUTPUT_WINDOW_LIMIT);
    assert!(
        tail.len() <= crate::runner::OUTPUT_WINDOW_LIMIT + 1,
        "{} bytes",
        tail.len()
    );
    assert!(tail.len() > 250 * 1024, "{} bytes", tail.len());
}

#[test]
fn a_success_shows_the_last_line_of_output() {
    let outcome = RunOutcome {
        status: 0,
        stdout: "first\nlast line\n".to_string(),
        stderr: String::new(),
        duration: Duration::from_millis(1),
        stopped: false,
    };
    assert_eq!(outcome.success_message("Sleep"), "last line");
}

#[test]
fn a_success_with_nothing_printed_says_it_ran() {
    let outcome = RunOutcome {
        status: 0,
        stdout: String::new(),
        stderr: String::new(),
        duration: Duration::from_millis(1),
        stopped: false,
    };
    assert_eq!(outcome.success_message("Sleep"), "Ran Sleep");
}

#[test]
fn a_failure_shows_the_error_output() {
    let outcome = RunOutcome {
        status: 1,
        stdout: String::new(),
        stderr: "something went wrong".to_string(),
        duration: Duration::from_millis(1),
        stopped: false,
    };
    assert_eq!(outcome.failure_message(), "something went wrong");
}

#[test]
fn a_failure_shows_at_most_the_last_8_kb() {
    let outcome = RunOutcome {
        status: 1,
        stdout: String::new(),
        stderr: "y".repeat(20 * 1024),
        duration: Duration::from_millis(1),
        stopped: false,
    };
    let message = outcome.failure_message();
    assert!(message.len() <= crate::runner::ERROR_OUTPUT_LIMIT);
    assert!(message.len() > 7 * 1024);
}

#[test]
fn the_last_line_of_nothing_is_nothing() {
    assert_eq!(last_line(""), None);
    assert_eq!(last_line("\n\n  \n"), None);
    assert_eq!(last_line("a\nb\n"), Some("b".to_string()));
}

#[test]
fn a_command_that_is_not_found_is_recognised() {
    assert!(is_command_not_found(
        "The term 'nonsense' is not recognized as the name of a cmdlet, function, script file, or operable program."
    ));
    assert!(is_command_not_found(
        "'nonsense' is not recognized as an internal or external command"
    ));
    assert!(!is_command_not_found("everything is fine"));
}

#[test]
fn the_command_text_does_not_stay_on_disk() {
    let marker = "a-very-unique-command-text";
    let command = format!("Write-Output '{marker}'");
    let plan = Shell::PowerShell
        .plan(&command, &[], false, None)
        .expect("the plan is made");
    let script = plan
        .script
        .clone()
        .expect("the command is written to a file");
    assert!(script.exists());
    let mut running = ShellRunner::spawn(&plan).expect("the command starts");
    let outcome = running.wait();
    assert!(
        !script.exists(),
        "the file the command ran from is removed when it ends"
    );
    assert!(!outcome.stdout.contains(&format!("Write-Output '{marker}'")));
}

#[test]
fn a_command_that_cannot_start_is_reported() {
    let plan = Shell::Cmd
        .plan("echo hi", &[], false, None)
        .expect("the plan is made");
    let mut broken = plan.clone();
    broken.program = Path::new("this-program-does-not-exist.exe").to_path_buf();
    assert!(ShellRunner::spawn(&broken).is_err());
}
