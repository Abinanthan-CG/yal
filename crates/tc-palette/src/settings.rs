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

/// The keys the palette's own settings are stored under.
pub const KEY_APPEARANCE: &str = "palette.appearance";
/// The key the animations switch is stored under.
pub const KEY_ANIMATIONS: &str = "palette.animations";
/// The key the interface size is stored under.
pub const KEY_INTERFACE_SIZE: &str = "palette.interfaceSize";
/// The key the window mode is stored under.
pub const KEY_WINDOW_MODE: &str = "palette.windowMode";
/// The key the escape behaviour is stored under.
pub const KEY_ESCAPE_BEHAVIOUR: &str = "palette.escapeBehaviour";
/// The key the pop-to-root delay is stored under.
pub const KEY_POP_TO_ROOT: &str = "palette.popToRoot";

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

impl PaletteSettings {
    /// Set the appearance from its stored value.
    ///
    /// `false` when the value is not one of the three.
    pub fn set_appearance(&mut self, value: &str) -> bool {
        match value.to_lowercase().as_str() {
            "system" => self.appearance = Appearance::System,
            "light" => self.appearance = Appearance::Light,
            "dark" => self.appearance = Appearance::Dark,
            _ => return false,
        }
        true
    }

    /// Set the animations switch from its stored value.
    pub fn set_animations(&mut self, value: &str) -> bool {
        match value.to_lowercase().as_str() {
            "true" | "on" => self.animations = true,
            "false" | "off" => self.animations = false,
            _ => return false,
        }
        true
    }

    /// Set the interface size from its stored value.
    pub fn set_interface_size(&mut self, value: &str) -> bool {
        match value.to_lowercase().as_str() {
            "default" => self.interface_size = InterfaceSize::Default,
            "large" => self.interface_size = InterfaceSize::Large,
            "larger" => self.interface_size = InterfaceSize::Larger,
            _ => return false,
        }
        true
    }

    /// Set the window mode from its stored value.
    pub fn set_window_mode(&mut self, value: &str) -> bool {
        match value.to_lowercase().as_str() {
            "compact" => self.window_mode = WindowMode::Compact,
            "expanded" => self.window_mode = WindowMode::Expanded,
            _ => return false,
        }
        true
    }

    /// Set the escape behaviour from its stored value.
    ///
    /// The value may be the name or the label, with or without the spaces.
    pub fn set_escape_behaviour(&mut self, value: &str) -> bool {
        let normalised: String = value
            .to_lowercase()
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();
        match normalised.as_str() {
            "navigatebackorclosewindow" => {
                self.escape_behaviour = EscapeBehaviour::NavigateBackOrCloseWindow
            }
            "closewindowandpoptoroot" => {
                self.escape_behaviour = EscapeBehaviour::CloseWindowAndPopToRoot
            }
            _ => return false,
        }
        true
    }

    /// Set the pop-to-root delay from its stored value.
    ///
    /// The value is a number of seconds, and zero means straight back. The
    /// label, "30 seconds", is understood as well as the bare number.
    pub fn set_pop_to_root(&mut self, value: &str) -> bool {
        let trimmed = value.trim();
        let number = trimmed
            .strip_suffix(" seconds")
            .or_else(|| trimmed.strip_suffix(" second"))
            .unwrap_or(trimmed);
        let seconds: u32 = match number.to_lowercase().as_str() {
            "immediately" => 0,
            other => match other.parse() {
                Ok(seconds) => seconds,
                Err(_) => return false,
            },
        };
        self.pop_to_root = if seconds == 0 {
            PopToRoot::Immediately
        } else {
            PopToRoot::After(seconds)
        };
        true
    }

    /// The stored value of one setting.
    pub fn value_of(&self, key: &str) -> Option<String> {
        match key {
            KEY_APPEARANCE => Some(self.appearance.label().to_string()),
            KEY_ANIMATIONS => Some(self.animations.to_string()),
            KEY_INTERFACE_SIZE => Some(self.interface_size.label().to_string()),
            KEY_WINDOW_MODE => Some(self.window_mode.label().to_string()),
            KEY_ESCAPE_BEHAVIOUR => Some(self.escape_behaviour.label().to_string()),
            KEY_POP_TO_ROOT => Some(self.pop_to_root.label()),
            _ => None,
        }
    }

    /// The value a setting takes next.
    ///
    /// The switches turn on and off; the others go round their options.
    pub fn next_value(&self, key: &str) -> Option<String> {
        match key {
            KEY_ANIMATIONS => Some((!self.animations).to_string()),
            KEY_APPEARANCE => {
                let index = Appearance::OPTIONS
                    .iter()
                    .position(|option| *option == self.appearance)
                    .unwrap_or(0);
                Some(
                    Appearance::OPTIONS[(index + 1) % Appearance::OPTIONS.len()]
                        .label()
                        .to_string(),
                )
            }
            KEY_INTERFACE_SIZE => {
                let index = InterfaceSize::OPTIONS
                    .iter()
                    .position(|option| *option == self.interface_size)
                    .unwrap_or(0);
                Some(
                    InterfaceSize::OPTIONS[(index + 1) % InterfaceSize::OPTIONS.len()]
                        .label()
                        .to_string(),
                )
            }
            KEY_WINDOW_MODE => {
                let index = WindowMode::OPTIONS
                    .iter()
                    .position(|option| *option == self.window_mode)
                    .unwrap_or(0);
                Some(
                    WindowMode::OPTIONS[(index + 1) % WindowMode::OPTIONS.len()]
                        .label()
                        .to_string(),
                )
            }
            KEY_ESCAPE_BEHAVIOUR => {
                let index = EscapeBehaviour::OPTIONS
                    .iter()
                    .position(|option| *option == self.escape_behaviour)
                    .unwrap_or(0);
                Some(
                    EscapeBehaviour::OPTIONS[(index + 1) % EscapeBehaviour::OPTIONS.len()]
                        .label()
                        .to_string(),
                )
            }
            KEY_POP_TO_ROOT => {
                let index = PopToRoot::OPTIONS
                    .iter()
                    .position(|option| *option == self.pop_to_root)
                    .unwrap_or(0);
                Some(PopToRoot::OPTIONS[(index + 1) % PopToRoot::OPTIONS.len()].label())
            }
            _ => None,
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

    #[test]
    fn a_setting_is_read_back_from_its_stored_value() {
        let mut settings = PaletteSettings::default();
        assert!(settings.set_appearance("Dark"));
        assert_eq!(settings.appearance, Appearance::Dark);
        assert!(settings.set_window_mode("expanded"));
        assert_eq!(settings.window_mode, WindowMode::Expanded);
        assert!(settings.set_interface_size("larger"));
        assert_eq!(settings.interface_size, InterfaceSize::Larger);
        assert!(settings.set_animations("off"));
        assert!(!settings.animations);
    }

    #[test]
    fn a_value_that_is_not_one_of_the_options_is_refused() {
        let mut settings = PaletteSettings::default();
        assert!(!settings.set_appearance("Rainbow"));
        assert_eq!(settings.appearance, Appearance::System);
        assert!(!settings.set_window_mode("enormous"));
        assert_eq!(settings.window_mode, WindowMode::Compact);
        assert!(!settings.set_pop_to_root("whenever"));
        assert_eq!(settings.pop_to_root, PopToRoot::Immediately);
    }

    #[test]
    fn the_escape_behaviour_is_read_from_its_name_or_its_label() {
        let mut settings = PaletteSettings::default();
        assert!(settings.set_escape_behaviour("closewindowandpoptoroot"));
        assert_eq!(
            settings.escape_behaviour,
            EscapeBehaviour::CloseWindowAndPopToRoot
        );
        assert!(settings.set_escape_behaviour("Close window and pop to root"));
        assert_eq!(
            settings.escape_behaviour,
            EscapeBehaviour::CloseWindowAndPopToRoot
        );
        assert!(settings.set_escape_behaviour("Navigate back or close window"));
        assert_eq!(
            settings.escape_behaviour,
            EscapeBehaviour::NavigateBackOrCloseWindow
        );
    }

    #[test]
    fn the_pop_to_root_delay_is_read_from_its_number_or_its_label() {
        let mut settings = PaletteSettings::default();
        assert!(settings.set_pop_to_root("30"));
        assert_eq!(settings.pop_to_root, PopToRoot::After(30));
        assert!(settings.set_pop_to_root("30 seconds"));
        assert_eq!(settings.pop_to_root, PopToRoot::After(30));
        assert!(settings.set_pop_to_root("Immediately"));
        assert_eq!(settings.pop_to_root, PopToRoot::Immediately);
    }

    #[test]
    fn a_setting_moves_to_its_next_value_and_round_about() {
        let mut settings = PaletteSettings::default();
        let start = settings.appearance.label().to_string();
        for _ in 0..Appearance::OPTIONS.len() {
            let next = settings
                .next_value(KEY_APPEARANCE)
                .expect("the appearance has a next value");
            settings.set_appearance(&next);
        }
        assert_eq!(settings.appearance.label(), start);
    }

    #[test]
    fn the_animations_switch_turns_on_and_off() {
        let settings = PaletteSettings::default();
        let next = settings.next_value(KEY_ANIMATIONS).expect("a next value");
        assert_eq!(next, "false");
        let mut changed = settings.clone();
        assert!(changed.set_animations(&next));
        assert!(!changed.animations);
        let back = changed.next_value(KEY_ANIMATIONS).expect("a next value");
        assert_eq!(back, "true");
    }

    #[test]
    fn the_value_of_a_setting_is_its_label() {
        let settings = PaletteSettings::default();
        assert_eq!(
            settings.value_of(KEY_WINDOW_MODE).as_deref(),
            Some("Compact")
        );
        assert_eq!(
            settings.value_of(KEY_ESCAPE_BEHAVIOUR).as_deref(),
            Some("Navigate back or close window")
        );
        assert!(settings.value_of("palette.nothing").is_none());
    }

    #[test]
    fn a_setting_that_is_not_the_palettes_own_has_no_next_value() {
        let settings = PaletteSettings::default();
        assert!(settings.next_value("commands.enabled").is_none());
    }
}
