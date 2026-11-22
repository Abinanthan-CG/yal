//! The palette, drawn by the system rather than by a browser.
//!
//! `tc-ui` is the native surface: a borderless window, painted with Direct2D
//! and typed into with a real Windows edit control. It exists because the
//! browser engine a web view brings costs more memory than the whole rest of
//! YAL put together; see `docs/port-notes/024-the-weight-of-the-app.md`.

pub mod event;
pub mod model;
pub mod paint;
pub mod theme;
pub mod tokens;
pub mod window;

pub use event::{ignore, Handler, PaletteEvent, PaletteReply};
pub use model::{Icon, Model, Row};
pub use theme::{Appearance, Color, Metrics, Theme, PALETTE};
pub use tokens::{TokenColor, TokenColors, TokenMetrics, Tokens};
pub use window::{
    is_visible, post_close, post_command, post_quit, post_show, post_toggle_from_tray, Palette,
    PaletteHandle,
};
