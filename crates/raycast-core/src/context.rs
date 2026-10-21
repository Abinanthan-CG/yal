//! The runtime context available while the palette is open.
//!
//! Context is what lets an action depend on the moment it is invoked: the
//! focused window, the current directory, the contents of the clipboard. It is
//! captured once when the palette opens and passed by value, so an action can
//! never observe a half-updated world.

use std::path::PathBuf;

/// A snapshot of the environment an action executes against.
#[derive(Debug, Clone, Default)]
pub struct Context {
    /// The directory the shell was launched from.
    pub cwd: Option<PathBuf>,
    /// The text currently on the clipboard, if it could be read.
    pub clipboard: Option<String>,
    /// The title of the window that had focus when the palette opened.
    pub focused_window: Option<String>,
    /// The identifier of the previously focused window.
    pub focused_window_id: Option<u64>,
    /// The application that owned the previously focused window.
    pub focused_application: Option<String>,
    /// The user's preferred text editor, used by the "open in editor" action.
    pub editor: Option<String>,
    /// The user's preferred terminal, used by the "open in terminal" action.
    pub terminal: Option<String>,
    /// The user's preferred file manager.
    pub file_manager: Option<String>,
    /// The user's preferred web browser.
    pub browser: Option<String>,
}

impl Context {
    /// An empty context.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Set the working directory.
    pub fn with_cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    /// Set the clipboard contents.
    pub fn with_clipboard(mut self, text: impl Into<String>) -> Self {
        self.clipboard = Some(text.into());
        self
    }

    /// Set the focused window description.
    pub fn with_focus(
        mut self,
        application: Option<String>,
        window_title: Option<String>,
        window_id: Option<u64>,
    ) -> Self {
        self.focused_application = application;
        self.focused_window = window_title;
        self.focused_window_id = window_id;
        self
    }
}
