//! The native palette: the Direct2D window the user types into.
//!
//! Pure Win32 + Direct2D + DirectWrite — no WebView2, no browser runtime.
//! Renders real Windows application icons, section headers, rich Calculator
//! hero cards, keycap footer hints, and live-themed native Settings.

use std::collections::{HashMap, HashSet};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use tc_platform::{FileSystem, Settings};
use tc_platform_win::{HotkeyTrigger, WindowsSettings};
use tc_ui::{
    Color, Icon, IconBitmap, Model, Palette, PaletteEvent, PaletteHandle, PaletteReply, Row, Theme,
    Tokens,
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
    /// (including native Settings persistence and live hotkey/theme updates).
    pub fn open_with_context(
        launcher: Arc<LauncherState>,
        runner: Arc<Runner>,
        context: NativeContext,
        theme: Theme,
    ) -> Result<NativePalette, String> {
        let palette = build(launcher, runner, Some(context), false, theme)?;
        Ok(NativePalette { palette })
    }

    pub fn handle(&self) -> PaletteHandle {
        self.palette.handle()
    }

    pub fn show(&self) {
        self.palette.show();
    }

    pub fn run(&self) {
        self.palette.run();
    }
}

#[derive(Default, PartialEq, Eq)]
enum Mode {
    #[default]
    Launcher,
    Settings,
}

struct Shown {
    mode: Mode,
    query: String,
    rows: Vec<ResultRow>,
    settings_rows: Vec<SettingRow>,
    icons: Arc<Mutex<HashMap<String, Option<Arc<IconBitmap>>>>>,
    queued: HashSet<String>,
    icon_tx: Option<mpsc::Sender<String>>,
}

impl Shown {
    fn new(
        icons: Arc<Mutex<HashMap<String, Option<Arc<IconBitmap>>>>>,
        icon_tx: mpsc::Sender<String>,
    ) -> Self {
        Self {
            mode: Mode::Launcher,
            query: String::new(),
            rows: Vec::new(),
            settings_rows: Vec::new(),
            icons,
            queued: HashSet::new(),
            icon_tx: Some(icon_tx),
        }
    }

    /// Non-blocking O(1) cache lookup. If an icon is not yet decoded, queues it
    /// to the background `yal-icons` worker so the UI thread never stalls.
    fn lookup_or_queue_icon(&mut self, target: &str) -> Option<Arc<IconBitmap>> {
        if target.trim().is_empty() {
            return None;
        }
        if let Ok(cache) = self.icons.lock() {
            if let Some(cached) = cache.get(target) {
                return cached.clone();
            }
        }
        if self.queued.insert(target.to_string()) {
            if let Some(tx) = self.icon_tx.as_ref() {
                let _ = tx.send(target.to_string());
            }
        }
        None
    }
}

fn spawn_icon_worker(
    icons: Arc<Mutex<HashMap<String, Option<Arc<IconBitmap>>>>>,
    handle_slot: Arc<Mutex<Option<PaletteHandle>>>,
) -> mpsc::Sender<String> {
    let (tx, rx) = mpsc::channel::<String>();
    let _ = std::thread::Builder::new()
        .name("yal-icons".to_string())
        .spawn(move || {
            while let Ok(first) = rx.recv() {
                let mut batch = vec![first];
                while let Ok(next) = rx.recv_timeout(Duration::from_millis(12)) {
                    batch.push(next);
                    if batch.len() >= 12 {
                        break;
                    }
                }
                let mut decoded_any = false;
                for target in batch {
                    let already = icons
                        .lock()
                        .ok()
                        .is_some_and(|map| map.contains_key(&target));
                    if already {
                        continue;
                    }
                    let extracted = tc_platform_win::extract_icon(&target).map(|img| {
                        Arc::new(IconBitmap {
                            width: img.width,
                            height: img.height,
                            premultiplied_rgba: img.premultiplied_rgba,
                        })
                    });
                    if extracted.is_some() {
                        decoded_any = true;
                    }
                    if let Ok(mut map) = icons.lock() {
                        if map.len() >= 512 {
                            map.clear();
                        }
                        map.insert(target, extracted);
                    }
                }
                if decoded_any {
                    if let Ok(guard) = handle_slot.lock() {
                        if let Some(handle) = *guard {
                            tc_ui::post_refresh(handle);
                        }
                    }
                }
            }
        });
    tx
}

fn build(
    launcher: Arc<LauncherState>,
    runner: Arc<Runner>,
    context: Option<NativeContext>,
    quit_on_close: bool,
    theme: Theme,
) -> Result<Palette, String> {
    let icons: Arc<Mutex<HashMap<String, Option<Arc<IconBitmap>>>>> =
        Arc::new(Mutex::new(HashMap::new()));
    let handle_slot: Arc<Mutex<Option<PaletteHandle>>> = Arc::new(Mutex::new(None));
    let icon_tx = spawn_icon_worker(Arc::clone(&icons), Arc::clone(&handle_slot));
    let shown: Arc<Mutex<Shown>> = Arc::new(Mutex::new(Shown::new(icons, icon_tx)));
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
                PaletteReply::Update(model_for_with_icons("", &rows, true, &mut shown))
            }
            PaletteEvent::Refresh => {
                let mut shown = handler_shown.lock().expect("the palette's state");
                if shown.mode != Mode::Launcher {
                    return PaletteReply::Keep;
                }
                let query = shown.query.clone();
                let rows = shown.rows.clone();
                let is_suggestions = query.trim().is_empty();
                PaletteReply::Update(model_for_with_icons(
                    &query,
                    &rows,
                    is_suggestions,
                    &mut shown,
                ))
            }
            PaletteEvent::Command(COMMAND_OPEN_SETTINGS) | PaletteEvent::OpenSettings => {
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
                        let is_suggestions = query.trim().is_empty();
                        let rows = if is_suggestions {
                            handler_launcher.suggestions()
                        } else {
                            handler_launcher.search(&query)
                        };
                        shown.query = query.clone();
                        shown.rows = rows.clone();
                        PaletteReply::Update(model_for_with_icons(
                            &query,
                            &rows,
                            is_suggestions,
                            &mut shown,
                        ))
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
                        let model = settings_model_for(&query, &updated, clamped);
                        if key == tc_palette::KEY_APPEARANCE {
                            PaletteReply::UpdateTheme(
                                Box::new(theme_from_settings(&ctx.settings)),
                                model,
                            )
                        } else {
                            PaletteReply::Update(model)
                        }
                    }
                }
            }
            PaletteEvent::Escape => {
                let mut shown = handler_shown.lock().expect("the palette's state");
                if shown.mode == Mode::Settings {
                    if let Some(ctx) = handler_context.as_ref() {
                        ctx.palette.back_to_root();
                    }
                    let rows = handler_launcher.suggestions();
                    shown.mode = Mode::Launcher;
                    shown.query = String::new();
                    shown.rows = rows.clone();
                    shown.settings_rows.clear();
                    return PaletteReply::Update(model_for_with_icons("", &rows, true, &mut shown));
                }
                if quit_on_close {
                    PaletteReply::Quit
                } else {
                    PaletteReply::Close
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
    let initial_model = {
        let mut guard = shown.lock().expect("the palette's state");
        guard.rows = rows.clone();
        model_for_with_icons("", &rows, true, &mut guard)
    };

    let palette = Palette::with_handler(theme, initial_model, Box::new(handler))?;
    *handle_slot.lock().expect("icon handle slot") = Some(palette.handle());
    Ok(palette)
}

fn theme_from_settings(settings: &WindowsSettings) -> Theme {
    let appearance = settings
        .get(tc_palette::KEY_APPEARANCE)
        .unwrap_or_else(|| "System".to_string());
    match appearance.trim().to_ascii_lowercase().as_str() {
        "light" => Theme::light(),
        "dark" => Theme::dark(),
        _ => {
            if tc_platform_win::apps_use_dark_theme() {
                Theme::dark()
            } else {
                Theme::light()
            }
        }
    }
}

/// Collect all native Settings rows (Hotkey + General), filtered by query.
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

fn model_for_with_icons(
    query: &str,
    rows: &[ResultRow],
    is_suggestions: bool,
    shown: &mut Shown,
) -> Model {
    // Only extract real icons for up to the first 30 rows to keep typing instant.
    let ui_rows: Vec<Row> = rows
        .iter()
        .enumerate()
        .map(|(idx, result)| {
            let mut row = row_of(result, query, is_suggestions);
            if idx < 30 && matches!(result.kind.as_str(), APPLICATIONS | FILE_SECTION) {
                if let Some(bitmap) = shown.lookup_or_queue_icon(&result.target) {
                    row = row.with_bitmap(result.target.clone(), bitmap);
                }
            }
            row
        })
        .collect();

    let count_label = match ui_rows.len() {
        0 => "No results".to_string(),
        1 => "1 result".to_string(),
        n => format!("{n} results"),
    };

    Model {
        query: query.to_string(),
        rows: ui_rows,
        selected: 0,
        hovered: None,
        footer: count_label.clone(),
        placeholder: "Search apps, commands, or calculate…".to_string(),
        header_badge: None,
        result_count: Some(count_label),
        footer_hints: vec![
            ("↵".to_string(), "Open".to_string()),
            ("↑↓".to_string(), "Move".to_string()),
            ("Ctrl+,".to_string(), "Settings".to_string()),
            ("Esc".to_string(), "Close".to_string()),
        ],
        scroll_offset: 0.0,
    }
}

/// Pure helper without live shell icon lookup (used by unit tests).
#[cfg(test)]
fn model_for(query: &str, rows: &[ResultRow]) -> Model {
    let ui_rows: Vec<Row> = rows
        .iter()
        .map(|r| row_of(r, query, query.trim().is_empty()))
        .collect();
    Model {
        query: query.to_string(),
        rows: ui_rows,
        selected: 0,
        hovered: None,
        footer: "Enter to open".to_string(),
        placeholder: "Search apps, commands, or calculate…".to_string(),
        header_badge: None,
        result_count: None,
        footer_hints: vec![
            ("↵".to_string(), "Open".to_string()),
            ("Esc".to_string(), "Close".to_string()),
        ],
        scroll_offset: 0.0,
    }
}

/// The palette's content for the native Settings screen.
fn settings_model_for(query: &str, rows: &[SettingRow], selected: usize) -> Model {
    let ui_rows: Vec<Row> = rows
        .iter()
        .map(|row| {
            let section = if row.key == KEY_LAUNCHER_HOTKEY {
                "Hotkeys"
            } else {
                "General"
            };
            let glyph = if row.key == KEY_LAUNCHER_HOTKEY {
                "⌘"
            } else {
                "⚙"
            };
            let display_value = match row.value.trim().to_ascii_lowercase().as_str() {
                "true" => "On".to_string(),
                "false" => "Off".to_string(),
                _ => row.value.clone(),
            };
            let description = match row.key.as_str() {
                KEY_LAUNCHER_HOTKEY => "Global shortcut to summon YAL",
                tc_palette::KEY_APPEARANCE => "Color theme",
                tc_palette::KEY_ANIMATIONS => "Motion & transitions",
                tc_palette::KEY_INTERFACE_SIZE => "Palette text scale",
                tc_palette::KEY_WINDOW_MODE => "Compact or expanded list",
                tc_palette::KEY_ESCAPE_BEHAVIOUR => "Action on Esc key",
                tc_palette::KEY_POP_TO_ROOT => "Reset screen after hiding",
                _ => "",
            };
            Row {
                name: row.label.clone(),
                detail: description.to_string(),
                shortcut: display_value,
                icon: Some(Icon::new(glyph, Color::rgb(0x5e, 0x8b, 0xff))),
                section: section.to_string(),
                ..Row::default()
            }
        })
        .collect();

    let count_label = match ui_rows.len() {
        1 => "1 setting".to_string(),
        n => format!("{n} settings"),
    };

    Model {
        query: query.to_string(),
        rows: ui_rows,
        selected,
        hovered: None,
        footer: count_label.clone(),
        placeholder: "Filter settings…".to_string(),
        header_badge: Some("Settings".to_string()),
        result_count: Some(count_label),
        footer_hints: vec![
            ("↵".to_string(), "Cycle".to_string()),
            ("↑↓".to_string(), "Move".to_string()),
            ("Esc".to_string(), "Back".to_string()),
        ],
        scroll_offset: 0.0,
    }
}

/// A result, as the palette draws it.
fn row_of(result: &ResultRow, query: &str, is_suggestions: bool) -> Row {
    if result.kind == CALCULATOR_SECTION {
        let expr = if result.detail.is_empty() {
            query.trim().to_string()
        } else {
            result.detail.clone()
        };
        let source_badge = result
            .calc_source_badge
            .clone()
            .filter(|b| !b.is_empty())
            .unwrap_or_else(|| "Expression".to_string());
        let badge = result
            .calc_badge
            .clone()
            .filter(|b| !b.is_empty())
            .unwrap_or_else(|| "Result".to_string());
        return Row {
            name: result.name.clone(),
            detail: expr,
            shortcut: String::new(),
            icon: Some(Icon::new("=", color_of(&result.kind))),
            section: CALCULATOR_SECTION.to_string(),
            source_badge,
            badge,
            is_calculator: true,
            color_swatch: result.color_swatch,
            ..Row::default()
        };
    }

    let section = if is_suggestions && result.kind == APPLICATIONS {
        "Suggestions".to_string()
    } else {
        result.kind.clone()
    };

    let detail = if result.detail.is_empty() && result.kind != APPLICATIONS {
        result.kind.clone()
    } else {
        result.detail.clone()
    };

    Row {
        name: result.name.clone(),
        detail,
        shortcut: String::new(),
        icon: Some(icon_of(result)),
        section,
        ..Row::default()
    }
}

fn icon_of(result: &ResultRow) -> Icon {
    let glyph = match result.kind.as_str() {
        CALCULATOR_SECTION => "=".to_string(),
        COMMAND_SECTION => "›".to_string(),
        CUSTOM_COMMAND_SECTION => ">_".to_string(),
        WINDOW_COMMAND_SECTION => "⊞".to_string(),
        SYSTEM_ACTION_SECTION => "⚡".to_string(),
        FILE_SECTION => "📄".to_string(),
        _ => letter_of(&result.name),
    };
    Icon::new(glyph, color_of(&result.kind))
}

fn letter_of(name: &str) -> String {
    match name.chars().next() {
        Some(letter) => letter.to_uppercase().to_string(),
        None => "·".to_string(),
    }
}

fn color_of(kind: &str) -> Color {
    match kind {
        APPLICATIONS => Color::rgb(0x5e, 0x8b, 0xff),
        FILE_SECTION => Color::rgb(0x38, 0xbd, 0xf8),
        COMMAND_SECTION => Color::rgb(0x34, 0xd3, 0x99),
        CUSTOM_COMMAND_SECTION => Color::rgb(0xa7, 0x8b, 0xfa),
        WINDOW_COMMAND_SECTION => Color::rgb(0xfb, 0x92, 0x3c),
        SYSTEM_ACTION_SECTION => Color::rgb(0xf8, 0x71, 0x71),
        CALCULATOR_SECTION => Color::rgb(0x60, 0xa5, 0xfa),
        _ => Color::rgb(0x94, 0xa3, 0xb8),
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
            calc_source_badge: None,
            calc_badge: None,
            color_swatch: None,
        }
    }

    #[test]
    fn a_result_becomes_a_row_with_its_name_and_detail() {
        let row = row_of(&result("Notepad", "Documents", APPLICATIONS), "no", false);
        assert_eq!(row.name, "Notepad");
        assert_eq!(row.detail, "Documents");
        assert_eq!(row.icon.map(|icon| icon.letter), Some("N".to_string()));
        assert_eq!(row.section, APPLICATIONS);
    }

    #[test]
    fn calculator_result_renders_as_hero_card_with_badge() {
        let mut calc = result("17.00 USD", "20% tip on $85", CALCULATOR_SECTION);
        calc.calc_badge = Some("Tip".to_string());
        let row = row_of(&calc, "20% tip on $85", false);
        assert!(row.is_calculator);
        assert_eq!(row.name, "17.00 USD");
        assert_eq!(row.detail, "20% tip on $85");
        assert_eq!(row.badge, "Tip");
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
        assert_eq!(color_of("Something Else"), Color::rgb(0x94, 0xa3, 0xb8));
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
        assert!(!model.footer_hints.is_empty());
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
        assert_eq!(model.header_badge.as_deref(), Some("Settings"));
    }
}
