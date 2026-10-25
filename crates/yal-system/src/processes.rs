//! Running processes: what is on the machine and how to stop one.
//!
//! Terminate goes through the process handle rather than through a shell
//! utility, so a failure comes back as a real reason rather than a message
//! that has to be parsed.

use sysinfo::{Pid, System};

/// A failure while working with processes.
#[derive(Debug, thiserror::Error)]
pub enum ProcessError {
    #[error("the process is gone")]
    Gone,

    #[error("the process could not be ended: {0}")]
    Failed(String),
}

/// One running process.
#[derive(Debug, Clone)]
pub struct Running {
    /// The system identifier.
    pub process_id: u32,

    /// The executable's file name.
    pub name: String,

    /// The working set, in bytes.
    pub memory: u64,
}

/// Every running process.
///
/// The snapshot is refreshed before it is read so the list matches what is
/// running now rather than what was running when this process started.
pub fn running() -> Vec<Running> {
    let mut system = System::new();
    system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    let mut found: Vec<Running> = system
        .processes()
        .iter()
        .map(|(pid, process)| Running {
            process_id: pid.as_u32(),
            name: process.name().to_string_lossy().to_string(),
            memory: process.memory(),
        })
        .collect();

    found.sort_by_key(|process| std::cmp::Reverse(process.memory));
    found
}

/// End a process.
///
/// The process is asked to terminate. A process may ignore the request; a
/// real launcher reports that rather than forcing it, which is why there is
/// no kill-by-force path here.
pub fn terminate(process_id: u32) -> Result<(), ProcessError> {
    let mut system = System::new();
    system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[Pid::from_u32(process_id)]), true);

    let Some(process) = system.process(Pid::from_u32(process_id)) else {
        return Err(ProcessError::Gone);
    };

    if process.kill() {
        Ok(())
    } else {
        Err(ProcessError::Failed(format!(
            "process {process_id} refused to end"
        )))
    }
}

/// The name a process runs as, or a reason.
pub fn process_name(process_id: u32) -> Option<String> {
    let mut system = System::new();
    system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[Pid::from_u32(process_id)]), true);
    system
        .process(Pid::from_u32(process_id))
        .map(|process| process.name().to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminates_a_process_it_started() {
        // A short-lived process that is not our own and harms nothing.
        #[cfg(windows)]
        let spawn_path = std::path::PathBuf::from("ping.exe");
        #[cfg(windows)]
        let spawn_args = vec!["-n".to_string(), "30".to_string(), "127.0.0.1".to_string()];
        #[cfg(not(windows))]
        let spawn_path = std::path::PathBuf::from("sleep");
        #[cfg(not(windows))]
        let spawn_args = vec!["30".to_string()];

        let mut child = std::process::Command::new(spawn_path)
            .args(spawn_args)
            .spawn()
            .expect("the test needs a child process");
        let pid = child.id();

        // Give the process a moment to appear in the snapshot.
        std::thread::sleep(std::time::Duration::from_millis(400));
        assert!(process_name(pid).is_some(), "the child was not found");

        terminate(pid).expect("the child should end");

        // Wait for the registry to notice, then check it is gone.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while std::time::Instant::now() < deadline {
            if process_name(pid).is_none() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        assert!(process_name(pid).is_none(), "the child should be gone");

        let _ = child.wait();
    }

    #[test]
    fn an_ended_process_reports_gone() {
        assert!(matches!(
            terminate(u32::MAX - 7),
            Err(ProcessError::Gone)
        ));
    }
}
