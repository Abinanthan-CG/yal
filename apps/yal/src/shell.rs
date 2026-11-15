//! The application shell: the palette window, the tray icon, the global
//! shortcut and the single-instance lock.
//!
//! The palette window is created once and hidden, so the hotkey shows it
//! instantly instead of building a window on every press.

use std::sync::Arc;
use tauri::Emitter;
use tauri::Manager;
use tauri::State;
use tc_commands::catalog::{settings_key, CommandSettings};
use tc_commands::store::CustomCommandStore;
use tc_commands::COMMANDS_FILE;
use tc_core::CommandId;
use tc_platform::{Clipboard, FileSystem, Settings};
use tc_ui::Theme;

use crate::launcher::{FileSearch, LauncherState, ResultRow, RANKING_FILE};
use crate::native::NativePalette;
use crate::open::{run_custom, Runner};
use crate::palette::{KeyOutcome, KeyStroke, PaletteState, PaletteView, SettingRow};

/// The palette's label, so the window can be found again.
const PALETTE: &str = "palette";

/// The setting that turns window management on.
const WINDOW_MANAGEMENT_ENABLED: &str = "windowManagement.enabled";
/// The setting that lists the window commands in the launcher.
const WINDOW_MANAGEMENT_SHOW: &str = "windowManagement.showInLauncher";
/// The setting that holds the gap between windows, in points.
const WINDOW_MANAGEMENT_GAP: &str = "windowManagement.gap";
/// The setting that holds the cycling mode.
const WINDOW_MANAGEMENT_CYCLE: &str = "windowManagement.cycle";

/// The setting that turns the commands pane on.
const COMMANDS_ENABLED: &str = "commands.enabled";
/// The setting that turns the custom commands on. Off by default.
const CUSTOM_COMMANDS_ENABLED: &str = "commands.custom.enabled";
/// The setting that lists the custom commands in the launcher.
const CUSTOM_COMMANDS_SHOW: &str = "commands.custom.showInLauncher";

/// The setting that turns the file search on. Off by default, the way
/// upstream ships it.
pub const FILE_SEARCH_ENABLED: &str = "fileSearch.enabled";
/// The setting that lists the files in the launcher.
pub const FILE_SEARCH_SHOW: &str = "fileSearch.showInLauncher";

/// The name of the file index in the app's own folder.
///
/// "Off means gone: turning File Search off deletes the index."
pub const INDEX_FILE: &str = "file-index.sqlite3";

/// The tray menu's items, in the order they appear.
const TRAY_ITEMS: [&str; 6] = [
    "Open YAL",
    "Settings",
    "Check for Updates",
    "About YAL",
    "Support Tinycast",
    "Quit YAL",
];

/// The text `yal.exe --help` prints.
pub fn usage() -> String {
    "YAL - a Windows keyboard launcher, an unofficial port of Tinycast\n\
     \n\
     Usage: yal [options]\n\
     \n\
     Options:\n\
     \x20 --self-test        Run a headless check of every subsystem and print JSON\n\
     \x20 --version         Print the version\n\
     \x20 --help            Print this text\n\
     \x20 --autostart       Start hidden, the way the login item starts it\n\
     \x20 --quit-running    Ask a running instance to quit\n\
     \x20 <link>            Open a tinycast:// link\n"
        .to_string()
}

/// Run the application.
pub fn run(arguments: &[String]) -> Result<(), String> {
    if arguments.iter().any(|a| a == "--quit-running") {
        return quit_running();
    }

    let autostart = arguments.iter().any(|a| a == "--autostart");
    let link = arguments
        .iter()
        .find(|a| a.starts_with("tinycast://"))
        .cloned();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            // A second launch hands its arguments to the running instance
            // rather than starting another one.
            if let Some(window) = app.get_webview_window(PALETTE) {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, shortcut, event| {
                    if event.state() != tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        return;
                    }
                    let _ = shortcut;
                    toggle_palette(app);
                })
                .build(),
        )
        .setup(move |app| {
            let handle = app.handle().clone();
            // The data folder has to exist before anything can be written to
            // it, and the user should not have to know that.
            let paths = tc_platform_win::windows_paths();
            let data = tc_platform_win::WindowsFileSystem::new();
            let _ = data.ensure_dir(&paths.data_dir);
            let _ = data.ensure_dir(&paths.cache_dir);
            // The index and the learned ranking live in the app's own folder,
            // the way upstream keeps them in Application Support.
            let mut launcher = LauncherState::new(paths.data_dir.join(RANKING_FILE));
            // The settings say whether window management is on, whether its
            // commands are listed in the launcher, how far apart two windows
            // sit and what a repeated half does. Window management is off
            // until it is turned on, the way upstream ships it.
            let settings = tc_platform_win::WindowsSettings::load(paths.settings_file.clone());
            if settings.get(WINDOW_MANAGEMENT_ENABLED).as_deref() == Some("true") {
                let show = settings.get(WINDOW_MANAGEMENT_SHOW).as_deref() != Some("false");
                launcher.set_show_window_commands(show);
            }
            // The built-in commands, and the switches that decide which of them
            // exist. "Enable Commands" turns off every command in the Commands
            // pane, and the custom commands follow their own switch.
            let mut commands = CommandSettings::new();
            if settings.get(COMMANDS_ENABLED).as_deref() == Some("false") {
                commands.switches.commands_enabled = false;
            }
            if settings.get(CUSTOM_COMMANDS_ENABLED).as_deref() == Some("true") {
                commands.switches.custom_commands_enabled = true;
            }
            if settings.get(CUSTOM_COMMANDS_SHOW).as_deref() == Some("false") {
                commands.switches.custom_commands_in_launcher = false;
            }
            for command in CommandId::ALL {
                let key = settings_key(command);
                if let Some(alias) = settings.get(&format!("commands.{key}.alias")) {
                    commands.set_alias(command, Some(alias));
                }
                if let Some(shortcut) = settings.get(&format!("commands.{key}.shortcut")) {
                    commands.set_shortcut(command, Some(shortcut));
                }
                if settings.get(&format!("commands.{key}.listed")).as_deref() == Some("false") {
                    commands.set_listed(command, false);
                }
            }
            launcher.set_command_settings(commands);
            // The user's own commands live in a file beside the app's other
            // data, so a backup can carry them.
            let store = CustomCommandStore::open(paths.data_dir.join(COMMANDS_FILE))
                .unwrap_or_else(|_| CustomCommandStore::in_memory());
            launcher.set_custom_commands(store.commands().to_vec());
            // The file search, when it is on. While it is off there is no
            // index on the disk at all, the way upstream ships it.
            if settings.get(FILE_SEARCH_ENABLED).as_deref() == Some("true") {
                let index = paths.data_dir.join(INDEX_FILE);
                if let Ok(store) = tc_files::store::IndexStore::open(&index) {
                    launcher.set_file_search(FileSearch::new(
                        store,
                        paths.home_dir.clone(),
                        Vec::new(),
                    ));
                }
            }
            if settings.get(FILE_SEARCH_SHOW).as_deref() == Some("true") {
                launcher.set_show_files(true);
            }
            // The launcher is shared: the native palette searches it on its
            // own thread, and the browser palette searches it here.
            let launcher = Arc::new(launcher);
            app.manage(Arc::clone(&launcher));
            // The palette: the screens, the keys and the settings it follows.
            let palette = PaletteState::new();
            {
                let mut palette_settings = palette.lock();
                let mut own = tc_palette::PaletteSettings::default();
                if let Some(value) = settings.get(tc_palette::KEY_APPEARANCE) {
                    own.set_appearance(&value);
                }
                if let Some(value) = settings.get(tc_palette::KEY_ANIMATIONS) {
                    own.set_animations(&value);
                }
                if let Some(value) = settings.get(tc_palette::KEY_INTERFACE_SIZE) {
                    own.set_interface_size(&value);
                }
                if let Some(value) = settings.get(tc_palette::KEY_WINDOW_MODE) {
                    own.set_window_mode(&value);
                }
                if let Some(value) = settings.get(tc_palette::KEY_ESCAPE_BEHAVIOUR) {
                    own.set_escape_behaviour(&value);
                }
                if let Some(value) = settings.get(tc_palette::KEY_POP_TO_ROOT) {
                    own.set_pop_to_root(&value);
                }
                palette_settings.set_settings(own);
            }
            app.manage(palette);
            // The window commands, with the memory of the windows they have
            // moved. The memory lives in this process and is never written
            // down.
            let gap = settings
                .get(WINDOW_MANAGEMENT_GAP)
                .and_then(|value| value.parse::<f64>().ok())
                .unwrap_or(0.0);
            let cycle = settings
                .get(WINDOW_MANAGEMENT_CYCLE)
                .and_then(|value| tc_windows::geometry::WindowCycle::parse(&value))
                .unwrap_or(tc_windows::geometry::WindowCycle::Off);
            let window_commands = Arc::new(tc_platform_win::window_control::WindowCommands::new(
                gap, cycle,
            ));
            app.manage(Arc::clone(&window_commands));
            // The runner opens what the user chooses, whichever palette they
            // chose it in. Quitting is the application's own business, so it
            // is handed over rather than guessed at.
            let quitter = handle.clone();
            let runner = Arc::new(
                Runner::new(Arc::clone(&launcher))
                    .with_windows(Arc::clone(&window_commands))
                    .with_quit(Arc::new(move || quitter.exit(0))),
            );
            app.manage(Arc::clone(&runner));
            // The settings store itself, so a setting that is changed in the
            // palette is written down and kept.
            let theme = native_theme(&settings);
            app.manage(settings);
            // The native palette: the window the user actually types into.
            // It lives on a thread of its own, because a window with a
            // message loop cannot live on a thread that is already busy.
            let native = NativePalette::spawn(Arc::clone(&launcher), Arc::clone(&runner), theme)?;
            app.manage(native);
            // The installer asks a running instance to quit before it replaces
            // the program files, so the app has to be listening for that. The
            // watcher holds the quit event open, so it is kept in the app's
            // state rather than dropped here.
            let quitting = handle.clone();
            if let Ok(watcher) = tc_platform_win::QuitWatcher::spawn(move || {
                quitting.exit(0);
            }) {
                app.manage(watcher);
            }
            // The browser palette is the settings' home for now; the launcher
            // itself is the native window above.
            if let Some(window) = app.get_webview_window(PALETTE) {
                let _ = window.hide();
            }
            if !autostart {
                toggle_palette(&handle);
            }
            build_tray(&handle)?;
            if let Some(link) = &link {
                let _ = handle.emit_to("palette", "deeplink", link);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            search,
            suggestions,
            open_result,
            run_custom_command,
            hide_palette,
            palette_key,
            palette_view,
            palette_menu_query,
            palette_open_screen,
            palette_back,
            palette_back_to_root,
            file_rows,
            open_file,
            reveal_file,
            palette_settings,
            palette_advance_setting,
            palette_set_setting,
            file_action
        ])
        .run(tauri::generate_context!())
        .map_err(|error| format!("the application could not start: {error}"))
}

/// The rows that match the query, best first.
#[tauri::command]
fn search(launcher: State<'_, Arc<LauncherState>>, query: String) -> Vec<ResultRow> {
    launcher.search(&query)
}

/// The rows with nothing typed.
#[tauri::command]
fn suggestions(launcher: State<'_, Arc<LauncherState>>) -> Vec<ResultRow> {
    launcher.suggestions()
}

/// Open the row, and learn that the user opened it.
#[tauri::command]
fn open_result(
    app: tauri::AppHandle,
    runner: State<'_, Arc<Runner>>,
    id: String,
    query: String,
    confirmed: bool,
) -> Result<(), String> {
    let outcome = runner.open(&id, &query, confirmed);
    // A row that could not be run has nothing to show, so the palette stays
    // open: it is where the confirmation is asked for.
    if outcome.is_ok() {
        if let Some(window) = app.get_webview_window(PALETTE) {
            let _ = window.hide();
        }
    }
    outcome
}

/// Run one of the user's own commands, with the values it asked for.
///
/// The command runs off the interface thread, so a long command does not
/// freeze the palette.
#[tauri::command]
async fn run_custom_command(
    launcher: State<'_, Arc<LauncherState>>,
    id: String,
    values: Vec<String>,
) -> Result<String, String> {
    let command = launcher
        .custom_command_of(&id)
        .cloned()
        .ok_or_else(|| format!("`{id}` is not one of your commands"))?;
    tauri::async_runtime::spawn_blocking(move || run_custom(&command, &values))
        .await
        .map_err(|error| error.to_string())?
}

/// Hide the palette.
#[tauri::command]
fn hide_palette(app: tauri::AppHandle) {
    if let Some(window) = app.get_webview_window(PALETTE) {
        let _ = window.hide();
    }
}

/// Press a key.
///
/// The palette decides what the key means; the window is told what to do.
#[tauri::command]
fn palette_key(palette: State<'_, PaletteState>, stroke: KeyStroke, query: String) -> KeyOutcome {
    palette.key(&stroke, &query)
}

/// What the palette looks like right now.
#[tauri::command]
fn palette_view(palette: State<'_, PaletteState>) -> PaletteView {
    palette.view()
}

/// Type in the actions menu's search field.
#[tauri::command]
fn palette_menu_query(palette: State<'_, PaletteState>, query: String) {
    palette.menu_query(&query);
}

/// Open a screen by name.
#[tauri::command]
fn palette_open_screen(palette: State<'_, PaletteState>, screen: String) {
    palette.open_screen(&screen);
}

/// Go back one screen. `false` when there is nothing behind it.
#[tauri::command]
fn palette_back(palette: State<'_, PaletteState>) -> bool {
    palette.back()
}

/// Go back to the root search from any screen.
#[tauri::command]
fn palette_back_to_root(palette: State<'_, PaletteState>) {
    palette.back_to_root();
}

/// The rows of the file-search screen.
///
/// With nothing typed the list is the recent files; with something typed it
/// is the results, narrowed by the filter the palette holds.
#[tauri::command]
fn file_rows(
    launcher: State<'_, Arc<LauncherState>>,
    palette: State<'_, PaletteState>,
    query: String,
) -> Vec<ResultRow> {
    let filter = palette.lock().file_filter();
    launcher.file_screen_rows(&query, filter)
}

/// Open a file or a folder.
#[tauri::command]
fn open_file(app: tauri::AppHandle, path: String) -> Result<(), String> {
    tc_platform_win::open(&path)?;
    if let Some(window) = app.get_webview_window(PALETTE) {
        let _ = window.hide();
    }
    Ok(())
}

/// Show a file in the file manager, with it selected.
///
/// This is the second action of a file row: "Show in File Explorer".
#[tauri::command]
fn reveal_file(app: tauri::AppHandle, path: String) -> Result<(), String> {
    tc_platform_win::reveal(&path)?;
    if let Some(window) = app.get_webview_window(PALETTE) {
        let _ = window.hide();
    }
    Ok(())
}

/// The Settings pane's rows.
#[tauri::command]
fn palette_settings(palette: State<'_, PaletteState>) -> Vec<SettingRow> {
    palette.settings_rows()
}

/// Move a setting to its next value, and write it down.
#[tauri::command]
fn palette_advance_setting(
    palette: State<'_, PaletteState>,
    settings: State<'_, tc_platform_win::WindowsSettings>,
    key: String,
) -> Result<String, String> {
    let value = palette
        .advance_setting(&key)
        .ok_or_else(|| format!("`{key}` is not a setting of the palette"))?;
    settings.set(&key, &value)?;
    Ok(value)
}

/// Set a setting to a value, and write it down.
#[tauri::command]
fn palette_set_setting(
    palette: State<'_, PaletteState>,
    settings: State<'_, tc_platform_win::WindowsSettings>,
    key: String,
    value: String,
) -> Result<(), String> {
    if !palette.set_setting(&key, &value) {
        return Err(format!("`{key}` is not a setting of the palette"));
    }
    settings.set(&key, &value)?;
    Ok(())
}

/// Run one of a file row's actions.
///
/// The action is the one the palette decided on, and the path is the row's
/// own file.
#[tauri::command]
fn file_action(app: tauri::AppHandle, action: String, path: String) -> Result<String, String> {
    let name = std::path::Path::new(&path)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| path.clone());
    match action.as_str() {
        "CopyName" => {
            tc_platform_win::clipboard().write_text(&name)?;
            Ok("The name is on the clipboard.".to_string())
        }
        "CopyPath" => {
            tc_platform_win::clipboard().write_text(&path)?;
            Ok("The path is on the clipboard.".to_string())
        }
        "MoveToRecycleBin" => {
            tc_platform_win::recycle(&path)?;
            if let Some(window) = app.get_webview_window(PALETTE) {
                let _ = window.hide();
            }
            Ok("It is in the Recycle Bin.".to_string())
        }
        other => Err(format!("`{other}` is not an action of a file row")),
    }
}

/// Show the palette when it is hidden, hide it when it is shown.
fn toggle_palette(app: &tauri::AppHandle) {
    // The palette is a window too, so the window that was in front before it
    // opened is the one the commands move. It is remembered here, before the
    // palette takes the front.
    tc_platform_win::window_control::remember_foreground();
    let Some(native) = app.try_state::<tc_ui::PaletteHandle>() else {
        return;
    };
    if tc_ui::is_visible(*native) {
        tc_ui::post_close(*native);
    } else {
        tc_ui::post_show(*native);
    }
}

/// The theme the native palette wears.
///
/// "Light" and "Dark" say what they say. "System" - which is what the
/// setting is called when it has not been changed - follows Windows, which
/// says whether it is dark in its own registry.
fn native_theme(settings: &tc_platform_win::WindowsSettings) -> Theme {
    let appearance = settings
        .get(tc_palette::KEY_APPEARANCE)
        .unwrap_or_else(|| "System".to_string());
    match appearance.trim().to_lowercase().as_str() {
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

/// Build the notification-area icon and its menu.
fn build_tray(app: &tauri::AppHandle) -> Result<(), String> {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    let mut items: Vec<MenuItem<tauri::Wry>> = Vec::new();
    for label in TRAY_ITEMS {
        items.push(
            MenuItem::with_id(app, label, label, true, None::<&str>)
                .map_err(|error| format!("the tray menu could not be built: {error}"))?,
        );
    }
    let refs: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> = items
        .iter()
        .map(|item| item as &dyn tauri::menu::IsMenuItem<tauri::Wry>)
        .collect();
    let menu = Menu::with_items(app, &refs)
        .map_err(|error| format!("the tray menu could not be built: {error}"))?;

    let icon = tauri::image::Image::from_bytes(include_bytes!("../../../assets/yal.png"))
        .map_err(|error| format!("the tray icon could not be read: {error}"))?;
    TrayIconBuilder::with_id("yal-tray")
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            let id = event.id().as_ref();
            match id {
                "Open YAL" => toggle_palette(app),
                "Quit YAL" => app.exit(0),
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_palette(tray.app_handle());
            }
        })
        .build(app)
        .map_err(|error| format!("the tray icon could not be built: {error}"))?;
    Ok(())
}

/// Ask a running instance to quit.
fn quit_running() -> Result<(), String> {
    if tc_platform_win::signal_quit() {
        println!("yal: a running instance was asked to quit");
        Ok(())
    } else {
        println!("yal: no running instance");
        Ok(())
    }
}
