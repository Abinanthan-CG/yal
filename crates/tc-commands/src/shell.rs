//! The shell a command runs under, and the process it turns into.
//!
//! Upstream has one shell, zsh, and runs `zsh -lc <command>` with the values
//! after it. Windows has more than one, so the port lets a command name its
//! shell: PowerShell, cmd, Git Bash or WSL. The rules are the same everywhere:
//!
//! * the values are passed positionally and never spliced into the command
//!   text,
//! * the command runs without a console window,
//! * the folder it starts in is the Run In folder, or the home folder,
//! * the environment is inherited, plus `TINYCAST=1`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The shell a command runs under.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Shell {
    /// Windows PowerShell. The default.
    #[default]
    #[serde(rename = "powershell")]
    PowerShell,
    /// The classic command processor.
    #[serde(rename = "cmd")]
    Cmd,
    /// The bash that comes with Git for Windows.
    #[serde(rename = "git-bash")]
    GitBash,
    /// The bash of a Linux distribution, through WSL.
    #[serde(rename = "wsl")]
    Wsl,
}

impl Shell {
    /// Every shell, in the order the editor lists them.
    pub const ALL: [Shell; 4] = [Shell::PowerShell, Shell::Cmd, Shell::GitBash, Shell::Wsl];

    /// The name the editor shows.
    pub fn label(self) -> &'static str {
        match self {
            Shell::PowerShell => "PowerShell",
            Shell::Cmd => "cmd",
            Shell::GitBash => "Git Bash",
            Shell::Wsl => "WSL",
        }
    }

    /// The shell a name stands for.
    pub fn parse(name: &str) -> Option<Shell> {
        match name.trim().to_lowercase().as_str() {
            "powershell" | "powershell.exe" | "pwsh" => Some(Shell::PowerShell),
            "cmd" | "cmd.exe" | "command prompt" => Some(Shell::Cmd),
            "git bash" | "git-bash" | "gitbash" | "bash" => Some(Shell::GitBash),
            "wsl" | "wsl.exe" => Some(Shell::Wsl),
            _ => None,
        }
    }

    /// The program the shell runs in.
    pub fn program(self) -> Result<PathBuf, LaunchError> {
        match self {
            Shell::PowerShell => Ok(PathBuf::from("powershell.exe")),
            Shell::Cmd => Ok(PathBuf::from("cmd.exe")),
            Shell::Wsl => Ok(PathBuf::from("wsl.exe")),
            Shell::GitBash => git_bash().ok_or_else(|| {
                LaunchError::ShellNotFound(
                    "Git Bash was not found. Install Git for Windows, or pick another shell."
                        .to_string(),
                )
            }),
        }
    }

    /// The process that runs the command.
    ///
    /// * `command` is the text the user wrote.
    /// * `values` are the argument values, in `$1`, `$2` order.
    /// * `loads_shell_environment` asks for the shell's own start-up files.
    /// * `working_directory` is the Run In folder; empty means the home folder.
    pub fn plan(
        self,
        command: &str,
        values: &[String],
        loads_shell_environment: bool,
        working_directory: Option<&str>,
    ) -> Result<LaunchPlan, LaunchError> {
        let directory = resolve_working_directory(working_directory)?;
        match self {
            Shell::PowerShell => {
                let script = write_temp_script("ps1", &powershell_body(command, values.len()))?;
                let mut arguments = vec!["-NoLogo".to_string()];
                if !loads_shell_environment {
                    // The profile is the shell environment: without it, aliases and
                    // functions from the profile are missing.
                    arguments.push("-NoProfile".to_string());
                }
                arguments.push("-ExecutionPolicy".to_string());
                arguments.push("Bypass".to_string());
                arguments.push("-File".to_string());
                arguments.push(script.to_string_lossy().into_owned());
                arguments.extend(values.iter().cloned());
                Ok(LaunchPlan {
                    program: self.program()?,
                    arguments,
                    working_directory: directory,
                    script: Some(script),
                })
            }
            Shell::Cmd => {
                let script = write_temp_script("cmd", &cmd_body(command))?;
                let mut arguments = vec![
                    "/d".to_string(),
                    "/c".to_string(),
                    script.to_string_lossy().into_owned(),
                ];
                arguments.extend(values.iter().cloned());
                Ok(LaunchPlan {
                    program: self.program()?,
                    arguments,
                    working_directory: directory,
                    script: Some(script),
                })
            }
            Shell::GitBash | Shell::Wsl => {
                // The values follow the command, so the shell sees them as `$1`,
                // `$2` and never as part of the command text.
                let flag = if loads_shell_environment {
                    "-ilc"
                } else {
                    "-lc"
                };
                let mut arguments = vec![flag.to_string(), command.to_string(), "yal".to_string()];
                arguments.extend(values.iter().cloned());
                let (program, arguments) = if self == Shell::Wsl {
                    let mut wsl = vec!["--exec".to_string(), "bash".to_string()];
                    wsl.extend(arguments);
                    (self.program()?, wsl)
                } else {
                    (self.program()?, arguments)
                };
                Ok(LaunchPlan {
                    program,
                    arguments,
                    working_directory: directory,
                    script: None,
                })
            }
        }
    }
}

/// What a command turns into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchPlan {
    /// The program to run.
    pub program: PathBuf,
    /// Its arguments, without the program itself.
    pub arguments: Vec<String>,
    /// The folder it starts in.
    pub working_directory: PathBuf,
    /// A temporary file that belongs to the run and is deleted when it ends.
    pub script: Option<PathBuf>,
}

/// Why a command could not be started.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LaunchError {
    #[error("The folder \"{0}\" no longer exists.")]
    WorkingDirectoryMissing(String),
    #[error("{0}")]
    ShellNotFound(String),
    #[error("The script could not be written.")]
    ScriptNotWritable,
    #[error("The output could not be captured.")]
    OutputNotWritable,
}

/// The body of the PowerShell script that runs a command.
///
/// Windows PowerShell's `-Command` cannot carry both the command text and the
/// values, so the command is written to a file and run with `-File`. The values
/// arrive in `$args`, and the first lines bind them to `$1`, `$2` … so the
/// command text reads the way the documentation says it does.
fn powershell_body(command: &str, value_count: usize) -> String {
    let mut body = String::new();
    for index in 0..value_count {
        body.push_str(&format!("${} = $args[{}]\n", index + 1, index));
    }
    body.push_str(command);
    body.push_str("\nif ($null -ne $LASTEXITCODE) { exit $LASTEXITCODE }\n");
    body
}

/// The body of the batch file that runs a command.
///
/// A batch file's own `%1`, `%2` … are the values, so the command text reads
/// `%1` where the documentation says `$1`.
fn cmd_body(command: &str) -> String {
    format!("@echo off\r\n{}\r\nexit /b %ERRORLEVEL%\r\n", command)
}

/// The folder a command starts in.
///
/// "If the Run In folder no longer exists, the command doesn't run, and Tinycast
/// tells you which folder is missing instead of running it somewhere else."
pub fn resolve_working_directory(path: Option<&str>) -> Result<PathBuf, LaunchError> {
    let trimmed = path.unwrap_or("").trim();
    if trimmed.is_empty() {
        return Ok(home_directory());
    }
    let expanded = expand_home(trimmed);
    if expanded.is_dir() {
        return Ok(expanded);
    }
    Err(LaunchError::WorkingDirectoryMissing(trimmed.to_string()))
}

/// The user's home folder.
pub fn home_directory() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .filter(|path| path.is_dir())
        .unwrap_or_else(std::env::temp_dir)
}

/// `~` and `~\` stand for the home folder.
pub fn expand_home(path: &str) -> PathBuf {
    if path == "~" || path == "~\\" || path == "~/" {
        return home_directory();
    }
    if let Some(rest) = path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\")) {
        return home_directory().join(rest);
    }
    PathBuf::from(path)
}

/// The bash that comes with Git for Windows.
fn git_bash() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("YAL_GIT_BASH") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(program_files) = std::env::var_os("ProgramFiles") {
        candidates.push(
            PathBuf::from(program_files)
                .join("Git")
                .join("bin")
                .join("bash.exe"),
        );
    }
    if let Some(program_files_x86) = std::env::var_os("ProgramFiles(x86)") {
        candidates.push(
            PathBuf::from(program_files_x86)
                .join("Git")
                .join("bin")
                .join("bash.exe"),
        );
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        candidates.push(
            PathBuf::from(local)
                .join("Programs")
                .join("Git")
                .join("bin")
                .join("bash.exe"),
        );
    }
    candidates.into_iter().find(|path| path.is_file())
}

/// Writes a temporary script and returns its path.
fn write_temp_script(extension: &str, body: &str) -> Result<PathBuf, LaunchError> {
    let path = std::env::temp_dir().join(format!(
        "yal-command-{}.{extension}",
        Uuid::new_v4().simple()
    ));
    std::fs::write(&path, body).map_err(|_| LaunchError::ScriptNotWritable)?;
    Ok(path)
}
