//! System-level commands: lock, sleep, shutdown, restart, log out and
//! emptying the recycle bin.
//!
//! These commands act immediately when they are carried out. Nothing in the
//! launcher runs them on its own behalf; each is bound to an explicit user
//! choice.
//!
//! Lock, sleep and emptying the bin are made through the system interfaces
//! directly. Shutdown, restart and logging out go through the system
//! utilities, which handle the privilege and the grace period for us.

/// A failure while carrying out a system command.
#[derive(Debug, thiserror::Error)]
pub enum SystemError {
    #[error("{0}")]
    Failed(String),
}

/// Lock the workstation.
///
/// The user's session stays open behind the lock screen.
#[cfg(windows)]
pub fn lock() -> Result<(), SystemError> {
    use windows::Win32::System::Shutdown::LockWorkStation;

    unsafe { LockWorkStation().map_err(|err| SystemError::Failed(err.to_string())) }
}

/// Lock the workstation on a host without the command.
#[cfg(not(windows))]
pub fn lock() -> Result<(), SystemError> {
    Err(SystemError::Failed(
        "this command is not implemented on this platform".into(),
    ))
}

/// Put the machine to sleep.
///
/// Hibernate is off, so a sleep is an ordinary suspend: the session stays in
/// memory and the machine wakes where it left off.
#[cfg(windows)]
pub fn sleep() -> Result<(), SystemError> {
    use windows::Win32::System::Power::SetSuspendState;

    unsafe {
        if !SetSuspendState(false, false, false) {
            return Err(SystemError::Failed(
                "the system did not begin the suspend".into(),
            ));
        }
    }
    Ok(())
}

/// Sleep on a host without the command.
#[cfg(not(windows))]
pub fn sleep() -> Result<(), SystemError> {
    Err(SystemError::Failed(
        "this command is not implemented on this platform".into(),
    ))
}

/// Shut the machine down.
///
/// A short grace period is left so the user can change their mind; Windows
/// cancels a pending shutdown when it is asked again with the cancel flag.
#[cfg(windows)]
pub fn shutdown() -> Result<(), SystemError> {
    run_system_utility("/s", "/t", "5")
}

/// Restart the machine.
#[cfg(windows)]
pub fn restart() -> Result<(), SystemError> {
    run_system_utility("/r", "/t", "5")
}

/// End the session, returning to the sign-in screen.
#[cfg(windows)]
pub fn log_out() -> Result<(), SystemError> {
    run_system_utility("/l", "", "")
}

/// Shut down through the system utility.
#[cfg(windows)]
fn run_system_utility(action: &str, timeout_flag: &str, timeout: &str) -> Result<(), SystemError> {
    use std::process::Command;

    let mut command = Command::new("shutdown");
    command.arg(action);
    if !timeout_flag.is_empty() {
        command.arg(timeout_flag).arg(timeout);
    }
    let status = command
        .status()
        .map_err(|err| SystemError::Failed(err.to_string()))?;
    if !status.success() {
        return Err(SystemError::Failed(
            "the system did not perform the command".into(),
        ));
    }
    Ok(())
}

/// Empty the recycle bin.
///
/// `HERB_NOCONFIRMATION` and `HERB_NOPROGRESSUI` skip the two prompts that
/// would otherwise appear; the launcher has already asked the user once, in
/// the palette.
#[cfg(windows)]
pub fn empty_recycle_bin() -> Result<(), SystemError> {
    use windows::Win32::UI::Shell::{
        SHEmptyRecycleBinW, SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND,
    };

    unsafe {
        let flags = SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND;
        SHEmptyRecycleBinW(None, windows::core::PCWSTR::null(), flags)
            .map_err(|err| SystemError::Failed(err.to_string()))
    }
}

/// Empty the bin on a host without the command.
#[cfg(not(windows))]
pub fn empty_recycle_bin() -> Result<(), SystemError> {
    Err(SystemError::Failed(
        "this command is not implemented on this platform".into(),
    ))
}

/// A system command the palette offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemCommand {
    Lock,
    Sleep,
    Shutdown,
    Restart,
    LogOut,
    EmptyBin,
}

impl SystemCommand {
    /// What the user sees as the result title.
    pub fn title(self) -> &'static str {
        match self {
            SystemCommand::Lock => "Lock screen",
            SystemCommand::Sleep => "Put the machine to sleep",
            SystemCommand::Shutdown => "Shut down",
            SystemCommand::Restart => "Restart",
            SystemCommand::LogOut => "Log out",
            SystemCommand::EmptyBin => "Empty recycle bin",
        }
    }

    /// Terms the command can be found by in the palette.
    pub fn keywords(self) -> &'static [&'static str] {
        match self {
            SystemCommand::Lock => &["lock", "screen", "password", "secure"],
            SystemCommand::Sleep => &["sleep", "suspend", "standby", "power"],
            SystemCommand::Shutdown => &["shutdown", "power off", "turn off", "halt"],
            SystemCommand::Restart => &["restart", "reboot", "power cycle"],
            SystemCommand::LogOut => &["logout", "log out", "sign out", "session"],
            SystemCommand::EmptyBin => &["empty", "trash", "recycle", "bin", "clean"],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_command_has_a_title_and_keywords() {
        for command in [
            SystemCommand::Lock,
            SystemCommand::Sleep,
            SystemCommand::Shutdown,
            SystemCommand::Restart,
            SystemCommand::LogOut,
            SystemCommand::EmptyBin,
        ] {
            assert!(!command.title().is_empty());
            assert!(!command.keywords().is_empty());
        }
    }
}
