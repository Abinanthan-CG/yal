//! `xtask` - build orchestration for the YAL port.
//!
//! Subcommands:
//!   parity           validate `parity/parity.toml` and regenerate `PARITY.md`
//!   parity --summary print per-tier counts only (no write)
//!   i18n             regenerate the localized string table from upstream's xcstrings
//!   dist             build the release binaries, installer, portable zip and checksums
//!   selftest         run `yal.exe --self-test` and print its JSON report
//!
//! Run with: `cargo xtask <command>`

use anyhow::{bail, Context, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

mod parity;
mod tools;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("help");
    let rest = &args[args.len().min(1)..];

    match command {
        "parity" => parity::run(&parity::Options {
            summary_only: rest.iter().any(|a| a == "--summary"),
            check_only: rest.iter().any(|a| a == "--check"),
        }),
        "i18n" => tools::i18n(),
        "dist" => tools::dist(rest),
        "selftest" => tools::selftest(),
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        other => bail!("unknown xtask command `{other}`; run `cargo xtask help`"),
    }
}

fn print_help() {
    println!(
        "xtask - YAL build orchestration\n\
         \n\
         Usage: cargo xtask <command>\n\
         \n\
         Commands:\n\
         \x20 parity            Validate parity/parity.toml and regenerate PARITY.md\n\
         \x20 parity --summary  Print the per-tier counts without writing files\n\
         \x20 parity --check    Validate only; fail if PARITY.md is out of date\n\
         \x20 i18n              Regenerate the localized string table\n\
         \x20 dist              Build binaries, installer, portable zip and checksums\n\
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

/// Every Rust source file in the workspace, excluding build output.
pub fn workspace_sources() -> Vec<PathBuf> {
    let root = repo_root();
    let mut out = Vec::new();
    for dir in ["crates", "apps", "xtask", "tests"] {
        let base = root.join(dir);
        if !base.is_dir() {
            continue;
        }
        for entry in walkdir::WalkDir::new(&base).into_iter().filter_map(Result::ok) {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                out.push(path.to_path_buf());
            }
        }
    }
    out
}

/// The names of every `#[test]` function in the workspace, keyed by crate directory name.
pub fn test_functions_by_crate() -> BTreeMap<String, BTreeSet<String>> {
    let root = repo_root();
    let mut map: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for path in workspace_sources() {
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let crate_name = match path
            .strip_prefix(&root)
            .ok()
            .and_then(|p| p.components().next())
            .and_then(|c| c.as_os_str().to_str())
        {
            Some(c) => c.to_string(),
            None => continue,
        };
        let entry = map.entry(crate_name).or_default();
        for line in text.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("fn ") {
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    entry.insert(name);
                }
            }
        }
    }
    map
}

/// Run a command in the repository root, streaming its output.
pub fn run_tool(program: &str, args: &[&str], context: &str) -> Result<()> {
    let root = repo_root();
    let mut cmd = Command::new(program);
    cmd.current_dir(&root).args(args);
    let status = cmd.status().with_context(|| format!("{context}: failed to spawn {program}"))?;
    if !status.success() {
        bail!("{context}: {program} {:?} exited with {status}", args);
    }
    Ok(())
}

/// Format a byte count for the generated report.
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
