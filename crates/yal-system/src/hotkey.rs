//! The global hotkey.
//!
//! The hotkey is registered with the window manager rather than polled, so
//! the launcher stays idle until the key combination is pressed.
//!
//! The listener runs on its own thread. A global hotkey notification is
//! posted to the message queue of the thread that registered it, and the
//! thread that runs a window's event loop consumes every message in that
//! queue — including the hotkey notification — before the owner can see it.
//! Giving the hotkey its own thread and its own queue keeps the two
//! independent.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::thread::JoinHandle;

/// The modifier keys that may accompany a hotkey.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HotkeyModifiers {
    pub alt: bool,
    pub control: bool,
    pub shift: bool,
    pub super_key: bool,
}

impl HotkeyModifiers {
    /// The modifier set as the Windows virtual-key flags.
    ///
    /// These values match the `MOD_*` constants used by `RegisterHotKey`.
    pub fn flags(self) -> u32 {
        let mut flags = 0;
        if self.alt {
            flags |= 0x0001;
        }
        if self.control {
            flags |= 0x0002;
        }
        if self.shift {
            flags |= 0x0004;
        }
        if self.super_key {
            flags |= 0x0008;
        }
        flags
    }
}

/// A failure while registering the hotkey.
#[derive(Debug, thiserror::Error)]
pub enum HotkeyError {
    #[error("another application already owns this hotkey")]
    AlreadyRegistered,

    #[error("the hotkey could not be registered: {0}")]
    Failed(String),
}

/// A registered global hotkey.
///
/// The registration is released when the value is dropped.
pub struct Hotkey {
    #[cfg(windows)]
    id: i32,
    #[cfg(not(windows))]
    _private: (),
}

impl Hotkey {
    /// Register a hotkey on the calling thread.
    ///
    /// `virtual_key` is a Windows virtual key code, for example `0x41` for
    /// the `A` key.
    #[cfg(windows)]
    pub fn register(
        virtual_key: u32,
        modifiers: HotkeyModifiers,
        id: i32,
    ) -> Result<Self, HotkeyError> {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN, RegisterHotKey,
        };

        let mut flags = 0u32;
        if modifiers.alt {
            flags |= MOD_ALT.0;
        }
        if modifiers.control {
            flags |= MOD_CONTROL.0;
        }
        if modifiers.shift {
            flags |= MOD_SHIFT.0;
        }
        if modifiers.super_key {
            flags |= MOD_WIN.0;
        }

        // A null window handle means "this thread", which is what a
        // background hotkey listener wants.
        let result = unsafe {
            RegisterHotKey(None, id, HOT_KEY_MODIFIERS(flags), virtual_key)
        };

        if result.is_err() {
            return Err(HotkeyError::AlreadyRegistered);
        }

        Ok(Self { id })
    }

    /// Register a hotkey on a host without hotkeys.
    #[cfg(not(windows))]
    pub fn register(
        _virtual_key: u32,
        _modifiers: HotkeyModifiers,
        _id: i32,
    ) -> Result<Self, HotkeyError> {
        Err(HotkeyError::Failed(
            "global hotkeys are not implemented on this platform".into(),
        ))
    }

    /// The identifier the hotkey was registered under.
    pub fn id(&self) -> i32 {
        #[cfg(windows)]
        {
            self.id
        }
        #[cfg(not(windows))]
        {
            0
        }
    }
}

#[cfg(windows)]
impl Drop for Hotkey {
    fn drop(&mut self) {
        use windows::Win32::UI::Input::KeyboardAndMouse::UnregisterHotKey;
        unsafe {
            let _ = UnregisterHotKey(None, self.id);
        }
    }
}

/// A hotkey listener running on its own thread.
///
/// The listener records presses; [`HotkeyListener::poll`] reports and clears
/// them.
pub struct HotkeyListener {
    pressed: Arc<AtomicBool>,
    available: Arc<AtomicBool>,
    failed: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    thread_id: Arc<AtomicU32>,
    handle: Option<JoinHandle<()>>,
}

impl HotkeyListener {
    /// Start listening for a hotkey.
    ///
    /// The registration happens on the listener thread, so this waits for it
    /// to finish before returning. The wait is bounded: a caller should not
    /// be held up by a system that is slow to answer, and the answer is
    /// reported by [`HotkeyListener::is_available`] either way.
    pub fn spawn(virtual_key: u32, modifiers: HotkeyModifiers, id: i32) -> Self {
        let pressed = Arc::new(AtomicBool::new(false));
        let available = Arc::new(AtomicBool::new(false));
        let failed = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let thread_id = Arc::new(AtomicU32::new(0));

        let thread_pressed = pressed.clone();
        let thread_available = available.clone();
        let thread_failed = failed.clone();
        let thread_stop = stop.clone();
        let thread_thread_id = thread_id.clone();

        let handle = std::thread::Builder::new()
            .name("hotkey-listener".into())
            .spawn(move || {
                // A thread must own a message queue before it can register a
                // thread-wide hotkey. Pumping one message creates the queue;
                // without this the registration fails outright.
                ensure_message_queue();

                match Hotkey::register(virtual_key, modifiers, id) {
                    Ok(_hotkey) => {
                        thread_available.store(true, Ordering::Relaxed);
                        if let Some(id) = current_thread_id() {
                            thread_thread_id.store(id, Ordering::Relaxed);
                        }
                        wait_for_hotkey(virtual_key, modifiers, &thread_pressed, &thread_stop);
                    }
                    Err(_) => {
                        thread_failed.store(true, Ordering::Relaxed);
                    }
                }
            })
            .ok();

        // Wait for the registration to be decided.
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(500);
        while std::time::Instant::now() < deadline {
            if available.load(Ordering::Relaxed) || failed.load(Ordering::Relaxed) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }

        Self {
            pressed,
            available,
            failed,
            stop,
            thread_id,
            handle,
        }
    }

    /// Whether the hotkey was successfully registered.
    pub fn is_available(&self) -> bool {
        self.available.load(Ordering::Relaxed)
    }

    /// Whether registration was attempted and refused.
    ///
    /// A refusal means another application already owns the combination.
    pub fn is_taken(&self) -> bool {
        self.failed.load(Ordering::Relaxed)
    }

    /// Whether the hotkey was pressed since the last poll.
    pub fn poll(&self) -> bool {
        self.pressed.swap(false, Ordering::Relaxed)
    }

    /// Stop listening.
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            // The listener thread blocks in `GetMessage`, so it is woken by
            // posting a quit message before joining.
            let id = self.thread_id.load(Ordering::Relaxed);
            if id != 0 {
                wake_listener(id);
            }
            let _ = handle.join();
        }
    }
}

impl Drop for HotkeyListener {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Wait for a hotkey press.
///
/// Two detections run together, because each one alone misses presses on a
/// real machine:
///
/// - The window manager's `WM_HOTKEY` notification, queued onto this very
///   thread, is the primary source. It fires while the key is down, so a
///   quick press-and-release between two state reads is never lost. The
///   state read below is what it falls back for; a thread whose
///   notification is somehow silent still sees the key.
/// - A direct read of the keyboard state, which has no dependency on
///   anything but the key itself. It fires on the rising edge, so holding
///   the combination down produces one press rather than a stream.
///
/// Stopping happens by flag; a posted quit message is drained with the
/// same pump, so the closing path stays responsive too.
#[cfg(windows)]
fn wait_for_hotkey(
    virtual_key: u32,
    modifiers: HotkeyModifiers,
    pressed: &AtomicBool,
    stop: &AtomicBool,
) {
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    use windows::Win32::UI::WindowsAndMessaging::{MSG, PEEK_MESSAGE_REMOVE_TYPE, PeekMessageW, WM_QUIT};

    let mut was_down = false;
    while !stop.load(Ordering::Relaxed) {
        // Every queued notification is read out before the state read; a
        // queue that is never pumped would hold the presses forever.
        let mut message = MSG::default();
        while unsafe {
            PeekMessageW(
                &mut message,
                None,
                0,
                0,
                PEEK_MESSAGE_REMOVE_TYPE(1), // PM_REMOVE
            )
        }
        .as_bool()
        {
            if message.message == WM_QUIT {
                return;
            }
            // WM_HOTKEY: this thread owns exactly one hotkey, so every
            // notification belongs to it.
            if message.message == 0x0312 {
                pressed.store(true, Ordering::Relaxed);
            }
        }

        let down = combination_down(virtual_key, modifiers);
        if down && !was_down {
            pressed.store(true, Ordering::Relaxed);
        }
        was_down = down;
        std::thread::sleep(std::time::Duration::from_millis(25));
    }

    // Keep the import used on hosts where the state read is the only path.
    let _ = (GetAsyncKeyState, LPARAM(0), WPARAM(0));
}

/// Whether the key and every required modifier are currently held.
#[cfg(windows)]
fn combination_down(virtual_key: u32, modifiers: HotkeyModifiers) -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;

    fn held(key: u32) -> bool {
        // The low bit reports the current state; the high bit reports
        // whether the key was pressed since the last call.
        unsafe { GetAsyncKeyState(key as i32) as u32 & 0x8000 != 0 }
    }

    if !held(virtual_key) {
        return false;
    }
    if modifiers.control && !held(0x11) {
        return false;
    }
    if modifiers.alt && !held(0x12) {
        return false;
    }
    if modifiers.shift && !held(0x10) {
        return false;
    }
    if modifiers.super_key && !held(0x5B) && !held(0x5C) {
        return false;
    }
    true
}

/// Wait for a hotkey press on a host without hotkeys.
#[cfg(not(windows))]
fn wait_for_hotkey(
    _virtual_key: u32,
    _modifiers: HotkeyModifiers,
    _pressed: &AtomicBool,
    stop: &AtomicBool,
) {
    while !stop.load(Ordering::Relaxed) {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

/// Wake a listener that is blocked waiting for a message.
#[cfg(windows)]
fn wake_listener(thread_id: u32) {
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_QUIT};

    unsafe {
        let _ = PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
    }
}

/// Wake a listener on a host without hotkeys.
#[cfg(not(windows))]
fn wake_listener(_thread_id: u32) {}

/// Create this thread's message queue if it does not have one.
///
/// A thread that has never called a user32 function has no message queue,
/// and `RegisterHotKey` fails when it is asked to associate a hotkey with a
/// thread that has none.
#[cfg(windows)]
fn ensure_message_queue() {
    use windows::Win32::UI::WindowsAndMessaging::{MSG, PeekMessageW, PM_NOREMOVE};

    unsafe {
        let mut message = MSG::default();
        let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
    }
}

/// Create this thread's message queue if it does not have one.
#[cfg(not(windows))]
fn ensure_message_queue() {}

/// The identifier of the calling thread.
#[cfg(windows)]
fn current_thread_id() -> Option<u32> {
    use windows::Win32::System::Threading::GetCurrentThreadId;
    Some(unsafe { GetCurrentThreadId() })
}

/// The identifier of the calling thread.
#[cfg(not(windows))]
fn current_thread_id() -> Option<u32> {
    None
}

/// A hotkey written as text, such as `ctrl+shift+space`.
///
/// The launcher stores its hotkey in this form so that the user can change
/// it without recompiling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HotkeySpec {
    /// The virtual key code.
    pub virtual_key: u32,
    /// The required modifiers.
    pub modifiers: HotkeyModifiers,
}

impl HotkeySpec {
    /// The default launcher hotkey.
    pub fn default_spec() -> Self {
        Self {
            virtual_key: 0x20,
            modifiers: HotkeyModifiers {
                control: true,
                ..Default::default()
            },
        }
    }

    /// Parse a hotkey written as text.
    ///
    /// The parts are separated by `+`. The last part is the key; the others
    /// are modifiers. Names are case-insensitive, and `space` and `spacebar`
    /// both mean the space bar.
    pub fn parse(text: &str) -> Option<Self> {
        let parts: Vec<&str> = text
            .split('+')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .collect();
        if parts.len() < 2 {
            return None;
        }

        let mut modifiers = HotkeyModifiers::default();

        for part in &parts[..parts.len() - 1] {
            match part.to_lowercase().as_str() {
                "ctrl" | "control" => modifiers.control = true,
                "alt" | "option" => modifiers.alt = true,
                "shift" => modifiers.shift = true,
                "win" | "super" | "cmd" | "command" => modifiers.super_key = true,
                _ => return None,
            }
        }

        let virtual_key = named_key(&parts[parts.len() - 1].to_lowercase())?;
        Some(Self {
            virtual_key,
            modifiers,
        })
    }

    /// The hotkey written as text.
    pub fn to_text(self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if self.modifiers.control {
            parts.push("ctrl".to_string());
        }
        if self.modifiers.alt {
            parts.push("alt".to_string());
        }
        if self.modifiers.shift {
            parts.push("shift".to_string());
        }
        if self.modifiers.super_key {
            parts.push("win".to_string());
        }
        let key = match self.virtual_key {
            0x20 => "space",
            0x0D => "enter",
            0x1B => "escape",
            0x09 => "tab",
            0x08 => "backspace",
            other => return format!("{}+{}", parts.join("+"), virtual_key_name(other)),
        };
        parts.push(key.to_string());
        parts.join("+")
    }
}

/// The virtual key code for a named key.
fn named_key(name: &str) -> Option<u32> {
    let key = match name {
        "space" | "spacebar" => 0x20,
        "enter" | "return" => 0x0D,
        "escape" | "esc" => 0x1B,
        "tab" => 0x09,
        "backspace" => 0x08,
        "delete" | "del" => 0x2E,
        "home" => 0x24,
        "end" => 0x23,
        "pageup" => 0x21,
        "pagedown" => 0x22,
        "left" => 0x25,
        "up" => 0x26,
        "right" => 0x27,
        "down" => 0x28,
        "f1" => 0x70,
        "f2" => 0x71,
        "f3" => 0x72,
        "f4" => 0x73,
        "f5" => 0x74,
        "f6" => 0x75,
        "f7" => 0x76,
        "f8" => 0x77,
        "f9" => 0x78,
        "f10" => 0x79,
        "f11" => 0x7A,
        "f12" => 0x7B,
        other => {
            // A single letter or digit.
            let mut chars = other.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) if c.is_ascii_alphanumeric() => {
                    c.to_ascii_uppercase() as u32
                }
                _ => return None,
            }
        }
    };
    Some(key)
}

/// The name of a virtual key code.
fn virtual_key_name(key: u32) -> String {
    if (0x30..=0x39).contains(&key) || (0x41..=0x5A).contains(&key) {
        return ((key as u8 as char).to_ascii_lowercase()).to_string();
    }
    format!("vk{key}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The default launcher hotkey: Control and Space.
    pub const DEFAULT_VIRTUAL_KEY: u32 = 0x20;

    /// The default launcher modifiers.
    pub fn default_modifiers() -> HotkeyModifiers {
        HotkeyModifiers {
            control: true,
            ..Default::default()
        }
    }

    #[test]
    fn parses_a_hotkey() {
        let spec = HotkeySpec::parse("ctrl+shift+space").unwrap();
        assert_eq!(spec.virtual_key, 0x20);
        assert!(spec.modifiers.control);
        assert!(spec.modifiers.shift);
        assert!(!spec.modifiers.alt);
    }

    #[test]
    fn parses_a_single_letter_key() {
        let spec = HotkeySpec::parse("ctrl+alt+k").unwrap();
        assert_eq!(spec.virtual_key, 0x4B);
        assert!(spec.modifiers.alt);
    }

    #[test]
    fn parses_the_super_key() {
        let spec = HotkeySpec::parse("cmd+space").unwrap();
        assert!(spec.modifiers.super_key);
    }

    #[test]
    fn rejects_a_hotkey_without_modifiers() {
        assert!(HotkeySpec::parse("space").is_none());
    }

    #[test]
    fn rejects_an_unknown_modifier() {
        assert!(HotkeySpec::parse("hyper+space").is_none());
    }

    #[test]
    fn round_trips_through_text() {
        for text in ["ctrl+space", "ctrl+shift+space", "alt+win+k"] {
            let spec = HotkeySpec::parse(text).unwrap();
            assert_eq!(spec.to_text(), text);
        }
    }

    #[test]
    fn modifier_flags_are_distinct() {
        assert_eq!(HotkeyModifiers::default().flags(), 0);
        assert_eq!(HotkeyModifiers { alt: true, ..Default::default() }.flags(), 1);
        assert_eq!(
            HotkeyModifiers {
                control: true,
                ..Default::default()
            }
            .flags(),
            2
        );
        assert_eq!(
            HotkeyModifiers {
                shift: true,
                ..Default::default()
            }
            .flags(),
            4
        );
        assert_eq!(
            HotkeyModifiers {
                super_key: true,
                ..Default::default()
            }
            .flags(),
            8
        );
    }

    #[test]
    fn a_registered_hotkey_reports_its_identifier() {
        // Registration can legitimately fail when another application owns
        // the combination, so only the success path is asserted.
        if let Ok(hotkey) = Hotkey::register(0xA5, HotkeyModifiers::default(), 1) {
            assert_eq!(hotkey.id(), 1);
        }
    }

    #[test]
    fn a_listener_reports_no_press_when_nothing_was_pressed() {
        let listener = HotkeyListener::spawn(DEFAULT_VIRTUAL_KEY, default_modifiers(), 7);
        // Give the listener thread a moment to register.
        std::thread::sleep(std::time::Duration::from_millis(150));
        assert!(!listener.poll());
    }
}
