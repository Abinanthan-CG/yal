//! What the palette says, and what the app says back.

use crate::model::Model;
use crate::theme::Theme;

/// High-level screen action chords forwarded from the palette window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionKey {
    /// `Tab` — cycle between Launcher and Clipboard (or next screen).
    Tab,
    /// `Ctrl+P` — pin/unpin item (Clipboard/Emoji) or cycle filter (File Search).
    CtrlP,
    /// `Ctrl+K` — toggle quick actions panel / view.
    CtrlK,
    /// `Ctrl+Shift+Backspace` or `Ctrl+Delete` — remove selected item.
    CtrlDelete,
}

/// Something the palette tells the app.
#[derive(Clone, Debug, PartialEq)]
pub enum PaletteEvent {
    /// The palette is being shown.
    Shown,
    /// The query in the search field changed.
    QueryChanged(String),
    /// The user chose the item (list row or grid cell) at this index (`Enter` / click).
    Activated(usize),
    /// The user triggered the secondary action on this index (`Ctrl+Enter`).
    SecondaryActivated(usize),
    /// The user pressed an action chord (`Tab`, `Ctrl+P`, `Ctrl+K`, `Ctrl+Delete`)
    /// along with the currently selected row/cell index.
    Action(ActionKey, usize),
    /// The user pressed Escape with an empty search box.
    Escape,
    /// The user pressed `Ctrl+,` to open Settings.
    OpenSettings,
    /// The user dismissed the palette (blur or Escape at root).
    Dismissed,
    /// An application-defined command posted from another thread (e.g. tray or hotkey).
    Command(u32),
    /// Silent in-place refresh (e.g. background-loaded app icons became ready).
    Refresh,
}

/// What the app says back.
#[derive(Clone, Debug)]
pub enum PaletteReply {
    /// Nothing changes.
    Keep,
    /// Replace the palette's content and repaint (preserving current search box text).
    Update(Model),
    /// Navigate to another screen: clears the search box silently and applies the new model.
    Navigate(Model),
    /// Replace both the live theme (dark/light/style pack) and content immediately.
    UpdateTheme(Box<Theme>, Model),
    /// Hide the palette window.
    Close,
    /// Destroy the window and exit the message loop.
    Quit,
}

impl PaletteReply {
    /// The model the reply carries, if it carries one.
    pub fn model(&self) -> Option<&Model> {
        match self {
            PaletteReply::Keep | PaletteReply::Close | PaletteReply::Quit => None,
            PaletteReply::Update(model)
            | PaletteReply::Navigate(model)
            | PaletteReply::UpdateTheme(_, model) => Some(model),
        }
    }

    /// Whether the reply closes or quits the palette.
    pub fn closes(&self) -> bool {
        matches!(self, PaletteReply::Close | PaletteReply::Quit)
    }
}

/// What the app does when the palette says something.
pub type Handler = Box<dyn Fn(PaletteEvent) -> PaletteReply>;

/// A handler that does nothing.
pub fn ignore(_event: PaletteEvent) -> PaletteReply {
    PaletteReply::Keep
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model_with(query: &str) -> Model {
        Model {
            query: query.to_string(),
            ..Model::default()
        }
    }

    #[test]
    fn a_reply_that_changes_nothing_carries_no_model() {
        assert!(PaletteReply::Keep.model().is_none());
        assert!(!PaletteReply::Keep.closes());
    }

    #[test]
    fn a_reply_that_closes_carries_no_model() {
        assert!(PaletteReply::Close.model().is_none());
        assert!(PaletteReply::Close.closes());
    }

    #[test]
    fn a_reply_that_replaces_carries_the_new_content() {
        let reply = PaletteReply::Update(model_with("rust"));
        assert_eq!(
            reply.model().map(|model| model.query.as_str()),
            Some("rust")
        );
        assert!(!reply.closes());
    }

    #[test]
    fn the_quiet_handler_keeps_everything() {
        for event in [
            PaletteEvent::Shown,
            PaletteEvent::QueryChanged("rust".to_string()),
            PaletteEvent::Activated(0),
            PaletteEvent::SecondaryActivated(0),
            PaletteEvent::Action(ActionKey::Tab, 0),
            PaletteEvent::Escape,
            PaletteEvent::OpenSettings,
            PaletteEvent::Dismissed,
        ] {
            let reply = ignore(event);
            assert!(!reply.closes());
            assert!(reply.model().is_none());
        }
    }

    #[test]
    fn the_event_says_what_happened() {
        assert_eq!(
            PaletteEvent::QueryChanged("wi".to_string()),
            PaletteEvent::QueryChanged("wi".to_string())
        );
        assert_ne!(
            PaletteEvent::Activated(0),
            PaletteEvent::Activated(1),
            "two different rows are two different events"
        );
        assert_ne!(PaletteEvent::Dismissed, PaletteEvent::Activated(0));
        assert_ne!(PaletteEvent::Shown, PaletteEvent::Dismissed);
    }
}
