//! Running a command, and stopping it.
//!
//! Upstream runs a command in a pty, reads the output from the master side
//! and keeps the last 256 KB of it. Windows has no pty, so the port captures
//! the output into files and reads the tail of them; the rules about what the
//! user sees are the same.
//!
//! * The command runs with no console window.
//! * Its input is closed, so anything that reads input gets an end of file.
//! * The environment is inherited, plus `TINYCAST=1`.
//! * The command and everything it starts stay in a job object, so stopping it
//!   stops the whole tree, and quitting the app leaves it running.

use std::io::{Read, Seek};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::shell::{LaunchError, LaunchPlan};

/// How much of the standard output the app keeps for a short message.
pub const STANDARD_OUTPUT_LIMIT: usize = 4 * 1024;

////// How much of the error output a failure dialog shows.
pub const ERROR_OUTPUT_LIMIT: usize = 8 * 1024;

/// How much output the output window keeps.
pub const OUTPUT_WINDOW_LIMIT: usize = 256 * 1024;

/// The name the environment carries, so a shell profile can tell that the
/// command was started by the launcher.
pub const ENVIRONMENT_MARKER: &str = "TINYCAST";

/// What a finished command left behind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOutcome {
    /// The command's exit code.
    pub status: i32,
    /// The last of what it printed.
    pub stdout: String,
    /// The last of what it complained about.
    pub stderr: String,
    /// How long it took.
    pub duration: Duration,
    /// Whether it was stopped rather than finishing on its own.
    pub stopped: bool,
}

impl RunOutcome {
    /// Whether the command finished well.
    pub fn succeeded(&self) -> bool {
        self.status == 0 && !self.stopped
    }

    /// The message a successful command shows.
    ///
    /// "a short message shows the last line of output, or `Ran <name>` if the
    /// command printed nothing."
    pub fn success_message(&self, name: &str) -> String {
        last_line(&self.stdout).unwrap_or_else(|| format!("Ran {name}"))
    }

    /// The message a failed command shows.
    ///
    /// "a dialog shows the error output, up to the last 8 KB."
    pub fn failure_message(&self) -> String {
        let tail: String = self.stderr.chars().rev().take(ERROR_OUTPUT_LIMIT).collect();
        tail.chars().rev().collect::<String>().trim().to_string()
    }
}

/// The last line of some output that has something in it.
pub fn last_line(output: &str) -> Option<String> {
    output
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_string)
}

/// Whether the output says the shell could not find the command.
///
/// Upstream looks for exit code 127 with the shell environment switched off.
/// Windows PowerShell has no exit code for that: it writes a message
/// instead, so the message is what the port looks for.
pub fn is_command_not_found(output: &str) -> bool {
    let output = output.to_lowercase();
    output.contains("is not recognized as the name of a cmdlet")
        || output.contains("is not recognized as an internal or external command")
        || output.contains("command not found")
        || output.contains("was not found")
}

/// A command that is running.
pub struct RunningCommand {
    inner: platform::RunningCommand,
    stdout_path: PathBuf,
    stderr_path: PathBuf,
    script_path: Option<PathBuf>,
    started: Instant,
    stopped: bool,
}

impl RunningCommand {
    /// Starts the command.
    pub fn spawn(plan: &LaunchPlan) -> Result<RunningCommand, LaunchError> {
        let started = Instant::now();
        let (inner, stdout_path, stderr_path) = platform::spawn(plan)?;
        Ok(RunningCommand {
            inner,
            stdout_path,
            stderr_path,
            script_path: plan.script.clone(),
            started,
            stopped: false,
        })
    }

    /// Waits for the command to finish.
    pub fn wait(&mut self) -> RunOutcome {
        let status = self.inner.wait();
        let duration = self.started.elapsed();
        let stdout = tail(&self.stdout_path, OUTPUT_WINDOW_LIMIT);
        let stderr = tail(&self.stderr_path, OUTPUT_WINDOW_LIMIT);
        self.clean_up();
        RunOutcome {
            status,
            stdout,
            stderr,
            duration,
            stopped: self.stopped,
        }
    }

    /// Stops the command and everything it started.
    pub fn stop(&mut self) {
        self.stopped = true;
        self.inner.stop();
        self.clean_up();
    }

    /// Removes the temporary script the command ran from.
    fn clean_up(&mut self) {
        if let Some(path) = self.script_path.take() {
            let _ = std::fs::remove_file(path);
        }
    }

    /// Whether the command has been stopped.
    pub fn is_stopped(&self) -> bool {
        self.stopped
    }

    /// What the command has printed so far.
    pub fn output(&self) -> String {
        tail(&self.stdout_path, OUTPUT_WINDOW_LIMIT)
    }

    /// What the command has complained about so far.
    pub fn error_output(&self) -> String {
        tail(&self.stderr_path, OUTPUT_WINDOW_LIMIT)
    }
}

/// Starts commands.
pub struct ShellRunner;

impl ShellRunner {
    /// Starts a command.
    pub fn spawn(plan: &LaunchPlan) -> Result<RunningCommand, LaunchError> {
        RunningCommand::spawn(plan)
    }
}

/// The last `limit` bytes of a file, as text.
pub fn tail_of_file(path: &Path, limit: usize) -> String {
    tail(path, limit)
}

/// The last `limit` bytes of a file, as text.
fn tail(path: &Path, limit: usize) -> String {
    let Ok(mut file) = std::fs::File::open(path) else {
        return String::new();
    };
    let Ok(length) = file.metadata().map(|meta| meta.len()) else {
        return String::new();
    };
    let start = length.saturating_sub(limit as u64);
    if file.seek(std::io::SeekFrom::Start(start)).is_err() {
        return String::new();
    }
    let mut bytes = Vec::new();
    if file.read_to_end(&mut bytes).is_err() {
        return String::new();
    }
    String::from_utf8_lossy(&bytes)
        .trim_start_matches(['\u{feff}', '\r', '\n'])
        .to_string()
}

#[cfg(windows)]
mod platform {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::AsRawHandle;
    use std::path::PathBuf;

    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::Foundation::{
        CloseHandle, SetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT,
    };
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, TerminateJobObject,
    };
    use windows::Win32::System::Threading::{
        CreateProcessW, GetExitCodeProcess, ResumeThread, WaitForSingleObject, CREATE_NO_WINDOW,
        CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT, PROCESS_INFORMATION, STARTF_USESTDHANDLES,
        STARTUPINFOW,
    };

    use crate::shell::{LaunchError, LaunchPlan};

    /// The exit code Windows reports while a process is still running.
    const STILL_ACTIVE: u32 = 259;

    /// A command that is running.
    pub struct RunningCommand {
        process: HANDLE,
        job: HANDLE,
    }

    impl Drop for RunningCommand {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.process);
                let _ = CloseHandle(self.job);
            }
        }
    }

    impl RunningCommand {
        /// Waits for the command to finish and returns its exit code.
        pub fn wait(&self) -> i32 {
            unsafe {
                let _ = WaitForSingleObject(self.process, u32::MAX);
                let mut code: u32 = 0;
                let _ = GetExitCodeProcess(self.process, &mut code);
                if code == STILL_ACTIVE {
                    return -1;
                }
                code as i32
            }
        }

        /// Stops the command and everything it started.
        pub fn stop(&self) {
            unsafe {
                let _ = TerminateJobObject(self.job, 1);
            }
        }
    }

    /// Starts the command described by the plan.
    pub fn spawn(plan: &LaunchPlan) -> Result<(RunningCommand, PathBuf, PathBuf), LaunchError> {
        let stdout_path = temp_path("out");
        let stderr_path = temp_path("err");
        let stdout_file =
            std::fs::File::create(&stdout_path).map_err(|_| LaunchError::OutputNotWritable)?;
        let stderr_file =
            std::fs::File::create(&stderr_path).map_err(|_| LaunchError::OutputNotWritable)?;
        let stdin_file = std::fs::File::open("NUL").map_err(|_| {
            LaunchError::ShellNotFound("The null device could not be opened.".to_string())
        })?;

        make_inheritable(stdin_file.as_raw_handle())?;
        make_inheritable(stdout_file.as_raw_handle())?;
        make_inheritable(stderr_file.as_raw_handle())?;

        let job = unsafe { CreateJobObjectW(None, PCWSTR::null()) }.map_err(|_| {
            LaunchError::ShellNotFound("The command could not be started.".to_string())
        })?;

        let startup = STARTUPINFOW {
            cb: std::mem::size_of::<STARTUPINFOW>() as u32,
            dwFlags: STARTF_USESTDHANDLES,
            hStdInput: HANDLE(stdin_file.as_raw_handle()),
            hStdOutput: HANDLE(stdout_file.as_raw_handle()),
            hStdError: HANDLE(stderr_file.as_raw_handle()),
            ..Default::default()
        };

        // The application name is left empty on purpose: Windows only searches
        // PATH when the program is named in the command line.
        let mut command_line = wide_text(&build_command_line(plan));
        let environment = environment_block();
        let directory = wide(plan.working_directory.as_os_str());

        let mut info = PROCESS_INFORMATION::default();
        unsafe {
            CreateProcessW(
                PCWSTR::null(),
                Some(PWSTR(command_line.as_mut_ptr())),
                None,
                None,
                true,
                CREATE_NO_WINDOW | CREATE_SUSPENDED | CREATE_UNICODE_ENVIRONMENT,
                Some(environment.as_ptr() as *const std::ffi::c_void),
                PCWSTR(directory.as_ptr()),
                &startup,
                &mut info,
            )
            .map_err(|error| {
                LaunchError::ShellNotFound(format!(
                    "{} could not be started: {error}",
                    plan.program.to_string_lossy()
                ))
            })?;

            // The child is suspended, so it cannot start anything before it is
            // in the job object.
            if AssignProcessToJobObject(job, info.hProcess).is_err() {
                let _ = CloseHandle(info.hProcess);
                let _ = CloseHandle(info.hThread);
                return Err(LaunchError::ShellNotFound(
                    "The command could not be started.".to_string(),
                ));
            }
            let _ = ResumeThread(info.hThread);
            let _ = CloseHandle(info.hThread);
        }

        Ok((
            RunningCommand {
                process: info.hProcess,
                job,
            },
            stdout_path,
            stderr_path,
        ))
    }

    /// Lets a handle pass to the child process.
    fn make_inheritable(handle: *mut std::ffi::c_void) -> Result<(), LaunchError> {
        unsafe {
            SetHandleInformation(HANDLE(handle), HANDLE_FLAG_INHERIT.0, HANDLE_FLAG_INHERIT)
                .map_err(|_| {
                    LaunchError::ShellNotFound("The command could not be started.".to_string())
                })
        }
    }

    /// The command line, quoted the way Windows expects.
    fn build_command_line(plan: &LaunchPlan) -> String {
        let mut parts = vec![quote(&plan.program.to_string_lossy())];
        for argument in &plan.arguments {
            parts.push(quote(argument));
        }
        parts.join(" ")
    }

    /// One argument, quoted so the child reads it back exactly as it was.
    fn quote(argument: &str) -> String {
        if !argument.is_empty()
            && !argument
                .chars()
                .any(|c| c == ' ' || c == '\t' || c == '\n' || c == '"' || c == '\u{0}')
        {
            return argument.to_string();
        }
        let mut out = String::from("\"");
        let mut slashes = 0usize;
        for character in argument.chars() {
            if character == '\\' {
                slashes += 1;
                continue;
            }
            if character == '"' {
                out.push_str(&"\\".repeat(slashes * 2 + 1));
                out.push('"');
                slashes = 0;
                continue;
            }
            out.push_str(&"\\".repeat(slashes));
            slashes = 0;
            out.push(character);
        }
        out.push_str(&"\\".repeat(slashes * 2));
        out.push('"');
        out
    }

    /// The environment the child gets: the app's, plus the marker.
    fn environment_block() -> Vec<u16> {
        let mut variables: Vec<(String, String)> = std::env::vars().collect();
        let marker = super::ENVIRONMENT_MARKER;
        match variables
            .iter_mut()
            .find(|(key, _)| key.eq_ignore_ascii_case(marker))
        {
            Some(entry) => entry.1 = "1".to_string(),
            None => variables.push((marker.to_string(), "1".to_string())),
        }
        variables.sort_by_key(|(key, _)| key.to_lowercase());
        let mut block: Vec<u16> = Vec::new();
        for (key, value) in variables {
            block.extend(key.encode_utf16());
            block.push(b'=' as u16);
            block.extend(value.encode_utf16());
            block.push(0);
        }
        block.push(0);
        block
    }

    /// A wide string, with the terminator the Win32 calls expect.
    fn wide(value: &OsStr) -> Vec<u16> {
        let mut wide: Vec<u16> = value.encode_wide().collect();
        wide.push(0);
        wide
    }

    /// A wide string from text, with the terminator the Win32 calls expect.
    fn wide_text(value: &str) -> Vec<u16> {
        let mut wide: Vec<u16> = value.encode_utf16().collect();
        wide.push(0);
        wide
    }

    /// A path for the captured output.
    fn temp_path(kind: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "yal-command-{kind}-{}.log",
            uuid::Uuid::new_v4().simple()
        ));
        path
    }
}

#[cfg(not(windows))]
mod platform {
    use std::path::PathBuf;

    use crate::shell::{LaunchError, LaunchPlan};

    /// A command that is running.
    pub struct RunningCommand;

    impl RunningCommand {
        /// Waits for the command to finish and returns its exit code.
        pub fn wait(&self) -> i32 {
            0
        }

        /// Stops the command and everything it started.
        pub fn stop(&self) {}
    }

    /// Starts the command described by the plan.
    ///
    /// The port is Windows-only; this keeps the crate compiling elsewhere.
    pub fn spawn(_plan: &LaunchPlan) -> Result<(RunningCommand, PathBuf, PathBuf), LaunchError> {
        Err(LaunchError::ShellNotFound(
            "Commands run on Windows only.".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests;
