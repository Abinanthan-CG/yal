//! The native palette: the Direct2D window the user types into.
//!
//! Pure Win32 + Direct2D + DirectWrite — no WebView2, no browser runtime.
//! Supports both the main Launcher screen and the native Settings screen.

use std::sync::{Arc, Mutex};

use tc_platform::{FileSystem, Settings};
use tc_platform_win::{HotkeyTrigger, WindowsSettings};
use tc_ui::{
    Color, Icon, Model, Palette, PaletteEvent, PaletteHandle, PaletteReply, Row, Theme, Tokens,
};

use crate::launcher::{
    LauncherState, ResultRow, CALCULATOR_SECTION, COMMAND_SECTION, CUSTOM_COMMAND_SECTION,
    FILE_SECTION, RANKING_FILE, SYSTEM_ACTION_SECTION, WINDOW_COMMAND_SECTION,
};
use crate::open::Runner;
use crate::palette::{PaletteState, SettingRow};

/// Cross-thread command code asking the native palette to open Settings.
pub const COMMAND_OPEN_SETTINGS: u32 = 1;

/// Setting key storing the launcher's global hotkey in `settings.json`.
pub const KEY_LAUNCHER_HOTKEY: &str = "launcher.hotkey";

/// Built-in hotkey presets cycled in the native Settings pane.
const HOTKEY_PRESETS: [&str; 4] = ["Alt+Space", "Ctrl+Space", "Double Ctrl", "Double Alt"];

/// The hint at the bottom of the launcher palette.
const FOOTER: &str = "Enter to open  ·  Up/Down to choose  ·  Esc to dismiss";

/// The hint at the bottom of the settings screen.
const SETTINGS_FOOTER: &str = "Settings  ·  Enter to change  ·  Esc to close";

/// The section an application belongs to.
const APPLICATIONS: &str = "Applications";

/// Callback invoked when the user changes the launcher hotkey in Settings.
pub type HotkeyChangedCallback = Arc<dyn Fn(HotkeyTrigger) + Send + Sync>;

/// Optional context enabling native Settings inside the palette.
pub struct NativeContext {
    pub palette: Arc<PaletteState>,
    pub settings: Arc<WindowsSettings>,
    pub on_hotkey_changed: Option<HotkeyChangedCallback>,
}

/// The native palette window.
///
/// The Win32 window belongs to the thread that created it. Other threads
/// (global hotkey, system tray, single-instance watcher) interact with it
/// through [`PaletteHandle`].
pub struct NativePalette {
    palette: Palette,
}

impl NativePalette {
    /// Opens the palette on the current thread (standalone mode: exits on dismiss/activate).
    pub fn open(
        launcher: Arc<LauncherState>,
        runner: Arc<Runner>,
        theme: Theme,
    ) -> Result<NativePalette, String> {
        let palette = build(launcher, runner, None, true, theme)?;
        Ok(NativePalette { palette })
    }

    /// Opens the palette on the current thread with full application context
    /// (including native Settings persistence and live hotkey updates).
    pub fn open_with_context(
        launcher: Arc<LauncherState>,
        runner: Arc<Runner>,
        context: NativeContext,
        theme: Theme,
    ) -> Result<NativePalette, String> {
        let palette = build(launcher, runner, Some(context), false, theme)?;
        Ok(NativePalette { palette })
    }

    /// The cross-thread handle to this palette window.
    pub fn handle(&self) -> PaletteHandle {
        self.palette.handle()
    }

    /// Shows the palette and gives it the keyboard.
    pub fn show(&self) {
        self.palette.show();
    }

    /// Runs the window's blocking Win32 message loop until closed/quit.
    pub fn run(&self) {
        self.palette.run();
    }
}

#[derive(Default)]
enum Mode {
    #[default]
    Launcher,
    Settings,
}

/// What the palette is showing right now.
#[derive(Default)]
struct Shown {
    mode: Mode,
    query: String,
    rows: Vec<ResultRow>,
    settings_rows: Vec<SettingRow>,
}

fn build(
    launcher: Arc<LauncherState>,
    runner: Arc<Runner>,
    context: Option<NativeContext>,
    quit_on_close: bool,
    theme: Theme,
) -> Result<Palette, String> {
    let shown: Arc<Mutex<Shown>> = Arc::new(Mutex::new(Shown::default()));
    let context = context.map(Arc::new);

    let handler_shown = Arc::clone(&shown);
    let handler_launcher = Arc::clone(&launcher);
    let handler_runner = Arc::clone(&runner);
    let handler_context = context.clone();

    let handler = move |event: PaletteEvent| -> PaletteReply {
        match event {
            PaletteEvent::Shown => {
                if let Some(ctx) = handler_context.as_ref() {
                    ctx.palette.back_to_root();
                }
                let rows = handler_launcher.suggestions();
                let mut shown = handler_shown.lock().expect("the palette's state");
                shown.mode = Mode::Launcher;
                shown.query = String::new();
                shown.rows = rows.clone();
                shown.settings_rows.clear();
                PaletteReply::Update(model_for("", &rows))
            }
            PaletteEvent::Command(COMMAND_OPEN_SETTINGS) => {
                let Some(ctx) = handler_context.as_ref() else {
                    return PaletteReply::Keep;
                };
                ctx.palette.open_screen("Settings");
                let rows = collect_settings_rows(ctx, "");
                let mut shown = handler_shown.lock().expect("the palette's state");
                shown.mode = Mode::Settings;
                shown.query = String::new();
                shown.rows.clear();
                shown.settings_rows = rows.clone();
                PaletteReply::Update(settings_model_for("", &rows, 0))
            }
            PaletteEvent::Command(_) => PaletteReply::Keep,
            PaletteEvent::QueryChanged(query) => {
                let mut shown = handler_shown.lock().expect("the palette's state");
                match shown.mode {
                    Mode::Launcher => {
                        let rows = if query.trim().is_empty() {
                            handler_launcher.suggestions()
                        } else {
                            handler_launcher.search(&query)
                        };
                        shown.query = query.clone();
                        shown.rows = rows.clone();
                        PaletteReply::Update(model_for(&query, &rows))
                    }
                    Mode::Settings => {
                        let Some(ctx) = handler_context.as_ref() else {
                            return PaletteReply::Keep;
                        };
                        let rows = collect_settings_rows(ctx, &query);
                        shown.query = query.clone();
                        shown.settings_rows = rows.clone();
                        PaletteReply::Update(settings_model_for(&query, &rows, 0))
                    }
                }
            }
            PaletteEvent::Activated(index) => {
                let mut shown = handler_shown.lock().expect("the palette's state");
                match shown.mode {
                    Mode::Launcher => {
                        let Some(row) = shown.rows.get(index) else {
                            return PaletteReply::Keep;
                        };
                        let id = row.id.clone();
                        let query = shown.query.clone();
                        drop(shown);
                        let _ = handler_runner.open(&id, &query, false);
                        if quit_on_close {
                            PaletteReply::Quit
                        } else {
                            PaletteReply::Close
                        }
                    }
                    Mode::Settings => {
                        let Some(ctx) = handler_context.as_ref() else {
                            return PaletteReply::Keep;
                        };
                        let Some(setting) = shown.settings_rows.get(index) else {
                            return PaletteReply::Keep;
                        };
                        let key = setting.key.clone();
                        let query = shown.query.clone();
                        advance_native_setting(ctx, &key);
                        let updated = collect_settings_rows(ctx, &query);
                        let clamped = index.min(updated.len().saturating_sub(1));
                        shown.settings_rows = updated.clone();
                        PaletteReply::Update(settings_model_for(&query, &updated, clamped))
                    }
                }
            }
            PaletteEvent::Dismissed => {
                if quit_on_close {
                    PaletteReply::Quit
                } else {
                    PaletteReply::Keep
                }
            }
        }
    };

    let rows = launcher.suggestions();
    let model = model_for("", &rows);

    Palette::with_handler(theme, model, Box::new(handler))
}

/// Collect all native Settings rows (General + Launcher Hotkey), filtered by query.
fn collect_settings_rows(ctx: &NativeContext, query: &str) -> Vec<SettingRow> {
    let mut rows = ctx.palette.settings_rows();
    let current_hotkey = ctx
        .settings
        .get(KEY_LAUNCHER_HOTKEY)
        .unwrap_or_else(|| HotkeyTrigger::default_launcher().format());
    rows.insert(
        0,
        SettingRow {
            key: KEY_LAUNCHER_HOTKEY.to_string(),
            label: "Launcher Hotkey".to_string(),
            value: current_hotkey,
            options: HOTKEY_PRESETS.iter().map(|s| (*s).to_string()).collect(),
        },
    );

    let trimmed = query.trim().to_ascii_lowercase();
    if trimmed.is_empty() {
        return rows;
    }
    rows.into_iter()
        .filter(|row| {
            row.label.to_ascii_lowercase().contains(&trimmed)
                || row.value.to_ascii_lowercase().contains(&trimmed)
        })
        .collect()
}

/// Cycle one setting in native Settings and persist it to `settings.json`.
fn advance_native_setting(ctx: &NativeContext, key: &str) {
    if key == KEY_LAUNCHER_HOTKEY {
        let current = ctx
            .settings
            .get(KEY_LAUNCHER_HOTKEY)
            .unwrap_or_else(|| HOTKEY_PRESETS[0].to_string());
        let pos = HOTKEY_PRESETS
            .iter()
            .position(|preset| preset.eq_ignore_ascii_case(&current))
            .map(|i| (i + 1) % HOTKEY_PRESETS.len())
            .unwrap_or(0);
        let next = HOTKEY_PRESETS[pos];
        let _ = ctx.settings.set(KEY_LAUNCHER_HOTKEY, next);
        if let (Some(cb), Some(trigger)) = (&ctx.on_hotkey_changed, HotkeyTrigger::parse(next)) {
            cb(trigger);
        }
        return;
    }

    if let Some(next) = ctx.palette.advance_setting(key) {
        let _ = ctx.settings.set(key, &next);
    }
}

/// The palette's content for a set of launcher results.
fn model_for(query: &str, rows: &[ResultRow]) -> Model {
    Model {
        query: query.to_string(),
        rows: rows.iter().map(row_of).collect(),
        selected: 0,
        hovered: None,
        footer: FOOTER.to_string(),
    }
}

/// The palette's content for the native Settings screen.
fn settings_model_for(query: &str, rows: &[SettingRow], selected: usize) -> Model {
    let ui_rows = rows
        .iter()
        .map(|row| Row {
            name: row.label.clone(),
            detail: row.options.join("  /  "),
            shortcut: row.value.clone(),
            icon: Some(Icon::new("S", Color::rgb(0x4f, 0x46, 0xe5))),
        })
        .collect();
    Model {
        query: query.to_string(),
        rows: ui_rows,
        selected,
        hovered: None,
        footer: SETTINGS_FOOTER.to_string(),
    }
}

/// A result, as the palette draws it.
fn row_of(result: &ResultRow) -> Row {
    Row {
        name: result.name.clone(),
        detail: result.detail.clone(),
        shortcut: String::new(),
        icon: Some(icon_of(result)),
    }
}

/// The tile that leads a row.
fn icon_of(result: &ResultRow) -> Icon {
    Icon::new(letter_of(&result.name), color_of(&result.kind))
}

/// The letter in the tile.
fn letter_of(name: &str) -> String {
    match name.chars().next() {
        Some(letter) => letter.to_uppercase().to_string(),
        None => "·".to_string(),
    }
}

/// The colour of a section.
fn color_of(kind: &str) -> Color {
    match kind {
        APPLICATIONS => Color::rgb(0x0a, 0x63, 0xd8),
        FILE_SECTION => Color::rgb(0x1c, 0x93, 0xa8),
        COMMAND_SECTION => Color::rgb(0x2f, 0xa3, 0x6a),
        CUSTOM_COMMAND_SECTION => Color::rgb(0x7a, 0x4f, 0xd6),
        WINDOW_COMMAND_SECTION => Color::rgb(0xe0, 0x7b, 0x28),
        SYSTEM_ACTION_SECTION => Color::rgb(0xd6, 0x3b, 0x3b),
        CALCULATOR_SECTION => Color::rgb(0x0a, 0x63, 0xd8),
        _ => Color::rgb(0x64, 0x74, 0x8b),
    }
}

/// Runs the native palette on its own (`--native-palette`), without tray or global hotkey.
pub fn run(arguments: &[String]) -> Result<(), String> {
    let paths = tc_platform_win::windows_paths();
    let data = tc_platform_win::WindowsFileSystem::new();
    let _ = data.ensure_dir(&paths.data_dir);
    let _ = data.ensure_dir(&paths.cache_dir);

    let launcher = Arc::new(LauncherState::new(paths.data_dir.join(RANKING_FILE)));
    let runner = Arc::new(Runner::new(Arc::clone(&launcher)));

    let theme = theme(arguments)?;
    let palette = NativePalette::open(launcher, runner, theme)?;
    palette.show();
    palette.run();
    Ok(())
}

/// The theme the palette wears when invoked with `--style-pack`.
fn theme(arguments: &[String]) -> Result<Theme, String> {
    let position = arguments
        .iter()
        .position(|argument| argument == "--style-pack");
    match position.and_then(|at| arguments.get(at + 1)) {
        Some(path) => {
            let tokens = Tokens::load(std::path::Path::new(path))
                .map_err(|error| format!("the style pack could not be worn: {error}"))?;
            Ok(tokens.theme())
        }
        None => Ok(Theme::dark()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(name: &str, detail: &str, kind: &str) -> ResultRow {
        ResultRow {
            id: format!("app:{name}"),
            name: name.to_string(),
            detail: detail.to_string(),
            kind: kind.to_string(),
            target: name.to_string(),
            needs_confirm: false,
            arguments: Vec::new(),
            is_dir: None,
        }
    }

    #[test]
    fn a_result_becomes_a_row_with_its_name_and_detail() {
        let row = row_of(&result("Notepad", "Documents", APPLICATIONS));
        assert_eq!(row.name, "Notepad");
        assert_eq!(row.detail, "Documents");
        assert_eq!(row.icon.map(|icon| icon.letter), Some("N".to_string()));
    }

    #[test]
    fn the_letter_is_the_first_one_and_it_is_upper_case() {
        assert_eq!(letter_of("notepad"), "N");
        assert_eq!(letter_of("Éclair"), "É");
        assert_eq!(letter_of("7-Zip"), "7");
    }

    #[test]
    fn a_row_with_no_name_still_has_a_tile() {
        assert_eq!(letter_of(""), "·");
    }

    #[test]
    fn each_section_has_its_own_colour() {
        let applications = color_of(APPLICATIONS);
        let files = color_of(FILE_SECTION);
        let commands = color_of(COMMAND_SECTION);
        assert_ne!(applications, files);
        assert_ne!(files, commands);
        assert_ne!(applications, commands);
    }

    #[test]
    fn a_section_nobody_heard_of_gets_the_neutral_colour() {
        assert_eq!(color_of("Something Else"), Color::rgb(0x64, 0x74, 0x8b));
    }

    #[test]
    fn the_model_carries_the_query_and_the_rows() {
        let rows = vec![
            result("Notepad", "Documents", APPLICATIONS),
            result("Notes.txt", "Desktop", FILE_SECTION),
        ];
        let model = model_for("no", &rows);
        assert_eq!(model.query, "no");
        assert_eq!(model.rows.len(), 2);
        assert_eq!(model.rows[0].name, "Notepad");
        assert_eq!(model.rows[1].name, "Notes.txt");
        assert_eq!(model.selected, 0);
        assert!(model.footer.contains("Enter to open"));
    }

    #[test]
    fn an_empty_result_set_is_an_empty_list() {
        let model = model_for("nothing", &[]);
        assert!(model.rows.is_empty());
        assert_eq!(model.query, "nothing");
    }

    #[test]
    fn settings_model_displays_current_values_in_shortcut_column() {
        let rows = vec![SettingRow {
            key: KEY_LAUNCHER_HOTKEY.to_string(),
            label: "Launcher Hotkey".to_string(),
            value: "Alt+Space".to_string(),
            options: vec!["Alt+Space".to_string(), "Ctrl+Space".to_string()],
        }];
        let model = settings_model_for("", &rows, 0);
        assert_eq!(model.rows.len(), 1);
        assert_eq!(model.rows[0].name, "Launcher Hotkey");
        assert_eq!(model.rows[0].shortcut, "Alt+Space");
        assert!(model.footer.contains("Settings"));
    }
}
