//! What the palette says, and what the app says back.
//!
//! A palette that cannot be driven is a picture of a palette. The window
//! itself knows about the keyboard and the mouse, but it does not know what
//! to *search* for and what to *do* with the row the user chose - those are
//! the app's business.
//!
//! So the palette reports what happened, and the app answers. The answer
//! carries the new content, because the app is the one that knows what the
//! content should be: the palette does not search, it shows.

use crate::model::Model;

/// Something the palette tells the app.
#[derive(Clone, Debug, PartialEq)]
pub enum PaletteEvent {
    /// The palette is being shown.
    ///
    /// The palette starts afresh every time it is shown, so this is when the
    /// app puts its suggestions back.
    Shown,
    /// The query in the search field changed.
    ///
    /// The palette sends the whole query, not just the character that was
    /// typed, so the app never has to keep the field's text itself. The hint
    /// the field shows while it is empty is not a query, and is not sent.
    QueryChanged(String),
    /// The user chose the row at this position in the list.
    ///
    /// The position, not the row: the app built the list, so the app knows
    /// what is at each position, and a position cannot go stale between the
    /// keystroke and the answer.
    Activated(usize),
    /// The user dismissed the palette.
    Dismissed,
}

/// What the app says back.
#[derive(Clone, Debug)]
pub enum PaletteReply {
    /// Nothing changes.
    Keep,
    /// The palette's content is this now.
    ///
    /// The app searches, so the app builds the model; the palette shows it.
    Update(Model),
    /// The palette should close.
    Close,
}

impl PaletteReply {
    /// The model the reply carries, if it carries one.
    pub fn model(&self) -> Option<&Model> {
        match self {
            PaletteReply::Keep | PaletteReply::Close => None,
            PaletteReply::Update(model) => Some(model),
        }
    }

    /// Whether the reply closes the palette.
    pub fn closes(&self) -> bool {
        matches!(self, PaletteReply::Close)
    }
}

/// What the app does when the palette says something.
pub type Handler = Box<dyn Fn(PaletteEvent) -> PaletteReply>;

/// A handler that does nothing.
///
/// The palette is useful on its own - the probe opens one with fixed rows -
/// and a window with no handler still shows, still searches its own text and
/// still takes the arrow keys.
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
