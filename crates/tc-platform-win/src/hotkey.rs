//! Global hotkeys and modifier-tap detection on Windows.
//!
//! Two layers keep the rules testable without a live desktop:
//!
//! 1. [`HotkeyMatcher`] is a pure, deterministic state machine that tracks left
//!    and right modifier states, matches exact chords, detects single and double
//!    modifier taps within the 250 ms tap window (handling 49.7-day `u32` tick
//!    wraparound), and checks conflicts.
//! 2. [`GlobalHotkeyService`] runs a dedicated thread with a blocking
//!    `GetMessageW` loop (0% idle CPU) and a `WH_KEYBOARD_LL` hook that feeds
//!    events into [`HotkeyMatcher`] in microseconds, suppresses Alt/Win menu
//!    activation when swallowing chords, and dispatches matched binding IDs
//!    asynchronously so the hook never stalls.

use std::collections::HashSet;
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::JoinHandle;

use tc_platform::Hotkey;
use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, RegisterHotKey, SendInput, UnregisterHotKey, HOT_KEY_MODIFIERS, INPUT,
    INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT,
    MOD_SHIFT, MOD_WIN, VIRTUAL_KEY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, PeekMessageW, PostThreadMessageW,
    SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT,
    LLKHF_INJECTED, MSG, PM_NOREMOVE, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_QUIT, WM_SYSKEYDOWN,
    WM_SYSKEYUP,
};

/// Maximum duration of a modifier press (and gap between two taps in a
/// double-tap) in milliseconds, matching Tinycast's 250 ms tap window.
pub const TAP_WINDOW_MS: u32 = 250;

/// Modifier virtual-key constants.
const VK_LSHIFT: u32 = 0xA0;
const VK_RSHIFT: u32 = 0xA1;
const VK_LCONTROL: u32 = 0xA2;
const VK_RCONTROL: u32 = 0xA3;
const VK_LMENU: u32 = 0xA4;
const VK_RMENU: u32 = 0xA5;
const VK_LWIN: u32 = 0x5B;
const VK_RWIN: u32 = 0x5C;

/// Unassigned virtual key injected on Alt/Win chord swallow so Windows does not
/// interpret the solo modifier release as `SC_KEYMENU` or Start Menu.
const VK_MENU_MASK: u16 = 0xFF;

/// Which logical modifier key a tap targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModifierKey {
    Ctrl,
    Alt,
    Shift,
    Win,
}

impl ModifierKey {
    /// Human-readable name.
    pub fn label(self) -> &'static str {
        match self {
            ModifierKey::Ctrl => "Ctrl",
            ModifierKey::Alt => "Alt",
            ModifierKey::Shift => "Shift",
            ModifierKey::Win => "Win",
        }
    }
}

/// Which physical side of a modifier key a tap targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ModifierSide {
    #[default]
    Any,
    Left,
    Right,
}

impl ModifierSide {
    fn matches(self, actual: ModifierSide) -> bool {
        self == ModifierSide::Any || self == actual
    }
}

/// A set of held modifiers for a chord hotkey.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct HotkeyModifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
}

impl HotkeyModifiers {
    /// Whether no modifier is held.
    pub fn is_empty(self) -> bool {
        !self.ctrl && !self.alt && !self.shift && !self.win
    }

    /// Win32 `MOD_*` flags for `RegisterHotKey`.
    pub fn win32_flags(self) -> HOT_KEY_MODIFIERS {
        let mut flags = MOD_NOREPEAT.0;
        if self.alt {
            flags |= MOD_ALT.0;
        }
        if self.ctrl {
            flags |= MOD_CONTROL.0;
        }
        if self.shift {
            flags |= MOD_SHIFT.0;
        }
        if self.win {
            flags |= MOD_WIN.0;
        }
        HOT_KEY_MODIFIERS(flags)
    }
}

/// What activates a global hotkey binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HotkeyTrigger {
    /// One or more modifiers held while pressing a virtual key (e.g. `Alt+Space`).
    Chord { modifiers: HotkeyModifiers, vk: u32 },
    /// Pressing and releasing a modifier key alone within [`TAP_WINDOW_MS`].
    SingleTap {
        key: ModifierKey,
        side: ModifierSide,
    },
    /// Two quick taps of a modifier key within [`TAP_WINDOW_MS`].
    DoubleTap {
        key: ModifierKey,
        side: ModifierSide,
    },
}

impl HotkeyTrigger {
    /// Default launcher shortcut on Windows (`Alt+Space`, translated from
    /// Tinycast's macOS `⌥Space`).
    pub fn default_launcher() -> Self {
        HotkeyTrigger::Chord {
            modifiers: HotkeyModifiers {
                alt: true,
                ..Default::default()
            },
            vk: 0x20,
        }
    }

    /// Secondary fallback launcher shortcut (`Ctrl+Space`).
    pub fn fallback_launcher() -> Self {
        HotkeyTrigger::Chord {
            modifiers: HotkeyModifiers {
                ctrl: true,
                ..Default::default()
            },
            vk: 0x20,
        }
    }

    /// Parse a human-readable hotkey specification.
    ///
    /// Supported forms:
    /// - Chords: `Alt+Space`, `Ctrl+Space`, `Ctrl+Shift+V`, `Ctrl+Alt+K`
    /// - Double taps: `Double Ctrl`, `Double Alt`, `Double Shift`, `Double Right Alt`
    /// - Single taps: `Right Alt`, `Single Right Alt`, `Left Alt`
    pub fn parse(text: &str) -> Option<Self> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return None;
        }

        let lower = trimmed.to_ascii_lowercase();
        if let Some(rest) = lower
            .strip_prefix("double ")
            .or_else(|| lower.strip_prefix("double-"))
            .or_else(|| lower.strip_prefix("2x "))
        {
            let (key, side) = parse_modifier_with_side(rest.trim())?;
            return Some(HotkeyTrigger::DoubleTap { key, side });
        }

        if let Some(rest) = lower
            .strip_prefix("single ")
            .or_else(|| lower.strip_prefix("tap "))
        {
            let (key, side) = parse_modifier_with_side(rest.trim())?;
            return Some(HotkeyTrigger::SingleTap { key, side });
        }

        if !trimmed.contains('+') {
            if let Some((key, side)) = parse_modifier_with_side(&lower) {
                if side != ModifierSide::Any {
                    return Some(HotkeyTrigger::SingleTap { key, side });
                }
            }
            return None;
        }

        let parts: Vec<&str> = trimmed
            .split('+')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .collect();
        if parts.len() < 2 {
            return None;
        }

        let mut modifiers = HotkeyModifiers::default();
        for part in &parts[..parts.len() - 1] {
            match part.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => modifiers.ctrl = true,
                "alt" | "option" | "opt" => modifiers.alt = true,
                "shift" => modifiers.shift = true,
                "win" | "super" | "meta" | "cmd" | "command" => modifiers.win = true,
                _ => return None,
            }
        }
        if modifiers.is_empty() {
            return None;
        }

        let vk = named_key_to_vk(&parts[parts.len() - 1].to_ascii_lowercase())?;
        Some(HotkeyTrigger::Chord { modifiers, vk })
    }

    /// Format this trigger cleanly for display and persistence in `settings.json`.
    pub fn format(self) -> String {
        match self {
            HotkeyTrigger::Chord { modifiers, vk } => {
                let mut parts: Vec<String> = Vec::new();
                if modifiers.ctrl {
                    parts.push("Ctrl".to_string());
                }
                if modifiers.alt {
                    parts.push("Alt".to_string());
                }
                if modifiers.shift {
                    parts.push("Shift".to_string());
                }
                if modifiers.win {
                    parts.push("Win".to_string());
                }
                parts.push(vk_to_name(vk));
                parts.join("+")
            }
            HotkeyTrigger::SingleTap { key, side } => match side {
                ModifierSide::Any => format!("Single {}", key.label()),
                ModifierSide::Left => format!("Left {}", key.label()),
                ModifierSide::Right => format!("Right {}", key.label()),
            },
            HotkeyTrigger::DoubleTap { key, side } => match side {
                ModifierSide::Any => format!("Double {}", key.label()),
                ModifierSide::Left => format!("Double Left {}", key.label()),
                ModifierSide::Right => format!("Double Right {}", key.label()),
            },
        }
    }
}

/// A named global hotkey binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotkeyBinding {
    /// Application-defined identifier (for example `"launcher.toggle"` or a command id).
    pub id: String,
    /// The chord or modifier tap that fires this binding.
    pub trigger: HotkeyTrigger,
}

impl HotkeyBinding {
    pub fn new(id: impl Into<String>, trigger: HotkeyTrigger) -> Self {
        Self {
            id: id.into(),
            trigger,
        }
    }
}

/// Why a candidate trigger cannot be assigned cleanly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictKind {
    /// Shadows standard Windows text editing (`Ctrl+C`, `Ctrl+V`, `Ctrl+X`, `Ctrl+Z`).
    ReservedTextEditing,
    /// Bare Windows key is reserved for the OS shell.
    ReservedWindowsKey,
    /// Another binding already owns this trigger.
    Duplicate(String),
}

/// Check whether `trigger` conflicts with reserved Windows shortcuts or an
/// existing set of bindings (optionally ignoring `ignore_id` when rebinding an
/// existing action).
pub fn check_conflict(
    trigger: HotkeyTrigger,
    existing: &[HotkeyBinding],
    ignore_id: Option<&str>,
) -> Option<ConflictKind> {
    match trigger {
        HotkeyTrigger::Chord { modifiers, vk } => {
            if modifiers.ctrl
                && !modifiers.alt
                && !modifiers.shift
                && !modifiers.win
                && matches!(vk, 0x43 | 0x56 | 0x58 | 0x5A)
            {
                return Some(ConflictKind::ReservedTextEditing);
            }
        }
        HotkeyTrigger::SingleTap {
            key: ModifierKey::Win,
            ..
        } => {
            return Some(ConflictKind::ReservedWindowsKey);
        }
        HotkeyTrigger::DoubleTap { .. } | HotkeyTrigger::SingleTap { .. } => {}
    }

    for binding in existing {
        if ignore_id == Some(binding.id.as_str()) {
            continue;
        }
        if binding.trigger == trigger {
            return Some(ConflictKind::Duplicate(binding.id.clone()));
        }
    }

    None
}

/// Probe whether Windows lets this process register a chord via `RegisterHotKey`.
///
/// Immediately unregisters on success. Useful for warning the user in Settings
/// when another application already owns a chord.
pub fn is_chord_available_in_os(modifiers: HotkeyModifiers, vk: u32) -> bool {
    const PROBE_ID: i32 = 0x7A10;
    let ok = unsafe { RegisterHotKey(None, PROBE_ID, modifiers.win32_flags(), vk) }.is_ok();
    if ok {
        unsafe {
            let _ = UnregisterHotKey(None, PROBE_ID);
        }
    }
    ok
}

/// A single low-level keyboard event fed to [`HotkeyMatcher`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    /// Windows virtual-key code (`KBDLLHOOKSTRUCT::vkCode`).
    pub vk: u32,
    /// `true` for key down (`WM_KEYDOWN` / `WM_SYSKEYDOWN`), `false` for key up.
    pub is_down: bool,
    /// Tick count in milliseconds (`KBDLLHOOKSTRUCT::time`, wraps every ~49.7 days).
    pub timestamp_ms: u32,
    /// Whether the event was synthesized (`LLKHF_INJECTED`).
    pub injected: bool,
}

/// What the hook procedure should do after feeding a [`KeyEvent`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookDecision {
    /// Pass the key event through to Windows unchanged.
    Pass,
    /// Swallow the key event without triggering a new binding (e.g. held chord repeat or key-up).
    Swallow,
    /// Fire the matched binding ID and swallow the keystroke (used for chords).
    /// `mask_menu` is `true` when `Alt` or `Win` was held so the hook injects a
    /// harmless mask key preventing `SC_KEYMENU` / Start Menu activation.
    TriggerSwallow { id: String, mask_menu: bool },
    /// Fire the matched binding ID and let the keystroke pass through (used for modifier taps).
    TriggerPass(String),
}

#[derive(Debug, Clone, Copy, Default)]
struct PhysicalModifiers {
    lctrl: bool,
    rctrl: bool,
    lalt: bool,
    ralt: bool,
    lshift: bool,
    rshift: bool,
    lwin: bool,
    rwin: bool,
}

impl PhysicalModifiers {
    fn logical(self) -> HotkeyModifiers {
        HotkeyModifiers {
            ctrl: self.lctrl || self.rctrl,
            alt: self.lalt || self.ralt,
            shift: self.lshift || self.rshift,
            win: self.lwin || self.rwin,
        }
    }

    fn count_held(self) -> usize {
        [
            self.lctrl,
            self.rctrl,
            self.lalt,
            self.ralt,
            self.lshift,
            self.rshift,
            self.lwin,
            self.rwin,
        ]
        .into_iter()
        .filter(|b| *b)
        .count()
    }

    fn get(&self, key: ModifierKey, side: ModifierSide) -> bool {
        match (key, side) {
            (ModifierKey::Ctrl, ModifierSide::Left) => self.lctrl,
            (ModifierKey::Ctrl, ModifierSide::Right) => self.rctrl,
            (ModifierKey::Alt, ModifierSide::Left) => self.lalt,
            (ModifierKey::Alt, ModifierSide::Right) => self.ralt,
            (ModifierKey::Shift, ModifierSide::Left) => self.lshift,
            (ModifierKey::Shift, ModifierSide::Right) => self.rshift,
            (ModifierKey::Win, ModifierSide::Left) => self.lwin,
            (ModifierKey::Win, ModifierSide::Right) => self.rwin,
            (ModifierKey::Ctrl, ModifierSide::Any) => self.lctrl || self.rctrl,
            (ModifierKey::Alt, ModifierSide::Any) => self.lalt || self.ralt,
            (ModifierKey::Shift, ModifierSide::Any) => self.lshift || self.rshift,
            (ModifierKey::Win, ModifierSide::Any) => self.lwin || self.rwin,
        }
    }

    fn set(&mut self, key: ModifierKey, side: ModifierSide, down: bool) {
        match (key, side) {
            (ModifierKey::Ctrl, ModifierSide::Left) => self.lctrl = down,
            (ModifierKey::Ctrl, ModifierSide::Right) => self.rctrl = down,
            (ModifierKey::Alt, ModifierSide::Left) => self.lalt = down,
            (ModifierKey::Alt, ModifierSide::Right) => self.ralt = down,
            (ModifierKey::Shift, ModifierSide::Left) => self.lshift = down,
            (ModifierKey::Shift, ModifierSide::Right) => self.rshift = down,
            (ModifierKey::Win, ModifierSide::Left) => self.lwin = down,
            (ModifierKey::Win, ModifierSide::Right) => self.rwin = down,
            (_, ModifierSide::Any) => {}
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct PendingTap {
    key: ModifierKey,
    side: ModifierSide,
    pressed_at_ms: u32,
    tainted: bool,
}

#[derive(Debug, Clone, Copy)]
struct CompletedTap {
    key: ModifierKey,
    side: ModifierSide,
    released_at_ms: u32,
}

/// Pure, testable global hotkey & modifier-tap state machine.
#[derive(Debug, Default)]
pub struct HotkeyMatcher {
    bindings: Vec<HotkeyBinding>,
    modifiers: PhysicalModifiers,
    pending_tap: Option<PendingTap>,
    last_tap: Option<CompletedTap>,
    suppressed_vks: HashSet<u32>,
}

impl HotkeyMatcher {
    pub fn new(bindings: Vec<HotkeyBinding>) -> Self {
        Self {
            bindings,
            ..Self::default()
        }
    }

    /// Replace the active bindings without disturbing held modifier state.
    pub fn set_bindings(&mut self, bindings: Vec<HotkeyBinding>) {
        self.bindings = bindings;
        self.pending_tap = None;
        self.last_tap = None;
        self.suppressed_vks.clear();
    }

    /// Clear tracked modifiers and suppressed VKs that Windows says are no
    /// longer physically held (e.g. after desktop lock or UAC), skipping
    /// `exclude_vk` (the key currently arriving in this hook event).
    pub fn clear_stale_keys(
        &mut self,
        exclude_vk: u32,
        is_vk_physically_down: impl Fn(u32) -> bool,
    ) {
        let mut stale_found = false;
        let checks = [
            (VK_LCONTROL, &mut self.modifiers.lctrl),
            (VK_RCONTROL, &mut self.modifiers.rctrl),
            (VK_LMENU, &mut self.modifiers.lalt),
            (VK_RMENU, &mut self.modifiers.ralt),
            (VK_LSHIFT, &mut self.modifiers.lshift),
            (VK_RSHIFT, &mut self.modifiers.rshift),
            (VK_LWIN, &mut self.modifiers.lwin),
            (VK_RWIN, &mut self.modifiers.rwin),
        ];
        for (vk, slot) in checks {
            if vk != exclude_vk && *slot && !is_vk_physically_down(vk) {
                *slot = false;
                stale_found = true;
            }
        }
        if stale_found {
            self.pending_tap = None;
            self.last_tap = None;
            self.suppressed_vks.clear();
        } else if !self.suppressed_vks.is_empty() {
            self.suppressed_vks
                .retain(|&vk| vk == exclude_vk || is_vk_physically_down(vk));
        }
    }

    /// Process one keyboard event and decide whether to trigger and/or swallow.
    pub fn feed(&mut self, event: KeyEvent) -> HookDecision {
        if event.injected {
            return HookDecision::Pass;
        }

        if let Some((key, side)) = classify_modifier_vk(event.vk) {
            return self.handle_modifier(key, side, event.is_down, event.timestamp_ms);
        }

        // Any non-modifier key taints a pending modifier tap and breaks any double-tap sequence.
        if let Some(pending) = self.pending_tap.as_mut() {
            pending.tainted = true;
        }
        self.last_tap = None;

        if event.is_down {
            if self.suppressed_vks.contains(&event.vk) {
                return HookDecision::Swallow;
            }
            let current = self.modifiers.logical();
            if !current.is_empty() {
                if let Some(binding) = self.bindings.iter().find(|b| {
                    matches!(
                        b.trigger,
                        HotkeyTrigger::Chord { modifiers, vk }
                            if modifiers == current && vk == event.vk
                    )
                }) {
                    self.suppressed_vks.insert(event.vk);
                    return HookDecision::TriggerSwallow {
                        id: binding.id.clone(),
                        mask_menu: current.alt || current.win,
                    };
                }
            }
            HookDecision::Pass
        } else if self.suppressed_vks.remove(&event.vk) {
            HookDecision::Swallow
        } else {
            HookDecision::Pass
        }
    }

    fn handle_modifier(
        &mut self,
        key: ModifierKey,
        side: ModifierSide,
        is_down: bool,
        timestamp_ms: u32,
    ) -> HookDecision {
        if is_down {
            // Auto-repeat while holding the same physical modifier: ignore.
            if self.modifiers.get(key, side) {
                return HookDecision::Pass;
            }
            self.modifiers.set(key, side, true);
            if self.modifiers.count_held() > 1 {
                if let Some(pending) = self.pending_tap.as_mut() {
                    pending.tainted = true;
                }
                self.last_tap = None;
            } else {
                self.pending_tap = Some(PendingTap {
                    key,
                    side,
                    pressed_at_ms: timestamp_ms,
                    tainted: false,
                });
            }
            return HookDecision::Pass;
        }

        // Key up.
        self.modifiers.set(key, side, false);
        let Some(pending) = self.pending_tap.take() else {
            return HookDecision::Pass;
        };

        let within_window = timestamp_ms.wrapping_sub(pending.pressed_at_ms) <= TAP_WINDOW_MS;
        if pending.tainted || pending.key != key || pending.side != side || !within_window {
            self.last_tap = None;
            return HookDecision::Pass;
        }

        // Check double-tap first if there is a recent completed tap of the same modifier.
        if let Some(previous) = self.last_tap {
            let gap_ok = timestamp_ms.wrapping_sub(previous.released_at_ms) <= TAP_WINDOW_MS;
            if gap_ok && previous.key == key {
                if let Some(binding) = self.bindings.iter().find(|b| {
                    matches!(
                        b.trigger,
                        HotkeyTrigger::DoubleTap {
                            key: bkey,
                            side: bside,
                        } if bkey == key && bside.matches(previous.side) && bside.matches(side)
                    )
                }) {
                    self.last_tap = None;
                    return HookDecision::TriggerPass(binding.id.clone());
                }
            }
        }

        // Check single-tap bindings.
        if let Some(binding) = self.bindings.iter().find(|b| {
            matches!(
                b.trigger,
                HotkeyTrigger::SingleTap {
                    key: bkey,
                    side: bside,
                } if bkey == key && bside.matches(side)
            )
        }) {
            self.last_tap = None;
            return HookDecision::TriggerPass(binding.id.clone());
        }

        // Record this clean tap in case a second tap arrives within TAP_WINDOW_MS.
        self.last_tap = Some(CompletedTap {
            key,
            side,
            released_at_ms: timestamp_ms,
        });
        HookDecision::Pass
    }
}

fn classify_modifier_vk(vk: u32) -> Option<(ModifierKey, ModifierSide)> {
    match vk {
        VK_LCONTROL => Some((ModifierKey::Ctrl, ModifierSide::Left)),
        VK_RCONTROL => Some((ModifierKey::Ctrl, ModifierSide::Right)),
        VK_LMENU => Some((ModifierKey::Alt, ModifierSide::Left)),
        VK_RMENU => Some((ModifierKey::Alt, ModifierSide::Right)),
        VK_LSHIFT => Some((ModifierKey::Shift, ModifierSide::Left)),
        VK_RSHIFT => Some((ModifierKey::Shift, ModifierSide::Right)),
        VK_LWIN => Some((ModifierKey::Win, ModifierSide::Left)),
        VK_RWIN => Some((ModifierKey::Win, ModifierSide::Right)),
        _ => None,
    }
}

fn parse_modifier_with_side(text: &str) -> Option<(ModifierKey, ModifierSide)> {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let (side, key_str) = match tokens.as_slice() {
        [only] => (ModifierSide::Any, *only),
        ["left" | "l", key] => (ModifierSide::Left, *key),
        ["right" | "r", key] => (ModifierSide::Right, *key),
        _ => return None,
    };
    let key = match key_str {
        "ctrl" | "control" => ModifierKey::Ctrl,
        "alt" | "option" | "opt" => ModifierKey::Alt,
        "shift" => ModifierKey::Shift,
        "win" | "super" | "meta" | "cmd" | "command" => ModifierKey::Win,
        _ => return None,
    };
    Some((key, side))
}

fn named_key_to_vk(name: &str) -> Option<u32> {
    let vk = match name {
        "space" | "spacebar" => 0x20,
        "enter" | "return" => 0x0D,
        "escape" | "esc" => 0x1B,
        "tab" => 0x09,
        "backspace" => 0x08,
        "delete" | "del" => 0x2E,
        "insert" | "ins" => 0x2D,
        "home" => 0x24,
        "end" => 0x23,
        "pageup" | "pgup" => 0x21,
        "pagedown" | "pgdn" => 0x22,
        "left" => 0x25,
        "up" => 0x26,
        "right" => 0x27,
        "down" => 0x28,
        "," | "comma" => 0xBC,
        "." | "period" => 0xBE,
        "/" | "slash" => 0xBF,
        ";" | "semicolon" => 0xBA,
        "'" | "quote" => 0xDE,
        "[" | "lbracket" => 0xDB,
        "]" | "rbracket" => 0xDD,
        "-" | "minus" => 0xBD,
        "=" | "equal" | "plus" => 0xBB,
        "`" | "backtick" | "grave" => 0xC0,
        "\\" | "backslash" => 0xDC,
        other => {
            if let Some(num) = other.strip_prefix('f').and_then(|s| s.parse::<u32>().ok()) {
                if (1..=20).contains(&num) {
                    return Some(0x6F + num);
                }
                return None;
            }
            let mut chars = other.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) if c.is_ascii_alphanumeric() => c.to_ascii_uppercase() as u32,
                _ => return None,
            }
        }
    };
    Some(vk)
}

fn vk_to_name(vk: u32) -> String {
    match vk {
        0x20 => "Space".to_string(),
        0x0D => "Enter".to_string(),
        0x1B => "Esc".to_string(),
        0x09 => "Tab".to_string(),
        0x08 => "Backspace".to_string(),
        0x2E => "Delete".to_string(),
        0x2D => "Insert".to_string(),
        0x24 => "Home".to_string(),
        0x23 => "End".to_string(),
        0x21 => "PageUp".to_string(),
        0x22 => "PageDown".to_string(),
        0x25 => "Left".to_string(),
        0x26 => "Up".to_string(),
        0x27 => "Right".to_string(),
        0x28 => "Down".to_string(),
        0xBC => ",".to_string(),
        0xBE => ".".to_string(),
        0xBF => "/".to_string(),
        0xBA => ";".to_string(),
        0xDE => "'".to_string(),
        0xDB => "[".to_string(),
        0xDD => "]".to_string(),
        0xBD => "-".to_string(),
        0xBB => "=".to_string(),
        0xC0 => "`".to_string(),
        0xDC => "\\".to_string(),
        0x70..=0x83 => format!("F{}", vk - 0x6F),
        0x30..=0x39 | 0x41..=0x5A => (vk as u8 as char).to_string(),
        other => format!("VK_{other:02X}"),
    }
}

/// Inject an unassigned keystroke (which carries `LLKHF_INJECTED` so our own
/// matcher ignores it) to prevent Windows from interpreting an `Alt` or `Win`
/// release after a swallowed chord as a menu-bar or Start Menu activation.
unsafe fn suppress_modifier_menu_activation() {
    let inputs = [
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(VK_MENU_MASK),
                    ..Default::default()
                },
            },
        },
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(VK_MENU_MASK),
                    dwFlags: KEYEVENTF_KEYUP,
                    ..Default::default()
                },
            },
        },
    ];
    let _ = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
}

/// Shared runtime state read by the active `WH_KEYBOARD_LL` callback.
struct ActiveHookState {
    matcher: Arc<Mutex<HotkeyMatcher>>,
    tx: mpsc::Sender<String>,
}

static HOOK_STATE: OnceLock<Mutex<Option<ActiveHookState>>> = OnceLock::new();

fn hook_state_cell() -> &'static Mutex<Option<ActiveHookState>> {
    HOOK_STATE.get_or_init(|| Mutex::new(None))
}

/// A live background global hotkey service backed by `WH_KEYBOARD_LL`.
///
/// The hook thread blocks in `GetMessageW` (0% idle CPU) and sends matched
/// binding IDs to a dedicated dispatch thread so the low-level hook callback
/// always finishes in microseconds.
pub struct GlobalHotkeyService {
    matcher: Arc<Mutex<HotkeyMatcher>>,
    hook_thread_id: u32,
    hook_thread: Option<JoinHandle<()>>,
    dispatch_thread: Option<JoinHandle<()>>,
}

impl GlobalHotkeyService {
    /// Start listening for `bindings`. `on_trigger` is called on a background
    /// dispatch thread (never inside the keyboard hook itself).
    pub fn spawn(
        bindings: Vec<HotkeyBinding>,
        on_trigger: impl Fn(String) + Send + 'static,
    ) -> Result<Self, String> {
        let matcher = Arc::new(Mutex::new(HotkeyMatcher::new(bindings)));
        let (event_tx, event_rx) = mpsc::channel::<String>();

        {
            let mut slot = hook_state_cell().lock().expect("hotkey hook state lock");
            if slot.is_some() {
                return Err("a global hotkey service is already running".to_string());
            }
            *slot = Some(ActiveHookState {
                matcher: Arc::clone(&matcher),
                tx: event_tx,
            });
        }

        let dispatch_thread = std::thread::Builder::new()
            .name("yal-hotkey-dispatch".to_string())
            .spawn(move || {
                while let Ok(id) = event_rx.recv() {
                    on_trigger(id);
                }
            })
            .map_err(|error| {
                *hook_state_cell().lock().expect("hotkey hook state lock") = None;
                format!("the hotkey dispatch thread could not be started: {error}")
            })?;

        let (ready_tx, ready_rx) = mpsc::channel::<Result<u32, String>>();
        let hook_thread = std::thread::Builder::new()
            .name("yal-hotkey-hook".to_string())
            .spawn(move || {
                // Ensure this thread has a message queue before installing the hook.
                unsafe {
                    let mut msg = MSG::default();
                    let _ = PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE);
                }
                let tid = unsafe { GetCurrentThreadId() };
                let module = match unsafe { GetModuleHandleW(None) } {
                    Ok(h) => Some(HINSTANCE(h.0)),
                    Err(error) => {
                        let _ = ready_tx.send(Err(format!("GetModuleHandleW failed: {error}")));
                        return;
                    }
                };
                let hook = match unsafe {
                    SetWindowsHookExW(WH_KEYBOARD_LL, Some(service_hook_proc), module, 0)
                } {
                    Ok(h) => h,
                    Err(error) => {
                        let _ = ready_tx.send(Err(format!(
                            "the global keyboard hook could not be installed: {error}"
                        )));
                        return;
                    }
                };

                let _ = ready_tx.send(Ok(tid));

                unsafe {
                    let mut msg = MSG::default();
                    while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
                        let _ = TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                    let _ = UnhookWindowsHookEx(hook);
                }
            })
            .map_err(|error| {
                *hook_state_cell().lock().expect("hotkey hook state lock") = None;
                format!("the hotkey thread could not be started: {error}")
            })?;

        let hook_thread_id = match ready_rx.recv() {
            Ok(Ok(tid)) => tid,
            Ok(Err(error)) => {
                *hook_state_cell().lock().expect("hotkey hook state lock") = None;
                let _ = hook_thread.join();
                let _ = dispatch_thread.join();
                return Err(error);
            }
            Err(_) => {
                *hook_state_cell().lock().expect("hotkey hook state lock") = None;
                let _ = hook_thread.join();
                let _ = dispatch_thread.join();
                return Err("the hotkey thread exited before installing the hook".to_string());
            }
        };

        Ok(Self {
            matcher,
            hook_thread_id,
            hook_thread: Some(hook_thread),
            dispatch_thread: Some(dispatch_thread),
        })
    }

    /// Update the active hotkey bindings live without reinstalling the hook.
    pub fn set_bindings(&self, bindings: Vec<HotkeyBinding>) {
        if let Ok(mut matcher) = self.matcher.lock() {
            matcher.set_bindings(bindings);
        }
    }

    /// Stop the hook and release resources cleanly.
    pub fn stop(&mut self) {
        if self.hook_thread_id != 0 {
            unsafe {
                let _ = PostThreadMessageW(self.hook_thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
            }
            self.hook_thread_id = 0;
        }
        if let Some(handle) = self.hook_thread.take() {
            let _ = handle.join();
        }
        // Clear the global state (dropping the Sender so dispatch_thread exits cleanly).
        if let Ok(mut slot) = hook_state_cell().lock() {
            *slot = None;
        }
        if let Some(handle) = self.dispatch_thread.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for GlobalHotkeyService {
    fn drop(&mut self) {
        self.stop();
    }
}

unsafe extern "system" fn service_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let msg = wparam.0 as u32;
        let is_down = match msg {
            WM_KEYDOWN | WM_SYSKEYDOWN => Some(true),
            WM_KEYUP | WM_SYSKEYUP => Some(false),
            _ => None,
        };
        if let Some(is_down) = is_down {
            let info = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
            let event = KeyEvent {
                vk: info.vkCode,
                is_down,
                timestamp_ms: info.time,
                injected: (info.flags.0 & LLKHF_INJECTED.0) != 0,
            };

            let decision = if let Ok(slot) = hook_state_cell().try_lock() {
                if let Some(active) = slot.as_ref() {
                    if let Ok(mut matcher) = active.matcher.try_lock() {
                        // On key-downs (modifier or non-modifier), reconcile any
                        // stale keys Windows says are no longer physically held.
                        if is_down {
                            matcher.clear_stale_keys(event.vk, |vk| {
                                (GetAsyncKeyState(vk as i32) as u16 & 0x8000) != 0
                            });
                        }
                        let decision = matcher.feed(event);
                        match &decision {
                            HookDecision::TriggerSwallow { id, .. }
                            | HookDecision::TriggerPass(id) => {
                                let _ = active.tx.send(id.clone());
                            }
                            HookDecision::Pass | HookDecision::Swallow => {}
                        }
                        decision
                    } else {
                        HookDecision::Pass
                    }
                } else {
                    HookDecision::Pass
                }
            } else {
                HookDecision::Pass
            };

            match decision {
                HookDecision::TriggerSwallow { mask_menu, .. } => {
                    if mask_menu {
                        suppress_modifier_menu_activation();
                    }
                    return LRESULT(1);
                }
                HookDecision::Swallow => return LRESULT(1),
                HookDecision::Pass | HookDecision::TriggerPass(_) => {}
            }
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

/// Lightweight probe implementing [`tc_platform::Hotkey`] for `--self-test`.
#[derive(Debug, Default)]
pub struct WindowsHotkey {
    hook: Mutex<Option<HHOOK>>,
}

unsafe impl Send for WindowsHotkey {}
unsafe impl Sync for WindowsHotkey {}

impl WindowsHotkey {
    pub fn new() -> WindowsHotkey {
        WindowsHotkey::default()
    }

    /// Install a probe hook to verify the desktop allows keyboard hooks.
    pub fn install(&self) -> Option<HHOOK> {
        let mut guard = self.hook.lock().expect("hook lock");
        if let Some(hook) = *guard {
            return Some(hook);
        }
        let module = unsafe { GetModuleHandleW(None).ok()? };
        let module = Some(HINSTANCE(module.0));
        let hook =
            unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(probe_hook_proc), module, 0) }.ok()?;
        *guard = Some(hook);
        Some(hook)
    }

    /// Remove the probe hook if installed.
    pub fn uninstall(&self) {
        let taken = self.hook.lock().expect("hook lock").take();
        if let Some(hook) = taken {
            unsafe {
                let _ = UnhookWindowsHookEx(hook);
            }
        }
    }
}

impl Drop for WindowsHotkey {
    fn drop(&mut self) {
        self.uninstall();
    }
}

impl Hotkey for WindowsHotkey {
    fn is_available(&self) -> bool {
        if self.install().is_some() {
            self.uninstall();
            true
        } else {
            false
        }
    }

    fn unavailable_reason(&self) -> Option<String> {
        if self.is_available() {
            None
        } else {
            Some(
                "The keyboard hook is unavailable. The secure desktop, the lock screen and \
                 administrator windows all block it."
                    .to_string(),
            )
        }
    }
}

unsafe extern "system" fn probe_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    CallNextHookEx(None, code, wparam, lparam)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn down(vk: u32, t: u32) -> KeyEvent {
        KeyEvent {
            vk,
            is_down: true,
            timestamp_ms: t,
            injected: false,
        }
    }

    fn up(vk: u32, t: u32) -> KeyEvent {
        KeyEvent {
            vk,
            is_down: false,
            timestamp_ms: t,
            injected: false,
        }
    }

    #[test]
    fn parses_and_formats_chords_and_taps() {
        let alt_space = HotkeyTrigger::parse("Alt+Space").expect("Alt+Space");
        assert_eq!(alt_space, HotkeyTrigger::default_launcher());
        assert_eq!(alt_space.format(), "Alt+Space");

        let ctrl_shift_v = HotkeyTrigger::parse("ctrl + shift + v").expect("Ctrl+Shift+V");
        assert_eq!(ctrl_shift_v.format(), "Ctrl+Shift+V");

        let double_ctrl = HotkeyTrigger::parse("Double Ctrl").expect("Double Ctrl");
        assert_eq!(double_ctrl.format(), "Double Ctrl");

        let right_alt = HotkeyTrigger::parse("Right Alt").expect("Right Alt");
        assert_eq!(right_alt.format(), "Right Alt");

        assert!(HotkeyTrigger::parse("Space").is_none());
        assert!(HotkeyTrigger::parse("").is_none());
    }

    #[test]
    fn chord_triggers_once_masks_alt_menu_and_swallows_repeats_and_keyup() {
        let mut matcher = HotkeyMatcher::new(vec![HotkeyBinding::new(
            "launcher.toggle",
            HotkeyTrigger::default_launcher(),
        )]);

        assert_eq!(matcher.feed(down(VK_LMENU, 100)), HookDecision::Pass);
        assert_eq!(
            matcher.feed(down(0x20, 120)),
            HookDecision::TriggerSwallow {
                id: "launcher.toggle".to_string(),
                mask_menu: true,
            }
        );
        // Holding Space produces auto-repeat keydowns — swallowed without re-firing.
        assert_eq!(matcher.feed(down(0x20, 200)), HookDecision::Swallow);
        // Keyup of Space is swallowed cleanly.
        assert_eq!(matcher.feed(up(0x20, 220)), HookDecision::Swallow);
        // Releasing Alt passes through so Windows modifier state stays synced.
        assert_eq!(matcher.feed(up(VK_LMENU, 240)), HookDecision::Pass);
    }

    #[test]
    fn ctrl_chord_does_not_request_menu_masking() {
        let mut matcher = HotkeyMatcher::new(vec![HotkeyBinding::new(
            "launcher.toggle",
            HotkeyTrigger::fallback_launcher(),
        )]);

        matcher.feed(down(VK_LCONTROL, 100));
        assert_eq!(
            matcher.feed(down(0x20, 120)),
            HookDecision::TriggerSwallow {
                id: "launcher.toggle".to_string(),
                mask_menu: false,
            }
        );
    }

    #[test]
    fn extra_modifier_does_not_fire_narrower_chord() {
        let mut matcher = HotkeyMatcher::new(vec![HotkeyBinding::new(
            "launcher.toggle",
            HotkeyTrigger::default_launcher(),
        )]);

        matcher.feed(down(VK_LMENU, 100));
        matcher.feed(down(VK_LSHIFT, 110));
        assert_eq!(matcher.feed(down(0x20, 120)), HookDecision::Pass);
    }

    #[test]
    fn double_tap_ctrl_fires_within_250ms_window_including_tick_wraparound() {
        let mut matcher = HotkeyMatcher::new(vec![HotkeyBinding::new(
            "launcher.toggle",
            HotkeyTrigger::DoubleTap {
                key: ModifierKey::Ctrl,
                side: ModifierSide::Any,
            },
        )]);

        // Cross u32::MAX cleanly (49.7-day uptime wraparound).
        let t0 = u32::MAX - 100;
        assert_eq!(matcher.feed(down(VK_LCONTROL, t0)), HookDecision::Pass);
        assert_eq!(
            matcher.feed(up(VK_LCONTROL, t0.wrapping_add(80))),
            HookDecision::Pass
        );
        assert_eq!(
            matcher.feed(down(VK_LCONTROL, t0.wrapping_add(160))),
            HookDecision::Pass
        );
        assert_eq!(
            matcher.feed(up(VK_LCONTROL, t0.wrapping_add(230))),
            HookDecision::TriggerPass("launcher.toggle".to_string())
        );
    }

    #[test]
    fn double_tap_fails_if_gap_exceeds_250ms_or_hold_too_long() {
        let mut matcher = HotkeyMatcher::new(vec![HotkeyBinding::new(
            "launcher.toggle",
            HotkeyTrigger::DoubleTap {
                key: ModifierKey::Ctrl,
                side: ModifierSide::Any,
            },
        )]);

        // Gap > 250 ms between taps.
        matcher.feed(down(VK_LCONTROL, 1000));
        matcher.feed(up(VK_LCONTROL, 1080));
        matcher.feed(down(VK_LCONTROL, 1400));
        assert_eq!(matcher.feed(up(VK_LCONTROL, 1450)), HookDecision::Pass);

        // Held longer than 250 ms.
        matcher.feed(down(VK_LCONTROL, 2000));
        matcher.feed(up(VK_LCONTROL, 2400));
        matcher.feed(down(VK_LCONTROL, 2450));
        assert_eq!(matcher.feed(up(VK_LCONTROL, 2500)), HookDecision::Pass);
    }

    #[test]
    fn ctrl_c_never_triggers_single_or_double_ctrl_tap() {
        let mut matcher = HotkeyMatcher::new(vec![HotkeyBinding::new(
            "single.ctrl",
            HotkeyTrigger::SingleTap {
                key: ModifierKey::Ctrl,
                side: ModifierSide::Left,
            },
        )]);

        matcher.feed(down(VK_LCONTROL, 100));
        matcher.feed(down(0x43, 130)); // 'C'
        matcher.feed(up(0x43, 150));
        assert_eq!(matcher.feed(up(VK_LCONTROL, 180)), HookDecision::Pass);
    }

    #[test]
    fn single_tap_respects_left_vs_right_side() {
        let mut matcher = HotkeyMatcher::new(vec![HotkeyBinding::new(
            "right.alt",
            HotkeyTrigger::SingleTap {
                key: ModifierKey::Alt,
                side: ModifierSide::Right,
            },
        )]);

        matcher.feed(down(VK_LMENU, 100));
        assert_eq!(matcher.feed(up(VK_LMENU, 160)), HookDecision::Pass);

        matcher.feed(down(VK_RMENU, 500));
        assert_eq!(
            matcher.feed(up(VK_RMENU, 560)),
            HookDecision::TriggerPass("right.alt".to_string())
        );
    }

    #[test]
    fn injected_events_are_ignored() {
        let mut matcher = HotkeyMatcher::new(vec![HotkeyBinding::new(
            "launcher.toggle",
            HotkeyTrigger::default_launcher(),
        )]);

        matcher.feed(down(VK_LMENU, 100));
        let injected_space = KeyEvent {
            vk: 0x20,
            is_down: true,
            timestamp_ms: 120,
            injected: true,
        };
        assert_eq!(matcher.feed(injected_space), HookDecision::Pass);
    }

    #[test]
    fn stale_modifier_recovery_clears_stuck_bits_and_suppressed_keys() {
        let mut matcher = HotkeyMatcher::new(vec![HotkeyBinding::new(
            "launcher.toggle",
            HotkeyTrigger::default_launcher(),
        )]);

        matcher.feed(down(VK_LMENU, 100));
        matcher.feed(down(0x20, 120));
        // Simulate missed key-ups (e.g. Win+L): OS says neither Alt nor Space is held.
        matcher.clear_stale_keys(0x20, |_| false);
        assert_eq!(matcher.feed(down(0x20, 500)), HookDecision::Pass);
    }

    #[test]
    fn conflict_check_blocks_text_editing_win_tap_and_duplicates() {
        let existing = vec![HotkeyBinding::new(
            "launcher.toggle",
            HotkeyTrigger::default_launcher(),
        )];

        assert_eq!(
            check_conflict(HotkeyTrigger::parse("Ctrl+C").unwrap(), &existing, None),
            Some(ConflictKind::ReservedTextEditing)
        );
        assert_eq!(
            check_conflict(HotkeyTrigger::parse("Ctrl+V").unwrap(), &existing, None),
            Some(ConflictKind::ReservedTextEditing)
        );
        assert_eq!(
            check_conflict(
                HotkeyTrigger::SingleTap {
                    key: ModifierKey::Win,
                    side: ModifierSide::Left,
                },
                &existing,
                None,
            ),
            Some(ConflictKind::ReservedWindowsKey)
        );
        assert_eq!(
            check_conflict(HotkeyTrigger::default_launcher(), &existing, None),
            Some(ConflictKind::Duplicate("launcher.toggle".to_string()))
        );
        assert_eq!(
            check_conflict(
                HotkeyTrigger::default_launcher(),
                &existing,
                Some("launcher.toggle"),
            ),
            None
        );
    }

    #[test]
    fn global_hotkey_service_starts_updates_and_stops_cleanly() {
        let service = GlobalHotkeyService::spawn(
            vec![HotkeyBinding::new(
                "launcher.toggle",
                HotkeyTrigger::default_launcher(),
            )],
            |_| {},
        );
        if let Ok(mut svc) = service {
            svc.set_bindings(vec![HotkeyBinding::new(
                "launcher.toggle",
                HotkeyTrigger::fallback_launcher(),
            )]);
            svc.stop();
        }
    }
}
