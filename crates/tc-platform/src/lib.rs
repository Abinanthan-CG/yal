//! The traits every OS capability is reached through.
//!
//! Nothing in this crate touches Windows. The rules that must be testable
//! without a desktop live behind these traits, and `fakes` provides in-memory
//! implementations so a test can drive them.

pub mod fakes;

use std::collections::BTreeMap;

/// Where the app keeps its data on disk.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Paths {
    /// The app's own data folder, `%APPDATA%\yal` on Windows.
    pub data_dir: std::path::PathBuf,
    /// The cache folder, `%LOCALAPPDATA%\yal` on Windows.
    pub cache_dir: std::path::PathBuf,
    /// The user's home directory.
    pub home_dir: std::path::PathBuf,
    /// The settings mirror, `%USERPROFILE%\.config\tinycast\settings.json`.
    pub settings_file: std::path::PathBuf,
}

/// A window the app can see.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowInfo {
    /// The window handle, as an opaque number.
    pub handle: u64,
    pub title: String,
    /// The process that owns the window.
    pub process_id: u32,
    pub rect: WindowRect,
}

/// A window's position and size, in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl WindowRect {
    pub fn width(&self) -> i32 {
        self.right - self.left
    }

    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }
}

/// The integrity level of a process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum IntegrityLevel {
    Untrusted,
    Low,
    Medium,
    High,
    System,
}

impl IntegrityLevel {
    /// Whether a normal process may read this process's windows.
    pub fn is_elevated(self) -> bool {
        matches!(self, IntegrityLevel::High | IntegrityLevel::System)
    }
}

/// The clipboard, as the app sees it.
pub trait Clipboard: Send + Sync {
    /// Read the clipboard's text.
    fn read_text(&self) -> Option<String>;
    /// Put text on the clipboard.
    fn write_text(&self, text: &str) -> Result<(), String>;
    /// The formats the clipboard currently holds, by their Windows format name.
    fn available_formats(&self) -> Vec<String>;
}

/// The windows the app can see and act on.
pub trait WindowSystem: Send + Sync {
    /// The window in front, if there is one.
    fn foreground_window(&self) -> Option<WindowInfo>;
    /// Every top-level window that could be switched to.
    fn switchable_windows(&self) -> Vec<WindowInfo>;
    /// The integrity level of the process that owns a window.
    fn integrity_level(&self, process_id: u32) -> IntegrityLevel;
}

/// The keyboard hook, used for global shortcuts, modifier taps and keyword
/// expansion.
pub trait Hotkey: Send + Sync {
    /// Whether a low-level hook can be installed right now.
    fn is_available(&self) -> bool;
    /// Why the hook is unavailable, in words the user can read.
    fn unavailable_reason(&self) -> Option<String>;
}

/// The lock that keeps one instance running.
pub trait SingleInstance: Send + Sync {
    /// Try to become the one instance. `false` means another one holds it.
    fn acquire(&self) -> bool;
    /// Release the lock, if this process holds it.
    fn release(&self);
}

/// The clock, so timings can be tested without sleeping.
pub trait Clock: Send + Sync {
    /// Milliseconds since an arbitrary but fixed point.
    fn now_millis(&self) -> u64;
}

/// The folders the app writes to.
pub trait FileSystem: Send + Sync {
    /// The app's paths.
    fn paths(&self) -> Paths;
    /// Create a folder and every missing parent.
    fn ensure_dir(&self, path: &std::path::Path) -> Result<(), String>;
    /// Read a file as text.
    fn read_to_string(&self, path: &std::path::Path) -> Result<String, String>;
    /// Write a file as text, creating the parent folders.
    fn write_string(&self, path: &std::path::Path, contents: &str) -> Result<(), String>;
    /// Whether a path exists.
    fn exists(&self, path: &std::path::Path) -> bool;
}

/// The settings, as a map of keys to values.
pub trait Settings: Send + Sync {
    /// Every stored value.
    fn all(&self) -> BTreeMap<String, String>;
    /// One stored value.
    fn get(&self, key: &str) -> Option<String>;
    /// Store one value.
    fn set(&self, key: &str, value: &str) -> Result<(), String>;
}

/// Everything the app needs from the operating system.
pub trait Platform: Send + Sync {
    fn files(&self) -> &dyn FileSystem;
    fn clock(&self) -> &dyn Clock;
    fn clipboard(&self) -> &dyn Clipboard;
    fn windows(&self) -> &dyn WindowSystem;
    fn hotkey(&self) -> &dyn Hotkey;
    fn settings(&self) -> &dyn Settings;
}
