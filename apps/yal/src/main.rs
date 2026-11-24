//! YAL - a native Windows keyboard launcher, an unofficial community port of Tinycast.
//!
//! Pure Rust + Win32 + Direct2D: wires the platform traits to real Windows
//! APIs, owns the Direct2D palette window, native system tray icon, global
//! hotkey engine, and single-instance lock.
//!
//! Run `yal.exe --self-test` for a headless check of every subsystem.

mod actions;
mod features;
mod launcher;
mod native;
mod open;
mod palette;
mod screens;
mod selftest;
mod shell;
mod win_settings;

use std::process::ExitCode;

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();

    if arguments.iter().any(|a| a == "--self-test") {
        return selftest::run();
    }
    if arguments.iter().any(|a| a == "--version") {
        println!("yal {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    if arguments.iter().any(|a| a == "--help" || a == "-h") {
        println!("{}", shell::usage());
        return ExitCode::SUCCESS;
    }
    // The native palette on its own, which is what it looks like when the
    // whole application is native.
    if arguments.iter().any(|a| a == "--native-palette") {
        return match native::run(&arguments) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("yal: {error}");
                ExitCode::FAILURE
            }
        };
    }

    match shell::run(&arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("yal: {error}");
            ExitCode::FAILURE
        }
    }
}
