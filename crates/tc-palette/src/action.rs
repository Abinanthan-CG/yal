//! What a key does in the palette, and the menu that lists it.
//!
//! "<kbd>⌘</kbd><kbd>K</kbd> is the best way to learn the app. Every screen
//! lists all of its actions there, with shortcuts beside actions that have
//! them. Type in the menu to filter its actions; <kbd>esc</kbd> clears the
//! menu search first, then closes the menu."

use serde::{Deserialize, Serialize};

use crate::keys::{Key, Stroke};

/// A direction a selection can move in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    /// Up.
    Up,
    /// Down.
    Down,
    /// Left.
    Left,
    /// Right.
    Right,
}

/// What a key does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaletteAction {
    /// The main action for the selection.
    MainAction,
    /// The second action for the selection, usually "show in the file
    /// manager" or "copy".
    SecondAction,
    /// Open the actions menu.
    ActionsMenu,
    /// Move the selection up or down.
    MoveSelection(i8),
    /// Move the selection on a grid.
    MoveGrid(Direction),
    /// Move the text cursor left or right.
    MoveCursor(i8),
    /// Go to the next screen of the Tab cycle.
    NextScreen,
    /// Go to the previous screen.
    PreviousScreen,
    /// Clear the search, go back a screen, then close.
    Escape,
    /// Go back to the root search from any screen.
    BackToRoot,
    /// Go back one screen.
    Back,
    /// Open the settings.
    Settings,
    /// Close the palette.
    CloseWindow,
    /// Move to the next field of a row that takes arguments.
    NextField,
    /// Move to the previous field of a row that takes arguments.
    PreviousField,
    /// Expand the list of the compact bar.
    ExpandList,
    /// Open the item that is selected in the compact bar.
    OpenCompactItem,
    /// Move the compact bar's selection left or right.
    MoveCompactSelection(i8),
    /// Narrow the file search to another kind of file.
    CycleFileFilter,
    /// Copy the name of the file.
    CopyName,
    /// Copy the path of the file.
    CopyPath,
    /// Move the file to the Recycle Bin.
    MoveToRecycleBin,
}

/// One entry of the actions menu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionItem {
    /// What the entry does.
    pub action: PaletteAction,
    /// The name the entry is shown under.
    pub label: String,
    /// The chord, when the action has one.
    pub shortcut: Option<String>,
}

impl ActionItem {
    /// An action without a shortcut.
    pub fn plain(action: PaletteAction, label: &str) -> ActionItem {
        ActionItem {
            action,
            label: label.to_string(),
            shortcut: None,
        }
    }

    /// An action with a shortcut.
    pub fn bound(action: PaletteAction, label: &str, stroke: Stroke) -> ActionItem {
        ActionItem {
            action,
            label: label.to_string(),
            shortcut: Some(stroke.label()),
        }
    }
}

/// The actions that are the same on every screen.
///
/// The table is the one in `docs/SHORTCUTS.md`, under *Everywhere in the
/// palette*.
pub fn palette_actions() -> Vec<ActionItem> {
    vec![
        ActionItem::bound(
            PaletteAction::MainAction,
            "Main Action",
            Stroke::plain(Key::Enter),
        ),
        ActionItem::bound(
            PaletteAction::SecondAction,
            "Second Action",
            Stroke::ctrl(Key::Enter),
        ),
        ActionItem::bound(
            PaletteAction::ActionsMenu,
            "Actions Menu",
            Stroke::ctrl(Key::Character('k')),
        ),
        ActionItem::bound(
            PaletteAction::MoveSelection(-1),
            "Move Selection Up",
            Stroke::plain(Key::Up),
        ),
        ActionItem::bound(
            PaletteAction::MoveSelection(1),
            "Move Selection Down",
            Stroke::plain(Key::Down),
        ),
        ActionItem::bound(
            PaletteAction::MoveSelection(1),
            "Move Selection Down",
            Stroke::alt(Key::Character('n')),
        ),
        ActionItem::bound(
            PaletteAction::MoveSelection(-1),
            "Move Selection Up",
            Stroke::alt(Key::Character('p')),
        ),
        ActionItem::bound(
            PaletteAction::MoveCursor(1),
            "Move Cursor Right",
            Stroke::alt(Key::Character('f')),
        ),
        ActionItem::bound(
            PaletteAction::MoveCursor(-1),
            "Move Cursor Left",
            Stroke::alt(Key::Character('b')),
        ),
        ActionItem::bound(
            PaletteAction::NextScreen,
            "Next Screen",
            Stroke::plain(Key::Tab),
        ),
        ActionItem::bound(
            PaletteAction::Escape,
            "Clear Search, Go Back, Close",
            Stroke::plain(Key::Escape),
        ),
        ActionItem::bound(
            PaletteAction::BackToRoot,
            "Back to Root Search",
            Stroke::ctrl(Key::Escape),
        ),
        ActionItem::bound(
            PaletteAction::Back,
            "Go Back One Screen",
            Stroke::plain(Key::Backspace),
        ),
        ActionItem::bound(
            PaletteAction::Settings,
            "Settings",
            Stroke::ctrl(Key::Character(',')),
        ),
        ActionItem::bound(
            PaletteAction::CloseWindow,
            "Close the Palette",
            Stroke::ctrl(Key::Character('w')),
        ),
    ]
}

/// The file-search screen's own actions.
///
/// The table is the one in `docs/SHORTCUTS.md`, under *File search*.
///
/// Only the actions this build really runs are listed. A screen that offers
/// an action that does nothing is a dead end, and a dead end is worse than a
/// missing row.
pub fn file_actions() -> Vec<ActionItem> {
    vec![
        ActionItem::bound(
            PaletteAction::MainAction,
            "Open File / Open Folder",
            Stroke::plain(Key::Enter),
        ),
        ActionItem::bound(
            PaletteAction::SecondAction,
            "Show in File Explorer",
            Stroke::ctrl(Key::Enter),
        ),
        ActionItem::bound(
            PaletteAction::CycleFileFilter,
            "Filter by Type",
            Stroke::ctrl(Key::Character('p')),
        ),
        ActionItem::bound(
            PaletteAction::CopyName,
            "Copy Name",
            Stroke::ctrl_alt(Key::Character('c')),
        ),
        ActionItem::bound(
            PaletteAction::CopyPath,
            "Copy Path",
            Stroke::ctrl_alt_shift(Key::Character('c')),
        ),
        ActionItem::bound(
            PaletteAction::MoveToRecycleBin,
            "Move to the Recycle Bin",
            Stroke::alt(Key::Character('x')),
        ),
    ]
}

/// The actions menu, and the text typed into it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionMenu {
    /// The text typed into the menu's search field.
    pub query: String,
    /// Every action the screen offers.
    pub items: Vec<ActionItem>,
    /// Which entry is highlighted.
    pub selected: usize,
}

impl ActionMenu {
    /// A menu over the given actions.
    pub fn new(items: Vec<ActionItem>) -> ActionMenu {
        ActionMenu {
            query: String::new(),
            items,
            selected: 0,
        }
    }

    /// The palette's own menu.
    pub fn palette() -> ActionMenu {
        ActionMenu::new(palette_actions())
    }

    /// The file-search screen's menu.
    pub fn files() -> ActionMenu {
        ActionMenu::new(file_actions())
    }

    /// Type in the menu's search field.
    pub fn set_query(&mut self, query: &str) {
        self.query = query.to_string();
        self.selected = 0;
    }

    /// The actions that survive the filter.
    ///
    /// "Type in the menu to filter its actions."
    pub fn visible(&self) -> Vec<&ActionItem> {
        let wanted = self.query.trim().to_lowercase();
        if wanted.is_empty() {
            return self.items.iter().collect();
        }
        self.items
            .iter()
            .filter(|item| item.label.to_lowercase().contains(&wanted))
            .collect()
    }

    /// Whether the filter leaves anything at all.
    pub fn is_empty(&self) -> bool {
        self.visible().is_empty()
    }

    /// Highlight the entry above.
    pub fn move_up(&mut self) {
        let count = self.visible().len();
        if count == 0 {
            return;
        }
        self.selected = if self.selected == 0 {
            count - 1
        } else {
            self.selected - 1
        };
    }

    /// Highlight the entry below.
    pub fn move_down(&mut self) {
        let count = self.visible().len();
        if count == 0 {
            return;
        }
        self.selected = (self.selected + 1) % count;
    }

    /// The highlighted entry's action.
    pub fn selected_action(&self) -> Option<PaletteAction> {
        self.visible().get(self.selected).map(|item| item.action)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::{MacChord, MacModifier};

    #[test]
    fn every_action_of_the_palette_is_listed() {
        let menu = ActionMenu::palette();
        let labels: Vec<&str> = menu.items.iter().map(|item| item.label.as_str()).collect();
        assert!(labels.contains(&"Main Action"));
        assert!(labels.contains(&"Actions Menu"));
        assert!(labels.contains(&"Settings"));
        assert!(labels.contains(&"Close the Palette"));
    }

    #[test]
    fn the_actions_carry_the_chords_from_the_documentation() {
        let menu = ActionMenu::palette();
        let settings = menu
            .items
            .iter()
            .find(|item| item.action == PaletteAction::Settings)
            .expect("the settings action");
        assert_eq!(settings.shortcut.as_deref(), Some("Ctrl+,"));
    }

    #[test]
    fn typing_in_the_menu_filters_the_actions() {
        let mut menu = ActionMenu::palette();
        menu.set_query("palette");
        let visible = menu.visible();
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].action, PaletteAction::CloseWindow);
    }

    #[test]
    fn the_filter_ignores_the_case() {
        let mut menu = ActionMenu::palette();
        menu.set_query("PALETTE");
        assert_eq!(menu.visible().len(), 1);
    }

    #[test]
    fn an_empty_menu_search_leaves_every_action() {
        let menu = ActionMenu::palette();
        assert_eq!(menu.visible().len(), menu.items.len());
    }

    #[test]
    fn a_filter_that_matches_nothing_leaves_an_empty_menu() {
        let mut menu = ActionMenu::palette();
        menu.set_query("nothing matches this");
        assert!(menu.is_empty());
    }

    #[test]
    fn the_menu_highlight_stays_inside_the_filtered_list() {
        let mut menu = ActionMenu::palette();
        menu.move_up();
        let count = menu.visible().len();
        assert!(menu.selected < count);
        menu.move_down();
        assert!(menu.selected < count);
    }

    #[test]
    fn the_menu_highlight_wraps_around() {
        let mut menu = ActionMenu::palette();
        menu.move_up();
        assert_eq!(menu.selected, menu.visible().len() - 1);
    }

    #[test]
    fn the_upstream_chords_translate_to_the_chords_the_menu_shows() {
        // The menu is built from Windows chords, and the upstream table is
        // the source of the truth, so the two have to agree.
        let upstream = MacChord::new(Key::Character(','), &[MacModifier::Command]);
        let translated = crate::keys::translate(&upstream);
        let label = translated[0].label();
        let menu = ActionMenu::palette();
        let settings = menu
            .items
            .iter()
            .find(|item| item.action == PaletteAction::Settings)
            .expect("the settings action");
        assert_eq!(settings.shortcut.as_deref(), Some(label.as_str()));
    }

    #[test]
    fn the_escape_chord_is_listed_once() {
        let menu = ActionMenu::palette();
        let escapes = menu
            .items
            .iter()
            .filter(|item| item.action == PaletteAction::Escape)
            .count();
        assert_eq!(escapes, 1);
    }

    #[test]
    fn the_up_and_down_arrows_move_the_selection_in_both_directions() {
        let menu = ActionMenu::palette();
        let up = menu
            .items
            .iter()
            .filter(|item| item.action == PaletteAction::MoveSelection(-1))
            .count();
        let down = menu
            .items
            .iter()
            .filter(|item| item.action == PaletteAction::MoveSelection(1))
            .count();
        assert_eq!(up, 2, "the arrow key and the emacs chord");
        assert_eq!(down, 2);
    }

    #[test]
    fn the_file_actions_match_the_documented_table() {
        let menu = ActionMenu::files();
        let shortcut_of = |action: PaletteAction| {
            menu.items
                .iter()
                .find(|item| item.action == action)
                .and_then(|item| item.shortcut.clone())
        };
        assert_eq!(
            shortcut_of(PaletteAction::CopyName).as_deref(),
            Some("Ctrl+Alt+c")
        );
        assert_eq!(
            shortcut_of(PaletteAction::CopyPath).as_deref(),
            Some("Ctrl+Alt+Shift+c")
        );
        assert_eq!(
            shortcut_of(PaletteAction::MoveToRecycleBin).as_deref(),
            Some("Alt+x")
        );
        assert_eq!(
            shortcut_of(PaletteAction::CycleFileFilter).as_deref(),
            Some("Ctrl+p")
        );
    }

    #[test]
    fn every_file_action_is_one_this_build_can_run() {
        // A screen that offers an action that does nothing is a dead end.
        let menu = ActionMenu::files();
        for item in &menu.items {
            let runnable = matches!(
                item.action,
                PaletteAction::MainAction
                    | PaletteAction::SecondAction
                    | PaletteAction::CycleFileFilter
                    | PaletteAction::CopyName
                    | PaletteAction::CopyPath
                    | PaletteAction::MoveToRecycleBin
            );
            assert!(runnable, "`{}` is listed but does nothing", item.label);
        }
    }

    #[test]
    fn the_copy_name_and_copy_path_chords_are_different() {
        let menu = ActionMenu::files();
        let name = menu
            .items
            .iter()
            .find(|item| item.action == PaletteAction::CopyName)
            .and_then(|item| item.shortcut.clone());
        let path = menu
            .items
            .iter()
            .find(|item| item.action == PaletteAction::CopyPath)
            .and_then(|item| item.shortcut.clone());
        assert_ne!(name, path);
    }

    #[test]
    fn a_shortcut_that_has_none_is_still_listed() {
        let item = ActionItem::plain(PaletteAction::ExpandList, "Expand the List");
        assert!(item.shortcut.is_none());
    }

    #[test]
    fn the_windows_key_is_never_in_a_shortcut() {
        let menu = ActionMenu::palette();
        for item in &menu.items {
            if let Some(shortcut) = &item.shortcut {
                assert!(
                    !shortcut.contains("Win"),
                    "{} takes the Windows key",
                    item.label
                );
            }
        }
    }

    #[test]
    fn the_editing_chords_are_never_offered_as_actions() {
        for menu in [ActionMenu::palette(), ActionMenu::files()] {
            for item in &menu.items {
                if let Some(shortcut) = &item.shortcut {
                    assert_ne!(shortcut, "Ctrl+c");
                    assert_ne!(shortcut, "Ctrl+v");
                    assert_ne!(shortcut, "Ctrl+x");
                    assert_ne!(shortcut, "Ctrl+z");
                }
            }
        }
    }
}
