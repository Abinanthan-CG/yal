//! Host integration.
//!
//! Everything in this crate talks to the operating system. Each module is
//! read-mostly and side-effecting only where the caller asked for it.

pub mod clipboard;
pub mod hotkey;
pub mod launch;
pub mod screenshot;
pub mod tray;
pub mod watcher;
pub mod windows;

pub use clipboard::{Clipboard, ClipboardError};
pub use hotkey::{Hotkey, HotkeyError, HotkeyModifiers};
pub use launch::{LaunchError, launch};
pub use screenshot::{ScreenshotError, capture_screen};
pub use tray::{Tray, TrayCommand};
pub use watcher::ClipboardWatcher;
pub use windows::{Window, WindowError, list_windows};
