//! The palette, drawn by the system rather than by a browser.
//!
//! `tc-ui` is the native surface: a borderless window, painted with Direct2D
//! and typed into with a real Windows edit control.

pub mod event;
pub mod hud;
pub mod model;
pub mod paint;
pub mod theme;
pub mod tokens;
pub mod window;

pub use event::{ignore, ActionKey, Handler, PaletteEvent, PaletteReply};
pub use hud::show_hud;
pub use model::{
    ActionMenuItem, ActionMenuModel, DetailPane, GridCell, GridModel, Icon, IconBitmap, IconKind,
    InlineArgBanner, Model, PaletteLayout, Row, ToastKind, ToastModel,
};
pub use theme::{Appearance, Color, Metrics, Theme, PALETTE};
pub use tokens::{TokenColor, TokenColors, TokenMetrics, Tokens};
pub use window::{
    is_visible, post_close, post_command, post_quit, post_refresh, post_reset_position, post_show,
    post_toggle_from_tray, Palette, PaletteHandle,
};
