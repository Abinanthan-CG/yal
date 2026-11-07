//! Launching applications and opening paths.
//!
//! Almost everything goes through the shell (`ShellExecuteW`), because
//! the shell knows Windows better than any loader call does: it honours
//! a shortcut's own arguments, working directory, compatibility flags
//! and elevation level, starts Store applications only through their
//! shell identity, and answers with a code the failure can be read
//! from. A Store application's parsing name is
//! `shell:AppsFolder\<AUMID>` — the one way to start such an
//! application, which cannot be spawned from its executable directly.

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

/// The parts of one application launch, shaped as the discovery sees
/// them. At most one of program, shortcut or AUMID is meant to act:
/// the AUMID wins when present (a Store application), then the shortcut
/// (the shell applies everything stored inside it), then the program.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ApplicationLaunch<'a> {
    /// The program to start, with `arguments` and `working_directory` applied.
    pub program: Option<&'a Path>,
    /// The shortcut file to hand the shell: `.lnk`, `.url`, `.appref-ms`.
    pub shortcut: Option<&'a Path>,
    /// Arguments for the program, when the launch is a program.
    pub arguments: Option<&'a str>,
    /// The working directory the program starts in.
    pub working_directory: Option<&'a Path>,
    /// The Store application's AUMID.
    pub aumid: Option<&'a str>,
}

/// Open anything the shell knows how to open.
///
/// The answer code from `ShellExecuteW` is the shell's own verdict:
/// values above 32 are success, values of 32 and below are the failure
/// table (no association, no access, out of memory…).
fn shell_open(
    target: &str,
    parameters: Option<&str>,
    working_directory: Option<&Path>,
) -> Result<(), LaunchError> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::{PCWSTR, w};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let wide: Vec<u16> = std::ffi::OsStr::new(target)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let params: Vec<u16> = parameters
        .map(|p| std::ffi::OsStr::new(p).encode_wide().chain(std::iter::once(0)).collect::<Vec<_>>())
        .unwrap_or_default();
    let directory: Vec<u16> = working_directory
        .map(|d| d.as_os_str().encode_wide().chain(std::iter::once(0)).collect::<Vec<_>>())
        .unwrap_or_default();

    // COM is initialised should the shell want it; the balance is kept:
    // only a successful initialise here gets an uninitialise later.
    let com_initialised =
        unsafe { windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED) }.is_ok();

    let instance = unsafe {
        ShellExecuteW(
            Some(HWND::default()),
            PCWSTR(w!("open").as_ptr()),
            PCWSTR(wide.as_ptr()),
            if params.is_empty() { PCWSTR::null() } else { PCWSTR(params.as_ptr()) },
            if directory.is_empty() { PCWSTR::null() } else { PCWSTR(directory.as_ptr()) },
            SW_SHOWNORMAL,
        )
    };
    if com_initialised {
        unsafe { windows::Win32::System::Com::CoUninitialize() };
    }

    if instance.0 as isize > 32 {
        Ok(())
    } else {
        Err(LaunchError::Failed(
            target.to_string(),
            format!("the shell answered {}", instance.0 as isize),
        ))
    }
}

/// Launch an installed application — the highest-level launch: an
/// executable, a shortcut file, or a Store application by its AUMID.
pub fn application(launch: ApplicationLaunch) -> Result<(), LaunchError> {
    if let Some(aumid) = launch.aumid {
        return shell_open(&format!("shell:AppsFolder\\{aumid}"), None, None);
    }
    if let Some(shortcut) = launch.shortcut {
        return shell_open(
            &shortcut.to_string_lossy(),
            None,
            launch.working_directory,
        );
    }
    let Some(program) = launch.program else {
        return Err(LaunchError::Failed(
            "(no target)".to_string(),
            "the launch carries no program, shortcut or Store identity".to_string(),
        ));
    };
    if !program.exists() {
        return Err(LaunchError::Missing(program.display().to_string()));
    }
    shell_open(
        &program.to_string_lossy(),
        launch.arguments,
        launch.working_directory,
    )
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

/// Launch a program and its arguments, with no window of its own.
///
/// For console programs launched on the user's behalf from the palette:
/// the child runs detached from the palette, and no host console is
/// created for it, so a PowerShell command does not flash a window over
/// the palette for the moment of its run.
pub fn launch_program(program: &str, args: &[String]) -> Result<(), LaunchError> {
    let mut command = Command::new(program);
    command.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: 0x08000000, stable since Windows NT 4 and
        // published in the Process Creation documentation.
        command.creation_flags(0x08000000);
    }
    command
        .spawn()
        .map(|_| ())
        .map_err(|err| LaunchError::Failed(program.to_string(), err.to_string()))
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
