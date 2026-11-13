//! The palette itself: which screen is on top, what is typed, and what a key
//! does about it.
//!
//! The decisions live here rather than in the window, because a decision that
//! cannot be tested is not a decision. The window asks what a key does and
//! does what it is told.
//!
//! The rules, all from upstream's *palette* page:
//!
//! - <kbd>esc</kbd> clears the search, then goes back a screen, then closes.
//! - <kbd>⌘</kbd><kbd>esc</kbd> goes back to the root search from any screen.
//! - <kbd>tab</kbd> cycles the launcher, the AI and the clipboard, skipping
//!   the ones whose feature is off.
//! - <kbd>⌃</kbd><kbd>N</kbd> and <kbd>⌃</kbd><kbd>P</kbd> are
//!   <kbd>↓</kbd> and <kbd>↑</kbd>. <kbd>⌃</kbd><kbd>F</kbd> and
//!   <kbd>⌃</kbd><kbd>B</kbd> move the text cursor, except on the emoji grid,
//!   where all four move the selection.
//! - A row that takes arguments gets the <kbd>tab</kbd> first: the tab moves
//!   through its fields.

use crate::action::{ActionMenu, Direction, PaletteAction};
use crate::files::FileType;
use crate::keys::{Key, Stroke};
use crate::screen::{Screen, ScreenStack};
use crate::settings::{EscapeBehaviour, Features, PaletteSettings, WindowMode};

/// What the escape key did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscapeOutcome {
    /// The text in the search field was cleared.
    ClearedText,
    /// The palette collapsed into its slim bar.
    Collapsed,
    /// The palette went back one screen.
    WentBack,
    /// The palette closed.
    Closed,
}

/// What the slim bar of the compact mode is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompactBar {
    /// The search field.
    Search,
    /// One of the shortcuts, by its place in the list.
    Shortcut(usize),
}

/// The inline fields of a row that takes arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fields {
    /// How many fields there are.
    pub count: usize,
    /// Which field has the focus.
    pub focused: usize,
}

impl Fields {
    /// The fields of a row, with the first one focused.
    pub fn new(count: usize) -> Fields {
        Fields { count, focused: 0 }
    }
}

/// The palette: its screens, its text, and what the keys do.
#[derive(Debug, Clone)]
pub struct Palette {
    /// The screens, with the launcher at the bottom.
    stack: ScreenStack,
    /// The settings the palette follows.
    settings: PaletteSettings,
    /// Which features are on.
    features: Features,
    /// The text in the search field.
    query: String,
    /// The actions menu, when it is open.
    menu: Option<ActionMenu>,
    /// The fields of the selected row, when they are open.
    fields: Option<Fields>,
    /// How many fields the selected row takes, even before they are open.
    row_fields: usize,
    /// Whether the list of the compact mode is showing.
    expanded: bool,
    /// What the slim bar is showing.
    compact: CompactBar,
    /// The kind of file the file search is narrowed to.
    file_filter: FileType,
    /// What the last escape key did.
    last_escape: Option<EscapeOutcome>,
}

impl Default for Palette {
    fn default() -> Self {
        Palette::new()
    }
}

impl Palette {
    /// A palette at its root, with upstream's settings.
    pub fn new() -> Palette {
        Palette {
            stack: ScreenStack::new(),
            settings: PaletteSettings::default(),
            features: Features::default(),
            query: String::new(),
            menu: None,
            fields: None,
            row_fields: 0,
            expanded: false,
            compact: CompactBar::Search,
            file_filter: FileType::All,
            last_escape: None,
        }
    }

    /// What the last escape key did.
    ///
    /// The escape key can clear the text, collapse the palette, go back a
    /// screen and close the window, and the window has to know which of
    /// those happened.
    pub fn escape_outcome(&self) -> Option<EscapeOutcome> {
        self.last_escape
    }

    /// The settings the palette follows.
    pub fn settings(&self) -> &PaletteSettings {
        &self.settings
    }

    /// The settings the palette follows.
    pub fn set_settings(&mut self, settings: PaletteSettings) {
        self.settings = settings;
    }

    /// Which features are on.
    pub fn features(&self) -> &Features {
        &self.features
    }

    /// Which features are on.
    pub fn set_features(&mut self, features: Features) {
        self.features = features;
    }

    /// The screen on top.
    pub fn screen(&self) -> Screen {
        self.stack.current()
    }

    /// The screens, with the launcher at the bottom.
    pub fn stack(&self) -> &ScreenStack {
        &self.stack
    }

    /// The text in the search field.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// The actions menu, when it is open.
    pub fn menu(&self) -> Option<&ActionMenu> {
        self.menu.as_ref()
    }

    /// Type in the actions menu's search field.
    ///
    /// The menu has to be open for this to do anything.
    pub fn set_menu_query(&mut self, query: &str) {
        if let Some(menu) = self.menu.as_mut() {
            menu.set_query(query);
        }
    }

    /// The fields of the selected row, when they are open.
    pub fn fields(&self) -> Option<&Fields> {
        self.fields.as_ref()
    }

    /// What the slim bar is showing.
    pub fn compact_bar(&self) -> CompactBar {
        self.compact
    }

    /// The kind of file the file search is narrowed to.
    pub fn file_filter(&self) -> FileType {
        self.file_filter
    }

    /// Whether the list is showing.
    ///
    /// The expanded mode always shows it; the compact mode shows it once the
    /// user has typed or asked for it.
    pub fn is_expanded(&self) -> bool {
        match self.settings.window_mode {
            WindowMode::Expanded => true,
            WindowMode::Compact => self.expanded,
        }
    }

    /// Type in the search field.
    ///
    /// "Typing from any shortcut starts a search", so the bar goes back to
    /// the search field and the list comes out.
    pub fn set_query(&mut self, text: &str) {
        self.query = text.to_string();
        if !text.is_empty() {
            self.compact = CompactBar::Search;
            self.expanded = true;
        }
    }

    /// Tell the palette how many fields the selected row takes.
    ///
    /// Zero means the row takes none.
    pub fn set_row_fields(&mut self, count: usize) {
        self.row_fields = count;
    }

    /// Open a screen by name, on top of the launcher.
    pub fn open_screen(&mut self, screen: Screen) {
        self.stack.push(screen);
    }

    /// Open a screen the way its own global shortcut does.
    pub fn open_global(&mut self, screen: Screen, palette_was_visible: bool) {
        self.stack.open_global(screen, palette_was_visible);
    }

    /// Go back one screen. `false` when there is nothing behind it.
    pub fn back(&mut self) -> bool {
        self.stack.pop().is_some()
    }

    /// Go back to the root search from any screen.
    pub fn back_to_root(&mut self) {
        self.stack.pop_to_root();
    }

    /// Narrow the file search to another kind of file.
    pub fn cycle_file_filter(&mut self) -> FileType {
        self.file_filter = self.file_filter.next();
        self.file_filter
    }

    /// Narrow the file search to one kind of file.
    pub fn set_file_filter(&mut self, filter: FileType) {
        self.file_filter = filter;
    }

    /// Start the palette again.
    ///
    /// "It resets each time you open the palette."
    pub fn reset(&mut self) {
        self.stack.pop_to_root();
        self.query.clear();
        self.menu = None;
        self.fields = None;
        self.row_fields = 0;
        self.expanded = false;
        self.compact = CompactBar::Search;
        self.file_filter = FileType::All;
    }

    /// The screens of the Tab cycle, with the ones whose feature is off left
    /// out.
    pub fn cycle(&self) -> Vec<Screen> {
        let mut cycle = vec![Screen::Launcher];
        if self.features.ai {
            cycle.push(Screen::QuickAi);
        }
        if self.features.clipboard {
            cycle.push(Screen::Clipboard);
        }
        cycle
    }

    /// The screen the Tab key goes to.
    ///
    /// "Other screens, like Calculator History or Search Quicklinks, aren't
    /// part of the cycle. On screens that don't use tab themselves, it
    /// returns to the launcher with your search text."
    pub fn tab_target(&self) -> Screen {
        let cycle = self.cycle();
        let current = self.stack.current();
        match cycle.iter().position(|screen| *screen == current) {
            Some(index) => cycle[(index + 1) % cycle.len()],
            None => Screen::Launcher,
        }
    }

    /// Go to the next screen of the cycle.
    pub fn tab(&mut self) -> Screen {
        let target = self.tab_target();
        if target.is_root() {
            self.stack.pop_to_root();
        } else {
            self.stack.pop();
            self.stack.push(target);
        }
        target
    }

    /// Move the slim bar's selection.
    ///
    /// "In the slim bar, ← and → select Search or a shortcut, stopping at
    /// either end."
    pub fn move_compact_selection(&mut self, delta: i8) {
        let count = self.settings.compact_shortcuts.items().len();
        let current = match self.compact {
            CompactBar::Search => 0i32,
            CompactBar::Shortcut(index) => 1 + index as i32,
        };
        let last = count as i32;
        let next = (current + i32::from(delta)).clamp(0, last);
        self.compact = if next == 0 {
            CompactBar::Search
        } else {
            CompactBar::Shortcut((next - 1) as usize)
        };
    }

    /// The shortcut the slim bar is showing.
    pub fn compact_shortcut(&self) -> Option<crate::settings::CompactShortcut> {
        match self.compact {
            CompactBar::Search => None,
            CompactBar::Shortcut(index) => {
                self.settings.compact_shortcuts.items().get(index).copied()
            }
        }
    }

    /// What the escape key does.
    ///
    /// "In both cases, the first press clears any text you typed." Then the
    /// compact mode collapses the palette, and only then does the setting
    /// decide between going back and closing.
    pub fn escape(&mut self) -> EscapeOutcome {
        if !self.query.is_empty() {
            self.query.clear();
            return EscapeOutcome::ClearedText;
        }
        if self.settings.window_mode == WindowMode::Compact
            && self.expanded
            && self.stack.current() == Screen::Launcher
        {
            self.expanded = false;
            return EscapeOutcome::Collapsed;
        }
        match self.settings.escape_behaviour {
            EscapeBehaviour::NavigateBackOrCloseWindow => {
                if self.stack.pop().is_some() {
                    EscapeOutcome::WentBack
                } else {
                    EscapeOutcome::Closed
                }
            }
            EscapeBehaviour::CloseWindowAndPopToRoot => {
                self.stack.pop_to_root();
                EscapeOutcome::Closed
            }
        }
    }

    /// What a key does.
    ///
    /// `None` means the key belongs to the text field and the palette leaves
    /// it alone.
    pub fn key(&mut self, stroke: Stroke) -> Option<PaletteAction> {
        self.last_escape = None;
        if self.menu.is_some() {
            return self.key_in_menu(stroke);
        }
        if self.fields.is_some() {
            return self.key_in_fields(stroke);
        }
        if self.settings.window_mode == WindowMode::Compact
            && !self.expanded
            && self.stack.current() == Screen::Launcher
        {
            if let Some(action) = self.key_in_compact_bar(stroke) {
                return Some(action);
            }
        }
        self.key_in_screen(stroke)
    }

    /// The keys the actions menu takes.
    fn key_in_menu(&mut self, stroke: Stroke) -> Option<PaletteAction> {
        let mut close = false;
        let action = {
            let menu = self.menu.as_mut().expect("the menu is open");
            if stroke == Stroke::plain(Key::Escape) {
                if menu.query.is_empty() {
                    close = true;
                } else {
                    // "esc clears the menu search first, then closes the menu."
                    menu.query.clear();
                }
                Some(PaletteAction::Escape)
            } else if stroke == Stroke::ctrl(Key::Character('k')) {
                close = true;
                Some(PaletteAction::ActionsMenu)
            } else if stroke == Stroke::plain(Key::Up) {
                menu.move_up();
                Some(PaletteAction::MoveSelection(-1))
            } else if stroke == Stroke::plain(Key::Down) {
                menu.move_down();
                Some(PaletteAction::MoveSelection(1))
            } else if stroke == Stroke::plain(Key::Enter) {
                let chosen = menu.selected_action();
                if chosen.is_some() {
                    close = true;
                }
                chosen
            } else {
                None
            }
        };
        if close {
            self.menu = None;
        }
        action
    }

    /// The keys a row's fields take.
    fn key_in_fields(&mut self, stroke: Stroke) -> Option<PaletteAction> {
        let mut leave = false;
        let action = {
            let fields = self.fields.as_mut().expect("the fields are open");
            let count = fields.count.max(1);
            if stroke == Stroke::plain(Key::Tab) {
                fields.focused = (fields.focused + 1) % count;
                Some(PaletteAction::NextField)
            } else if stroke == Stroke::shift(Key::Tab) {
                fields.focused = (fields.focused + count - 1) % count;
                Some(PaletteAction::PreviousField)
            } else if stroke == Stroke::plain(Key::Enter) {
                leave = true;
                Some(PaletteAction::MainAction)
            } else if stroke == Stroke::plain(Key::Escape) {
                leave = true;
                Some(PaletteAction::Back)
            } else {
                None
            }
        };
        if leave {
            self.fields = None;
        }
        action
    }

    /// The keys the slim bar takes.
    fn key_in_compact_bar(&mut self, stroke: Stroke) -> Option<PaletteAction> {
        let down = stroke == Stroke::plain(Key::Down) || stroke == Stroke::alt(Key::Character('n'));
        if stroke == Stroke::plain(Key::Left) {
            self.move_compact_selection(-1);
            return Some(PaletteAction::MoveCompactSelection(-1));
        }
        if stroke == Stroke::plain(Key::Right) {
            self.move_compact_selection(1);
            return Some(PaletteAction::MoveCompactSelection(1));
        }
        if down || stroke == Stroke::plain(Key::Enter) {
            return Some(match self.compact {
                CompactBar::Search => {
                    // "↓ on Search expands the list and selects its first row."
                    self.expanded = true;
                    PaletteAction::ExpandList
                }
                CompactBar::Shortcut(_) => PaletteAction::OpenCompactItem,
            });
        }
        None
    }

    /// The keys a screen takes.
    fn key_in_screen(&mut self, stroke: Stroke) -> Option<PaletteAction> {
        let grid = self.stack.current() == Screen::Emoji;

        if stroke == Stroke::plain(Key::Enter) {
            return Some(PaletteAction::MainAction);
        }
        if stroke == Stroke::ctrl(Key::Enter) {
            return Some(PaletteAction::SecondAction);
        }
        if stroke == Stroke::ctrl(Key::Character('k')) {
            self.menu = Some(self.actions_menu());
            return Some(PaletteAction::ActionsMenu);
        }
        // The two ways to the root search: Ctrl+Esc, and Alt+Home.
        if stroke == Stroke::ctrl(Key::Escape) || stroke == Stroke::alt(Key::Home) {
            self.stack.pop_to_root();
            return Some(PaletteAction::BackToRoot);
        }
        if stroke == Stroke::ctrl(Key::Character(',')) {
            self.stack.push(Screen::Settings);
            return Some(PaletteAction::Settings);
        }
        if stroke == Stroke::ctrl(Key::Character('w')) {
            return Some(PaletteAction::CloseWindow);
        }
        if stroke == Stroke::plain(Key::Tab) {
            // "There's one exception: when the selected row takes arguments,
            // tab moves through its fields first."
            if self.row_fields > 0 {
                self.fields = Some(Fields::new(self.row_fields));
                return Some(PaletteAction::NextField);
            }
            self.tab();
            return Some(PaletteAction::NextScreen);
        }
        if stroke == Stroke::plain(Key::Escape) {
            self.last_escape = Some(self.escape());
            return Some(PaletteAction::Escape);
        }
        // "delete: in an empty search field, go back one screen."
        if stroke == Stroke::plain(Key::Backspace) && self.query.is_empty() {
            self.stack.pop();
            return Some(PaletteAction::Back);
        }
        if stroke == Stroke::plain(Key::Up) {
            return Some(PaletteAction::MoveSelection(-1));
        }
        if stroke == Stroke::plain(Key::Down) {
            return Some(PaletteAction::MoveSelection(1));
        }
        // The emacs chords: ⌃N is ↓ and ⌃P is ↑.
        if stroke == Stroke::alt(Key::Character('n')) {
            return Some(PaletteAction::MoveSelection(1));
        }
        if stroke == Stroke::alt(Key::Character('p')) {
            return Some(PaletteAction::MoveSelection(-1));
        }
        // "On the emoji grid, all four move the selection. Everywhere else,
        // ⌃F and ⌃B move the text cursor."
        if stroke == Stroke::alt(Key::Character('f')) {
            return Some(if grid {
                PaletteAction::MoveGrid(Direction::Right)
            } else {
                PaletteAction::MoveCursor(1)
            });
        }
        if stroke == Stroke::alt(Key::Character('b')) {
            return Some(if grid {
                PaletteAction::MoveGrid(Direction::Left)
            } else {
                PaletteAction::MoveCursor(-1)
            });
        }
        if grid {
            if stroke == Stroke::plain(Key::Left) {
                return Some(PaletteAction::MoveGrid(Direction::Left));
            }
            if stroke == Stroke::plain(Key::Right) {
                return Some(PaletteAction::MoveGrid(Direction::Right));
            }
        }
        if self.stack.current() == Screen::FileSearch && stroke == Stroke::ctrl(Key::Character('p'))
        {
            self.cycle_file_filter();
            return Some(PaletteAction::CycleFileFilter);
        }
        None
    }

    /// The actions menu of the screen on top.
    pub fn actions_menu(&self) -> ActionMenu {
        match self.stack.current() {
            Screen::FileSearch => ActionMenu::files(),
            _ => ActionMenu::palette(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{CompactShortcut, EscapeBehaviour, PaletteSettings};

    /// The general key table applies once the list is showing, which is the
    /// case as soon as the user has typed anything.
    fn palette() -> Palette {
        let mut palette = Palette::new();
        palette.set_settings(PaletteSettings {
            window_mode: WindowMode::Expanded,
            ..PaletteSettings::default()
        });
        palette
    }

    /// The palette as it opens: the slim bar of the compact mode.
    fn compact_palette() -> Palette {
        Palette::new()
    }

    #[test]
    fn the_palette_starts_at_the_launcher() {
        let palette = palette();
        assert_eq!(palette.screen(), Screen::Launcher);
        assert_eq!(palette.query(), "");
    }

    #[test]
    fn enter_is_the_main_action() {
        let mut palette = palette();
        assert_eq!(
            palette.key(Stroke::plain(Key::Enter)),
            Some(PaletteAction::MainAction)
        );
    }

    #[test]
    fn control_and_enter_is_the_second_action() {
        let mut palette = palette();
        assert_eq!(
            palette.key(Stroke::ctrl(Key::Enter)),
            Some(PaletteAction::SecondAction)
        );
    }

    #[test]
    fn control_and_k_opens_the_actions_menu() {
        let mut palette = palette();
        assert_eq!(
            palette.key(Stroke::ctrl(Key::Character('k'))),
            Some(PaletteAction::ActionsMenu)
        );
        assert!(palette.menu().is_some());
    }

    #[test]
    fn the_arrows_move_the_selection() {
        let mut palette = palette();
        assert_eq!(
            palette.key(Stroke::plain(Key::Down)),
            Some(PaletteAction::MoveSelection(1))
        );
        assert_eq!(
            palette.key(Stroke::plain(Key::Up)),
            Some(PaletteAction::MoveSelection(-1))
        );
    }

    #[test]
    fn the_emacs_chords_are_the_arrows() {
        let mut palette = palette();
        assert_eq!(
            palette.key(Stroke::alt(Key::Character('n'))),
            Some(PaletteAction::MoveSelection(1)),
            "⌃N is ↓"
        );
        assert_eq!(
            palette.key(Stroke::alt(Key::Character('p'))),
            Some(PaletteAction::MoveSelection(-1)),
            "⌃P is ↑"
        );
    }

    #[test]
    fn the_emacs_chords_move_the_text_cursor_everywhere_but_the_grid() {
        let mut palette = palette();
        assert_eq!(
            palette.key(Stroke::alt(Key::Character('f'))),
            Some(PaletteAction::MoveCursor(1)),
            "⌃F moves the cursor"
        );
        assert_eq!(
            palette.key(Stroke::alt(Key::Character('b'))),
            Some(PaletteAction::MoveCursor(-1)),
            "⌃B moves the cursor"
        );
    }

    #[test]
    fn on_the_emoji_grid_all_four_chords_move_the_selection() {
        let mut palette = palette();
        palette.open_screen(Screen::Emoji);
        assert_eq!(
            palette.key(Stroke::alt(Key::Character('f'))),
            Some(PaletteAction::MoveGrid(Direction::Right))
        );
        assert_eq!(
            palette.key(Stroke::alt(Key::Character('b'))),
            Some(PaletteAction::MoveGrid(Direction::Left))
        );
        assert_eq!(
            palette.key(Stroke::plain(Key::Down)),
            Some(PaletteAction::MoveSelection(1))
        );
    }

    #[test]
    fn the_escape_clears_the_text_first() {
        let mut palette = palette();
        palette.set_query("notepad");
        assert_eq!(palette.escape(), EscapeOutcome::ClearedText);
        assert_eq!(palette.query(), "");
    }

    #[test]
    fn the_escape_goes_back_a_screen_and_closes_at_the_root() {
        let mut palette = palette();
        palette.open_screen(Screen::FileSearch);
        assert_eq!(palette.escape(), EscapeOutcome::WentBack);
        assert_eq!(palette.screen(), Screen::Launcher);
        assert_eq!(palette.escape(), EscapeOutcome::Closed);
    }

    #[test]
    fn the_escape_can_close_right_away_and_pop_to_the_root() {
        let mut palette = palette();
        palette.set_settings(PaletteSettings {
            window_mode: WindowMode::Expanded,
            escape_behaviour: EscapeBehaviour::CloseWindowAndPopToRoot,
            ..PaletteSettings::default()
        });
        palette.open_screen(Screen::FileSearch);
        assert_eq!(palette.escape(), EscapeOutcome::Closed);
        assert_eq!(palette.screen(), Screen::Launcher);
    }

    #[test]
    fn the_compact_mode_collapses_before_it_closes() {
        let mut palette = compact_palette();
        palette.set_query("notepad");
        assert_eq!(palette.escape(), EscapeOutcome::ClearedText);
        assert_eq!(
            palette.escape(),
            EscapeOutcome::Collapsed,
            "the list folds away"
        );
        assert!(!palette.is_expanded());
        assert_eq!(palette.escape(), EscapeOutcome::Closed);
    }

    #[test]
    fn the_compact_mode_collapses_with_either_escape_setting() {
        let mut palette = compact_palette();
        palette.set_settings(PaletteSettings {
            escape_behaviour: EscapeBehaviour::CloseWindowAndPopToRoot,
            ..PaletteSettings::default()
        });
        palette.set_query("notepad");
        assert_eq!(palette.escape(), EscapeOutcome::ClearedText);
        assert_eq!(palette.escape(), EscapeOutcome::Collapsed);
    }

    #[test]
    fn the_expanded_mode_never_collapses() {
        let mut palette = palette();
        palette.open_screen(Screen::FileSearch);
        assert_eq!(palette.escape(), EscapeOutcome::WentBack);
        assert!(palette.is_expanded());
    }

    #[test]
    fn the_control_and_escape_goes_to_the_root_search() {
        let mut palette = palette();
        palette.open_screen(Screen::FileSearch);
        assert_eq!(
            palette.key(Stroke::ctrl(Key::Escape)),
            Some(PaletteAction::BackToRoot)
        );
        palette.back_to_root();
        assert_eq!(palette.screen(), Screen::Launcher);
    }

    #[test]
    fn the_alt_and_home_goes_to_the_root_search_too() {
        let mut palette = palette();
        palette.open_screen(Screen::FileSearch);
        assert_eq!(
            palette.key(Stroke::alt(Key::Home)),
            Some(PaletteAction::BackToRoot)
        );
    }

    #[test]
    fn the_delete_in_an_empty_field_goes_back_one_screen() {
        let mut palette = palette();
        palette.open_screen(Screen::FileSearch);
        assert_eq!(
            palette.key(Stroke::plain(Key::Backspace)),
            Some(PaletteAction::Back)
        );
    }

    #[test]
    fn the_delete_with_text_in_the_field_is_left_to_the_field() {
        let mut palette = palette();
        palette.set_query("notepad");
        assert_eq!(palette.key(Stroke::plain(Key::Backspace)), None);
    }

    #[test]
    fn the_comma_opens_the_settings_and_the_w_closes_the_palette() {
        let mut palette = palette();
        assert_eq!(
            palette.key(Stroke::ctrl(Key::Character(','))),
            Some(PaletteAction::Settings)
        );
        assert_eq!(
            palette.key(Stroke::ctrl(Key::Character('w'))),
            Some(PaletteAction::CloseWindow)
        );
    }

    #[test]
    fn the_tab_cycles_the_launcher_and_the_clipboard() {
        let mut palette = palette();
        palette.set_features(Features {
            ai: false,
            clipboard: true,
            ..Features::default()
        });
        assert_eq!(palette.tab_target(), Screen::Clipboard);
        palette.tab();
        assert_eq!(palette.screen(), Screen::Clipboard);
        assert_eq!(palette.tab_target(), Screen::Launcher);
        palette.tab();
        assert_eq!(palette.screen(), Screen::Launcher);
    }

    #[test]
    fn the_ai_is_skipped_while_it_is_off() {
        let mut palette = palette();
        palette.set_features(Features {
            ai: false,
            clipboard: true,
            ..Features::default()
        });
        let cycle = palette.cycle();
        assert!(!cycle.contains(&Screen::QuickAi));
        assert!(cycle.contains(&Screen::Clipboard));
    }

    #[test]
    fn the_ai_is_in_the_cycle_while_it_is_on() {
        let mut palette = palette();
        palette.set_features(Features {
            ai: true,
            clipboard: true,
            ..Features::default()
        });
        let cycle = palette.cycle();
        assert_eq!(
            cycle,
            vec![Screen::Launcher, Screen::QuickAi, Screen::Clipboard]
        );
    }

    #[test]
    fn the_clipboard_is_skipped_while_it_is_off() {
        let mut palette = palette();
        palette.set_features(Features {
            ai: false,
            clipboard: false,
            ..Features::default()
        });
        assert_eq!(palette.tab_target(), Screen::Launcher);
    }

    #[test]
    fn a_screen_outside_the_cycle_returns_to_the_launcher_with_the_tab() {
        let mut palette = palette();
        palette.open_screen(Screen::FileSearch);
        assert_eq!(palette.tab_target(), Screen::Launcher);
        palette.tab();
        assert_eq!(palette.screen(), Screen::Launcher);
    }

    #[test]
    fn a_row_that_takes_arguments_gets_the_tab_first() {
        let mut palette = palette();
        palette.set_row_fields(2);
        assert_eq!(
            palette.key(Stroke::plain(Key::Tab)),
            Some(PaletteAction::NextField)
        );
        let fields = palette.fields().expect("the fields are open");
        assert_eq!(fields.count, 2);
        assert_eq!(fields.focused, 0);
    }

    #[test]
    fn the_tab_moves_through_the_fields_and_around() {
        let mut palette = palette();
        palette.set_row_fields(3);
        palette.key(Stroke::plain(Key::Tab));
        assert_eq!(palette.fields().expect("the fields are open").focused, 0);
        palette.key(Stroke::plain(Key::Tab));
        assert_eq!(palette.fields().expect("the fields are open").focused, 1);
        palette.key(Stroke::plain(Key::Tab));
        assert_eq!(palette.fields().expect("the fields are open").focused, 2);
        palette.key(Stroke::plain(Key::Tab));
        assert_eq!(
            palette.fields().expect("the fields are open").focused,
            0,
            "the tab goes round"
        );
        palette.key(Stroke::shift(Key::Tab));
        assert_eq!(palette.fields().expect("the fields are open").focused, 2);
    }

    #[test]
    fn a_row_without_arguments_leaves_the_tab_to_the_screens() {
        let mut palette = palette();
        palette.set_row_fields(0);
        assert_eq!(
            palette.key(Stroke::plain(Key::Tab)),
            Some(PaletteAction::NextScreen)
        );
        assert!(palette.fields().is_none());
    }

    #[test]
    fn the_escape_leaves_the_fields_and_goes_back_to_the_results() {
        let mut palette = palette();
        palette.set_row_fields(2);
        palette.key(Stroke::plain(Key::Tab));
        assert_eq!(
            palette.key(Stroke::plain(Key::Escape)),
            Some(PaletteAction::Back)
        );
        assert!(palette.fields().is_none());
    }

    #[test]
    fn the_menu_swallows_the_keys_while_it_is_open() {
        let mut palette = palette();
        palette.key(Stroke::ctrl(Key::Character('k')));
        assert_eq!(
            palette.key(Stroke::plain(Key::Enter)),
            Some(PaletteAction::MainAction),
            "the highlighted action runs"
        );
        assert!(palette.menu().is_none(), "and the menu closes");
    }

    #[test]
    fn the_escape_clears_the_menu_search_before_it_closes_the_menu() {
        let mut palette = palette();
        palette.key(Stroke::ctrl(Key::Character('k')));
        palette.set_menu_query("close");
        assert_eq!(
            palette.key(Stroke::plain(Key::Escape)),
            Some(PaletteAction::Escape)
        );
        assert!(palette.menu().is_some(), "the menu is still open");
        assert_eq!(palette.menu().expect("the menu is open").query, "");
        palette.key(Stroke::plain(Key::Escape));
        assert!(palette.menu().is_none(), "the second escape closes it");
    }

    #[test]
    fn the_menu_can_be_closed_with_its_own_chord() {
        let mut palette = palette();
        palette.key(Stroke::ctrl(Key::Character('k')));
        palette.key(Stroke::ctrl(Key::Character('k')));
        assert!(palette.menu().is_none());
    }

    #[test]
    fn the_slim_bar_selects_the_search_field_and_the_shortcuts() {
        let mut palette = compact_palette();
        assert_eq!(palette.compact_bar(), CompactBar::Search);
        palette.key(Stroke::plain(Key::Right));
        assert_eq!(palette.compact_bar(), CompactBar::Shortcut(0));
        palette.key(Stroke::plain(Key::Right));
        assert_eq!(palette.compact_bar(), CompactBar::Shortcut(1));
    }

    #[test]
    fn the_slim_bar_stops_at_both_ends() {
        let mut palette = compact_palette();
        palette.key(Stroke::plain(Key::Left));
        assert_eq!(palette.compact_bar(), CompactBar::Search);
        for _ in 0..10 {
            palette.key(Stroke::plain(Key::Right));
        }
        let count = palette.settings().compact_shortcuts.items().len();
        assert_eq!(palette.compact_bar(), CompactBar::Shortcut(count - 1));
    }

    #[test]
    fn the_down_key_expands_the_list_from_the_search_field() {
        let mut palette = compact_palette();
        assert!(!palette.is_expanded());
        assert_eq!(
            palette.key(Stroke::plain(Key::Down)),
            Some(PaletteAction::ExpandList)
        );
        assert!(palette.is_expanded());
    }

    #[test]
    fn the_down_key_opens_a_shortcut_of_the_slim_bar() {
        let mut palette = compact_palette();
        palette.key(Stroke::plain(Key::Right));
        assert_eq!(
            palette.key(Stroke::plain(Key::Down)),
            Some(PaletteAction::OpenCompactItem)
        );
        assert_eq!(
            palette.compact_shortcut(),
            Some(CompactShortcut::ClipboardHistory)
        );
    }

    #[test]
    fn typing_from_a_shortcut_starts_a_search() {
        let mut palette = compact_palette();
        palette.key(Stroke::plain(Key::Right));
        palette.set_query("notepad");
        assert_eq!(palette.compact_bar(), CompactBar::Search);
        assert!(palette.is_expanded());
    }

    #[test]
    fn the_expanded_mode_always_shows_the_list() {
        let palette = palette();
        assert!(palette.is_expanded());
    }

    #[test]
    fn the_file_filter_cycles_and_resets_when_the_palette_opens() {
        let mut palette = palette();
        palette.cycle_file_filter();
        assert_eq!(palette.file_filter(), FileType::Folders);
        palette.reset();
        assert_eq!(palette.file_filter(), FileType::All);
    }

    #[test]
    fn the_control_and_p_narrows_the_file_search() {
        let mut palette = palette();
        palette.open_screen(Screen::FileSearch);
        assert_eq!(
            palette.key(Stroke::ctrl(Key::Character('p'))),
            Some(PaletteAction::CycleFileFilter)
        );
        assert_eq!(palette.file_filter(), FileType::Folders);
    }

    #[test]
    fn the_control_and_p_does_nothing_on_another_screen() {
        let mut palette = palette();
        assert_eq!(palette.key(Stroke::ctrl(Key::Character('p'))), None);
    }

    #[test]
    fn the_file_search_screen_offers_its_own_actions() {
        let mut palette = palette();
        palette.open_screen(Screen::FileSearch);
        palette.key(Stroke::ctrl(Key::Character('k')));
        let menu = palette.menu().expect("the menu is open");
        let labels: Vec<&str> = menu.items.iter().map(|item| item.label.as_str()).collect();
        assert!(labels.contains(&"Copy Path"));
        assert!(labels.contains(&"Move to the Recycle Bin"));
    }

    #[test]
    fn a_screen_opened_by_its_own_shortcut_starts_on_its_own() {
        let mut palette = palette();
        palette.open_global(Screen::Clipboard, false);
        assert_eq!(palette.screen(), Screen::Clipboard);
        assert_eq!(palette.stack().depth(), 1);
    }

    #[test]
    fn a_screen_opened_by_its_own_shortcut_lands_on_the_current_one() {
        let mut palette = palette();
        palette.open_screen(Screen::FileSearch);
        palette.open_global(Screen::Clipboard, true);
        assert_eq!(palette.screen(), Screen::Clipboard);
        assert_eq!(palette.stack().depth(), 3);
    }
}
