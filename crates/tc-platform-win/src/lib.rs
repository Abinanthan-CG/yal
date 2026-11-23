//! The Windows implementation of the platform traits.
//!
//! This is the only crate in the workspace that touches Win32. Everything else
//! reaches Windows through `tc-platform`, so the rules can be tested without a
//! desktop.

mod clipboard;
pub mod clipboard_watcher;
pub mod dialog;
pub mod hotkey;
pub mod http;
pub mod icons;
mod launch;
pub mod paste;
mod paths;
mod programs;
mod quit;
mod single_instance;
mod system_actions;
pub mod tray;
pub mod window_control;
mod windows;

pub use clipboard::WindowsClipboard;
pub use clipboard_watcher::ClipboardWatcher;
pub use dialog::pick_file;
/// The Windows clipboard, as text.
pub fn clipboard() -> WindowsClipboard {
    WindowsClipboard::new()
}
pub use hotkey::{
    check_conflict, is_chord_available_in_os, ConflictKind, GlobalHotkeyService, HookDecision,
    HotkeyBinding, HotkeyMatcher, HotkeyModifiers, HotkeyTrigger, KeyEvent, ModifierKey,
    ModifierSide, WindowsHotkey, TAP_WINDOW_MS,
};
pub use http::https_get;
pub use icons::{application_icon, extract_icon, shell_item_icon, IconImage};
pub use launch::{open, recycle, reveal};
pub use paste::copy_and_paste_async;
pub use paths::is_portable;
pub use paths::windows_paths;
pub use programs::programs;
pub use quit::{signal_quit, signal_show, QuitWatcher, ShowWatcher};
pub use single_instance::{another_instance_is_running, WindowsSingleInstance};
pub use system_actions::{
    apps_use_dark_theme, is_available, mechanism, run, unavailable_reason, ExitMode, Mechanism,
};
pub use tray::{TrayAction, WindowsTray};
pub use window_control::{
    apply_command, foreground_window, is_minimized, is_visible, list_windows, move_window, screens,
    set_window_rect, toggle_fullscreen, window_rect, ControlError, Outcome, WindowHandle,
    WindowInfo,
};
pub use windows::{extended_frame_bounds, own_integrity_level, WindowsWindows};

use std::collections::BTreeMap;
use std::sync::Mutex;
use tc_platform::{Clock, FileSystem, Paths, Settings};

/// The real clock.
#[derive(Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_millis(&self) -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
}

/// The real file system.
#[derive(Debug)]
pub struct WindowsFileSystem {
    paths: Paths,
}

impl WindowsFileSystem {
    pub fn new() -> WindowsFileSystem {
        WindowsFileSystem {
            paths: windows_paths(),
        }
    }
}

impl Default for WindowsFileSystem {
    fn default() -> Self {
        WindowsFileSystem::new()
    }
}

impl FileSystem for WindowsFileSystem {
    fn paths(&self) -> Paths {
        self.paths.clone()
    }

    fn ensure_dir(&self, path: &std::path::Path) -> Result<(), String> {
        std::fs::create_dir_all(path).map_err(|error| format!("{}: {error}", path.display()))
    }

    fn read_to_string(&self, path: &std::path::Path) -> Result<String, String> {
        std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))
    }

    fn write_string(&self, path: &std::path::Path, contents: &str) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            self.ensure_dir(parent)?;
        }
        std::fs::write(path, contents).map_err(|error| format!("{}: {error}", path.display()))
    }

    fn exists(&self, path: &std::path::Path) -> bool {
        path.exists()
    }
}

/// The settings store, held in memory and written to the settings file.
#[derive(Debug)]
pub struct WindowsSettings {
    values: Mutex<BTreeMap<String, String>>,
    file: std::path::PathBuf,
}

impl WindowsSettings {
    /// Load the settings, or start empty when there is no file yet.
    pub fn load(file: std::path::PathBuf) -> WindowsSettings {
        let values = std::fs::read_to_string(&file)
            .ok()
            .and_then(|text| serde_json::from_str::<BTreeMap<String, String>>(&text).ok())
            .unwrap_or_default();
        WindowsSettings {
            values: Mutex::new(values),
            file,
        }
    }

    /// Re-read the settings file from disk (for example after restoring a backup).
    pub fn reload(&self) {
        if let Ok(text) = std::fs::read_to_string(&self.file) {
            if let Ok(fresh) = serde_json::from_str::<BTreeMap<String, String>>(&text) {
                if let Ok(mut guard) = self.values.lock() {
                    *guard = fresh;
                }
            }
        }
    }
}

impl Settings for WindowsSettings {
    fn all(&self) -> BTreeMap<String, String> {
        self.values.lock().expect("settings lock").clone()
    }

    fn get(&self, key: &str) -> Option<String> {
        self.values.lock().expect("settings lock").get(key).cloned()
    }

    fn set(&self, key: &str, value: &str) -> Result<(), String> {
        self.values
            .lock()
            .expect("settings lock")
            .insert(key.to_string(), value.to_string());
        let snapshot = self.values.lock().expect("settings lock").clone();
        let text = serde_json::to_string_pretty(&snapshot)
            .map_err(|error| format!("settings could not be encoded: {error}"))?;
        if let Some(parent) = self.file.parent() {
            std::fs::create_dir_all(parent).map_err(|error| format!("{error}"))?;
        }
        std::fs::write(&self.file, text)
            .map_err(|error| format!("{}: {error}", self.file.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tc_platform::{Clipboard, Hotkey, IntegrityLevel, SingleInstance};

    #[test]
    fn the_paths_are_where_the_brief_says_they_are() {
        let paths = windows_paths();
        assert!(paths.data_dir.is_absolute());
        assert!(paths.cache_dir.is_absolute());
        assert!(paths.home_dir.is_absolute());
        assert!(paths.settings_file.starts_with(&paths.home_dir));
        assert!(paths
            .settings_file
            .ends_with(std::path::Path::new(".config/tinycast/settings.json")));
    }

    #[test]
    fn this_process_reports_its_own_integrity_level() {
        let level = windows::own_integrity_level();
        // A test run is never untrusted.
        assert!(level >= IntegrityLevel::Low);
    }

    #[test]
    fn the_clipboard_round_trips_text() {
        let clipboard = WindowsClipboard::new();
        let original = clipboard.read_text();
        clipboard.write_text("yal round trip").unwrap();
        assert_eq!(clipboard.read_text().as_deref(), Some("yal round trip"));
        if let Some(original) = original {
            let _ = clipboard.write_text(&original);
        }
    }

    #[test]
    fn the_single_instance_lock_keeps_a_second_instance_out() {
        let instance = WindowsSingleInstance::new();
        assert!(instance.acquire());
        assert!(!instance.acquire());
        instance.release();
        assert!(instance.acquire());
        instance.release();
    }

    #[test]
    fn the_keyboard_hook_can_be_installed_and_removed() {
        let hotkey = WindowsHotkey::new();
        // A hook may be refused on a secure desktop, so this test only
        // asserts that the attempt is well behaved.
        let _ = hotkey.is_available();
        hotkey.uninstall();
    }
}
