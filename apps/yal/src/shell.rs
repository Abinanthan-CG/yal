//! The application shell: the palette window, the tray icon, the global
//! shortcut and the single-instance lock.
//!
//! The palette window is created once and hidden, so the hotkey shows it
//! instantly instead of building a window on every press.

use tauri::Emitter;
use tauri::Manager;
use tauri::State;
use tc_platform::{Clipboard, FileSystem, Settings};

use crate::launcher::{LauncherState, ResultRow, CALCULATOR_PREFIX, RANKING_FILE};

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
            app.manage(launcher);
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
            app.manage(tc_platform_win::window_control::WindowCommands::new(
                gap, cycle,
            ));
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
            if let Some(window) = app.get_webview_window(PALETTE) {
                if autostart {
                    let _ = window.hide();
                } else {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
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
            hide_palette
        ])
        .run(tauri::generate_context!())
        .map_err(|error| format!("the application could not start: {error}"))
}

/// The rows that match the query, best first.
#[tauri::command]
fn search(launcher: State<'_, LauncherState>, query: String) -> Vec<ResultRow> {
    launcher.search(&query)
}

/// The rows with nothing typed.
#[tauri::command]
fn suggestions(launcher: State<'_, LauncherState>) -> Vec<ResultRow> {
    launcher.suggestions()
}

/// Open the row, and learn that the user opened it.
#[tauri::command]
fn open_result(
    app: tauri::AppHandle,
    launcher: State<'_, LauncherState>,
    id: String,
    query: String,
    confirmed: bool,
) -> Result<(), String> {
    // A system action is run, not opened, and the ones that end the session or
    // empty the bin have to be asked for first.
    if let Some(action) = LauncherState::system_action_of(&id) {
        let needs_confirm = action.confirmation() != tc_core::SystemActionConfirmation::None;
        if needs_confirm && !confirmed {
            return Err(format!("`{}` has to be confirmed", action.english_name()));
        }
        tc_platform_win::run(action)?;
        if let Some(window) = app.get_webview_window(PALETTE) {
            let _ = window.hide();
        }
        return Ok(());
    }
    // A window command is not opened either; it moves the window the user
    // was last using.
    if let Some(command) = LauncherState::window_command_of(&id) {
        let commands: State<'_, tc_platform_win::window_control::WindowCommands> = app.state();
        let Some(handle) = tc_platform_win::window_control::last_foreground() else {
            return Err("there is no window to move".to_string());
        };
        commands
            .run(handle, command)
            .map_err(|error| error.to_string())?;
        if let Some(window) = app.get_webview_window(PALETTE) {
            let _ = window.hide();
        }
        return Ok(());
    }
    // A calculator row is not opened; its answer is copied.
    if let Some(text) = id.strip_prefix(CALCULATOR_PREFIX) {
        tc_platform_win::clipboard().write_text(text)?;
        if let Some(window) = app.get_webview_window(PALETTE) {
            let _ = window.hide();
        }
        return Ok(());
    }
    let Some((target, _)) = launcher.target_of(&id) else {
        return Err(format!("`{id}` is not a result the launcher knows"));
    };
    tc_platform_win::open(&target)?;
    // The ranking learns the query only when the thing really opened.
    launcher.remember(&id, &query);
    if let Some(window) = app.get_webview_window(PALETTE) {
        let _ = window.hide();
    }
    Ok(())
}

/// Hide the palette.
#[tauri::command]
fn hide_palette(app: tauri::AppHandle) {
    if let Some(window) = app.get_webview_window(PALETTE) {
        let _ = window.hide();
    }
}

/// Show the palette when it is hidden, hide it when it is shown.
fn toggle_palette(app: &tauri::AppHandle) {
    // The palette is a window too, so the window that was in front before it
    // opened is the one the commands move. It is remembered here, before the
    // palette takes the front.
    tc_platform_win::window_control::remember_foreground();
    if let Some(window) = app.get_webview_window(PALETTE) {
        match window.is_visible() {
            Ok(true) => {
                let _ = window.hide();
            }
            _ => {
                let _ = window.show();
                let _ = window.set_focus();
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
