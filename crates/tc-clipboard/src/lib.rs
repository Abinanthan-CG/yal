//! `tc-clipboard` - the clipboard model layer of the YAL port.
//!
//! This crate holds the parts of the clipboard feature that are pure: what a
//! clipboard entry is, how entries are classified, filtered and searched, how
//! colours are parsed and rewritten, how a paste sequence walks the history,
//! and the rules the history store follows.
//!
//! Nothing here talks to Windows. The clipboard text arrives as an argument,
//! so every rule in this crate is testable without a desktop and without
//! touching the user's real `%APPDATA%`.
//!
//! The upstream authority for everything here is the macOS app Tinycast
//! (<https://github.com/abue-ammar/tinycast>, AGPL-3.0): the Swift files in
//! `Features/Clipboard/Model` and the tests in `Tests/clipboard-*-test.swift`.

pub mod color;
pub mod color_spaces;
pub mod file_kind;
pub mod filter;
pub mod image_blob;
pub mod item;
pub mod paste_sequence;
pub mod policy;
pub mod raycast;
pub mod store;
#[cfg(test)]
mod tests;

pub use color::{ColorFormat, ColorValue, Hsl};
pub use color_spaces::{Oklab, Oklch};
pub use file_kind::ClipboardFileKind;
pub use filter::ClipboardFilter;
pub use image_blob::{decode_png_rgba, encode_png_rgba};
pub use item::{ClipboardItem, EntryId, ItemKind, TextForm, Timestamp};
pub use paste_sequence::PasteSequence;
pub use policy::CapturePolicy;
pub use raycast::RaycastImport;
pub use store::{
    ClipboardChord, ClipboardDefaultAction, ClipboardRetention, ClipboardStore, StoreError,
    UpdateError,
};
