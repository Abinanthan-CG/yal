//! `tc-palette` - YAL port of Tinycast's palette.
//!
//! The palette is the floating panel the whole app lives in: the launcher is
//! its root screen, and every other screen stacks on it. This crate holds the
//! decisions that make it work:
//!
//! - [`keys`] - the keys the palette answers to, and how upstream's Mac
//!   chords translate to Windows.
//! - [`screen`] - the screens, and how they stack.
//! - [`settings`] - the palette's own settings, with upstream's defaults.
//! - [`action`] - what a key does, and the menu that lists it.
//! - [`palette`] - the palette itself: the state machine that turns a key
//!   into an action.
//! - [`files`] - the file-search screen, and the files the launcher offers
//!   alongside its own rows.
//!
//! The decisions live in Rust rather than in the window, because a decision
//! that cannot be tested is not a decision. The window asks what a key does
//! and does what it is told.
//!
//! See `docs/ARCHITECTURE.md` for the crate map, and `docs/SHORTCUTS.md` for
//! the shortcut table this crate implements.

pub mod action;
pub mod files;
pub mod keys;
pub mod palette;
pub mod screen;
pub mod settings;

pub use action::{file_actions, palette_actions, ActionItem, ActionMenu, Direction, PaletteAction};
pub use files::{
    classify, extension_of, launcher_rows, matches, screen_rows, FileType, LAUNCHER_FILES_LIMIT,
    LAUNCHER_FILES_MIN_CHARS,
};
pub use keys::{translate, Key, MacChord, MacModifier, Modifiers, Stroke};
pub use palette::{CompactBar, EscapeOutcome, Fields, Palette};
pub use screen::{Screen, ScreenStack};
pub use settings::{
    Appearance, CompactShortcut, CompactShortcuts, EscapeBehaviour, Feature, Features,
    InterfaceSize, PaletteSettings, PopToRoot, ShortcutState, WindowMode, MAX_COMPACT_SHORTCUTS,
};
