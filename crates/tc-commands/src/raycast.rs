//! Importing a folder of Raycast script commands.
//!
//! Upstream's importer is `Features/CustomCommands/Model/RaycastScriptImport.swift`.
//! It reads the `@raycast.*` block at the top of each script and turns it into
//! a custom command. Windows needs one extra step: a Raycast script names its
//! interpreter with a Unix path, so the importer resolves that name against
//! Windows and says plainly when it cannot.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::model::{CustomCommand, CustomCommandArgument};
use crate::shell::Shell;

/// How much of a script the importer reads.
const HEAD_SIZE: usize = 8 * 1024;

/// The marker a Raycast directive carries.
const DIRECTIVE_PREFIX: &str = "@raycast.";

/// The comment markers a directive may sit behind.
const COMMENT_MARKERS: [&str; 3] = ["#", "//", "--"];

/// A script that was not imported, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedScript {
    /// Where the script is.
    pub path: PathBuf,
    /// The name the script would have had.
    pub name: String,
    /// Why it was skipped.
    pub reason: String,
}

/// What an import found.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImportResult {
    /// The commands that were made.
    pub commands: Vec<CustomCommand>,
    /// The scripts that were not.
    pub skipped: Vec<SkippedScript>,
}

/// Reads a folder of Raycast script commands.
pub struct RaycastScriptImport;

impl RaycastScriptImport {
    /// Reads every script command in a folder.
    ///
    /// "It's a one-time import: afterward they're regular commands you can
    /// edit or delete." The folder is not read into subfolders, like
    /// Raycast's own script directories.
    pub fn scan(directory: &Path) -> ImportResult {
        let mut result = ImportResult::default();
        let Ok(entries) = std::fs::read_dir(directory) else {
            return result;
        };
        let mut paths: Vec<PathBuf> = entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| path.is_file())
            .collect();
        paths.sort();
        for path in paths {
            match Self::command_at(&path) {
                Ok(command) => result.commands.push(command),
                Err(reason) => result.skipped.push(SkippedScript {
                    name: path
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    path,
                    reason,
                }),
            }
        }
        result
    }

    /// The command a script file makes, if it makes one.
    pub fn command_at(path: &Path) -> Result<CustomCommand, String> {
        let source = read_head(path).ok_or_else(|| "The script could not be read.".to_string())?;
        command_from_source(path, &source)
    }
}

/// The command a script's text makes, if it makes one.
pub fn command_from_source(path: &Path, source: &str) -> Result<CustomCommand, String> {
    let interpreter = shebang(source).ok_or_else(|| {
        "The script has no shebang line, so it is not a script command.".to_string()
    })?;
    let directives = directives(source);
    let title = directives.get("title").cloned().ok_or_else(|| {
        "The script has no @raycast.title, so it is not a script command.".to_string()
    })?;

    let (program, shell) = resolve_interpreter(&interpreter)?;
    let script_path = path.to_string_lossy().into_owned();
    let working_directory = directives
        .get("currentDirectoryPath")
        .cloned()
        .unwrap_or_else(|| {
            path.parent()
                .map(|parent| parent.to_string_lossy().into_owned())
                .unwrap_or_default()
        });

    let mut command = CustomCommand::new(title, script_command_text(shell, &program, &script_path));
    command.shell = shell;
    command.requires_confirmation =
        directives.get("needsConfirmation").map(String::as_str) == Some("true");
    command.shows_output = matches!(
        directives.get("mode").map(String::as_str),
        Some("compact") | Some("fullOutput")
    );
    command.arguments = arguments(&directives);
    command.working_directory = Some(working_directory);
    Ok(command)
}

/// The command text that runs a script and hands it the values.
///
/// The values are forwarded, never written into the text.
fn script_command_text(shell: Shell, program: &str, script_path: &str) -> String {
    match shell {
        Shell::PowerShell => {
            format!("{} '{}' @args", program, script_path.replace('\'', "''"))
        }
        Shell::Cmd => format!("\"{}\" \"{}\" %*", program, script_path),
        Shell::GitBash | Shell::Wsl => {
            format!("{} {} \"$@\"", program, single_quoted(script_path))
        }
    }
}

/// A path in single quotes, so a space or a quote in it is still one word.
fn single_quoted(path: &str) -> String {
    format!("'{}'", path.replace('\'', "'\\''"))
}

/// The interpreter a shebang names.
fn shebang(source: &str) -> Option<String> {
    let first = source.lines().next()?;
    let trimmed = first.trim_end_matches('\r');
    let rest = trimmed.strip_prefix("#!")?.trim();
    if rest.is_empty() {
        None
    } else {
        Some(rest.to_string())
    }
}

/// The `@raycast.` block at the top of a script.
fn directives(source: &str) -> BTreeMap<String, String> {
    let mut values: BTreeMap<String, String> = BTreeMap::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Some(body) = comment_body(trimmed) else {
            break;
        };
        let Some(body) = body.strip_prefix(DIRECTIVE_PREFIX) else {
            continue;
        };
        let Some(separator) = body.find(char::is_whitespace) else {
            continue;
        };
        let key = body[..separator].to_string();
        let value = body[separator..].trim().to_string();
        if key.is_empty() || value.is_empty() {
            continue;
        }
        values.insert(key, value);
    }
    values
}

/// The text of a comment, without its marker.
fn comment_body(line: &str) -> Option<&str> {
    let marker = COMMENT_MARKERS
        .iter()
        .find(|marker| line.starts_with(**marker))?;
    Some(line[marker.len()..].trim())
}

/// The arguments a script declares.
///
/// "Raycast allows three, each a JSON object whose placeholder is what we
/// prompt with."
fn arguments(directives: &BTreeMap<String, String>) -> Vec<CustomCommandArgument> {
    (1..=3)
        .filter_map(|index| {
            let json = directives.get(&format!("argument{index}"))?.clone();
            let parsed: serde_json::Value = serde_json::from_str(&json).ok()?;
            let placeholder = parsed.get("placeholder")?.as_str()?.to_string();
            if placeholder.is_empty() {
                return None;
            }
            let optional = parsed
                .get("optional")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            Some(CustomCommandArgument::new(placeholder, optional))
        })
        .collect()
}

/// The program that runs a script on Windows, and the shell it runs under.
///
/// "resolves the shebang interpreter on Windows (python, node, bash via Git
/// Bash or WSL), with a clear message when it is missing."
fn resolve_interpreter(interpreter: &str) -> Result<(String, Shell), String> {
    let program = program_name(interpreter);
    match program.as_str() {
        "python" | "python3" | "py" => {
            let name = if program == "py" { "py" } else { "python" };
            if on_path(name) {
                Ok((name.to_string(), Shell::PowerShell))
            } else {
                Err("Python was not found on PATH.".to_string())
            }
        }
        "node" | "nodejs" => {
            if on_path("node") {
                Ok(("node".to_string(), Shell::PowerShell))
            } else {
                Err("Node was not found on PATH.".to_string())
            }
        }
        "bash" | "sh" => {
            if Shell::GitBash.program().is_ok() {
                Ok(("bash".to_string(), Shell::GitBash))
            } else if on_path("wsl") {
                Ok(("bash".to_string(), Shell::Wsl))
            } else {
                Err("Bash was not found. Install Git for Windows, or use WSL.".to_string())
            }
        }
        "zsh" => Err(
            "Zsh is not available on Windows. Install it, or change the script's shebang."
                .to_string(),
        ),
        other => {
            if on_path(other) {
                Ok((other.to_string(), Shell::PowerShell))
            } else {
                Err(format!("{other} was not found on PATH."))
            }
        }
    }
}

/// The program a shebang names, without its path.
fn program_name(interpreter: &str) -> String {
    let mut words = interpreter.split_whitespace();
    let first = words.next().unwrap_or("");
    let program = if first.ends_with("env") {
        words.next().unwrap_or(first)
    } else {
        first
    };
    let base = program.rsplit(['/', '\\']).next().unwrap_or(program).trim();
    base.strip_suffix(".exe").unwrap_or(base).to_lowercase()
}

/// Whether a program is on PATH.
fn on_path(program: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|directory| {
        let candidate = directory.join(program);
        candidate.is_file()
            || candidate.with_extension("exe").is_file()
            || candidate.with_extension("cmd").is_file()
            || candidate.with_extension("bat").is_file()
    })
}

/// The first few kilobytes of a script.
///
/// The directives are at the top, and a body running to megabytes is not.
fn read_head(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let slice = if bytes.len() > HEAD_SIZE {
        let mut end = HEAD_SIZE;
        while end > 0 && bytes[end] != b'\n' {
            end -= 1;
        }
        &bytes[..end]
    } else {
        &bytes[..]
    };
    Some(String::from_utf8_lossy(slice).into_owned())
}
