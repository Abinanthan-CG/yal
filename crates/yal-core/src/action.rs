//! What happens when an item is chosen.
//!
//! An action is a small, closed set of intents. Keeping it closed means the
//! shell can decide *how* to carry it out (which process, which window
//! owner, which clipboard) without the item needing to know.

use std::path::PathBuf;

/// The intent behind an action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionKind {
    /// Do nothing beyond dismissing the palette.
    None,

    /// Launch an installed application.
    OpenApplication,

    /// Open a URL in the default browser.
    OpenUrl,

    /// Open a file or directory with its default handler.
    OpenPath,

    /// Place text on the clipboard.
    CopyToClipboard,

    /// Enter text into the focused window of another application.
    PasteToClipboard,

    /// Write text into a file.
    WriteFile,

    /// Run a program with arguments.
    RunCommand,

    /// Reveal a path in the system file manager.
    RevealInFileManager,

    /// Start a search in another application.
    OpenInApplication,

    /// Perform a task the launcher itself owns.
    ///
    /// System commands, window management, stopping processes and timers
    /// are not intents about other programs: the launcher carries them out
    /// through its host integration.
    LauncherTask,
}

/// The payload carried by an action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenTarget {
    /// An application identifier from the installed-application index.
    Application(String),

    /// A URL.
    Url(String),

    /// A filesystem path.
    Path(PathBuf),

    /// Literal text to place on the clipboard.
    Text(String),

    /// An application launch, shaped as the index handed it in.
    ///
    /// At most one part acts: the Store identity when the item is a
    /// Store application, else the shortcut file the shell applies
    /// whole (its own arguments, working directory and compatibility
    /// settings), else the program with its arguments and working
    /// directory.
    Launch {
        /// The program to start.
        program: Option<PathBuf>,
        /// The shortcut file to hand the shell.
        shortcut: Option<PathBuf>,
        /// Arguments the launch passes to the program.
        args: Option<String>,
        /// The working directory the program starts in.
        working_directory: Option<PathBuf>,
        /// The Store application's AUMID.
        aumid: Option<String>,
    },

    /// A program and its arguments.
    Command {
        program: PathBuf,
        args: Vec<String>,
    },

    /// An application plus text to paste into it.
    ApplicationAndText {
        application: String,
        text: String,
    },

    /// A launcher-owned task and its arguments.
    Task {
        /// The task name, e.g. `system`, `kill`, `timer`.
        name: String,

        /// Everything the task needs to be carried out.
        arguments: Vec<String>,
    },
}

/// A complete, executable intent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    pub kind: ActionKind,
    pub target: OpenTarget,
}

impl Action {
    /// Launch an installed application.
    pub fn open_application(id: impl Into<String>) -> Self {
        Self {
            kind: ActionKind::OpenApplication,
            target: OpenTarget::Application(id.into()),
        }
    }

    /// Open a URL.
    pub fn open_url(url: impl Into<String>) -> Self {
        Self {
            kind: ActionKind::OpenUrl,
            target: OpenTarget::Url(url.into()),
        }
    }

    /// Open a path.
    pub fn open_path(path: impl Into<PathBuf>) -> Self {
        Self {
            kind: ActionKind::OpenPath,
            target: OpenTarget::Path(path.into()),
        }
    }

    /// Launch an installed application as its entry named it.
    ///
    /// The parts are the index's honest recording of the entry: a Store
    /// app gives its AUMID, a shortcut gives itself (the shell applies
    /// everything stored inside), a plain executable gives its path
    /// with arguments and a working directory when the entry carries
    /// them.
    ///
    /// The shell-exe fields are optional because a Store application
    /// has none: its path in `WindowsApps` is not even the launcher's
    /// to read.
    pub fn launch(
        program: Option<PathBuf>,
        shortcut: Option<PathBuf>,
        args: Option<String>,
        working_directory: Option<PathBuf>,
        aumid: Option<String>,
    ) -> Self {
        Self {
            kind: ActionKind::OpenApplication,
            target: OpenTarget::Launch {
                program,
                shortcut,
                args,
                working_directory,
                aumid,
            },
        }
    }

    /// Copy text to the clipboard.
    pub fn copy(text: impl Into<String>) -> Self {
        Self {
            kind: ActionKind::CopyToClipboard,
            target: OpenTarget::Text(text.into()),
        }
    }

    /// Paste text into the previously focused window.
    pub fn paste(text: impl Into<String>) -> Self {
        Self {
            kind: ActionKind::PasteToClipboard,
            target: OpenTarget::Text(text.into()),
        }
    }

    /// Reveal a path in the file manager.
    pub fn reveal(path: impl Into<PathBuf>) -> Self {
        Self {
            kind: ActionKind::RevealInFileManager,
            target: OpenTarget::Path(path.into()),
        }
    }

    /// Run a program.
    pub fn run(program: impl Into<PathBuf>, args: Vec<String>) -> Self {
        Self {
            kind: ActionKind::RunCommand,
            target: OpenTarget::Command {
                program: program.into(),
                args,
            },
        }
    }

    /// Open a path with a specific application.
    pub fn open_in(application: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            kind: ActionKind::OpenInApplication,
            target: OpenTarget::ApplicationAndText {
                application: application.into(),
                text: path.into().to_string_lossy().into_owned(),
            },
        }
    }

    /// Perform a task the launcher itself owns.
    pub fn task(name: impl Into<String>, arguments: Vec<String>) -> Self {
        Self {
            kind: ActionKind::LauncherTask,
            target: OpenTarget::Task {
                name: name.into(),
                arguments,
            },
        }
    }
}
