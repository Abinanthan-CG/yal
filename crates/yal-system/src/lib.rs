//! Host integration.
//!
//! Everything in this crate talks to the operating system. Each module is
//! read-mostly and side-effecting only where the caller asked for it.

pub mod clipboard;
pub mod hotkey;
pub mod keyboard;
pub mod launch;
pub mod screenshot;
pub mod tray;
pub mod watcher;
pub mod windows;
pub mod audio;
pub mod display;
pub mod identity;
pub mod notify;
pub mod processes;
pub mod fonts;
pub mod scripts;
pub mod exif;
pub mod system;
pub mod tabs;
pub mod window_controls;

pub use clipboard::{Clipboard, ClipboardError};
pub use hotkey::{Hotkey, HotkeyError, HotkeyModifiers};
pub use launch::{LaunchError, launch};
pub use screenshot::{Capture, ScreenshotError, capture_screen};
pub use tray::{Tray, TrayCommand};
pub use watcher::ClipboardWatcher;
pub use windows::{Window, WindowError, list_windows};
pub use audio::{AudioError, VolumeState};
pub use display::DisplayError;
pub use notify::{NotifyError, toast};
pub use processes::{ProcessError, Running};
pub use system::SystemCommand;
pub use tabs::BrowserTab;
pub use window_controls::{ControlError, WindowAction};
pub use fonts::FontFamily;
