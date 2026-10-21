//! Searchable items and their accessories.

use crate::action::Action;
use crate::icon::Icon;

/// Stable identity of an item within its owning command.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ItemId(pub String);

impl ItemId {
    pub fn new(id: impl Into<String>) -> Self {
        ItemId(id.into())
    }
}

impl std::fmt::Display for ItemId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Secondary text shown on the right of an item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Accessory {
    /// Plain text, e.g. a file path or a shortcut key.
    Text(String),

    /// A named symbol, e.g. `"arrow_right"`.
    Symbol(String),

    /// An emoji.
    Emoji(String),
}

impl Accessory {
    pub fn text(value: impl Into<String>) -> Self {
        Accessory::Text(value.into())
    }

    pub fn symbol(name: impl Into<String>) -> Self {
        Accessory::Symbol(name.into())
    }
}

impl std::fmt::Display for Accessory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Accessory::Text(s) => write!(f, "{s}"),
            Accessory::Symbol(s) => write!(f, ":{s}:"),
            Accessory::Emoji(s) => write!(f, "{s}"),
        }
    }
}

/// One candidate result inside a command.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    /// Identity within the owning command.
    pub id: ItemId,
    /// Primary text.
    pub title: String,
    /// Secondary text under the title.
    pub subtitle: Option<String>,
    /// Extra search terms that are not part of the visible title.
    pub keywords: Vec<String>,
    /// Right-hand accessories.
    pub accessories: Vec<Accessory>,
    /// Icon shown to the left.
    pub icon: Option<Icon>,
    /// What to do when the item is chosen.
    pub action: Option<Action>,
    /// Opaque payload carried through to the action executor.
    ///
    /// The domain layer does not interpret this. It exists so an item can
    /// carry structured data to a provider without the core knowing that
    /// provider's schema.
    pub payload: Option<serde_json::Value>,
}

impl Item {
    /// A minimal item with only a title.
    pub fn new(title: impl Into<String>) -> Self {
        let title = title.into();
        Self {
            id: ItemId::new(title.clone()),
            title,
            subtitle: None,
            keywords: Vec::new(),
            accessories: Vec::new(),
            icon: None,
            action: None,
            payload: None,
        }
    }

    /// Set the stable identity.
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = ItemId::new(id);
        self
    }

    /// Set the subtitle.
    pub fn with_subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// Add a search keyword.
    pub fn with_keyword(mut self, keyword: impl Into<String>) -> Self {
        self.keywords.push(keyword.into());
        self
    }

    /// Add an accessory.
    pub fn with_accessory(mut self, accessory: Accessory) -> Self {
        self.accessories.push(accessory);
        self
    }

    /// Set the icon.
    pub fn with_icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Set the action.
    pub fn with_action(mut self, action: Action) -> Self {
        self.action = Some(action);
        self
    }

    /// Set the opaque payload.
    pub fn with_payload(mut self, payload: serde_json::Value) -> Self {
        self.payload = Some(payload);
        self
    }

    /// Every string this item can be matched against.
    ///
    /// The title is matched with its own weighting; keywords are matched
    /// separately so that a hidden alias can never outrank a visible title.
    pub fn search_terms(&self) -> impl Iterator<Item = &str> {
        self.keywords.iter().map(String::as_str)
    }
}
