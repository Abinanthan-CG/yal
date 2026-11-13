//! The palette's own settings.
//!
//! Upstream keeps these in *Settings → General*, and the defaults are
//! upstream's defaults:
//!
//! | Setting | Options | Default |
//! |---|---|---|
//! | Appearance | System · Light · Dark | **System** |
//! | Show animations | On · Off | **On** |
//! | Interface size | Default · Large · Larger | **Default** |
//! | Window mode | Compact · Expanded | **Compact** |
//! | Escape Key Behavior | Navigate back or close window · Close window and pop to root | **Navigate back or close window** |
//! | Pop to Root Search | Immediately · 5 · 15 · 30 · 60 · 90 seconds | **Immediately** |
//! | Follow the cursor across displays | On · Off | **On** |
//! | Drag to move the launcher | On · Off | **Off** |
//! | Show in menu bar | On · Off | **On** |

/// How the palette is painted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Appearance {
    /// Follow Windows, and change when Windows does.
    #[default]
    System,
    /// The light design.
    Light,
    /// The dark design.
    Dark,
}

impl Appearance {
    /// The three options, in the order they are offered.
    pub const OPTIONS: [Appearance; 3] = [Appearance::System, Appearance::Light, Appearance::Dark];

    /// The name the setting is offered under.
    pub fn label(&self) -> &'static str {
        match self {
            Appearance::System => "System",
            Appearance::Light => "Light",
            Appearance::Dark => "Dark",
        }
    }
}

/// How big the palette and its floating windows are.
///
/// "Interface size makes the palette and its floating windows 10% or 20%
/// larger. The Settings window stays the same size."
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum InterfaceSize {
    /// The size the design was drawn at.
    #[default]
    Default,
    /// Ten percent larger.
    Large,
    /// Twenty percent larger.
    Larger,
}

impl InterfaceSize {
    /// The three options, in the order they are offered.
    pub const OPTIONS: [InterfaceSize; 3] = [
        InterfaceSize::Default,
        InterfaceSize::Large,
        InterfaceSize::Larger,
    ];

    /// The name the setting is offered under.
    pub fn label(&self) -> &'static str {
        match self {
            InterfaceSize::Default => "Default",
            InterfaceSize::Large => "Large",
            InterfaceSize::Larger => "Larger",
        }
    }

    /// The factor the interface is drawn at.
    pub fn scale(&self) -> f64 {
        match self {
            InterfaceSize::Default => 1.0,
            InterfaceSize::Large => 1.1,
            InterfaceSize::Larger => 1.2,
        }
    }
}

/// How the palette opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum WindowMode {
    /// A slim search bar that expands into the full list as you type.
    #[default]
    Compact,
    /// Always the full list.
    Expanded,
}

impl WindowMode {
    /// The two options, in the order they are offered.
    pub const OPTIONS: [WindowMode; 2] = [WindowMode::Compact, WindowMode::Expanded];

    /// The name the setting is offered under.
    pub fn label(&self) -> &'static str {
        match self {
            WindowMode::Compact => "Compact",
            WindowMode::Expanded => "Expanded",
        }
    }
}

/// What <kbd>esc</kbd> does when the search is empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum EscapeBehaviour {
    /// Go back a screen, and close only at the root.
    #[default]
    NavigateBackOrCloseWindow,
    /// Close right away, and start at the launcher next time.
    CloseWindowAndPopToRoot,
}

impl EscapeBehaviour {
    /// The two options, in the order they are offered.
    pub const OPTIONS: [EscapeBehaviour; 2] = [
        EscapeBehaviour::NavigateBackOrCloseWindow,
        EscapeBehaviour::CloseWindowAndPopToRoot,
    ];

    /// The name the setting is offered under.
    pub fn label(&self) -> &'static str {
        match self {
            EscapeBehaviour::NavigateBackOrCloseWindow => "Navigate back or close window",
            EscapeBehaviour::CloseWindowAndPopToRoot => "Close window and pop to root",
        }
    }
}

/// How long after the window closes the palette comes back to the launcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PopToRoot {
    /// Come straight back.
    #[default]
    Immediately,
    /// Come back after this many seconds.
    After(u32),
}

impl PopToRoot {
    /// The options, in the order they are offered.
    pub const OPTIONS: [PopToRoot; 6] = [
        PopToRoot::Immediately,
        PopToRoot::After(5),
        PopToRoot::After(15),
        PopToRoot::After(30),
        PopToRoot::After(60),
        PopToRoot::After(90),
    ];

    /// The name the setting is offered under.
    pub fn label(&self) -> String {
        match self {
            PopToRoot::Immediately => "Immediately".to_string(),
            PopToRoot::After(seconds) => format!("{seconds} seconds"),
        }
    }

    /// How many seconds the palette waits, and zero for straight back.
    pub fn seconds(&self) -> u32 {
        match self {
            PopToRoot::Immediately => 0,
            PopToRoot::After(seconds) => *seconds,
        }
    }
}

/// A feature that the palette can be asked about.
///
/// The compact shortcuts are listed even when the feature behind them is off,
/// so a shortcut that is switched off keeps its place and comes back when
/// the feature is turned on again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Feature {
    /// The clipboard history.
    Clipboard,
    /// The emoji grid.
    Emoji,
    /// The file search.
    FileSearch,
    /// The quicklinks.
    Quicklinks,
    /// The snippets.
    Snippets,
    /// The notes.
    Notes,
    /// The schedule.
    Schedule,
    /// The AI chat.
    Ai,
}

/// Which features are on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Features {
    /// The clipboard history.
    pub clipboard: bool,
    /// The emoji grid.
    pub emoji: bool,
    /// The file search.
    pub file_search: bool,
    /// The quicklinks.
    pub quicklinks: bool,
    /// The snippets.
    pub snippets: bool,
    /// The notes.
    pub notes: bool,
    /// The schedule.
    pub schedule: bool,
    /// The AI chat.
    pub ai: bool,
}

impl Default for Features {
    fn default() -> Self {
        Features {
            clipboard: true,
            emoji: true,
            file_search: false,
            quicklinks: true,
            snippets: true,
            notes: true,
            schedule: false,
            ai: false,
        }
    }
}

impl Features {
    /// Whether the feature is on.
    pub fn is_on(&self, feature: Feature) -> bool {
        match feature {
            Feature::Clipboard => self.clipboard,
            Feature::Emoji => self.emoji,
            Feature::FileSearch => self.file_search,
            Feature::Quicklinks => self.quicklinks,
            Feature::Snippets => self.snippets,
            Feature::Notes => self.notes,
            Feature::Schedule => self.schedule,
            Feature::Ai => self.ai,
        }
    }
}

/// One of the four slots in the slim bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompactShortcut {
    /// The clipboard history.
    ClipboardHistory,
    /// The emoji grid.
    EmojiAndSymbols,
    /// The file search.
    SearchFiles,
    /// The quicklink search.
    SearchQuicklinks,
    /// The snippet search.
    SearchSnippets,
    /// The notes.
    ShowNotes,
    /// The schedule.
    MySchedule,
    /// The AI chat.
    QuickAi,
}

impl CompactShortcut {
    /// The options a shortcut can be set to.
    pub const OPTIONS: [CompactShortcut; 8] = [
        CompactShortcut::ClipboardHistory,
        CompactShortcut::EmojiAndSymbols,
        CompactShortcut::SearchFiles,
        CompactShortcut::SearchQuicklinks,
        CompactShortcut::SearchSnippets,
        CompactShortcut::ShowNotes,
        CompactShortcut::MySchedule,
        CompactShortcut::QuickAi,
    ];

    /// The name the shortcut is offered under.
    pub fn label(&self) -> &'static str {
        match self {
            CompactShortcut::ClipboardHistory => "Clipboard History",
            CompactShortcut::EmojiAndSymbols => "Emoji & Symbols",
            CompactShortcut::SearchFiles => "Search Files",
            CompactShortcut::SearchQuicklinks => "Search Quicklinks",
            CompactShortcut::SearchSnippets => "Search Snippets",
            CompactShortcut::ShowNotes => "Show Notes",
            CompactShortcut::MySchedule => "My Schedule",
            CompactShortcut::QuickAi => "Quick AI",
        }
    }

    /// The feature the shortcut opens.
    pub fn feature(&self) -> Feature {
        match self {
            CompactShortcut::ClipboardHistory => Feature::Clipboard,
            CompactShortcut::EmojiAndSymbols => Feature::Emoji,
            CompactShortcut::SearchFiles => Feature::FileSearch,
            CompactShortcut::SearchQuicklinks => Feature::Quicklinks,
            CompactShortcut::SearchSnippets => Feature::Snippets,
            CompactShortcut::ShowNotes => Feature::Notes,
            CompactShortcut::MySchedule => Feature::Schedule,
            CompactShortcut::QuickAi => Feature::Ai,
        }
    }
}

/// Whether a shortcut is usable right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShortcutState {
    /// The feature behind it is on.
    On,
    /// The feature behind it is off. The shortcut keeps its place.
    Off,
}

/// The shortcuts in the slim bar.
///
/// "Add, change, remove and reorder up to four shortcuts, or remove them all
/// for a search-only bar."
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactShortcuts {
    items: Vec<CompactShortcut>,
}

/// The compact bar holds four shortcuts at most.
pub const MAX_COMPACT_SHORTCUTS: usize = 4;

/// What is wrong with a list of shortcuts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShortcutsError {
    /// More than four shortcuts were given.
    TooMany,
    /// The same shortcut was given twice.
    Repeated(CompactShortcut),
}

impl std::fmt::Display for ShortcutsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShortcutsError::TooMany => {
                write!(formatter, "there is room for four shortcuts, not more")
            }
            ShortcutsError::Repeated(shortcut) => {
                write!(formatter, "`{}` is already in the list", shortcut.label())
            }
        }
    }
}

impl std::error::Error for ShortcutsError {}

impl Default for CompactShortcuts {
    fn default() -> Self {
        CompactShortcuts::defaults()
    }
}

impl CompactShortcuts {
    /// The four shortcuts upstream ships with.
    pub fn defaults() -> CompactShortcuts {
        CompactShortcuts {
            items: vec![
                CompactShortcut::ClipboardHistory,
                CompactShortcut::EmojiAndSymbols,
                CompactShortcut::SearchFiles,
                CompactShortcut::SearchQuicklinks,
            ],
        }
    }

    /// The shortcuts, in the order they are shown.
    pub fn items(&self) -> &[CompactShortcut] {
        &self.items
    }

    /// Set the whole list.
    pub fn set(&mut self, items: Vec<CompactShortcut>) -> Result<(), ShortcutsError> {
        if items.len() > MAX_COMPACT_SHORTCUTS {
            return Err(ShortcutsError::TooMany);
        }
        for item in &items {
            if items.iter().filter(|other| *other == item).count() > 1 {
                return Err(ShortcutsError::Repeated(*item));
            }
        }
        self.items = items;
        Ok(())
    }

    /// Add a shortcut at the end of the list.
    pub fn add(&mut self, item: CompactShortcut) -> Result<(), ShortcutsError> {
        if self.items.contains(&item) {
            return Err(ShortcutsError::Repeated(item));
        }
        if self.items.len() >= MAX_COMPACT_SHORTCUTS {
            return Err(ShortcutsError::TooMany);
        }
        self.items.push(item);
        Ok(())
    }

    /// Take a shortcut out of the list.
    pub fn remove(&mut self, item: CompactShortcut) {
        self.items.retain(|other| *other != item);
    }

    /// Move a shortcut one place towards the front.
    pub fn move_up(&mut self, item: CompactShortcut) {
        if let Some(index) = self.items.iter().position(|other| *other == item) {
            if index > 0 {
                self.items.swap(index, index - 1);
            }
        }
    }

    /// Move a shortcut one place towards the back.
    pub fn move_down(&mut self, item: CompactShortcut) {
        if let Some(index) = self.items.iter().position(|other| *other == item) {
            if index + 1 < self.items.len() {
                self.items.swap(index, index + 1);
            }
        }
    }

    /// Put the four default shortcuts back.
    pub fn restore_defaults(&mut self) {
        self.items = CompactShortcuts::defaults().items;
    }

    /// Whether the shortcut is in the list.
    pub fn contains(&self, item: CompactShortcut) -> bool {
        self.items.contains(&item)
    }

    /// Whether the shortcut can be used right now.
    ///
    /// "A disabled feature's shortcut stays configured and marked Off in
    /// Settings, then reappears in its place when enabled again."
    pub fn state(&self, item: CompactShortcut, features: &Features) -> ShortcutState {
        if features.is_on(item.feature()) {
            ShortcutState::On
        } else {
            ShortcutState::Off
        }
    }
}

/// The palette's settings, with upstream's defaults.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteSettings {
    /// How the palette is painted.
    pub appearance: Appearance,
    /// Whether windows open, close and change with motion.
    pub animations: bool,
    /// How big the palette is.
    pub interface_size: InterfaceSize,
    /// How the palette opens.
    pub window_mode: WindowMode,
    /// What <kbd>esc</kbd> does when the search is empty.
    pub escape_behaviour: EscapeBehaviour,
    /// How long after the window closes the palette comes back.
    pub pop_to_root: PopToRoot,
    /// The shortcuts in the slim bar.
    pub compact_shortcuts: CompactShortcuts,
    /// Whether the palette opens on the display the pointer is on.
    pub follow_cursor: bool,
    /// Whether the palette can be dragged to another place.
    pub drag_to_move: bool,
    /// Whether the notification-area icon is shown.
    pub show_in_tray: bool,
}

impl Default for PaletteSettings {
    fn default() -> Self {
        PaletteSettings {
            appearance: Appearance::System,
            animations: true,
            interface_size: InterfaceSize::Default,
            window_mode: WindowMode::Compact,
            escape_behaviour: EscapeBehaviour::NavigateBackOrCloseWindow,
            pop_to_root: PopToRoot::Immediately,
            compact_shortcuts: CompactShortcuts::defaults(),
            follow_cursor: true,
            drag_to_move: false,
            show_in_tray: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_upstreams_defaults() {
        let settings = PaletteSettings::default();
        assert_eq!(settings.appearance, Appearance::System);
        assert!(settings.animations);
        assert_eq!(settings.interface_size, InterfaceSize::Default);
        assert_eq!(settings.window_mode, WindowMode::Compact);
        assert_eq!(
            settings.escape_behaviour,
            EscapeBehaviour::NavigateBackOrCloseWindow
        );
        assert_eq!(settings.pop_to_root, PopToRoot::Immediately);
        assert!(settings.follow_cursor);
        assert!(!settings.drag_to_move);
        assert!(settings.show_in_tray);
    }

    #[test]
    fn the_interface_size_is_ten_and_twenty_percent_larger() {
        assert_eq!(InterfaceSize::Default.scale(), 1.0);
        assert_eq!(InterfaceSize::Large.scale(), 1.1);
        assert_eq!(InterfaceSize::Larger.scale(), 1.2);
    }

    #[test]
    fn the_pop_to_root_delay_offers_the_five_documented_choices() {
        let delays: Vec<u32> = PopToRoot::OPTIONS
            .iter()
            .map(|option| option.seconds())
            .collect();
        assert_eq!(delays, vec![0, 5, 15, 30, 60, 90]);
    }

    #[test]
    fn the_four_default_shortcuts_are_the_ones_upstream_ships() {
        let shortcuts = CompactShortcuts::defaults();
        assert_eq!(
            shortcuts.items(),
            &[
                CompactShortcut::ClipboardHistory,
                CompactShortcut::EmojiAndSymbols,
                CompactShortcut::SearchFiles,
                CompactShortcut::SearchQuicklinks,
            ]
        );
    }

    #[test]
    fn a_fifth_shortcut_is_refused() {
        let mut shortcuts = CompactShortcuts::defaults();
        assert_eq!(
            shortcuts.add(CompactShortcut::SearchSnippets),
            Err(ShortcutsError::TooMany)
        );
        assert_eq!(shortcuts.items().len(), MAX_COMPACT_SHORTCUTS);
    }

    #[test]
    fn the_same_shortcut_cannot_be_twice() {
        let mut shortcuts = CompactShortcuts::defaults();
        assert_eq!(
            shortcuts.add(CompactShortcut::ClipboardHistory),
            Err(ShortcutsError::Repeated(CompactShortcut::ClipboardHistory))
        );
    }

    #[test]
    fn every_shortcut_can_be_removed_for_a_search_only_bar() {
        let mut shortcuts = CompactShortcuts::defaults();
        for item in shortcuts.items().to_vec() {
            shortcuts.remove(item);
        }
        assert!(shortcuts.items().is_empty());
    }

    #[test]
    fn the_defaults_can_be_brought_back() {
        let mut shortcuts = CompactShortcuts::defaults();
        shortcuts.remove(CompactShortcut::ClipboardHistory);
        shortcuts.restore_defaults();
        assert!(shortcuts.contains(CompactShortcut::ClipboardHistory));
        assert_eq!(shortcuts.items().len(), MAX_COMPACT_SHORTCUTS);
    }

    #[test]
    fn the_shortcuts_can_be_reordered() {
        let mut shortcuts = CompactShortcuts::defaults();
        shortcuts.move_down(CompactShortcut::ClipboardHistory);
        assert_eq!(shortcuts.items()[0], CompactShortcut::EmojiAndSymbols);
        assert_eq!(shortcuts.items()[1], CompactShortcut::ClipboardHistory);
        shortcuts.move_up(CompactShortcut::ClipboardHistory);
        assert_eq!(shortcuts.items()[0], CompactShortcut::ClipboardHistory);
    }

    #[test]
    fn a_moved_shortcut_stops_at_the_end() {
        let mut shortcuts = CompactShortcuts::defaults();
        shortcuts.move_up(CompactShortcut::ClipboardHistory);
        assert_eq!(
            shortcuts.items()[0],
            CompactShortcut::ClipboardHistory,
            "already at the front"
        );
    }

    #[test]
    fn the_shortcut_of_a_switched_off_feature_stays_and_is_marked_off() {
        let shortcuts = CompactShortcuts::defaults();
        let features = Features {
            file_search: false,
            ..Features::default()
        };
        assert!(shortcuts.contains(CompactShortcut::SearchFiles));
        assert_eq!(
            shortcuts.state(CompactShortcut::SearchFiles, &features),
            ShortcutState::Off
        );
        assert_eq!(
            shortcuts.state(CompactShortcut::ClipboardHistory, &features),
            ShortcutState::On
        );
    }
}
