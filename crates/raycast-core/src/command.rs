//! Commands: the top-level things a user can search within.
//!
//! A command owns a set of items and knows how to produce candidates for a
//! query. The palette shows a flat list of every command's best items, so
//! commands compete on the same terms.

use crate::item::Item;

/// Stable identity of a command.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CommandId(pub String);

impl CommandId {
    pub fn new(id: impl Into<String>) -> Self {
        CommandId(id.into())
    }
}

impl std::fmt::Display for CommandId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The category a command belongs to.
///
/// The category drives grouping and default ordering in the palette, and it
/// selects which provider handles the query.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CommandKind {
    /// Installed applications.
    Applications,
    /// Files and folders.
    Files,
    /// Clipboard history.
    Clipboard,
    /// Text snippets.
    Snippets,
    /// Saved links.
    Quicklinks,
    /// Emoji.
    Emoji,
    /// The expression calculator.
    Calculator,
    /// Colour conversion.
    Colors,
    /// Currency and unit conversion.
    Converters,
    /// Date, time and time zones.
    DateTime,
    /// Regular expressions.
    Regex,
    /// Password generation.
    Passwords,
    /// QR code generation.
    QrCode,
    /// Bookmarks.
    Bookmarks,
    /// Screenshots.
    Screenshots,
    /// Window management.
    Windows,
    /// Shell preferences.
    Settings,
    /// Web search.
    WebSearch,
    /// System commands: lock, sleep, volume, brightness, emptying the bin.
    System,
    /// Text transforms: case changes, encodings, hashes, UUIDs.
    Text,
    /// Processes: kill a running program.
    Process,
    /// Open browser tabs.
    BrowserTabs,
    /// Countdown timers.
    Timer,
    /// The user's script commands, run from their scripts folder.
    Scripts,
    /// A third-party extension.
    ///
    /// Reserved. No extension runtime ships in this milestone
    /// (`docs/observations.md` §6.2).
    Extension,
}

impl CommandKind {
    /// The section title this command's results appear under.
    pub fn section_title(self) -> &'static str {
        match self {
            CommandKind::Applications => "Applications",
            CommandKind::Files => "Files",
            CommandKind::Clipboard => "Clipboard History",
            CommandKind::Snippets => "Snippets",
            CommandKind::Quicklinks => "Quicklinks",
            CommandKind::Emoji => "Emoji",
            CommandKind::Calculator => "Calculator",
            CommandKind::Colors => "Colors",
            CommandKind::Converters => "Converters",
            CommandKind::DateTime => "Date & Time",
            CommandKind::Regex => "Regex",
            CommandKind::Passwords => "Passwords",
            CommandKind::QrCode => "QR Code",
            CommandKind::Bookmarks => "Bookmarks",
            CommandKind::Screenshots => "Screenshots",
            CommandKind::Windows => "Windows",
            CommandKind::Settings => "Preferences",
            CommandKind::WebSearch => "Search the Web",
            CommandKind::System => "System",
            CommandKind::Text => "Text",
            CommandKind::Process => "Processes",
            CommandKind::BrowserTabs => "Browser Tabs",
            CommandKind::Timer => "Timers",
            CommandKind::Scripts => "Script Commands",
            CommandKind::Extension => "Extensions",
        }
    }

    /// Whether results of this kind should be grouped under a section header.
    pub fn is_grouped(self) -> bool {
        !matches!(self, CommandKind::Calculator | CommandKind::Colors)
    }
}

/// Where a command's items came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandSource {
    /// Built into the launcher.
    Builtin,

    /// Read from the local database.
    Stored,

    /// Produced live by the operating system.
    System,

    /// Provided by an extension.
    Extension,
}

/// A searchable source of items.
#[derive(Debug, Clone)]
pub struct Command {
    pub id: CommandId,
    pub kind: CommandKind,
    pub name: String,
    /// Extra terms the command itself can be found by.
    pub keywords: Vec<String>,
    pub source: CommandSource,
}

impl Command {
    pub fn new(
        id: impl Into<String>,
        kind: CommandKind,
        name: impl Into<String>,
        source: CommandSource,
    ) -> Self {
        Self {
            id: CommandId::new(id),
            kind,
            name: name.into(),
            keywords: Vec::new(),
            source,
        }
    }

    /// A built-in command.
    pub fn builtin(id: impl Into<String>, kind: CommandKind, name: impl Into<String>) -> Self {
        Self::new(id, kind, name, CommandSource::Builtin)
    }

    /// Add a keyword.
    pub fn with_keyword(mut self, keyword: impl Into<String>) -> Self {
        self.keywords.push(keyword.into());
        self
    }
}

/// A command together with the items it currently offers.
#[derive(Debug, Clone)]
pub struct CommandResult {
    pub command: Command,
    pub items: Vec<Item>,
}
