//! Script commands: little programs the user keeps in a folder, so the
//! launcher can find and run them.
//!
//! This is an own definition, written down rather than copied: a script
//! command is a file in the scripts folder whose first comment lines may
//! carry metadata the launcher reads. Three things are defined precisely so
//! folder, parser and runner stay in step.
//!
//! The folder is `%USERPROFILE%\.yal\scripts`, or wherever the
//! `scripts_folder` setting points.
//!
//! The metadata is one directive per line, in the file's own comment style
//! (`#` or `::`), inside the first twenty lines:
//!
//! ```text
//! # @title Restart audio service
//! # @description stop and start the audio endpoint
//! # @mode compact
//! # @keywords audio, restart
//! ```
//!
//! `@mode` is one of `silent` (run detached, output nowhere), `compact`
//! (run in the background and toast the outcome) and `full` (as compact,
//! plus the output copied to the clipboard). `@title` falls back to the
//! file name; `@keywords` is a comma separated list.
//!
//! A file is a script command when its extension is one of the known ones
//! and it parses in UTF-8 or the system's wide characters. What is not on
//! the machine — python, a shell — makes the script visible but honestly
//! refuses to run.

/// A script command found in the folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Script {
    /// The title shown in the launcher.
    pub title: String,
    /// Where the script lives.
    pub path: std::path::PathBuf,
    /// The way the file is run.
    pub runner: Runner,
    /// What happens with the output.
    pub mode: Mode,
    /// One line of explanation, when the script carries one.
    pub description: Option<String>,
    /// Extra words that find it.
    pub keywords: Vec<String>,
}

/// The language a script file runs as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Runner {
    /// A PowerShell script.
    PowerShell,
    /// A Windows batch file or command script.
    Batch,
    /// A Python script.
    Python,
}

/// What happens with the output of a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Run detached; the output goes nowhere.
    Silent,
    /// Run in the background and announce the outcome with a toast.
    Compact,
    /// As compact, with the output copied to the clipboard.
    Full,
}

impl Mode {
    /// The mode named in the header, or the mode a script without one gets.
    pub fn from_slug(slug: &str) -> Mode {
        match slug.to_lowercase().as_str() {
            "silent" => Mode::Silent,
            "full" => Mode::Full,
            _ => Mode::Compact,
        }
    }

    /// The name the mode is written as in a header and reports.
    pub fn slug(self) -> &'static str {
        match self {
            Mode::Silent => "silent",
            Mode::Compact => "compact",
            Mode::Full => "full",
        }
    }
}

/// The folder the scripts live in, outside the launcher's own settings.
///
/// `%USERPROFILE%\.yal\scripts` — the launcher's data folder with a
/// `scripts` directory joined on. It is created on demand by `ensure_folder`.
///
/// A scripts folder left behind by the launcher's earlier name
/// (`%USERPROFILE%\.legacy-store\scripts`) is folded into the new one the
/// first time the default is read: the rename moves the folder whole, and
/// only runs when the new folder has nothing yet.
pub fn default_folder() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|home| {
        let legacy = home.join(".legacy-store");
        let current = home.join(".yal");
        if !current.exists() && legacy.exists() {
            std::fs::rename(&legacy, &current).ok();
        }
        current.join("scripts")
    })
}

/// Resolve the scripts folder: an explicit path first, the default after.
pub fn resolve_folder(explicit: Option<std::path::PathBuf>) -> Option<std::path::PathBuf> {
    explicit.or_else(default_folder)
}

/// Read the metadata out of a script file.
///
/// The extension decides the runner: anything else is not a script command
/// and is skipped silently, because the folder belongs to the user and not
/// every file in it is ours to interpret.
pub fn read(path: &std::path::Path) -> Option<Script> {
    let runner = match path.extension()?.to_str()?.to_lowercase().as_str() {
        "ps1" => Runner::PowerShell,
        "cmd" | "bat" => Runner::Batch,
        "py" => Runner::Python,
        _ => return None,
    };

    let text = std::fs::read_to_string(path)
        // A file of another encoding is kept over failing entirely: it
        // still runs, but its comment directives are read as best the
        // lossy decoder can.
        .unwrap_or_else(|error| {
            let _ = error;
            std::fs::read(path)
                .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
                .unwrap_or_default()
        });
    // Windows editors write a byte order mark in front of UTF-8 files;
    // with it in place the first directive would not be recognized.
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let file_name = path.to_string_lossy().into_owned();
    let mut title = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| file_name.clone());
    let mut mode = Mode::Compact;
    let mut description = None;
    let mut keywords = Vec::new();

    for line in text.lines().take(20) {
        let Some(directive) = directive_in(line) else {
            continue;
        };
        let (name, value) = directive;
        match name.as_str() {
            "title" => title = value.trim().to_string(),
            "description" => description = Some(value.trim().to_string()),
            "mode" => mode = Mode::from_slug(value),
            "keywords" => {
                keywords = value
                    .split(',')
                    .map(|word| word.trim().to_string())
                    .filter(|word| !word.is_empty())
                    .collect();
            }
            _ => {}
        }
    }

    if title.is_empty() {
        return None;
    }

    Some(Script {
        title,
        path: path.to_path_buf(),
        runner,
        mode,
        description,
        keywords,
    })
}

/// The directive a comment line carries, if any.
///
/// Accepts the two comment styles the languages we support use: `#` for
/// PowerShell and Python, `::` and REM for batch. The name is matched, not
/// searched for, so prose that merely contains "@title" does not count.
fn directive_in(line: &str) -> Option<(String, &str)> {
    let line = line.trim_start();
    let rest = ["::", "#", "rem", "REM"]
        .iter()
        .find_map(|prefix| line.strip_prefix(prefix))?;

    // The directive name must open the comment: "@title X" counts, both
    // "@titleX" and titles elsewhere on the line do not.
    let rest = rest.trim();
    if !rest.starts_with('@') {
        return None;
    }
    let (name, value) = rest[1..].split_once(char::is_whitespace)?;
    Some((name.to_lowercase(), value.trim()))
}

/// Every script command in the folder, sorted by title.
pub fn list(folder: &std::path::Path) -> Vec<Script> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut found: Vec<Script> = entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| read(&entry.path()))
        .collect();
    found.sort_by_key(|script| script.title.to_lowercase());
    found
}

/// The command line that runs a script.
pub fn runner_command(script: &Script) -> (std::path::PathBuf, Vec<String>) {
    match script.runner {
        // No profile and explicitly bypassing execution policy: a script
        // command the user chose to run is one they can run, and their
        // profile belongs to their prompts, not commands.
        Runner::PowerShell => (
            "powershell".into(),
            vec![
                "-NoProfile".into(),
                "-ExecutionPolicy".into(),
                "Bypass".into(),
                "-File".into(),
                script.path.to_string_lossy().into_owned(),
            ],
        ),
        Runner::Batch => ("cmd".into(), vec!["/C".into(), script.path.to_string_lossy().into_owned()]),
        Runner::Python => ("python".into(), vec![script.path.to_string_lossy().into_owned()]),
    }
}

/// Run a script and collect its output.
pub fn run(script: &Script) -> Result<std::process::Output, RunError> {
    let (program, args) = runner_command(script);
    std::process::Command::new(&program)
        .args(&args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|err| RunError::Launch {
            path: script.path.clone(),
            reason: err.to_string(),
        })
}

/// A failure while running a script.
#[derive(Debug, thiserror::Error)]
pub enum RunError {
    /// The runner could not be started — absent, usually.
    #[error("`{path}` could not be run: {reason}")]
    Launch {
        path: std::path::PathBuf,
        reason: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch scripts folder, made fresh for each test.
    fn scratch() -> std::path::PathBuf {
        let folder = std::env::temp_dir()
            .join("yal-script-tests")
            .join(chrono_seconds());
        std::fs::create_dir_all(&folder).unwrap();
        folder
    }

    fn chrono_seconds() -> String {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .to_string()
    }

    #[test]
    fn metadata_comes_from_the_comment_header() {
        let folder = scratch();
        let script = folder.join("restart.ps1");
        std::fs::write(
            &script,
            "# @title Restart the audio\n\
             # @description stop and start the endpoint\n\
             # @mode full\n\
             # @keywords audio, sound\n\
             Write-Output hello\n",
        )
        .unwrap();

        let script = read(&script).unwrap();
        assert_eq!(script.title, "Restart the audio");
        assert_eq!(script.description.as_deref(), Some("stop and start the endpoint"));
        assert_eq!(script.mode, Mode::Full);
        assert_eq!(script.runner, Runner::PowerShell);
        assert_eq!(script.keywords, vec!["audio", "sound"]);
    }

    #[test]
    fn prose_that_mentions_a_directive_is_not_a_directive() {
        let folder = scratch();
        let script = folder.join("notes.py");
        std::fs::write(
            &script,
            "# this script is one where @title appears in prose\n\
             print('hi')\n",
        )
        .unwrap();

        let script = read(&script).unwrap();
        // The title falls back to the file name.
        assert_eq!(script.title, "notes");
        assert_eq!(script.mode, Mode::Compact);
    }

    #[test]
    fn batch_directives_use_the_batch_comment() {
        let folder = scratch();
        let script = folder.join("cleanup.cmd");
        std::fs::write(
            &script,
            ":: @title Clean temp files\n\
             :: @mode silent\n\
             @echo off\n",
        )
        .unwrap();

        let script = read(&script).unwrap();
        assert_eq!(script.title, "Clean temp files");
        assert_eq!(script.mode, Mode::Silent);
        assert_eq!(script.runner, Runner::Batch);
    }

    #[test]
    fn unknown_files_are_not_scripts() {
        let folder = scratch();
        let notes = folder.join("readme.txt");
        std::fs::write(&notes, "# @title not a script\n").unwrap();
        assert!(read(&notes).is_none(), "an unknown extension is not a script command");
    }

    #[test]
    fn a_file_that_cannot_be_decoded_is_still_a_script_without_directives() {
        // The file runs, because the runner does not read comments; keep
        // it offered, with the file name as the title and no metadata.
        let folder = scratch();
        let unreadable = folder.join("broken.ps1");
        std::fs::write(&unreadable, b"\xff\xfe\x00\x00\x01\x02").unwrap();

        let script = read(&unreadable).expect("it is still a PowerShell file");
        assert_eq!(script.title, "broken");
        assert_eq!(script.description, None);
        assert_eq!(script.keywords, Vec::<String>::new());
    }

    #[test]
    fn the_runner_command_matches_the_language() {
        let folder = scratch();
        let script = folder.join("go.py");
        std::fs::write(&script, "print('go')\n").unwrap();
        let script = read(&script).unwrap();

        let (program, args) = runner_command(&script);
        assert_eq!(program.to_string_lossy(), "python");
        assert_eq!(args.len(), 1);
        assert!(args[0].ends_with("go.py"));
    }

    #[test]
    fn the_folder_resolves_explicit_paths_first() {
        let explicit = std::path::PathBuf::from("C:/scripts");
        assert_eq!(
            resolve_folder(Some(explicit.clone())).as_deref(),
            Some(explicit.as_path())
        );
        assert!(default_folder().is_some());
    }
}
