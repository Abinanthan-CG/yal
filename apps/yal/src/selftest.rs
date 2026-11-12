//! The headless check of every subsystem.
//!
//! `yal.exe --self-test` prints one JSON object per subsystem and exits
//! non-zero when any of them fails. CI and the installer smoke test call it,
//! so a build that cannot run is never published.

use serde_json::{json, Value};
use std::process::ExitCode;
use tc_platform::{Clock, Hotkey, SingleInstance, WindowSystem};

/// The result of one subsystem's check.
struct Check {
    subsystem: &'static str,
    ok: bool,
    detail: String,
}

impl Check {
    fn pass(subsystem: &'static str, detail: impl Into<String>) -> Check {
        Check {
            subsystem,
            ok: true,
            detail: detail.into(),
        }
    }

    fn fail(subsystem: &'static str, detail: impl Into<String>) -> Check {
        Check {
            subsystem,
            ok: false,
            detail: detail.into(),
        }
    }

    fn to_json(&self) -> Value {
        json!({
            "subsystem": self.subsystem,
            "ok": self.ok,
            "detail": self.detail,
        })
    }
}

/// Run every check and report the result.
pub fn run() -> ExitCode {
    let checks = collect();
    let failed = checks.iter().filter(|c| !c.ok).count();
    let report: Vec<Value> = checks.iter().map(Check::to_json).collect();
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "ok": failed == 0,
            "failed": failed,
            "checks": report,
        }))
        .unwrap_or_else(|_| "{\"ok\":false}".to_string())
    );
    if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Run every subsystem's check.
fn collect() -> Vec<Check> {
    let mut checks = Vec::new();
    checks.extend(check_core());
    checks.extend(check_platform());
    checks.extend(check_windows());
    checks.extend(check_launcher());
    checks.extend(check_window_commands());
    checks
}

/// The window commands: the catalogue, the displays, and the mover.
fn check_window_commands() -> Vec<Check> {
    let mut checks = Vec::new();

    let commands = tc_windows::command::WindowCommandCatalog::all();
    checks.push(if commands.len() == 39 {
        Check::pass("window.commands", format!("{} commands", commands.len()))
    } else {
        Check::fail(
            "window.commands",
            format!("{} commands, expected 39", commands.len()),
        )
    });

    let displays = tc_platform_win::window_control::screens();
    checks.push(if displays.is_empty() {
        Check::fail("window.displays", "no display was found")
    } else {
        Check::pass("window.displays", format!("{} displays", displays.len()))
    });

    // The mover is only exercised when there is a window to move, which a
    // headless run may not have.
    let mover = match tc_platform_win::window_control::foreground_window() {
        Some(handle) => {
            let commands_state = tc_platform_win::window_control::WindowCommands::new(
                0.0,
                tc_windows::geometry::WindowCycle::Off,
            );
            match commands_state.run(handle, tc_windows::command::WindowCommandId::LeftHalf) {
                Ok(outcome) => Check::pass(
                    "window.mover",
                    format!("the window is now at {:?}", outcome.frame),
                ),
                Err(error) => Check::fail("window.mover", error.to_string()),
            }
        }
        None => Check::pass("window.mover", "there was no window to move"),
    };
    checks.push(mover);
    checks
}

/// The launcher's index and its search.
fn check_launcher() -> Vec<Check> {
    let mut checks = Vec::new();

    let programs = tc_platform_win::programs();
    let count = programs.len();
    checks.push(if count > 0 {
        Check::pass(
            "launcher.programs",
            format!("{count} programs on this machine"),
        )
    } else {
        Check::fail(
            "launcher.programs",
            "no installed program was found on this machine",
        )
    });

    // The learned ranking is never written by the self-test, so the file is
    // only a name.
    let ranking = std::env::temp_dir().join("yal-self-test-ranking.json");
    let launcher = crate::launcher::LauncherState::from_programs(programs, ranking);
    checks.push(if launcher.program_count() == count {
        Check::pass("launcher.index", format!("{count} entries in the index"))
    } else {
        Check::fail(
            "launcher.index",
            "the index does not hold every program it was given",
        )
    });

    // Every program has to be findable by its own name.
    let probe = tc_platform_win::programs()
        .into_iter()
        .find(|program| !program.name.trim().is_empty());
    match probe {
        Some(program) => {
            let rows = launcher.search(&program.name);
            let found = rows.iter().any(|row| row.name == program.name);
            checks.push(if found {
                Check::pass(
                    "launcher.search",
                    format!("`{}` is found by its own name", program.name),
                )
            } else {
                Check::fail(
                    "launcher.search",
                    format!("`{}` was not found by its own name", program.name),
                )
            });
        }
        None => checks.push(Check::fail(
            "launcher.search",
            "there was no program to search for",
        )),
    }

    checks
}

/// The pure-logic subsystems.
fn check_core() -> Vec<Check> {
    let mut checks = Vec::new();

    let commands = tc_core::CommandId::ALL.len();
    checks.push(if commands == 46 {
        Check::pass("core.commands", format!("{commands} built-in commands"))
    } else {
        Check::fail(
            "core.commands",
            format!("expected 46 built-in commands, found {commands}"),
        )
    });

    let actions = tc_core::SystemActionId::ALL.len();
    checks.push(if actions == 32 {
        Check::pass("core.system-actions", format!("{actions} system actions"))
    } else {
        Check::fail(
            "core.system-actions",
            format!("expected 32 system actions, found {actions}"),
        )
    });

    let link = tc_core::DeepLink::parse("tinycast://command/clipboard-history", "tinycast");
    checks.push(match link {
        Some(tc_core::DeepLink::Entry { ref entry_id, .. }) => {
            Check::pass("core.deeplinks", format!("parsed {entry_id}"))
        }
        _ => Check::fail("core.deeplinks", "a known deeplink did not parse"),
    });

    let broken = tc_core::DeepLink::parse("tinycast://command/not-a-command", "tinycast");
    checks.push(if broken.is_none() {
        Check::pass(
            "core.deeplinks.rejects-unknown",
            "an unknown row is refused",
        )
    } else {
        Check::fail(
            "core.deeplinks.rejects-unknown",
            "an unknown row was accepted",
        )
    });

    checks
}

/// The platform traits and their Windows implementation.
fn check_platform() -> Vec<Check> {
    let mut checks = Vec::new();

    let paths = tc_platform_win::windows_paths();
    let data = paths.data_dir.display().to_string();
    checks.push(
        if paths.data_dir.is_absolute() && paths.home_dir.is_absolute() {
            Check::pass("platform.paths", format!("data folder {data}"))
        } else {
            Check::fail("platform.paths", "the app's folders are not absolute paths")
        },
    );

    let settings_file = paths.settings_file.display().to_string();
    checks.push(if paths.settings_file.starts_with(&paths.home_dir) {
        Check::pass(
            "platform.settings-path",
            format!("settings file {settings_file}"),
        )
    } else {
        Check::fail(
            "platform.settings-path",
            "the settings file is not inside the user's home folder",
        )
    });

    let clock = tc_platform_win::SystemClock;
    let start = clock.now_millis();
    let end = clock.now_millis();
    checks.push(if end >= start {
        Check::pass("platform.clock", format!("{start} ms since the epoch"))
    } else {
        Check::fail("platform.clock", "the clock went backwards")
    });

    checks
}

/// The Windows subsystems.
fn check_windows() -> Vec<Check> {
    let mut checks = Vec::new();

    let integrity = tc_platform_win::own_integrity_level();
    checks.push(Check::pass(
        "windows.integrity",
        format!("this process runs at {integrity:?} integrity"),
    ));

    let windows = tc_platform_win::WindowsWindows::new();
    let foreground = windows.foreground_window();
    checks.push(match foreground {
        Some(window) => Check::pass(
            "windows.foreground-window",
            format!("{} (pid {})", window.title, window.process_id),
        ),
        None => Check::pass("windows.foreground-window", "no foreground window"),
    });

    let count = windows.switchable_windows().len();
    checks.push(if count > 0 {
        Check::pass("windows.window-enumeration", format!("{count} windows"))
    } else {
        Check::fail(
            "windows.window-enumeration",
            "no window could be enumerated",
        )
    });

    let hotkey = tc_platform_win::WindowsHotkey::new();
    if hotkey.is_available() {
        checks.push(Check::pass(
            "windows.keyboard-hook",
            "the hook installs and removes",
        ));
    } else {
        checks.push(Check::fail(
            "windows.keyboard-hook",
            hotkey
                .unavailable_reason()
                .unwrap_or_else(|| "the hook could not be installed".to_string()),
        ));
    }

    let instance = tc_platform_win::WindowsSingleInstance::new();
    let first = instance.acquire();
    let second = instance.acquire();
    instance.release();
    checks.push(if first && !second {
        Check::pass("windows.single-instance", "the lock keeps one instance")
    } else {
        Check::fail(
            "windows.single-instance",
            "the lock did not keep a second instance out",
        )
    });

    checks
}
