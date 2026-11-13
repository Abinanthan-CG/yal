//! The screens the palette shows, and how they stack.
//!
//! Upstream's palette is one floating panel: the launcher is its root screen
//! and every other screen stacks on it. <kbd>esc</kbd> takes the user back one
//! screen at a time.
//!
//! Two ways in, and they stack differently:
//!
//! - **Typing the screen's name in the launcher** opens it on top of the
//!   launcher, so <kbd>esc</kbd> takes the user back to the search.
//! - **Pressing the screen's own global shortcut** opens it by itself when
//!   the palette is hidden, with nothing behind it, and opens it over the
//!   current screen when the palette is already showing.

/// The screens the palette can show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Screen {
    /// The root: the launcher.
    Launcher,
    /// The clipboard history.
    Clipboard,
    /// The file search.
    FileSearch,
    /// The emoji grid.
    Emoji,
    /// The settings.
    Settings,
    /// The AI chat.
    ///
    /// YAL ships no model provider, so this screen is never opened. It is
    /// here because the rule about the Tab cycle still has to hold: the
    /// cycle skips it while AI is off.
    QuickAi,
}

impl Screen {
    /// The name the screen is known by.
    pub fn label(&self) -> &'static str {
        match self {
            Screen::Launcher => "Launcher",
            Screen::Clipboard => "Clipboard History",
            Screen::FileSearch => "Search Files",
            Screen::Emoji => "Emoji & Symbols",
            Screen::Settings => "Settings",
            Screen::QuickAi => "Quick AI",
        }
    }

    /// Whether the screen is the root.
    pub fn is_root(&self) -> bool {
        *self == Screen::Launcher
    }

    /// Whether the screen is part of the Tab cycle.
    ///
    /// "Other screens, like Calculator History or Search Quicklinks, aren't
    /// part of the cycle."
    pub fn is_in_tab_cycle(&self) -> bool {
        matches!(self, Screen::Launcher | Screen::QuickAi | Screen::Clipboard)
    }
}

/// The stack of screens the palette is showing.
///
/// The launcher is always at the bottom, so the stack is never empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenStack {
    screens: Vec<Screen>,
}

impl Default for ScreenStack {
    fn default() -> Self {
        ScreenStack::new()
    }
}

impl ScreenStack {
    /// A stack with only the launcher on it.
    pub fn new() -> ScreenStack {
        ScreenStack {
            screens: vec![Screen::Launcher],
        }
    }

    /// The screen on top.
    pub fn current(&self) -> Screen {
        *self.screens.last().expect("the stack is never empty")
    }

    /// How many screens are stacked.
    pub fn depth(&self) -> usize {
        self.screens.len()
    }

    /// Whether the screen is anywhere in the stack.
    pub fn contains(&self, screen: Screen) -> bool {
        self.screens.contains(&screen)
    }

    /// Open a screen on top of the current one.
    ///
    /// The launcher is the root, so it is never pushed: opening it means
    /// going back to the root.
    pub fn push(&mut self, screen: Screen) {
        if screen.is_root() {
            self.pop_to_root();
            return;
        }
        self.screens.push(screen);
    }

    /// Go back one screen.
    ///
    /// `None` at the root: there is nothing behind it, so the caller closes
    /// the window.
    pub fn pop(&mut self) -> Option<Screen> {
        if self.depth() <= 1 {
            return None;
        }
        self.screens.pop()
    }

    /// Go back to the root, whatever is stacked.
    pub fn pop_to_root(&mut self) {
        self.screens.truncate(1);
    }

    /// Open a screen the way its own global shortcut does.
    ///
    /// When the palette was hidden the screen opens by itself, with nothing
    /// behind it. When the palette was already showing, the screen opens
    /// over the current one.
    pub fn open_global(&mut self, screen: Screen, palette_was_visible: bool) {
        if palette_was_visible {
            self.push(screen);
        } else {
            self.screens.clear();
            self.screens.push(screen);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_launcher_is_the_root() {
        let stack = ScreenStack::new();
        assert_eq!(stack.current(), Screen::Launcher);
        assert_eq!(stack.depth(), 1);
        assert!(stack.current().is_root());
    }

    #[test]
    fn a_screen_opened_by_name_stacks_on_the_launcher() {
        let mut stack = ScreenStack::new();
        stack.push(Screen::FileSearch);
        assert_eq!(stack.current(), Screen::FileSearch);
        assert_eq!(stack.depth(), 2);
        assert!(stack.contains(Screen::Launcher));
    }

    #[test]
    fn going_back_one_screen_returns_to_the_launcher() {
        let mut stack = ScreenStack::new();
        stack.push(Screen::FileSearch);
        assert_eq!(stack.pop(), Some(Screen::FileSearch));
        assert_eq!(stack.current(), Screen::Launcher);
    }

    #[test]
    fn there_is_nothing_behind_the_root() {
        let mut stack = ScreenStack::new();
        assert_eq!(stack.pop(), None);
        assert_eq!(stack.current(), Screen::Launcher);
    }

    #[test]
    fn a_screen_opened_by_its_own_shortcut_starts_on_its_own() {
        let mut stack = ScreenStack::new();
        stack.open_global(Screen::Clipboard, false);
        assert_eq!(stack.current(), Screen::Clipboard);
        assert_eq!(stack.depth(), 1);
        assert!(!stack.contains(Screen::Launcher));
    }

    #[test]
    fn a_screen_opened_by_its_own_shortcut_lands_on_the_current_one() {
        let mut stack = ScreenStack::new();
        stack.push(Screen::FileSearch);
        stack.open_global(Screen::Clipboard, true);
        assert_eq!(stack.current(), Screen::Clipboard);
        assert_eq!(stack.depth(), 3);
    }

    #[test]
    fn popping_to_the_root_drops_everything_above_it() {
        let mut stack = ScreenStack::new();
        stack.push(Screen::FileSearch);
        stack.push(Screen::Emoji);
        stack.pop_to_root();
        assert_eq!(stack.current(), Screen::Launcher);
        assert_eq!(stack.depth(), 1);
    }

    #[test]
    fn the_launcher_is_never_pushed_onto_itself() {
        let mut stack = ScreenStack::new();
        stack.push(Screen::FileSearch);
        stack.push(Screen::Launcher);
        assert_eq!(stack.current(), Screen::Launcher);
        assert_eq!(stack.depth(), 1);
    }

    #[test]
    fn only_three_screens_are_in_the_tab_cycle() {
        assert!(Screen::Launcher.is_in_tab_cycle());
        assert!(Screen::QuickAi.is_in_tab_cycle());
        assert!(Screen::Clipboard.is_in_tab_cycle());
        assert!(!Screen::FileSearch.is_in_tab_cycle());
        assert!(!Screen::Emoji.is_in_tab_cycle());
        assert!(!Screen::Settings.is_in_tab_cycle());
    }
}
