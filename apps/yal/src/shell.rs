//! Pure-native Win32 application shell: Direct2D palette, global hotkey
//! engine, system tray icon, single-instance lock, and cross-process signals.
//!
//! Zero browser runtime, zero WebView2, zero Node/Tauri.

use std::sync::{Arc, Mutex};

use tc_commands::catalog::{settings_key, CommandSettings};
use tc_commands::store::CustomCommandStore;
use tc_commands::COMMANDS_FILE;
use tc_core::CommandId;
use tc_platform::{FileSystem, Settings, SingleInstance};
use tc_platform_win::{
    GlobalHotkeyService, HotkeyBinding, HotkeyTrigger, QuitWatcher, ShowWatcher, TrayAction,
    WindowsSettings, WindowsSingleInstance, WindowsTray,
};
use tc_ui::{PaletteHandle, Theme, Tokens};

use crate::launcher::{FileSearch, LauncherState, RANKING_FILE};
use crate::native::{NativeContext, NativePalette, COMMAND_OPEN_SETTINGS, KEY_LAUNCHER_HOTKEY};
use crate::open::Runner;
use crate::palette::PaletteState;

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

/// The setting that turns file search on. Off by default.
pub const FILE_SEARCH_ENABLED: &str = "fileSearch.enabled";
/// The setting that lists files in the launcher.
pub const FILE_SEARCH_SHOW: &str = "fileSearch.showInLauncher";

/// The name of the file index in the app's own folder.
pub const INDEX_FILE: &str = "file-index.sqlite3";

/// Hotkey binding ID for toggling the launcher palette.
const BINDING_LAUNCHER_TOGGLE: &str = "launcher.toggle";

/// The text `yal.exe --help` prints.
pub fn usage() -> String {
    "YAL - a native Windows keyboard launcher, an unofficial port of Tinycast\n\
     \n\
     Usage: yal [options]\n\
     \n\
     Options:\n\
     \x20 --self-test            Run a headless check of every subsystem and print JSON\n\
     \x20 --version              Print the version\n\
     \x20 --help                 Print this text\n\
     \x20 --autostart            Start hidden in the system tray\n\
     \x20 --settings             Open the native settings screen\n\
     \x20 --style-pack <file>    Load a JSON style pack for the palette\n\
     \x20 --native-palette       Open the palette window alone without tray or hotkey\n\
     \x20 --quit-running         Ask a running instance to quit cleanly\n"
        .to_string()
}

/// Run the native YAL application.
pub fn run(arguments: &[String]) -> Result<(), String> {
    if arguments.iter().any(|a| a == "--quit-running") {
        return quit_running();
    }

    // Single-instance guard: if YAL is already running, wake it and exit.
    let single_instance = WindowsSingleInstance::new();
    if !single_instance.acquire() {
        let _ = tc_platform_win::signal_show();
        return Ok(());
    }

    // Shared slot filled once the native palette window is created on this thread.
    let handle_slot: Arc<Mutex<Option<PaletteHandle>>> = Arc::new(Mutex::new(None));

    // Create the wake watcher immediately after acquiring the single-instance lock
    // so a rapid second launch during startup never hits a missing named event.
    let show_slot = Arc::clone(&handle_slot);
    let show_watcher = ShowWatcher::spawn(move || {
        if let Some(handle) = *show_slot.lock().expect("handle slot") {
            toggle_palette(handle);
        }
    })
    .ok();

    let autostart = arguments.iter().any(|a| a == "--autostart");
    let open_settings = arguments.iter().any(|a| a == "--settings");

    let paths = tc_platform_win::windows_paths();
    let fs = tc_platform_win::WindowsFileSystem::new();
    let _ = fs.ensure_dir(&paths.data_dir);
    let _ = fs.ensure_dir(&paths.cache_dir);

    let settings = Arc::new(WindowsSettings::load(paths.settings_file.clone()));
    let launcher = Arc::new(build_launcher(&paths, &settings));
    let palette_state = Arc::new(build_palette_state(&settings));
    let window_commands = Arc::new(build_window_commands(&settings));

    let quit_slot = Arc::clone(&handle_slot);
    let runner = Arc::new(
        Runner::new(Arc::clone(&launcher))
            .with_windows(window_commands)
            .with_quit(Arc::new(move || {
                if let Some(handle) = *quit_slot.lock().expect("handle slot") {
                    tc_ui::post_quit(handle);
                }
            })),
    );

    // Slot for live-updating the global hotkey when changed in Settings.
    let hotkey_slot: Arc<Mutex<Option<GlobalHotkeyService>>> = Arc::new(Mutex::new(None));
    let hotkey_updater = Arc::clone(&hotkey_slot);

    let theme = resolve_theme(arguments, &settings)?;
    let native = NativePalette::open_with_context(
        Arc::clone(&launcher),
        Arc::clone(&runner),
        NativeContext {
            palette: Arc::clone(&palette_state),
            settings: Arc::clone(&settings),
            on_hotkey_changed: Some(Arc::new(move |trigger| {
                if let Ok(guard) = hotkey_updater.lock() {
                    if let Some(service) = guard.as_ref() {
                        service.set_bindings(vec![HotkeyBinding::new(
                            BINDING_LAUNCHER_TOGGLE,
                            trigger,
                        )]);
                    }
                }
            })),
        },
        theme,
    )?;

    let handle = native.handle();
    *handle_slot.lock().expect("handle slot") = Some(handle);

    // Start the global hotkey service (defaults to Alt+Space, configurable in settings.json).
    let trigger = configured_launcher_hotkey(&settings);
    match GlobalHotkeyService::spawn(
        vec![HotkeyBinding::new(BINDING_LAUNCHER_TOGGLE, trigger)],
        move |binding_id| {
            if binding_id == BINDING_LAUNCHER_TOGGLE {
                toggle_palette(handle);
            }
        },
    ) {
        Ok(service) => {
            *hotkey_slot.lock().expect("hotkey slot") = Some(service);
        }
        Err(error) => {
            eprintln!("yal: global hotkey unavailable: {error}");
        }
    }

    // Start the native Win32 system tray icon.
    let tray = match WindowsTray::spawn(move |action| match action {
        TrayAction::Open => tc_ui::post_toggle_from_tray(handle),
        TrayAction::Settings => tc_ui::post_command(handle, COMMAND_OPEN_SETTINGS),
        TrayAction::Support => {
            std::thread::spawn(|| {
                if let Err(error) = tc_platform_win::open("https://tinycast.dev/support") {
                    eprintln!("yal: {error}");
                }
            });
        }
        TrayAction::Quit => tc_ui::post_quit(handle),
    }) {
        Ok(tray) => Some(tray),
        Err(error) => {
            eprintln!("yal: tray icon unavailable: {error}");
            None
        }
    };

    // Listen for `--quit-running` (installer / CLI shutdown).
    let quit_watcher = QuitWatcher::spawn(move || {
        tc_ui::post_quit(handle);
    })
    .ok();

    // Initial visibility.
    if open_settings {
        tc_ui::post_command(handle, COMMAND_OPEN_SETTINGS);
    } else if !autostart {
        tc_platform_win::window_control::remember_foreground();
        native.show();
    }

    // Block on the native palette's Win32 message loop until Quit.
    native.run();

    // Explicit ordered teardown before releasing the single-instance lock.
    drop(show_watcher);
    drop(quit_watcher);
    drop(tray);
    if let Ok(mut guard) = hotkey_slot.lock() {
        if let Some(mut svc) = guard.take() {
            svc.stop();
        }
    }
    single_instance.release();
    Ok(())
}

fn build_launcher(paths: &tc_platform::Paths, settings: &WindowsSettings) -> LauncherState {
    let mut launcher = LauncherState::new(paths.data_dir.join(RANKING_FILE));

    if settings.get(WINDOW_MANAGEMENT_ENABLED).as_deref() == Some("true") {
        let show = settings.get(WINDOW_MANAGEMENT_SHOW).as_deref() != Some("false");
        launcher.set_show_window_commands(show);
    }

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

    let store = CustomCommandStore::open(paths.data_dir.join(COMMANDS_FILE))
        .unwrap_or_else(|_| CustomCommandStore::in_memory());
    launcher.set_custom_commands(store.commands().to_vec());

    if settings.get(FILE_SEARCH_ENABLED).as_deref() == Some("true") {
        let index = paths.data_dir.join(INDEX_FILE);
        if let Ok(store) = tc_files::store::IndexStore::open(&index) {
            launcher.set_file_search(FileSearch::new(store, paths.home_dir.clone(), Vec::new()));
        }
    }
    if settings.get(FILE_SEARCH_SHOW).as_deref() == Some("true") {
        launcher.set_show_files(true);
    }

    launcher
}

fn build_palette_state(settings: &WindowsSettings) -> PaletteState {
    let palette = PaletteState::new();
    let mut guard = palette.lock();
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
    guard.set_settings(own);
    drop(guard);
    palette
}

fn build_window_commands(
    settings: &WindowsSettings,
) -> tc_platform_win::window_control::WindowCommands {
    let gap = settings
        .get(WINDOW_MANAGEMENT_GAP)
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(0.0);
    let cycle = settings
        .get(WINDOW_MANAGEMENT_CYCLE)
        .and_then(|value| tc_windows::geometry::WindowCycle::parse(&value))
        .unwrap_or(tc_windows::geometry::WindowCycle::Off);
    tc_platform_win::window_control::WindowCommands::new(gap, cycle)
}

fn configured_launcher_hotkey(settings: &WindowsSettings) -> HotkeyTrigger {
    settings
        .get(KEY_LAUNCHER_HOTKEY)
        .and_then(|value| HotkeyTrigger::parse(&value))
        .unwrap_or_else(HotkeyTrigger::default_launcher)
}

fn toggle_palette(handle: PaletteHandle) {
    if tc_ui::is_visible(handle) {
        tc_ui::post_close(handle);
    } else {
        tc_platform_win::window_control::remember_foreground();
        tc_ui::post_show(handle);
    }
}

fn resolve_theme(arguments: &[String], settings: &WindowsSettings) -> Result<Theme, String> {
    if let Some(path) = arguments
        .iter()
        .position(|argument| argument == "--style-pack")
        .and_then(|at| arguments.get(at + 1))
    {
        let tokens = Tokens::load(std::path::Path::new(path))
            .map_err(|error| format!("the style pack could not be worn: {error}"))?;
        return Ok(tokens.theme());
    }

    let appearance = settings
        .get(tc_palette::KEY_APPEARANCE)
        .unwrap_or_else(|| "System".to_string());
    Ok(match appearance.trim().to_ascii_lowercase().as_str() {
        "light" => Theme::light(),
        "dark" => Theme::dark(),
        _ => {
            if tc_platform_win::apps_use_dark_theme() {
                Theme::dark()
            } else {
                Theme::light()
            }
        }
    })
}

fn quit_running() -> Result<(), String> {
    if tc_platform_win::signal_quit() {
        println!("yal: a running instance was asked to quit");
    } else {
        println!("yal: no running instance");
    }
    Ok(())
}
