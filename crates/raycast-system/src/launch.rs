//! Launching applications and opening paths.

use std::path::Path;
use std::process::Command;

/// A failure while launching something.
#[derive(Debug, thiserror::Error)]
pub enum LaunchError {
    #[error("`{0}` could not be started: {1}")]
    Failed(String, String),

    #[error("`{0}` does not exist")]
    Missing(String),
}

/// Launch an executable.
///
/// The child is started detached so the launcher does not wait for it and
/// does not inherit a console.
pub fn launch(program: &Path) -> Result<(), LaunchError> {
    if !program.exists() {
        return Err(LaunchError::Missing(program.display().to_string()));
    }
    Command::new(program)
        .spawn()
        .map(|_| ())
        .map_err(|err| LaunchError::Failed(program.display().to_string(), err.to_string()))
}

/// Open a path with its default handler.
pub fn open_path(path: &Path) -> Result<(), LaunchError> {
    if !path.exists() {
        return Err(LaunchError::Missing(path.display().to_string()));
    }
    Command::new("explorer")
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|err| LaunchError::Failed(path.display().to_string(), err.to_string()))
}

/// Open a URL in the default browser.
pub fn open_url(url: &str) -> Result<(), LaunchError> {
    Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn()
        .map(|_| ())
        .map_err(|err| LaunchError::Failed(url.to_string(), err.to_string()))
}

/// Reveal a path in the system file manager, selecting it.
pub fn reveal_in_file_manager(path: &Path) -> Result<(), LaunchError> {
    if !path.exists() {
        return Err(LaunchError::Missing(path.display().to_string()));
    }
    Command::new("explorer")
        .arg(format!("/select,{}", path.display()))
        .spawn()
        .map(|_| ())
        .map_err(|err| LaunchError::Failed(path.display().to_string(), err.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_program_is_reported() {
        let missing = Path::new("definitely-not-here-12345.exe");
        assert!(matches!(
            launch(missing),
            Err(LaunchError::Missing(_))
        ));
    }

    #[test]
    fn a_missing_path_is_reported() {
        let missing = Path::new("definitely-not-here-12345");
        assert!(matches!(
            open_path(missing),
            Err(LaunchError::Missing(_))
        ));
    }
}
