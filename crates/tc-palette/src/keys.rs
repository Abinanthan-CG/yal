//! The keys the palette answers to, and how upstream's chords translate.
//!
//! Upstream writes its shortcuts with the Mac modifier symbols, so the port
//! needs both: the chord as upstream means it, and the chord as Windows
//! delivers it. [`translate`] is the whole translation, and it is the only
//! place the rules live:
//!
//! | Upstream | YAL |
//! |---|---|
//! | <kbd>⌘</kbd> Command | <kbd>Ctrl</kbd> |
//! | <kbd>⌥</kbd> Option | <kbd>Alt</kbd> |
//! | <kbd>⇧</kbd> Shift | <kbd>Shift</kbd> |
//! | <kbd>⌃</kbd> Control | <kbd>Alt</kbd> |
//! | <kbd>⌘</kbd>+<kbd>⌃</kbd> | <kbd>Ctrl</kbd>+<kbd>Alt</kbd>+<kbd>Shift</kbd> |
//!
//! Three chords are not a plain swap, and they are listed in
//! `docs/SHORTCUTS.md`: <kbd>⌘</kbd><kbd>delete</kbd>,
//! <kbd>⌥</kbd><kbd>tab</kbd> and <kbd>⌘</kbd><kbd>esc</kbd>.
//!
//! **A built-in shortcut must never shadow standard text editing.** The
//! tests hold that line: <kbd>Ctrl</kbd>+<kbd>C</kbd>,
//! <kbd>Ctrl</kbd>+<kbd>V</kbd>, <kbd>Ctrl</kbd>+<kbd>X</kbd> and
//! <kbd>Ctrl</kbd>+<kbd>Z</kbd> keep their meaning in every text field.

/// A key on the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    /// A printable character.
    Character(char),
    /// <kbd>Enter</kbd>
    Enter,
    /// <kbd>Esc</kbd>
    Escape,
    /// <kbd>Tab</kbd>
    Tab,
    /// The key a Mac keyboard labels <kbd>delete</kbd>. It is a backspace.
    Delete,
    /// The <kbd>Backspace</kbd> key of a PC keyboard.
    Backspace,
    /// <kbd>Space</kbd>
    Space,
    /// <kbd>Home</kbd>
    Home,
    /// <kbd>End</kbd>
    End,
    /// <kbd>Page Up</kbd>
    PageUp,
    /// <kbd>Page Down</kbd>
    PageDown,
    /// <kbd>↑</kbd>
    Up,
    /// <kbd>↓</kbd>
    Down,
    /// <kbd>←</kbd>
    Left,
    /// <kbd>→</kbd>
    Right,
    /// A function key, <kbd>F1</kbd> to <kbd>F20</kbd>.
    Function(u8),
}

/// The modifier keys held down with a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Modifiers {
    /// <kbd>Ctrl</kbd>
    pub ctrl: bool,
    /// <kbd>Alt</kbd>
    pub alt: bool,
    /// <kbd>Shift</kbd>
    pub shift: bool,
    /// The Windows key. YAL never reserves it.
    pub win: bool,
}

impl Modifiers {
    /// No modifier at all.
    pub const NONE: Modifiers = Modifiers {
        ctrl: false,
        alt: false,
        shift: false,
        win: false,
    };

    /// <kbd>Ctrl</kbd> alone.
    pub const CTRL: Modifiers = Modifiers {
        ctrl: true,
        ..Modifiers::NONE
    };

    /// <kbd>Alt</kbd> alone.
    pub const ALT: Modifiers = Modifiers {
        alt: true,
        ..Modifiers::NONE
    };

    /// <kbd>Shift</kbd> alone.
    pub const SHIFT: Modifiers = Modifiers {
        shift: true,
        ..Modifiers::NONE
    };

    /// Whether no modifier is held.
    pub fn is_empty(&self) -> bool {
        *self == Modifiers::NONE
    }

    /// The name of the modifier combination, for a shortcut shown to the user.
    pub fn label(&self) -> String {
        let mut parts: Vec<&str> = Vec::new();
        if self.ctrl {
            parts.push("Ctrl");
        }
        if self.alt {
            parts.push("Alt");
        }
        if self.shift {
            parts.push("Shift");
        }
        if self.win {
            parts.push("Win");
        }
        parts.join("+")
    }
}

/// A key with the modifiers it was pressed with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Stroke {
    /// The key.
    pub key: Key,
    /// The modifiers held down with it.
    pub modifiers: Modifiers,
}

impl Stroke {
    /// A key with no modifier.
    pub const fn plain(key: Key) -> Stroke {
        Stroke {
            key,
            modifiers: Modifiers::NONE,
        }
    }

    /// A key with <kbd>Ctrl</kbd>.
    pub const fn ctrl(key: Key) -> Stroke {
        Stroke {
            key,
            modifiers: Modifiers::CTRL,
        }
    }

    /// A key with <kbd>Alt</kbd>.
    pub const fn alt(key: Key) -> Stroke {
        Stroke {
            key,
            modifiers: Modifiers::ALT,
        }
    }

    /// A key with <kbd>Ctrl</kbd> and <kbd>Shift</kbd>.
    pub const fn ctrl_shift(key: Key) -> Stroke {
        Stroke {
            key,
            modifiers: Modifiers {
                ctrl: true,
                shift: true,
                ..Modifiers::NONE
            },
        }
    }

    /// A key with <kbd>Shift</kbd> alone.
    pub const fn shift(key: Key) -> Stroke {
        Stroke {
            key,
            modifiers: Modifiers::SHIFT,
        }
    }

    /// A key with <kbd>Ctrl</kbd>, <kbd>Alt</kbd> and <kbd>Shift</kbd>.
    pub const fn ctrl_alt_shift(key: Key) -> Stroke {
        Stroke {
            key,
            modifiers: Modifiers {
                ctrl: true,
                alt: true,
                shift: true,
                ..Modifiers::NONE
            },
        }
    }

    /// A key with <kbd>Ctrl</kbd> and <kbd>Alt</kbd>.
    pub const fn ctrl_alt(key: Key) -> Stroke {
        Stroke {
            key,
            modifiers: Modifiers {
                ctrl: true,
                alt: true,
                ..Modifiers::NONE
            },
        }
    }

    /// The name of the chord, for a shortcut shown to the user.
    pub fn label(&self) -> String {
        let name = match self.key {
            Key::Character(character) => character.to_string(),
            Key::Enter => "Enter".to_string(),
            Key::Escape => "Esc".to_string(),
            Key::Tab => "Tab".to_string(),
            Key::Delete => "Delete".to_string(),
            Key::Backspace => "Backspace".to_string(),
            Key::Space => "Space".to_string(),
            Key::Home => "Home".to_string(),
            Key::End => "End".to_string(),
            Key::PageUp => "PageUp".to_string(),
            Key::PageDown => "PageDown".to_string(),
            Key::Up => "↑".to_string(),
            Key::Down => "↓".to_string(),
            Key::Left => "←".to_string(),
            Key::Right => "→".to_string(),
            Key::Function(number) => format!("F{number}"),
        };
        let modifiers = self.modifiers.label();
        if modifiers.is_empty() {
            name
        } else {
            format!("{modifiers}+{name}")
        }
    }
}

/// A modifier as upstream's documentation writes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MacModifier {
    /// <kbd>⌘</kbd> Command
    Command,
    /// <kbd>⌥</kbd> Option
    Option,
    /// <kbd>⇧</kbd> Shift
    Shift,
    /// <kbd>⌃</kbd> Control
    Control,
}

/// A chord as upstream's documentation writes it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MacChord {
    /// The key.
    pub key: Key,
    /// The modifiers, in the order upstream lists them.
    pub modifiers: Vec<MacModifier>,
}

impl MacChord {
    /// A chord with the given modifiers.
    pub fn new(key: Key, modifiers: &[MacModifier]) -> MacChord {
        MacChord {
            key,
            modifiers: modifiers.to_vec(),
        }
    }

    /// The chord with no modifier.
    pub fn plain(key: Key) -> MacChord {
        MacChord::new(key, &[])
    }

    /// Whether the chord holds the modifier.
    pub fn has(&self, modifier: MacModifier) -> bool {
        self.modifiers.contains(&modifier)
    }
}

/// The chords that are not a plain swap of the modifiers.
///
/// They are here because they are the ones a reader is most likely to get
/// wrong, and because each one has a reason.
pub fn translate(chord: &MacChord) -> Vec<Stroke> {
    // ⌘delete is ⌘ and the Mac's backspace key. Ctrl+Delete is "delete the
    // word to the right" in a text field, and that has to keep working.
    if chord.has(MacModifier::Command) && chord.key == Key::Delete {
        return vec![Stroke::ctrl_shift(Key::Backspace)];
    }
    // Alt+Tab is the Windows system shortcut for switching windows. It is
    // never intercepted.
    if chord.has(MacModifier::Option) && chord.key == Key::Tab {
        return vec![Stroke::ctrl(Key::Tab)];
    }
    // Ctrl+Esc opens the Windows Start menu. YAL only takes it while the
    // palette has the focus, and Alt+Home is the second way to the same
    // place.
    if chord.has(MacModifier::Command) && chord.key == Key::Escape {
        return vec![Stroke::ctrl(Key::Escape), Stroke::alt(Key::Home)];
    }

    let command = chord.has(MacModifier::Command);
    let option = chord.has(MacModifier::Option);
    let shift = chord.has(MacModifier::Shift);
    let control = chord.has(MacModifier::Control);

    let modifiers = if command && control {
        // ⌘ and ⌃ together would both want to be a modifier Windows already
        // uses, so the whole chord takes all three.
        Modifiers {
            ctrl: true,
            alt: true,
            shift: true,
            win: false,
        }
    } else {
        Modifiers {
            ctrl: command,
            alt: option || control,
            shift,
            win: false,
        }
    };

    // The Mac's delete key is a backspace.
    let key = match chord.key {
        Key::Delete => Key::Backspace,
        other => other,
    };
    vec![Stroke { key, modifiers }]
}

/// The chords that mean copy, paste, cut and undo.
///
/// YAL never takes these over: they work in every text field of the palette.
pub const EDITING_CHORDS: [Stroke; 4] = [
    Stroke::ctrl(Key::Character('c')),
    Stroke::ctrl(Key::Character('v')),
    Stroke::ctrl(Key::Character('x')),
    Stroke::ctrl(Key::Character('z')),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn mac(key: Key, modifiers: &[MacModifier]) -> MacChord {
        MacChord::new(key, modifiers)
    }

    #[test]
    fn a_command_chord_becomes_a_control_chord() {
        let strokes = translate(&mac(Key::Character('k'), &[MacModifier::Command]));
        assert_eq!(strokes, vec![Stroke::ctrl(Key::Character('k'))]);
    }

    #[test]
    fn an_option_chord_becomes_an_alt_chord() {
        let strokes = translate(&mac(Key::Character('p'), &[MacModifier::Option]));
        assert_eq!(strokes, vec![Stroke::alt(Key::Character('p'))]);
    }

    #[test]
    fn a_control_chord_becomes_an_alt_chord() {
        let strokes = translate(&mac(Key::Character('n'), &[MacModifier::Control]));
        assert_eq!(strokes, vec![Stroke::alt(Key::Character('n'))]);
    }

    #[test]
    fn a_shift_chord_stays_a_shift_chord() {
        let strokes = translate(&mac(
            Key::Character('c'),
            &[MacModifier::Command, MacModifier::Shift],
        ));
        assert_eq!(
            strokes,
            vec![Stroke {
                key: Key::Character('c'),
                modifiers: Modifiers {
                    ctrl: true,
                    shift: true,
                    ..Modifiers::NONE
                },
            }]
        );
    }

    #[test]
    fn command_and_control_together_take_all_three_modifiers() {
        let strokes = translate(&mac(
            Key::Character('c'),
            &[MacModifier::Command, MacModifier::Control],
        ));
        assert_eq!(
            strokes,
            vec![Stroke {
                key: Key::Character('c'),
                modifiers: Modifiers {
                    ctrl: true,
                    alt: true,
                    shift: true,
                    win: false,
                },
            }]
        );
    }

    #[test]
    fn the_option_and_control_chords_stay_apart() {
        let name = translate(&mac(
            Key::Character('c'),
            &[MacModifier::Option, MacModifier::Command],
        ));
        let path = translate(&mac(
            Key::Character('c'),
            &[MacModifier::Control, MacModifier::Command],
        ));
        assert_ne!(name, path, "copy name and copy path are different chords");
    }

    #[test]
    fn the_command_delete_chord_keeps_the_text_editing_one_working() {
        let strokes = translate(&mac(Key::Delete, &[MacModifier::Command]));
        assert_eq!(strokes, vec![Stroke::ctrl_shift(Key::Backspace)]);
    }

    #[test]
    fn the_option_tab_chord_does_not_take_the_windows_window_switcher() {
        let strokes = translate(&mac(Key::Tab, &[MacModifier::Option]));
        assert_eq!(strokes, vec![Stroke::ctrl(Key::Tab)]);
    }

    #[test]
    fn the_command_escape_chord_has_a_second_way_to_the_same_place() {
        let strokes = translate(&mac(Key::Escape, &[MacModifier::Command]));
        assert_eq!(
            strokes,
            vec![Stroke::ctrl(Key::Escape), Stroke::alt(Key::Home)]
        );
    }

    #[test]
    fn the_mac_delete_key_is_a_backspace() {
        let strokes = translate(&mac(Key::Delete, &[]));
        assert_eq!(strokes, vec![Stroke::plain(Key::Backspace)]);
    }

    /// The whole table in `docs/SHORTCUTS.md`, in one place, so the test
    /// below holds it still.
    fn upstream_chords() -> Vec<MacChord> {
        vec![
            mac(Key::Enter, &[]),
            mac(Key::Enter, &[MacModifier::Command]),
            mac(Key::Character('k'), &[MacModifier::Command]),
            mac(Key::Up, &[]),
            mac(Key::Down, &[]),
            mac(Key::Character('n'), &[MacModifier::Control]),
            mac(Key::Character('p'), &[MacModifier::Control]),
            mac(Key::Character('f'), &[MacModifier::Control]),
            mac(Key::Character('b'), &[MacModifier::Control]),
            mac(Key::Tab, &[]),
            mac(Key::Escape, &[]),
            mac(Key::Escape, &[MacModifier::Command]),
            mac(Key::Delete, &[]),
            mac(Key::Character(','), &[MacModifier::Command]),
            mac(Key::Character('w'), &[MacModifier::Command]),
            mac(Key::Character('p'), &[MacModifier::Command]),
            mac(
                Key::Character('c'),
                &[MacModifier::Command, MacModifier::Shift],
            ),
            mac(
                Key::Character('v'),
                &[MacModifier::Command, MacModifier::Shift],
            ),
            mac(
                Key::Character('c'),
                &[MacModifier::Option, MacModifier::Command],
            ),
            mac(
                Key::Character('c'),
                &[MacModifier::Control, MacModifier::Command],
            ),
            mac(
                Key::Character('f'),
                &[MacModifier::Command, MacModifier::Shift],
            ),
            mac(Key::Character('x'), &[MacModifier::Control]),
            mac(Key::Delete, &[MacModifier::Command]),
            mac(Key::Tab, &[MacModifier::Option]),
        ]
    }

    #[test]
    fn every_upstream_chord_becomes_a_distinct_windows_chord() {
        let mut seen: Vec<Stroke> = Vec::new();
        for chord in upstream_chords() {
            for stroke in translate(&chord) {
                assert!(
                    !seen.contains(&stroke),
                    "two upstream chords translate to the same Windows chord: {}",
                    stroke.label()
                );
                seen.push(stroke);
            }
        }
    }

    #[test]
    fn no_built_in_shortcut_shadows_the_standard_text_editing() {
        for chord in upstream_chords() {
            for stroke in translate(&chord) {
                assert!(
                    !EDITING_CHORDS.contains(&stroke),
                    "{} would shadow a standard text editing chord",
                    stroke.label()
                );
            }
        }
    }

    #[test]
    fn the_windows_key_is_never_reserved() {
        for chord in upstream_chords() {
            for stroke in translate(&chord) {
                assert!(!stroke.modifiers.win, "the Windows key stays free");
            }
        }
    }

    #[test]
    fn a_chord_is_named_the_way_it_is_shown() {
        assert_eq!(Stroke::ctrl(Key::Character('k')).label(), "Ctrl+k");
        assert_eq!(
            Stroke {
                key: Key::Backspace,
                modifiers: Modifiers {
                    ctrl: true,
                    shift: true,
                    ..Modifiers::NONE
                },
            }
            .label(),
            "Ctrl+Shift+Backspace"
        );
        assert_eq!(Stroke::plain(Key::Up).label(), "↑");
    }
}
