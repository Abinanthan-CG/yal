//! What the palette says, and what the app says back.

use crate::model::Model;
use crate::theme::Theme;

/// High-level screen action chords forwarded from the palette window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionKey {
    /// `Tab` — enter inline argument pill, drill into folder, or cycle screen.
    Tab,
    /// `Shift+Tab` — navigate folder up or step backward out of inline arguments.
    ShiftTab,
    /// `Ctrl+P` — pin/unpin item (Launcher/Clipboard/Emoji) or cycle filter (File Search).
    CtrlP,
    /// `Ctrl+K` — toggle the floating Action Panel popover.
    CtrlK,
    /// `Ctrl+I` — toggle the Master-Detail Info/Preview pane.
    CtrlI,
    /// `Ctrl+H` — toggle hidden files in File Search.
    CtrlH,
    /// `Ctrl+R` — refresh live rates or re-index files.
    CtrlR,
    /// `Alt+Left` or `Ctrl+[` — navigate back one screen.
    Back,
    /// `Ctrl+Shift+Backspace` or `Ctrl+Delete` — remove/recycle selected item.
    CtrlDelete,
    /// `Down` while in Compact Mode — expand the compact search bar to show Suggestions.
    Expand,
}

/// Something the palette tells the app.
#[derive(Clone, Debug, PartialEq)]
pub enum PaletteEvent {
    /// The palette is being shown.
    Shown,
    /// The query in the search field changed.
    QueryChanged(String),
    /// Selection moved to a different row/cell (lets split-view detail panes stay live).
    SelectionChanged(usize),
    /// The user chose the item at this index (`Enter` / click).
    Activated(usize),
    /// Secondary action (`Ctrl+Enter`).
    SecondaryActivated(usize),
    /// Tertiary action (`Alt+Enter` — e.g. Run as Admin / File Properties).
    TertiaryActivated(usize),
    /// Quaternary action (`Shift+Enter` — e.g. Show in Explorer).
    QuaternaryActivated(usize),
    /// Quick-select shortcut (`Ctrl+1`..`Ctrl+9`, 0-based index `0..8`).
    QuickSelect(usize),
    /// Copy chord (`Alt+C` — Copy Path / Copy Unformatted).
    CopyChord(usize),
    /// The user pressed an action chord along with the currently selected index.
    Action(ActionKey, usize),
    /// The user activated an entry inside the floating `Ctrl+K` Action Panel popover.
    ActionMenuActivated(String),
    /// The user dismissed the floating `Ctrl+K` Action Panel popover.
    ActionMenuDismissed,
    /// The user pressed Escape with an empty search box (or clicked Header Back `←`).
    Escape,
    /// The user pressed `Ctrl+,` to open Settings.
    OpenSettings,
    /// The user dismissed the palette (blur or Escape at root).
    Dismissed,
    /// An application-defined command posted from another thread (e.g. tray or hotkey).
    Command(u32),
    /// Silent in-place refresh (e.g. background icons/files/shell output became ready).
    Refresh,
}

/// What the app says back.
#[derive(Clone, Debug)]
pub enum PaletteReply {
    /// Nothing changes.
    Keep,
    /// Replace the palette's content and repaint (preserving current search box text).
    Update(Model),
    /// Replace both the search box text (caret at end) and the model cleanly.
    UpdateWithEdit(String, Model),
    /// Navigate to another screen: clears the search box silently and applies the new model.
    Navigate(Model),
    /// Replace both the live theme (dark/light/style pack) and content immediately.
    UpdateTheme(Box<Theme>, Model),
    /// Hide the palette window.
    Close,
    /// Hide the palette window and show a brief floating HUD pill (e.g. "Copied to Clipboard").
    CloseWithHud(String),
    /// Destroy the window and exit the message loop.
    Quit,
}

impl PaletteReply {
    /// The model the reply carries, if it carries one.
    pub fn model(&self) -> Option<&Model> {
        match self {
            PaletteReply::Keep
            | PaletteReply::Close
            | PaletteReply::CloseWithHud(_)
            | PaletteReply::Quit => None,
            PaletteReply::Update(model)
            | PaletteReply::UpdateWithEdit(_, model)
            | PaletteReply::Navigate(model)
            | PaletteReply::UpdateTheme(_, model) => Some(model),
        }
    }

    /// Whether the reply closes or quits the palette.
    pub fn closes(&self) -> bool {
        matches!(
            self,
            PaletteReply::Close | PaletteReply::CloseWithHud(_) | PaletteReply::Quit
        )
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
        assert!(PaletteReply::CloseWithHud("Copied".to_string()).closes());
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
            PaletteEvent::SelectionChanged(0),
            PaletteEvent::Activated(0),
            PaletteEvent::SecondaryActivated(0),
            PaletteEvent::TertiaryActivated(0),
            PaletteEvent::QuaternaryActivated(0),
            PaletteEvent::QuickSelect(0),
            PaletteEvent::CopyChord(0),
            PaletteEvent::Action(ActionKey::Tab, 0),
            PaletteEvent::ActionMenuActivated("open".to_string()),
            PaletteEvent::ActionMenuDismissed,
            PaletteEvent::Escape,
            PaletteEvent::OpenSettings,
            PaletteEvent::Dismissed,
        ] {
            let reply = ignore(event);
            assert!(!reply.closes());
            assert!(reply.model().is_none());
        }
    }
}
