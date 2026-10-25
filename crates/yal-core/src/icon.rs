//! Item icons.
//!
//! The observed launcher renders a per-item icon that can be an emoji, a file on disk, or a
//! named symbol. The observed frontend carries both `.svg` symbol assets and
//! `.png` raster assets, so the model keeps the distinction between "symbol
//! drawn by the app" and "image loaded from disk".

use std::fmt;
use std::path::PathBuf;

/// The visual identity attached to an item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Icon {
    /// A single emoji character, e.g. `"📁"`.
    Emoji(String),

    /// A named symbol the shell knows how to draw, e.g. `"gear"`.
    Symbol(String),

    /// An image file on disk.
    Image(PathBuf),

    /// The application that owns this item, resolved by the shell at render
    /// time from the installed-application index.
    Application(String),
}

impl Icon {
    /// A symbol icon.
    pub fn symbol(name: impl Into<String>) -> Self {
        Icon::Symbol(name.into())
    }

    /// An image icon.
    pub fn image(path: impl Into<PathBuf>) -> Self {
        Icon::Image(path.into())
    }

    /// Whether this icon needs a filesystem read before it can be drawn.
    pub fn requires_file_read(&self) -> bool {
        matches!(self, Icon::Image(_))
    }
}

impl fmt::Display for Icon {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Icon::Emoji(s) => write!(f, "{s}"),
            Icon::Symbol(s) => write!(f, ":{s}:"),
            Icon::Image(p) => write!(f, "{}", p.display()),
            Icon::Application(s) => write!(f, "@{s}"),
        }
    }
}
