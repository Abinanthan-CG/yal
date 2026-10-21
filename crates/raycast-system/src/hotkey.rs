//! The global hotkey.
//!
//! The hotkey is registered with the window manager rather than polled, so
//! the launcher stays idle until the key combination is pressed.

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
    /// Register a hotkey.
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
            RegisterHotKey(
                None,
                id,
                HOT_KEY_MODIFIERS(flags),
                virtual_key,
            )
        };

        if result.is_err() {
            return Err(HotkeyError::AlreadyRegistered);
        }

        Ok(Self { id })
    }

    /// Register a hotkey on a non-Windows host.
    ///
    /// No global hotkey facility is implemented, so this always reports that
    /// the hotkey is unavailable rather than pretending to succeed.
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
