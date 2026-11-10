//! In-memory implementations of the platform traits.
//!
//! These exist so the rules that depend on the operating system can be tested
//! without a desktop: the clipboard round trip, the hook's availability, the
//! single-instance lock and the settings store.

use super::{
    Clipboard, Clock, FileSystem, Hotkey, IntegrityLevel, Paths, Settings, SingleInstance, WindowInfo,
    WindowSystem,
};
use std::collections::BTreeMap;
use std::sync::Mutex;

/// A clock the test moves by hand.
#[derive(Debug, Default)]
pub struct FakeClock {
    millis: Mutex<u64>,
}

impl FakeClock {
    pub fn new() -> FakeClock {
        FakeClock::default()
    }

    /// Move the clock forward.
    pub fn advance(&self, millis: u64) {
        let mut guard = self.millis.lock().expect("clock lock");
        *guard += millis;
    }
}

impl Clock for FakeClock {
    fn now_millis(&self) -> u64 {
        *self.millis.lock().expect("clock lock")
    }
}

/// A clipboard held in memory.
#[derive(Debug, Default)]
pub struct FakeClipboard {
    text: Mutex<Option<String>>,
    formats: Mutex<Vec<String>>,
}

impl FakeClipboard {
    pub fn new() -> FakeClipboard {
        FakeClipboard::default()
    }
}

impl Clipboard for FakeClipboard {
    fn read_text(&self) -> Option<String> {
        self.text.lock().expect("clipboard lock").clone()
    }

    fn write_text(&self, text: &str) -> Result<(), String> {
        *self.text.lock().expect("clipboard lock") = Some(text.to_string());
        Ok(())
    }

    fn available_formats(&self) -> Vec<String> {
        self.formats.lock().expect("clipboard lock").clone()
    }
}

/// A window list held in memory.
#[derive(Debug, Default)]
pub struct FakeWindows {
    foreground: Mutex<Option<WindowInfo>>,
    windows: Mutex<Vec<WindowInfo>>,
    integrity: Mutex<BTreeMap<u32, IntegrityLevel>>,
}

impl FakeWindows {
    pub fn new() -> FakeWindows {
        FakeWindows::default()
    }

    /// Put a window in front.
    pub fn set_foreground(&self, window: WindowInfo) {
        *self.foreground.lock().expect("windows lock") = Some(window);
    }

    /// Add a switchable window.
    pub fn add_window(&self, window: WindowInfo) {
        self.windows.lock().expect("windows lock").push(window);
    }

    /// Set a process's integrity level.
    pub fn set_integrity(&self, process_id: u32, level: IntegrityLevel) {
        self.integrity
            .lock()
            .expect("windows lock")
            .insert(process_id, level);
    }
}

impl WindowSystem for FakeWindows {
    fn foreground_window(&self) -> Option<WindowInfo> {
        self.foreground.lock().expect("windows lock").clone()
    }

    fn switchable_windows(&self) -> Vec<WindowInfo> {
        self.windows.lock().expect("windows lock").clone()
    }

    fn integrity_level(&self, process_id: u32) -> IntegrityLevel {
        self.integrity
            .lock()
            .expect("windows lock")
            .get(&process_id)
            .copied()
            .unwrap_or(IntegrityLevel::Medium)
    }
}

/// A hook that is either available or not.
#[derive(Debug, Default)]
pub struct FakeHotkey {
    available: Mutex<bool>,
    reason: Mutex<Option<String>>,
}

impl FakeHotkey {
    pub fn new(available: bool) -> FakeHotkey {
        FakeHotkey {
            available: Mutex::new(available),
            reason: Mutex::new(None),
        }
    }

    /// Make the hook unavailable with a reason.
    pub fn set_unavailable(&self, reason: &str) {
        *self.available.lock().expect("hook lock") = false;
        *self.reason.lock().expect("hook lock") = Some(reason.to_string());
    }
}

impl Hotkey for FakeHotkey {
    fn is_available(&self) -> bool {
        *self.available.lock().expect("hook lock")
    }

    fn unavailable_reason(&self) -> Option<String> {
        self.reason.lock().expect("hook lock").clone()
    }
}

/// A lock that only one holder can take.
#[derive(Debug, Default)]
pub struct FakeSingleInstance {
    held: Mutex<bool>,
}

impl FakeSingleInstance {
    pub fn new() -> FakeSingleInstance {
        FakeSingleInstance::default()
    }
}

impl SingleInstance for FakeSingleInstance {
    fn acquire(&self) -> bool {
        let mut guard = self.held.lock().expect("instance lock");
        if *guard {
            false
        } else {
            *guard = true;
            true
        }
    }

    fn release(&self) {
        *self.held.lock().expect("instance lock") = false;
    }
}

/// A file system held in memory.
#[derive(Debug, Default)]
pub struct FakeFileSystem {
    paths: Mutex<Paths>,
    files: Mutex<BTreeMap<String, String>>,
}

impl FakeFileSystem {
    pub fn new(paths: Paths) -> FakeFileSystem {
        FakeFileSystem {
            paths: Mutex::new(paths),
            files: Mutex::new(BTreeMap::new()),
        }
    }
}

impl FileSystem for FakeFileSystem {
    fn paths(&self) -> Paths {
        self.paths.lock().expect("files lock").clone()
    }

    fn ensure_dir(&self, _path: &std::path::Path) -> Result<(), String> {
        Ok(())
    }

    fn read_to_string(&self, path: &std::path::Path) -> Result<String, String> {
        self.files
            .lock()
            .expect("files lock")
            .get(&path.to_string_lossy().to_string())
            .cloned()
            .ok_or_else(|| format!("no such file: {}", path.display()))
    }

    fn write_string(&self, path: &std::path::Path, contents: &str) -> Result<(), String> {
        self.files
            .lock()
            .expect("files lock")
            .insert(path.to_string_lossy().to_string(), contents.to_string());
        Ok(())
    }

    fn exists(&self, path: &std::path::Path) -> bool {
        self.files
            .lock()
            .expect("files lock")
            .contains_key(&path.to_string_lossy().to_string())
    }
}

/// A settings store held in memory.
#[derive(Debug, Default)]
pub struct FakeSettings {
    values: Mutex<BTreeMap<String, String>>,
}

impl FakeSettings {
    pub fn new() -> FakeSettings {
        FakeSettings::default()
    }
}

impl Settings for FakeSettings {
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
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_clock_only_moves_when_it_is_told() {
        let clock = FakeClock::new();
        assert_eq!(clock.now_millis(), 0);
        clock.advance(250);
        assert_eq!(clock.now_millis(), 250);
    }

    #[test]
    fn the_clipboard_round_trips_text() {
        let clipboard = FakeClipboard::new();
        assert_eq!(clipboard.read_text(), None);
        clipboard.write_text("hello").unwrap();
        assert_eq!(clipboard.read_text().as_deref(), Some("hello"));
    }

    #[test]
    fn only_one_instance_holds_the_lock() {
        let instance = FakeSingleInstance::new();
        assert!(instance.acquire());
        assert!(!instance.acquire());
        instance.release();
        assert!(instance.acquire());
    }

    #[test]
    fn the_settings_store_round_trips_values() {
        let settings = FakeSettings::new();
        assert_eq!(settings.get("key"), None);
        settings.set("key", "value").unwrap();
        assert_eq!(settings.get("key").as_deref(), Some("value"));
        assert_eq!(settings.all().len(), 1);
    }

    #[test]
    fn an_elevated_process_is_reported_as_elevated() {
        assert!(IntegrityLevel::High.is_elevated());
        assert!(IntegrityLevel::System.is_elevated());
        assert!(!IntegrityLevel::Medium.is_elevated());
    }
}
