//! YAL - a Windows keyboard launcher, an unofficial community port of Tinycast.
//!
//! The executable is the composition root: it wires the platform traits to the
//! real Windows implementation, owns the palette window, the tray icon, the
//! global hotkey engine and the single-instance lock, and exposes the
//! subsystems to the interface.
//!
//! Run `yal.exe --self-test` for a headless check of every subsystem.

mod launcher;
mod palette;
mod selftest;
mod shell;

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

    match shell::run(&arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("yal: {error}");
            ExitCode::FAILURE
        }
    }
}
