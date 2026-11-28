//! Global hotkey settings, alias persistence, conflict checks, and live
//! `GlobalHotkeyService` coordination for YAL.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use tc_platform::Settings;
use tc_platform_win::{
    ConflictKind, GlobalHotkeyService, HotkeyBinding, HotkeyTrigger, WindowsSettings,
};

use crate::features::FeatureStores;

pub const KEY_HOTKEY_LAUNCHER: &str = "hotkey.launcher";
pub const KEY_LEGACY_LAUNCHER_HOTKEY: &str = "launcher.hotkey";
pub const KEY_HOTKEY_HYPERKEY: &str = "hotkey.hyperkey";
pub const KEY_SNIPPETS_AUTO_EXPAND: &str = "snippets.autoExpand";
pub const HOTKEY_COMMAND_PREFIX: &str = "hotkey.command.";
pub const ALIAS_PREFIX: &str = "alias.";
pub const BINDING_LAUNCHER_TOGGLE: &str = "launcher.toggle";
pub const BINDING_COMMAND_PREFIX: &str = "cmd:";
/// Stored value when the user explicitly clears a command that otherwise has a built-in default.
pub const DISABLED_HOTKEY_VALUE: &str = "none";

/// Built-in global shortcut defaults for core YAL actions: `(entry_id, label, default_chord)`.
///
/// Users can override any of these in Settings -> Action Hotkeys (or via `Ctrl+K -> Record Hotkey`)
/// or clear them (stored as `"none"` so the default stays disabled).
pub const DEFAULT_COMMAND_HOTKEYS: &[(&str, &str, &str)] = &[
    (
        "command:clipboard-history",
        "Clipboard History",
        "Ctrl+Alt+V",
    ),
    ("command:search-emoji", "Emoji & Symbols", "Ctrl+Alt+E"),
    ("command:search-snippets", "Snippets", "Ctrl+Alt+S"),
    ("command:search-files", "Search Files", "Ctrl+Alt+F"),
    ("command:switch-windows", "Switch Windows", "Ctrl+Alt+W"),
    ("command:show-notes", "Quick Notes", "Ctrl+Alt+N"),
    ("command:search-quicklinks", "Quicklinks", "Ctrl+Alt+Q"),
    (
        "command:calculator-history",
        "Calculator History",
        "Ctrl+Alt+C",
    ),
    (
        "command:paste-sequentially",
        "Paste Next (Queue)",
        "Ctrl+Alt+P",
    ),
    ("command:screen-ocr", "Extract Text (OCR)", "Ctrl+Alt+T"),
    ("command:color-picker", "Pick Color", "Ctrl+Alt+X"),
];

/// Return the built-in default global shortcut for a command entry, if any.
pub fn default_command_hotkey(entry_id: &str) -> Option<HotkeyTrigger> {
    DEFAULT_COMMAND_HOTKEYS
        .iter()
        .find(|(id, _, _)| *id == entry_id)
        .and_then(|(_, _, chord)| HotkeyTrigger::parse(chord))
}

fn lock_or_recover<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

/// Read the configured launcher hotkey, preferring `hotkey.launcher` and
/// falling back to the legacy `launcher.hotkey` key and then `Alt+Space`.
pub fn configured_launcher_hotkey(settings: &impl Settings) -> HotkeyTrigger {
    for key in [KEY_HOTKEY_LAUNCHER, KEY_LEGACY_LAUNCHER_HOTKEY] {
        if let Some(raw) = settings.get(key) {
            let trimmed = raw.trim();
            if !trimmed.is_empty() {
                if let Some(trigger) = HotkeyTrigger::parse(trimmed) {
                    return trigger;
                }
            }
        }
    }
    HotkeyTrigger::default_launcher()
}

/// Persist the launcher hotkey to both the current and legacy settings keys.
pub fn save_launcher_hotkey(
    settings: &impl Settings,
    trigger: HotkeyTrigger,
) -> Result<(), String> {
    let formatted = trigger.format();
    settings.set(KEY_HOTKEY_LAUNCHER, &formatted)?;
    settings.set(KEY_LEGACY_LAUNCHER_HOTKEY, &formatted)
}

/// Whether CapsLock Hyperkey mode is enabled in settings.
pub fn hyperkey_enabled(settings: &impl Settings) -> bool {
    settings.get(KEY_HOTKEY_HYPERKEY).as_deref() == Some("true")
}

/// Persist the CapsLock Hyperkey setting.
pub fn set_hyperkey_enabled_setting(settings: &impl Settings, enabled: bool) -> Result<(), String> {
    settings.set(KEY_HOTKEY_HYPERKEY, if enabled { "true" } else { "false" })
}

/// Whether global snippet keyword auto-expansion is enabled (defaults to `true`).
pub fn snippets_auto_expand_enabled(settings: &impl Settings) -> bool {
    settings.get(KEY_SNIPPETS_AUTO_EXPAND).as_deref() != Some("false")
}

/// Persist the global snippet auto-expansion setting.
pub fn set_snippets_auto_expand_setting(
    settings: &impl Settings,
    enabled: bool,
) -> Result<(), String> {
    settings.set(
        KEY_SNIPPETS_AUTO_EXPAND,
        if enabled { "true" } else { "false" },
    )
}

/// Settings key used to store a per-command global hotkey.
pub fn command_hotkey_key(entry_id: &str) -> String {
    format!("{HOTKEY_COMMAND_PREFIX}{entry_id}")
}

/// Binding identifier registered with `GlobalHotkeyService` for a command entry.
pub fn command_binding_id(entry_id: &str) -> String {
    format!("{BINDING_COMMAND_PREFIX}{entry_id}")
}

/// Extract the launcher entry ID from a command hotkey binding identifier.
pub fn command_entry_from_binding(binding_id: &str) -> Option<&str> {
    binding_id.strip_prefix(BINDING_COMMAND_PREFIX)
}

/// Read a per-command global hotkey from settings, falling back to built-in
/// defaults unless the user explicitly cleared it (`"none"`).
pub fn get_command_hotkey(settings: &impl Settings, entry_id: &str) -> Option<HotkeyTrigger> {
    if let Some(raw) = settings.get(&command_hotkey_key(entry_id)) {
        let trimmed = raw.trim();
        if trimmed.eq_ignore_ascii_case(DISABLED_HOTKEY_VALUE) {
            return None;
        }
        if !trimmed.is_empty() {
            return HotkeyTrigger::parse(trimmed);
        }
    }
    default_command_hotkey(entry_id)
}

/// Save or clear a per-command global hotkey in settings.
///
/// Clearing a command with a built-in default persists `"none"` so it stays
/// disabled across restarts instead of silently reverting to the default.
pub fn set_command_hotkey(
    settings: &impl Settings,
    entry_id: &str,
    trigger: Option<HotkeyTrigger>,
) -> Result<(), String> {
    let value = match trigger {
        Some(t) => t.format(),
        None if default_command_hotkey(entry_id).is_some() => DISABLED_HOTKEY_VALUE.to_string(),
        None => String::new(),
    };
    settings.set(&command_hotkey_key(entry_id), &value)
}

/// Collect all effective per-command hotkeys (explicit user settings merged with
/// built-in defaults, skipping any default disabled via `"none"` or colliding
/// with the launcher toggle or a user-assigned shortcut).
pub fn all_command_hotkeys(settings: &impl Settings) -> Vec<(String, HotkeyTrigger)> {
    let launcher_trigger = configured_launcher_hotkey(settings);
    let mut map: BTreeMap<String, Option<HotkeyTrigger>> = BTreeMap::new();

    for (key, value) in settings.all() {
        let Some(entry_id) = key.strip_prefix(HOTKEY_COMMAND_PREFIX) else {
            continue;
        };
        if entry_id.is_empty() {
            continue;
        }
        let trimmed = value.trim();
        if trimmed.eq_ignore_ascii_case(DISABLED_HOTKEY_VALUE) {
            map.insert(entry_id.to_string(), None);
        } else if !trimmed.is_empty() {
            if let Some(trigger) = HotkeyTrigger::parse(trimmed) {
                map.insert(entry_id.to_string(), Some(trigger));
            }
        }
    }

    let mut used: Vec<HotkeyTrigger> = vec![launcher_trigger];
    for trigger in map.values().flatten() {
        used.push(*trigger);
    }

    for &(entry_id, _, _) in DEFAULT_COMMAND_HOTKEYS {
        if map.contains_key(entry_id) {
            continue;
        }
        if let Some(default_trigger) = default_command_hotkey(entry_id) {
            if !used.contains(&default_trigger) {
                used.push(default_trigger);
                map.insert(entry_id.to_string(), Some(default_trigger));
            }
        }
    }

    map.into_iter()
        .filter_map(|(id, trig)| trig.map(|t| (id, t)))
        .collect()
}

/// Build the full list of global hotkey bindings (launcher toggle + commands).
pub fn build_all_bindings(settings: &impl Settings) -> Vec<HotkeyBinding> {
    let mut bindings = vec![HotkeyBinding::new(
        BINDING_LAUNCHER_TOGGLE,
        configured_launcher_hotkey(settings),
    )];
    for (entry_id, trigger) in all_command_hotkeys(settings) {
        bindings.push(HotkeyBinding::new(command_binding_id(&entry_id), trigger));
    }
    bindings
}

/// Check if `trigger` conflicts with reserved shortcuts or existing bindings.
pub fn check_hotkey_conflict(
    settings: &impl Settings,
    trigger: HotkeyTrigger,
    ignore_binding_id: Option<&str>,
) -> Option<ConflictKind> {
    tc_platform_win::check_conflict(trigger, &build_all_bindings(settings), ignore_binding_id)
}

/// Format a human-readable explanation for a hotkey conflict.
pub fn conflict_message(kind: ConflictKind) -> String {
    match kind {
        ConflictKind::ReservedTextEditing => {
            "Reserved for standard text editing (Ctrl+C/V/X/Z)".to_string()
        }
        ConflictKind::ReservedWindowsKey => "Bare Windows key is reserved by Windows".to_string(),
        ConflictKind::Duplicate(id) => {
            if id == BINDING_LAUNCHER_TOGGLE {
                "Already assigned to Launcher Toggle".to_string()
            } else if let Some(entry_id) = command_entry_from_binding(&id) {
                format!("Already assigned to {entry_id}")
            } else {
                format!("Already assigned to {id}")
            }
        }
    }
}

/// Settings key used to store a custom alias for an entry.
pub fn alias_key(entry_id: &str) -> String {
    format!("{ALIAS_PREFIX}{entry_id}")
}

/// Read a custom alias for an entry, returning `None` if unset or blank.
pub fn get_alias(settings: &impl Settings, entry_id: &str) -> Option<String> {
    let raw = settings.get(&alias_key(entry_id))?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Persist or clear a custom alias for an entry.
pub fn set_alias(
    settings: &impl Settings,
    entry_id: &str,
    alias: Option<&str>,
) -> Result<(), String> {
    let trimmed = alias.map(str::trim).unwrap_or("");
    settings.set(&alias_key(entry_id), trimmed)
}

/// Collect all non-empty aliases (`entry_id -> alias`) stored in settings.
pub fn all_aliases(settings: &impl Settings) -> BTreeMap<String, String> {
    settings
        .all()
        .into_iter()
        .filter_map(|(key, value)| {
            let entry_id = key.strip_prefix(ALIAS_PREFIX)?;
            if entry_id.is_empty() {
                return None;
            }
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return None;
            }
            Some((entry_id.to_string(), trimmed.to_string()))
        })
        .collect()
}

/// Find entries whose alias matches `query`, returning `(exact_ids, prefix_ids)`.
pub fn find_alias_matches(settings: &impl Settings, query: &str) -> (Vec<String>, Vec<String>) {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let mut exact = Vec::new();
    let mut prefix = Vec::new();
    for (entry_id, alias) in all_aliases(settings) {
        let lower = alias.to_ascii_lowercase();
        if lower == q {
            exact.push(entry_id);
        } else if lower.starts_with(&q) {
            prefix.push(entry_id);
        }
    }
    (exact, prefix)
}

/// Coordinates the live `GlobalHotkeyService` and clipboard-watcher suppression
/// during snippet auto-expansion without holding locks across Win32 calls.
#[derive(Default)]
pub struct HotkeyController {
    service: Mutex<Option<Arc<GlobalHotkeyService>>>,
    suppress_clipboard_until_ms: Arc<AtomicU64>,
}

impl HotkeyController {
    pub fn new() -> Self {
        Self::default()
    }

    /// Attach a running `GlobalHotkeyService`.
    pub fn attach(&self, service: Arc<GlobalHotkeyService>) {
        *lock_or_recover(&self.service) = Some(service);
    }

    /// Detach and return the running `GlobalHotkeyService`, if any.
    pub fn detach(&self) -> Option<Arc<GlobalHotkeyService>> {
        lock_or_recover(&self.service).take()
    }

    fn service(&self) -> Option<Arc<GlobalHotkeyService>> {
        lock_or_recover(&self.service).as_ref().map(Arc::clone)
    }

    /// Rebuild all configured bindings from `settings` and push them live.
    pub fn reload_bindings(&self, settings: &WindowsSettings) {
        let bindings = build_all_bindings(settings);
        if let Some(service) = self.service() {
            service.set_bindings(bindings);
        }
    }

    /// Toggle CapsLock Hyperkey mode live on the attached service.
    pub fn set_hyperkey_enabled(&self, enabled: bool) {
        if let Some(service) = self.service() {
            service.set_hyperkey_enabled(enabled);
        }
    }

    /// Enter live hotkey capture mode on the attached service.
    /// Returns `true` when a live service accepted the capture request.
    ///
    /// `on_captured` receives `Some(trigger)` on completion or `None` when
    /// the user cancels capture with `Esc`.
    pub fn begin_capture(
        &self,
        on_captured: impl FnOnce(Option<HotkeyTrigger>) + Send + 'static,
    ) -> bool {
        let Some(service) = self.service() else {
            return false;
        };
        service.begin_capture(on_captured);
        true
    }

    /// Cancel any in-progress hotkey capture.
    pub fn cancel_capture(&self) {
        if let Some(service) = self.service() {
            service.cancel_capture();
        }
    }

    /// Suppress clipboard-history recording for `duration_ms` from now.
    pub fn suppress_clipboard_for_ms(&self, duration_ms: u64) {
        let until = now_epoch_ms().saturating_add(duration_ms);
        self.suppress_clipboard_until_ms
            .fetch_max(until, Ordering::AcqRel);
    }

    /// Whether clipboard-history recording should currently ignore updates.
    pub fn should_suppress_clipboard(&self) -> bool {
        now_epoch_ms() < self.suppress_clipboard_until_ms.load(Ordering::Acquire)
    }

    /// Synchronize enabled snippet auto-expansion keywords with the live hook.
    pub fn sync_snippet_keywords(&self, settings: &WindowsSettings, features: Arc<FeatureStores>) {
        let Some(service) = self.service() else {
            return;
        };
        if !snippets_auto_expand_enabled(settings) {
            service.set_snippet_keywords(Vec::new(), |_, _| {});
            return;
        }
        let keywords = features.auto_expand_keywords();
        let suppress = Arc::clone(&self.suppress_clipboard_until_ms);
        service.set_snippet_keywords(keywords, move |keyword, chars_to_erase| {
            // Never auto-expand snippets while typing inside YAL's own palette or dialogs.
            if tc_platform_win::is_foreground_current_process() {
                return;
            }
            if let Some(expanded) = features.expand_snippet_keyword(&keyword) {
                let until = now_epoch_ms().saturating_add(700);
                suppress.fetch_max(until, Ordering::AcqRel);
                let _ = tc_platform_win::erase_and_paste_async(chars_to_erase, expanded);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tc_platform::fakes::FakeSettings;

    #[test]
    fn launcher_hotkey_reads_current_legacy_and_default() {
        let settings = FakeSettings::new();
        assert_eq!(
            configured_launcher_hotkey(&settings),
            HotkeyTrigger::default_launcher()
        );

        settings
            .set(KEY_LEGACY_LAUNCHER_HOTKEY, "Ctrl+Space")
            .expect("set legacy");
        assert_eq!(
            configured_launcher_hotkey(&settings),
            HotkeyTrigger::parse("Ctrl+Space").expect("parse")
        );

        // Blank current key falls back to legacy.
        settings.set(KEY_HOTKEY_LAUNCHER, "   ").expect("set blank");
        assert_eq!(
            configured_launcher_hotkey(&settings),
            HotkeyTrigger::parse("Ctrl+Space").expect("parse")
        );

        // Saving writes both keys.
        let trigger = HotkeyTrigger::parse("Double Alt").expect("parse");
        save_launcher_hotkey(&settings, trigger).expect("save");
        assert_eq!(
            settings.get(KEY_HOTKEY_LAUNCHER).as_deref(),
            Some("Double Alt")
        );
        assert_eq!(
            settings.get(KEY_LEGACY_LAUNCHER_HOTKEY).as_deref(),
            Some("Double Alt")
        );
        assert_eq!(configured_launcher_hotkey(&settings), trigger);
    }

    #[test]
    fn command_hotkeys_persist_clear_build_bindings_and_detect_conflicts() {
        let settings = FakeSettings::new();
        // Built-in defaults are active out of the box.
        assert_eq!(
            get_command_hotkey(&settings, "command:clipboard-history"),
            HotkeyTrigger::parse("Ctrl+Alt+V")
        );
        assert_eq!(
            get_command_hotkey(&settings, "command:screen-ocr"),
            HotkeyTrigger::parse("Ctrl+Alt+T")
        );
        assert_eq!(
            get_command_hotkey(&settings, "command:color-picker"),
            HotkeyTrigger::parse("Ctrl+Alt+X")
        );
        assert_eq!(
            all_command_hotkeys(&settings).len(),
            DEFAULT_COMMAND_HOTKEYS.len()
        );

        let clip_trigger = HotkeyTrigger::parse("Ctrl+Shift+V").expect("parse");
        set_command_hotkey(&settings, "command:clipboard-history", Some(clip_trigger))
            .expect("save command hotkey");

        assert_eq!(
            get_command_hotkey(&settings, "command:clipboard-history"),
            Some(clip_trigger)
        );
        assert_eq!(
            all_command_hotkeys(&settings).len(),
            DEFAULT_COMMAND_HOTKEYS.len()
        );

        let bindings = build_all_bindings(&settings);
        assert_eq!(bindings.len(), 1 + DEFAULT_COMMAND_HOTKEYS.len());
        assert_eq!(bindings[0].id, BINDING_LAUNCHER_TOGGLE);
        let clip_binding = bindings
            .iter()
            .find(|b| b.id == command_binding_id("command:clipboard-history"))
            .expect("clipboard binding");
        assert_eq!(clip_binding.trigger, clip_trigger);
        assert_eq!(
            command_entry_from_binding(&clip_binding.id),
            Some("command:clipboard-history")
        );

        // Duplicate conflict against another command.
        let dup = check_hotkey_conflict(&settings, clip_trigger, None);
        assert_eq!(
            dup,
            Some(ConflictKind::Duplicate(command_binding_id(
                "command:clipboard-history"
            )))
        );
        assert!(conflict_message(dup.expect("conflict")).contains("command:clipboard-history"));

        // Ignoring self avoids self-conflict.
        let self_id = command_binding_id("command:clipboard-history");
        assert_eq!(
            check_hotkey_conflict(&settings, clip_trigger, Some(&self_id)),
            None
        );

        // Reserved text editing conflict.
        let copy_chord = HotkeyTrigger::parse("Ctrl+C").expect("parse");
        assert_eq!(
            check_hotkey_conflict(&settings, copy_chord, None),
            Some(ConflictKind::ReservedTextEditing)
        );

        // Clearing disables even a command with a built-in default.
        set_command_hotkey(&settings, "command:clipboard-history", None).expect("clear");
        assert_eq!(
            get_command_hotkey(&settings, "command:clipboard-history"),
            None
        );
        assert_eq!(
            all_command_hotkeys(&settings).len(),
            DEFAULT_COMMAND_HOTKEYS.len() - 1
        );
    }

    #[test]
    fn aliases_persist_clear_and_match_exact_and_prefix() {
        let settings = FakeSettings::new();
        set_alias(&settings, "app:vscode", Some(" code ")).expect("set alias");
        set_alias(&settings, "app:codex", Some("codepad")).expect("set alias");
        set_alias(&settings, "command:settings", Some("prefs")).expect("set alias");

        assert_eq!(get_alias(&settings, "app:vscode").as_deref(), Some("code"));
        assert_eq!(all_aliases(&settings).len(), 3);

        let (exact, prefix) = find_alias_matches(&settings, "CODE");
        assert_eq!(exact, vec!["app:vscode".to_string()]);
        assert_eq!(prefix, vec!["app:codex".to_string()]);

        let (empty_e, empty_p) = find_alias_matches(&settings, "   ");
        assert!(empty_e.is_empty() && empty_p.is_empty());

        set_alias(&settings, "app:vscode", Some("  ")).expect("clear alias");
        assert_eq!(get_alias(&settings, "app:vscode"), None);
        assert_eq!(all_aliases(&settings).len(), 2);
    }

    #[test]
    fn clipboard_suppression_window_tracks_active_period() {
        let controller = HotkeyController::new();
        assert!(!controller.should_suppress_clipboard());
        controller.suppress_clipboard_for_ms(2_000);
        assert!(controller.should_suppress_clipboard());
        assert!(!controller.begin_capture(|_| {}));
    }
}
