//! Running the row the user chose.
//!
//! Two palettes can show the same rows: the browser-based one and the native
//! one. A row has to behave the same way in both, so the behaviour lives
//! here, and each palette calls it.
//!
//! The runner is shared between them, so it holds what a row needs to be
//! run - the launcher, so the ranking learns what was opened, and the window
//! commands, when window management is on - and nothing that belongs to a
//! particular window.

use std::sync::Arc;

use tc_commands::catalog::display_name;
use tc_commands::model::CustomCommand;
use tc_commands::runner::{is_command_not_found, ShellRunner};
use tc_core::CommandId;
use tc_platform::Clipboard;
use tc_platform_win::window_control::WindowCommands;

use crate::launcher::{LauncherState, CALCULATOR_PREFIX, FILE_PREFIX};

/// Runs the rows the user chooses.
pub struct Runner {
    /// The launcher, so the ranking learns what the user opened.
    launcher: Arc<LauncherState>,
    /// The window commands, when window management is on.
    windows: Option<Arc<WindowCommands>>,
    /// What to do when the user runs the Quit command.
    quit: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl Runner {
    /// A runner for a launcher.
    pub fn new(launcher: Arc<LauncherState>) -> Runner {
        Runner {
            launcher,
            windows: None,
            quit: None,
        }
    }

    /// Gives the runner the window commands.
    pub fn with_windows(mut self, windows: Arc<WindowCommands>) -> Runner {
        self.windows = Some(windows);
        self
    }

    /// Gives the runner the way to quit the application.
    pub fn with_quit(mut self, quit: Arc<dyn Fn() + Send + Sync>) -> Runner {
        self.quit = Some(quit);
        self
    }

    /// Opens the row with this id, and learns the query that reached it.
    ///
    /// `confirmed` says the user has already answered the "are you sure"
    /// question. A row that needs the answer and has not been given it is
    /// refused, so the palette can ask.
    pub fn open(&self, id: &str, query: &str, confirmed: bool) -> Result<(), String> {
        // A system action is run, not opened, and the ones that end the
        // session or empty the bin have to be asked for first.
        if let Some(action) = LauncherState::system_action_of(id) {
            let needs_confirm = action.confirmation() != tc_core::SystemActionConfirmation::None;
            if needs_confirm && !confirmed {
                return Err(format!("`{}` has to be confirmed", action.english_name()));
            }
            tc_platform_win::run(action)?;
            return Ok(());
        }
        // A window command is not opened either; it moves the window the
        // user was last using.
        if let Some(command) = LauncherState::window_command_of(id) {
            let Some(commands) = self.windows.as_ref() else {
                return Err("window management is not on".to_string());
            };
            let Some(handle) = tc_platform_win::window_control::last_foreground() else {
                return Err("there is no window to move".to_string());
            };
            commands
                .run(handle, command)
                .map_err(|error| error.to_string())?;
            return Ok(());
        }
        // A calculator row is not opened; its answer is copied.
        if let Some(text) = id.strip_prefix(CALCULATOR_PREFIX) {
            tc_platform_win::clipboard().write_text(text)?;
            return Ok(());
        }
        // A file row is opened, not run: the target is the path itself.
        if let Some(path) = id.strip_prefix(FILE_PREFIX) {
            tc_platform_win::open(path)?;
            return Ok(());
        }
        // A built-in command is run, not opened.
        if let Some(command) = LauncherState::command_of(id) {
            return self.run_built_in(command);
        }
        // One of the user's own commands is run as well, but the palette asks
        // for its values first, so the values come with the request.
        if let Some(command) = self.launcher.custom_command_of(id) {
            let command = command.clone();
            run_custom(&command, &[])?;
            return Ok(());
        }
        let Some((target, _)) = self.launcher.target_of(id) else {
            return Err(format!("`{id}` is not a result the launcher knows"));
        };
        tc_platform_win::open(&target)?;
        // The ranking learns the query only when the thing really opened.
        self.launcher.remember(id, query);
        Ok(())
    }

    /// Runs one of the built-in commands.
    ///
    /// The commands whose features are not in this build yet say so, and do
    /// nothing else.
    fn run_built_in(&self, command: CommandId) -> Result<(), String> {
        match command {
            CommandId::Quit => {
                if let Some(quit) = self.quit.as_ref() {
                    quit();
                }
                Ok(())
            }
            CommandId::Support => {
                tc_platform_win::open("https://tinycast.dev/support")?;
                Ok(())
            }
            other => Err(format!(
                "`{}` is not in this build yet",
                display_name(other)
            )),
        }
    }
}

/// Runs one of the user's own commands, with the values it asked for.
///
/// The command runs off the interface thread, so a long command does not
/// freeze the palette.
pub fn run_custom(command: &CustomCommand, values: &[String]) -> Result<String, String> {
    let Some(values) = command.positional_values(values) else {
        return Err("this command needs a value for every required field".to_string());
    };
    let plan = command
        .shell
        .plan(
            &command.command,
            &values,
            command.loads_shell_environment,
            command.working_directory.as_deref(),
        )
        .map_err(|error| error.to_string())?;
    let mut running = ShellRunner::spawn(&plan).map_err(|error| error.to_string())?;
    let outcome = running.wait();
    if outcome.succeeded() {
        return Ok(outcome.success_message(&command.name));
    }
    // "When a command exits with 127 while this setting is off, the error
    // message says so and offers Open Settings." Windows PowerShell has no
    // exit code for that, so the message is what the port looks for.
    if is_command_not_found(&outcome.stderr) && !command.loads_shell_environment {
        return Err(format!(
            "`{}` was not found. The shell's own configuration was not read. \
             Turn on Load shell environment in the command's editor, or use the \
             full path to the program.",
            command.name
        ));
    }
    let message = outcome.failure_message();
    if message.is_empty() {
        Err(format!(
            "`{}` failed with code {}",
            command.name, outcome.status
        ))
    } else {
        Err(message)
    }
}
