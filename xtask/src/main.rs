//! `xtask` - build orchestration for YAL.
//!
//! Subcommands:
//!   dist      build the release binary, installer, portable zip and checksums
//!   selftest  run `yal.exe --self-test` and print its JSON report
//!
//! Run with: `cargo run -q -p xtask -- <command>`

use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

mod tools;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("help");
    let rest = &args[args.len().min(1)..];

    match command {
        "dist" => tools::dist(rest),
        "selftest" => tools::selftest(),
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        other => bail!("unknown xtask command `{other}`; run `cargo run -q -p xtask -- help`"),
    }
}

fn print_help() {
    println!(
        "xtask - YAL build orchestration\n\
         \n\
         Usage: cargo run -q -p xtask -- <command>\n\
         \n\
         Commands:\n\
         \x20 dist              Build yal.exe, installer, portable zip and checksums\n\
         \x20 selftest          Run yal.exe --self-test and print its JSON report\n"
    );
}

/// Repository root, derived from the manifest directory of this crate.
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives one level below the repository root")
        .to_path_buf()
}

/// Run a command in the repository root, streaming its output.
pub fn run_tool(program: &str, args: &[&str], context: &str) -> Result<()> {
    let root = repo_root();
    let mut cmd = Command::new(program);
    cmd.current_dir(&root).args(args);
    let status = cmd
        .status()
        .with_context(|| format!("{context}: failed to spawn {program}"))?;
    if !status.success() {
        bail!("{context}: {program} {:?} exited with {status}", args);
    }
    Ok(())
}

/// Format a byte count for reports.
pub fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0usize;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}
