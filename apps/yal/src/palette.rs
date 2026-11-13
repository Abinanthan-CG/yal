//! The palette's own state, and the commands the window asks it.
//!
//! The decisions live in [`tc_palette`]; this is the bridge. The window sends
//! the key it saw, and this tells the window what to do about it. The window
//! never decides anything about a key on its own, because a decision that
//! cannot be tested is not a decision.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tc_palette::{EscapeOutcome, Key, Modifiers, Palette, PaletteAction, Stroke};

/// A key press, as the window sees it.
#[derive(Debug, Clone, Deserialize)]
pub struct KeyStroke {
    /// The name the browser gives the key.
    pub key: String,
    /// Whether <kbd>Ctrl</kbd> was held.
    #[serde(default)]
    pub ctrl: bool,
    /// Whether <kbd>Alt</kbd> was held.
    #[serde(default)]
    pub alt: bool,
    /// Whether <kbd>Shift</kbd> was held.
    #[serde(default)]
    pub shift: bool,
    /// Whether the Windows key was held.
    #[serde(default)]
    pub win: bool,
}

impl KeyStroke {
    /// The key in the palette's own words.
    ///
    /// `None` for a key the palette has no name for, which is left to the
    /// text field.
    pub fn to_stroke(&self) -> Option<Stroke> {
        let key = match self.key.as_str() {
            "Enter" => Key::Enter,
            "Escape" => Key::Escape,
            "Tab" => Key::Tab,
            "Backspace" => Key::Backspace,
            "Delete" => Key::Delete,
            " " | "Spacebar" => Key::Space,
            "Home" => Key::Home,
            "End" => Key::End,
            "PageUp" => Key::PageUp,
            "PageDown" => Key::PageDown,
            "ArrowUp" => Key::Up,
            "ArrowDown" => Key::Down,
            "ArrowLeft" => Key::Left,
            "ArrowRight" => Key::Right,
            other => {
                if let Some(number) = other
                    .strip_prefix('F')
                    .and_then(|rest| rest.parse::<u8>().ok())
                {
                    if !(1..=20).contains(&number) {
                        return None;
                    }
                    Key::Function(number)
                } else {
                    let mut characters = other.chars();
                    let character = characters.next()?;
                    if characters.next().is_some() {
                        return None;
                    }
                    Key::Character(character)
                }
            }
        };
        Some(Stroke {
            key,
            modifiers: Modifiers {
                ctrl: self.ctrl,
                alt: self.alt,
                shift: self.shift,
                win: self.win,
            },
        })
    }
}

/// One entry of the actions menu, as the window shows it.
#[derive(Debug, Clone, Serialize)]
pub struct MenuItem {
    /// The name the entry is shown under.
    pub label: String,
    /// The chord, when the action has one.
    pub shortcut: Option<String>,
    /// Whether the entry is the highlighted one.
    pub selected: bool,
}

/// What the palette looks like right now.
#[derive(Debug, Clone, Serialize)]
pub struct PaletteView {
    /// The screen on top.
    pub screen: String,
    /// How many screens are stacked.
    pub depth: usize,
    /// The text in the search field.
    pub query: String,
    /// Whether the list is showing.
    pub expanded: bool,
    /// What the slim bar is showing: `search` or `shortcut`.
    pub compact_bar: String,
    /// The name of the kind of file the file search is narrowed to.
    pub file_filter: String,
    /// The actions of the menu, when it is open.
    pub menu: Vec<MenuItem>,
    /// Whether the menu is open.
    pub menu_open: bool,
    /// How many fields the selected row takes.
    pub fields: usize,
    /// Which field has the focus.
    pub focused: usize,
    /// How the palette is painted.
    pub appearance: String,
    /// Whether the palette moves when it opens and closes.
    pub animations: bool,
    /// The factor the interface is drawn at.
    pub scale: f64,
}

/// What a key press did.
#[derive(Debug, Clone, Serialize)]
pub struct KeyOutcome {
    /// The action, when the palette did something.
    pub action: Option<PaletteAction>,
    /// Whether the palette closed.
    pub closed: bool,
    /// Whether the palette went back a screen.
    pub went_back: bool,
    /// Whether the palette collapsed into its slim bar.
    pub collapsed: bool,
}

/// One row of the Settings pane.
#[derive(Debug, Clone, Serialize)]
pub struct SettingRow {
    /// The key the setting is stored under.
    pub key: String,
    /// The name the setting is shown under.
    pub label: String,
    /// The value the setting has now.
    pub value: String,
    /// The values it can take.
    pub options: Vec<String>,
}

/// The Settings pane's rows, in the order they are shown.
///
/// These are the settings of *Settings → General*, and each one is the
/// switch upstream puts there.
fn general_settings(settings: &tc_palette::PaletteSettings) -> Vec<SettingRow> {
    let rows: Vec<(&str, &str)> = vec![
        (tc_palette::KEY_APPEARANCE, "Appearance"),
        (tc_palette::KEY_ANIMATIONS, "Show Animations"),
        (tc_palette::KEY_INTERFACE_SIZE, "Interface Size"),
        (tc_palette::KEY_WINDOW_MODE, "Window Mode"),
        (tc_palette::KEY_ESCAPE_BEHAVIOUR, "Escape Key Behavior"),
        (tc_palette::KEY_POP_TO_ROOT, "Pop to Root Search"),
    ];
    rows.into_iter()
        .map(|(key, label)| {
            let options = options_of(settings, key);
            let value = settings.value_of(key).unwrap_or_default();
            SettingRow {
                key: key.to_string(),
                label: label.to_string(),
                value,
                options,
            }
        })
        .collect()
}

/// The values a setting can take.
fn options_of(settings: &tc_palette::PaletteSettings, key: &str) -> Vec<String> {
    match key {
        tc_palette::KEY_APPEARANCE => tc_palette::Appearance::OPTIONS
            .iter()
            .map(|option| option.label().to_string())
            .collect(),
        tc_palette::KEY_ANIMATIONS => vec!["On".to_string(), "Off".to_string()],
        tc_palette::KEY_INTERFACE_SIZE => tc_palette::InterfaceSize::OPTIONS
            .iter()
            .map(|option| option.label().to_string())
            .collect(),
        tc_palette::KEY_WINDOW_MODE => tc_palette::WindowMode::OPTIONS
            .iter()
            .map(|option| option.label().to_string())
            .collect(),
        tc_palette::KEY_ESCAPE_BEHAVIOUR => tc_palette::EscapeBehaviour::OPTIONS
            .iter()
            .map(|option| option.label().to_string())
            .collect(),
        tc_palette::KEY_POP_TO_ROOT => tc_palette::PopToRoot::OPTIONS
            .iter()
            .map(|option| option.label())
            .collect(),
        _ => {
            let _ = settings;
            Vec::new()
        }
    }
}

/// The palette the window drives.
pub struct PaletteState {
    /// The palette itself.
    palette: Mutex<Palette>,
}

impl Default for PaletteState {
    fn default() -> Self {
        PaletteState::new()
    }
}

impl PaletteState {
    /// A palette at its root.
    pub fn new() -> PaletteState {
        PaletteState {
            palette: Mutex::new(Palette::new()),
        }
    }

    /// The palette, locked.
    pub fn lock(&self) -> std::sync::MutexGuard<'_, Palette> {
        self.palette.lock().expect("the palette lock")
    }

    /// What the palette looks like right now.
    pub fn view(&self) -> PaletteView {
        let palette = self.lock();
        let menu = palette.menu();
        let items = menu
            .map(|menu| {
                menu.visible()
                    .iter()
                    .enumerate()
                    .map(|(index, item)| MenuItem {
                        label: item.label.clone(),
                        shortcut: item.shortcut.clone(),
                        selected: index == menu.selected,
                    })
                    .collect()
            })
            .unwrap_or_default();
        let fields = palette.fields();
        PaletteView {
            screen: palette.screen().label().to_string(),
            depth: palette.stack().depth(),
            query: palette.query().to_string(),
            expanded: palette.is_expanded(),
            compact_bar: match palette.compact_bar() {
                tc_palette::CompactBar::Search => "search".to_string(),
                tc_palette::CompactBar::Shortcut(_) => "shortcut".to_string(),
            },
            file_filter: palette.file_filter().label().to_string(),
            menu: items,
            menu_open: menu.is_some(),
            fields: fields.map(|fields| fields.count).unwrap_or(0),
            focused: fields.map(|fields| fields.focused).unwrap_or(0),
            appearance: palette.settings().appearance.label().to_string(),
            animations: palette.settings().animations,
            scale: palette.settings().interface_size.scale(),
        }
    }

    /// Press a key.
    pub fn key(&self, stroke: &KeyStroke, query: &str) -> KeyOutcome {
        let Some(stroke) = stroke.to_stroke() else {
            return KeyOutcome {
                action: None,
                closed: false,
                went_back: false,
                collapsed: false,
            };
        };
        let mut palette = self.lock();
        palette.set_query(query);
        let action = palette.key(stroke);
        let escape = palette.escape_outcome();
        KeyOutcome {
            action,
            closed: escape == Some(EscapeOutcome::Closed),
            went_back: escape == Some(EscapeOutcome::WentBack),
            collapsed: escape == Some(EscapeOutcome::Collapsed),
        }
    }

    /// Type in the actions menu's search field.
    pub fn menu_query(&self, query: &str) {
        self.lock().set_menu_query(query);
    }

    /// Open a screen by name.
    pub fn open_screen(&self, screen: &str) {
        let Some(screen) = screen_of(screen) else {
            return;
        };
        self.lock().open_screen(screen);
    }

    /// Go back one screen.
    pub fn back(&self) -> bool {
        self.lock().back()
    }

    /// Go back to the root search.
    pub fn back_to_root(&self) {
        self.lock().back_to_root();
    }

    /// The Settings pane's rows.
    pub fn settings_rows(&self) -> Vec<SettingRow> {
        let palette = self.lock();
        general_settings(palette.settings())
    }

    /// Move a setting to its next value.
    ///
    /// The new value is returned, so the caller can write it down.
    pub fn advance_setting(&self, key: &str) -> Option<String> {
        let mut palette = self.lock();
        let next = palette.settings().next_value(key)?;
        let mut settings = palette.settings().clone();
        if !apply_setting(&mut settings, key, &next) {
            return None;
        }
        palette.set_settings(settings);
        Some(next)
    }

    /// Set a setting to a value.
    pub fn set_setting(&self, key: &str, value: &str) -> bool {
        let mut palette = self.lock();
        let mut settings = palette.settings().clone();
        if !apply_setting(&mut settings, key, value) {
            return false;
        }
        palette.set_settings(settings);
        true
    }
}

/// Apply one stored value to the palette's settings.
fn apply_setting(settings: &mut tc_palette::PaletteSettings, key: &str, value: &str) -> bool {
    match key {
        tc_palette::KEY_APPEARANCE => settings.set_appearance(value),
        tc_palette::KEY_ANIMATIONS => settings.set_animations(value),
        tc_palette::KEY_INTERFACE_SIZE => settings.set_interface_size(value),
        tc_palette::KEY_WINDOW_MODE => settings.set_window_mode(value),
        tc_palette::KEY_ESCAPE_BEHAVIOUR => settings.set_escape_behaviour(value),
        tc_palette::KEY_POP_TO_ROOT => settings.set_pop_to_root(value),
        _ => false,
    }
}

/// The screen a name stands for.
fn screen_of(name: &str) -> Option<tc_palette::Screen> {
    match name {
        "Launcher" => Some(tc_palette::Screen::Launcher),
        "Clipboard" | "Clipboard History" => Some(tc_palette::Screen::Clipboard),
        "FileSearch" | "Search Files" => Some(tc_palette::Screen::FileSearch),
        "Emoji" | "Emoji & Symbols" => Some(tc_palette::Screen::Emoji),
        "Settings" => Some(tc_palette::Screen::Settings),
        "QuickAi" | "Quick AI" => Some(tc_palette::Screen::QuickAi),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stroke(key: &str) -> KeyStroke {
        KeyStroke {
            key: key.to_string(),
            ctrl: false,
            alt: false,
            shift: false,
            win: false,
        }
    }

    #[test]
    fn the_browser_names_are_the_palette_names() {
        assert_eq!(stroke("Enter").to_stroke().expect("a key").key, Key::Enter);
        assert_eq!(
            stroke("Escape").to_stroke().expect("a key").key,
            Key::Escape
        );
        assert_eq!(stroke("Tab").to_stroke().expect("a key").key, Key::Tab);
        assert_eq!(
            stroke("Backspace").to_stroke().expect("a key").key,
            Key::Backspace
        );
        assert_eq!(stroke("ArrowUp").to_stroke().expect("a key").key, Key::Up);
    }

    #[test]
    fn a_function_key_is_named_by_its_number() {
        let stroke = KeyStroke {
            key: "F5".to_string(),
            ctrl: false,
            alt: false,
            shift: false,
            win: false,
        };
        assert_eq!(stroke.to_stroke().expect("a key").key, Key::Function(5));
    }

    #[test]
    fn a_function_key_outside_the_range_has_no_name() {
        let stroke = KeyStroke {
            key: "F99".to_string(),
            ctrl: false,
            alt: false,
            shift: false,
            win: false,
        };
        assert!(stroke.to_stroke().is_none());
    }

    #[test]
    fn a_single_character_is_a_character() {
        assert_eq!(
            stroke("k").to_stroke().expect("a key").key,
            Key::Character('k')
        );
    }

    #[test]
    fn a_name_that_is_not_a_key_is_left_to_the_text_field() {
        assert!(stroke("Dead").to_stroke().is_none());
        assert!(stroke("Unidentified").to_stroke().is_none());
    }

    #[test]
    fn the_modifiers_come_across() {
        let pressed = KeyStroke {
            key: "k".to_string(),
            ctrl: true,
            alt: false,
            shift: false,
            win: false,
        };
        let stroke = pressed.to_stroke().expect("a key");
        assert!(stroke.modifiers.ctrl);
        assert!(!stroke.modifiers.alt);
    }

    #[test]
    fn the_escape_at_the_root_closes_the_palette() {
        let state = PaletteState::new();
        let outcome = state.key(&stroke("Escape"), "");
        assert_eq!(outcome.action, Some(PaletteAction::Escape));
        assert!(outcome.closed);
    }

    #[test]
    fn the_escape_with_text_only_clears_it() {
        let state = PaletteState::new();
        let outcome = state.key(&stroke("Escape"), "notepad");
        assert!(!outcome.closed);
        let view = state.view();
        assert_eq!(view.query, "");
    }

    #[test]
    fn the_view_reports_the_screen_and_the_stack() {
        let state = PaletteState::new();
        assert_eq!(state.view().screen, "Launcher");
        assert_eq!(state.view().depth, 1);
        state.open_screen("Search Files");
        assert_eq!(state.view().screen, "Search Files");
        assert_eq!(state.view().depth, 2);
    }

    #[test]
    fn a_screen_that_is_not_known_is_not_opened() {
        let state = PaletteState::new();
        state.open_screen("Nothing Like This");
        assert_eq!(state.view().screen, "Launcher");
    }

    #[test]
    fn going_back_one_screen_reports_whether_there_was_one() {
        let state = PaletteState::new();
        assert!(!state.back(), "there is nothing behind the root");
        state.open_screen("Settings");
        assert!(state.back());
        assert_eq!(state.view().screen, "Launcher");
    }

    #[test]
    fn the_actions_menu_is_reported_with_its_shortcuts() {
        let state = PaletteState::new();
        state.key(
            &KeyStroke {
                key: "k".to_string(),
                ctrl: true,
                alt: false,
                shift: false,
                win: false,
            },
            "",
        );
        let view = state.view();
        assert!(view.menu_open);
        let settings = view
            .menu
            .iter()
            .find(|item| item.label == "Settings")
            .expect("the settings action");
        assert_eq!(settings.shortcut.as_deref(), Some("Ctrl+,"));
    }

    #[test]
    fn the_menu_search_filters_the_actions() {
        let state = PaletteState::new();
        state.key(
            &KeyStroke {
                key: "k".to_string(),
                ctrl: true,
                alt: false,
                shift: false,
                win: false,
            },
            "",
        );
        state.menu_query("palette");
        let view = state.view();
        assert_eq!(view.menu.len(), 1);
        assert_eq!(view.menu[0].label, "Close the Palette");
    }

    #[test]
    fn the_view_reports_the_appearance_settings() {
        let state = PaletteState::new();
        let view = state.view();
        assert_eq!(view.appearance, "System");
        assert!(view.animations);
        assert_eq!(view.scale, 1.0);
    }

    #[test]
    fn the_settings_pane_lists_the_general_settings() {
        let state = PaletteState::new();
        let rows = state.settings_rows();
        let labels: Vec<&str> = rows.iter().map(|row| row.label.as_str()).collect();
        assert!(labels.contains(&"Appearance"));
        assert!(labels.contains(&"Window Mode"));
        assert!(labels.contains(&"Escape Key Behavior"));
        let appearance = rows
            .iter()
            .find(|row| row.key == tc_palette::KEY_APPEARANCE)
            .expect("the appearance row");
        assert_eq!(appearance.value, "System");
        assert_eq!(appearance.options.len(), 3);
    }

    #[test]
    fn a_setting_moves_to_its_next_value() {
        let state = PaletteState::new();
        let next = state
            .advance_setting(tc_palette::KEY_APPEARANCE)
            .expect("the appearance has a next value");
        assert_eq!(next, "Light");
        assert_eq!(state.view().appearance, "Light");
    }

    #[test]
    fn the_animations_switch_turns_on_and_off() {
        let state = PaletteState::new();
        state
            .advance_setting(tc_palette::KEY_ANIMATIONS)
            .expect("a next value");
        assert!(!state.view().animations);
        state
            .advance_setting(tc_palette::KEY_ANIMATIONS)
            .expect("a next value");
        assert!(state.view().animations);
    }

    #[test]
    fn a_setting_that_is_not_the_palettes_own_is_refused() {
        let state = PaletteState::new();
        assert!(state.advance_setting("commands.enabled").is_none());
        assert!(!state.set_setting("commands.enabled", "true"));
    }

    #[test]
    fn the_interface_size_changes_the_scale() {
        let state = PaletteState::new();
        state
            .advance_setting(tc_palette::KEY_INTERFACE_SIZE)
            .expect("a next value");
        assert_eq!(state.view().scale, 1.1);
        state
            .advance_setting(tc_palette::KEY_INTERFACE_SIZE)
            .expect("a next value");
        assert_eq!(state.view().scale, 1.2);
    }
}
