//! Typing text into the window that holds the keyboard focus.
//!
//! The launcher's snippets expand in place: the text is typed into the
//! application the user was in before the palette opened. The injection
//! sends Unicode key events through `SendInput`, which behave like a real
//! keyboard whatever application is on the receiving end. The system can
//! also refuse the injection — restricted sessions and secure desktops do
//! — so the caller receives an error it can degrade from.

/// One event the injection wants the operating system to see.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeyEvent {
    /// A character, as one UTF-16 code unit.
    Unicode(u16),
    /// A real key, for the characters that Unicode events do not carry
    /// into every application, such as Enter.
    Key(u16),
}

/// The events that type `text`, in order.
///
/// Line breaks become real Enter presses: many text fields accept a
/// Unicode line-feed event, but not all of them treat it as a return.
pub(crate) fn key_events(text: &str) -> Vec<KeyEvent> {
    const ENTER: u16 = 0x0D; // VK_RETURN

    text.encode_utf16()
        .map(|unit| match unit {
            0x0A | 0x0D => KeyEvent::Key(ENTER),
            unit => KeyEvent::Unicode(unit),
        })
        .collect()
}

/// Type the text into whatever window holds the keyboard focus.
///
/// The call returns when the system has accepted every event, not when an
/// application has finished displaying the characters — injection has no
/// way to observe the receiving end.
#[cfg(windows)]
pub fn type_text(text: &str) -> Result<(), String> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{INPUT, SendInput};

    let events = key_events(text);

    // Two events per key, a press and a release, sent in modest batches so
    // a long expansion cannot build an unbounded array in one call.
    let mut pressed = 0usize;
    for batch in events.chunks(100) {
        let mut inputs = Vec::with_capacity(batch.len() * 2);
        for event in batch {
            match event {
                KeyEvent::Unicode(unit) => {
                    inputs.push(unicode_input(*unit, false));
                    inputs.push(unicode_input(*unit, true));
                }
                KeyEvent::Key(virtual_key) => {
                    let down = keybd_input(*virtual_key, false);
                    let up = keybd_input(*virtual_key, true);
                    inputs.push(down);
                    inputs.push(up);
                }
            }
        }

        let sent = unsafe {
            SendInput(
                inputs.as_mut_slice(),
                std::mem::size_of::<INPUT>() as i32,
            )
        };
        pressed += sent as usize;
        if sent as usize != inputs.len() {
            return Err(format!(
                "the system accepted {pressed} of the key events and refused the rest"
            ));
        }
    }
    Ok(())
}

#[cfg(windows)]
fn unicode_input(unit: u16, release: bool) -> windows::Win32::UI::Input::KeyboardAndMouse::INPUT {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, VIRTUAL_KEY,
    };

    let mut flags = KEYEVENTF_UNICODE;
    if release {
        flags |= KEYEVENTF_KEYUP;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: unit,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0usize,
            },
        },
    }
}

#[cfg(windows)]
fn keybd_input(
    virtual_key: u16,
    release: bool,
) -> windows::Win32::UI::Input::KeyboardAndMouse::INPUT {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, VIRTUAL_KEY,
    };

    let flags = if release {
        KEYEVENTF_KEYUP
    } else {
        KEYBD_EVENT_FLAGS(0)
    };
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(virtual_key),
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0usize,
            },
        },
    }
}

/// Type the text into whatever window holds the keyboard focus.
#[cfg(not(windows))]
pub fn type_text(_text: &str) -> Result<(), String> {
    Err("text injection is implemented on Windows only".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn characters_become_one_event_per_utf16_unit() {
        // "ok" is two units; the £ sign is a single unit above the BMP
        // boundary's neighbours.
        assert_eq!(
            key_events("ok"),
            vec![KeyEvent::Unicode(b'o' as u16), KeyEvent::Unicode(b'k' as u16)]
        );
    }

    #[test]
    fn a_character_outside_the_bmp_takes_two_units() {
        // U+1F600, the grinning face, encodes as a surrogate pair.
        let events = key_events("\u{1F600}");
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], KeyEvent::Unicode(0xD83D)));
        assert!(matches!(events[1], KeyEvent::Unicode(0xDE00)));
    }

    #[test]
    fn line_breaks_become_real_enter_presses() {
        assert_eq!(
            key_events("a\nb"),
            vec![
                KeyEvent::Unicode(b'a' as u16),
                KeyEvent::Key(0x0D),
                KeyEvent::Unicode(b'b' as u16),
            ]
        );
    }
}
