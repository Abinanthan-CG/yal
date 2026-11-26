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
    ClipboardUpdate, ClipboardWatcher, FileWatcher, GlobalHotkeyService, QuitWatcher, ShowWatcher,
    TrayAction, WindowsSettings, WindowsSingleInstance, WindowsTray,
};
use tc_ui::{PaletteHandle, Theme, Tokens};

use crate::features::{default_index_roots, FeatureStores};
use crate::hotkeys::{self, HotkeyController, BINDING_LAUNCHER_TOGGLE};
use crate::launcher::{FileSearch, LauncherState, RANKING_FILE};
use crate::native::{
    NativeContext, NativePalette, COMMAND_DISPATCH_HOTKEY, COMMAND_OPEN_SETTINGS,
    INLINE_SNIPPET_PREFIX,
};
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

/// Cached live fiat & crypto exchange rates in `%APPDATA%\yal\`.
pub const RATES_FILE: &str = "currency-rates.json";

/// Setting key for CSS `1rem`/`1em` pixel base in Calculator (default `16`).
const KEY_CALCULATOR_REM_PX: &str = "calculator.remPx";

fn lock_or_recover<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The text `yal.exe --help` prints.
pub fn usage() -> String {
    "YAL - Yet Another Launcher, a fast native Windows keyboard launcher\n\
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

    // Create the wake and quit watchers immediately after acquiring the
    // single-instance lock so neither a second launch nor `--quit-running`
    // can ever race startup.
    let show_slot = Arc::clone(&handle_slot);
    let show_watcher = ShowWatcher::spawn(move || {
        if let Some(handle) = *lock_or_recover(&show_slot) {
            toggle_palette(handle);
        }
    })
    .ok();

    let quit_watcher_slot = Arc::clone(&handle_slot);
    let quit_watcher = QuitWatcher::spawn(move || {
        if let Some(handle) = *lock_or_recover(&quit_watcher_slot) {
            tc_ui::post_quit(handle);
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
    let features = Arc::new(FeatureStores::open(&paths));
    let launcher = Arc::new(build_launcher(&paths, &settings));
    if let Some(last_ans) = features.latest_calc_numeric_answer() {
        launcher.set_last_calc_answer(Some(last_ans));
    }
    spawn_rates_refresh(
        Arc::clone(&launcher),
        paths.data_dir.join(RATES_FILE),
        false,
    );

    let hotkeys = Arc::new(HotkeyController::new());
    let pending_hotkey_command: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

    let file_search_enabled = settings.get(FILE_SEARCH_ENABLED).as_deref() != Some("false");
    let file_watcher = if file_search_enabled {
        features.trigger_file_index(Arc::clone(&launcher), Some(Arc::clone(&handle_slot)));
        const MAX_WATCHED_ROOTS: usize = 32;
        let roots: Vec<_> = default_index_roots(&paths.home_dir)
            .into_iter()
            .take(MAX_WATCHED_ROOTS)
            .collect();
        let fw_features = Arc::clone(&features);
        let fw_launcher = Arc::clone(&launcher);
        let fw_handle_slot = Arc::clone(&handle_slot);
        match FileWatcher::start(roots, move || {
            fw_features
                .trigger_file_index(Arc::clone(&fw_launcher), Some(Arc::clone(&fw_handle_slot)));
        }) {
            Ok(watcher) => Some(watcher),
            Err(error) => {
                eprintln!("yal: file watcher unavailable: {error}");
                None
            }
        }
    } else {
        None
    };

    let clip_features = Arc::clone(&features);
    let clip_hotkeys = Arc::clone(&hotkeys);
    let clip_handle_slot = Arc::clone(&handle_slot);
    let clipboard_watcher = ClipboardWatcher::spawn_rich(move |update| {
        if clip_hotkeys.should_suppress_clipboard() {
            return false;
        }
        let source_exe = tc_platform_win::foreground_process_exe();
        let source_ref = source_exe.as_deref();
        match update {
            ClipboardUpdate::Text(text) => {
                clip_features.record_clipboard_text(&text, source_ref);
            }
            ClipboardUpdate::Files(paths) => {
                clip_features.record_clipboard_files(&paths, source_ref);
            }
            ClipboardUpdate::Image {
                width,
                height,
                rgba,
            } => {
                clip_features.record_clipboard_image(width, height, &rgba, source_ref);
            }
        }
        if let Some(handle) = *lock_or_recover(&clip_handle_slot) {
            tc_ui::post_refresh(handle);
        }
        true
    });
    if clipboard_watcher.is_none() {
        eprintln!("yal: clipboard watcher unavailable");
    }

    let palette_state = Arc::new(build_palette_state(&settings));
    let window_commands = Arc::new(build_window_commands(&settings));

    let quit_slot = Arc::clone(&handle_slot);
    let runner = Arc::new(
        Runner::new(Arc::clone(&launcher))
            .with_windows(window_commands)
            .with_quit(Arc::new(move || {
                if let Some(handle) = *lock_or_recover(&quit_slot) {
                    tc_ui::post_quit(handle);
                }
            })),
    );

    let theme = resolve_theme(arguments, &settings)?;
    let native = NativePalette::open_with_context(
        Arc::clone(&launcher),
        Arc::clone(&runner),
        NativeContext {
            palette: Arc::clone(&palette_state),
            settings: Arc::clone(&settings),
            features: Arc::clone(&features),
            hotkeys: Arc::clone(&hotkeys),
            pending_hotkey_command: Arc::clone(&pending_hotkey_command),
        },
        theme,
    )?;

    let handle = native.handle();
    *lock_or_recover(&handle_slot) = Some(handle);

    // Start the global hotkey service with launcher + per-command bindings,
    // optional CapsLock Hyperkey, and background auto-expanding snippets.
    let initial_bindings = hotkeys::build_all_bindings(&*settings);
    let hk_pending = Arc::clone(&pending_hotkey_command);
    let hk_runner = Arc::clone(&runner);
    let hk_features = Arc::clone(&features);
    let hk_controller = Arc::clone(&hotkeys);
    let hk_settings = Arc::clone(&settings);
    match GlobalHotkeyService::spawn(initial_bindings, move |binding_id| {
        if binding_id == BINDING_LAUNCHER_TOGGLE {
            toggle_palette(handle);
            return;
        }
        if let Some(entry_id) = hotkeys::command_entry_from_binding(&binding_id) {
            dispatch_global_command_hotkey(
                entry_id,
                handle,
                &hk_pending,
                &hk_runner,
                &hk_features,
                &hk_controller,
                &hk_settings,
            );
        }
    }) {
        Ok(service) => {
            hotkeys.attach(Arc::new(service));
            hotkeys.set_hyperkey_enabled(hotkeys::hyperkey_enabled(&*settings));
            hotkeys.sync_snippet_keywords(&settings, Arc::clone(&features));
        }
        Err(error) => {
            eprintln!("yal: global hotkey unavailable: {error}");
        }
    }

    // Start the native Win32 system tray icon.
    let tray = match WindowsTray::spawn(move |action| match action {
        TrayAction::Toggle => tc_ui::post_toggle_from_tray(handle),
        TrayAction::Open => tc_ui::post_show(handle),
        TrayAction::Settings => tc_ui::post_command(handle, COMMAND_OPEN_SETTINGS),
        TrayAction::Support => {
            std::thread::spawn(|| {
                if let Err(error) = tc_platform_win::open(tc_core::SUPPORT_URL) {
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
    drop(file_watcher);
    drop(clipboard_watcher);
    drop(show_watcher);
    drop(quit_watcher);
    drop(tray);
    drop(hotkeys.detach());
    single_instance.release();
    Ok(())
}

fn dispatch_global_command_hotkey(
    entry_id: &str,
    handle: PaletteHandle,
    pending_command: &Arc<Mutex<Option<String>>>,
    runner: &Arc<Runner>,
    features: &Arc<FeatureStores>,
    hotkeys: &Arc<HotkeyController>,
    settings: &WindowsSettings,
) {
    tc_platform_win::window_control::remember_foreground();
    let dark_hud = settings_prefers_dark(settings);

    if entry_id == CommandId::Quit.entry_id() {
        tc_ui::post_quit(handle);
        return;
    }

    if entry_id == CommandId::PasteSequentially.entry_id() {
        let (item, remaining) = match features.paste_queue_pop_next() {
            crate::features::PasteNext::Item { item, remaining } => (item, remaining),
            crate::features::PasteNext::Settling => return,
            crate::features::PasteNext::Empty => {
                tc_ui::show_hud("Paste Queue Empty", dark_hud);
                return;
            }
        };
        let hud = if remaining == 0 {
            "Paste Queue Complete".to_string()
        } else {
            format!("Pasted from Queue ({remaining} left)")
        };
        match item.kind {
            tc_clipboard::ItemKind::Image => {
                if let Some((w, h, rgba)) = features.load_clipboard_image_rgba(&item) {
                    hotkeys.suppress_clipboard_for_ms(700);
                    let _ = tc_platform_win::paste_image_async(w, h, rgba);
                }
            }
            tc_clipboard::ItemKind::File => {
                let paths = crate::screens::parse_clipboard_file_paths(&item);
                if !paths.is_empty() {
                    hotkeys.suppress_clipboard_for_ms(700);
                    let _ = tc_platform_win::paste_files_async(paths);
                }
            }
            tc_clipboard::ItemKind::Text => {
                if let Some(text) = item.text.clone() {
                    hotkeys.suppress_clipboard_for_ms(700);
                    let _ = tc_platform_win::copy_and_paste_async(text);
                }
            }
        }
        tc_ui::show_hud(hud, dark_hud);
        return;
    }

    if entry_id.starts_with("command:") {
        *lock_or_recover(pending_command) = Some(entry_id.to_string());
        tc_ui::post_command(handle, COMMAND_DISPATCH_HOTKEY);
        return;
    }

    if let Some(snippet_id) = entry_id
        .strip_prefix(INLINE_SNIPPET_PREFIX)
        .or_else(|| entry_id.strip_prefix("snippet:"))
    {
        if let Some(snippet) = features
            .search_snippets("")
            .into_iter()
            .find(|s| s.id() == snippet_id)
        {
            let expanded = features.expand_snippet(&snippet);
            hotkeys.suppress_clipboard_for_ms(700);
            if tc_platform_win::copy_and_paste_async(expanded).is_ok() {
                tc_ui::show_hud(format!("Pasted {}", snippet.snippet.name), dark_hud);
            }
        }
        return;
    }

    if let Some(ql_name) = entry_id.strip_prefix(crate::native::ROOT_QUICKLINK_PREFIX) {
        if let Some(link) = features
            .search_quicklinks("")
            .into_iter()
            .find(|l| l.name.eq_ignore_ascii_case(ql_name))
        {
            if link.needs_query() {
                *lock_or_recover(pending_command) = Some(entry_id.to_string());
                tc_ui::post_command(handle, COMMAND_DISPATCH_HOTKEY);
            } else {
                let url = link.expand_url("");
                let name = link.name.clone();
                std::thread::spawn(move || {
                    if tc_platform_win::open(&url).is_ok() {
                        tc_ui::show_hud(format!("Opened {name}"), dark_hud);
                    }
                });
            }
        }
        return;
    }

    let runner = Arc::clone(runner);
    let entry_id = entry_id.to_string();
    std::thread::spawn(
        move || match runner.open_with_feedback(&entry_id, "", false) {
            Ok(Some(feedback)) => tc_ui::show_hud(feedback, dark_hud),
            Ok(None) => {}
            Err(error) => eprintln!("yal: global command hotkey `{entry_id}` failed: {error}"),
        },
    );
}

fn settings_prefers_dark(settings: &WindowsSettings) -> bool {
    let appearance = settings
        .get(tc_palette::KEY_APPEARANCE)
        .unwrap_or_else(|| "System".to_string());
    match appearance.trim().to_ascii_lowercase().as_str() {
        "light" => false,
        "dark" => true,
        _ => tc_platform_win::apps_use_dark_theme(),
    }
}

fn build_launcher(paths: &tc_platform::Paths, settings: &WindowsSettings) -> LauncherState {
    let mut launcher = LauncherState::new(paths.data_dir.join(RANKING_FILE));

    // Window Management is enabled by default (toggleable in Settings).
    if settings.get(WINDOW_MANAGEMENT_ENABLED).as_deref() != Some("false") {
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

    if settings.get(FILE_SEARCH_ENABLED).as_deref() != Some("false") {
        let index = paths.data_dir.join(INDEX_FILE);
        if let Ok(store) = tc_files::store::IndexStore::open(&index) {
            launcher.set_file_search(FileSearch::new(store, paths.home_dir.clone(), Vec::new()));
        }
    }
    if settings.get(FILE_SEARCH_SHOW).as_deref() != Some("false") {
        launcher.set_show_files(true);
    }

    // Load cached exchange rates (or bundled fallback) immediately so currency &
    // crypto conversions work offline from millisecond zero.
    let cached_rates = tc_calc::fx::load_rates_cache(&paths.data_dir.join(RATES_FILE));
    launcher.set_rates(Arc::new(cached_rates));

    if let Some(rem) = settings
        .get(KEY_CALCULATOR_REM_PX)
        .and_then(|v| v.parse::<f64>().ok())
    {
        launcher.set_rem_px(rem);
    }

    launcher
}

/// Silently refresh fiat & crypto exchange rates in a background thread when the
/// cached snapshot is missing or older than 24 hours (or immediately if `force` is true).
/// Never blocks startup or the UI thread.
pub fn spawn_rates_refresh(
    launcher: Arc<LauncherState>,
    rates_path: std::path::PathBuf,
    force: bool,
) {
    let _ = std::thread::Builder::new()
        .name("yal-fx-refresh".to_string())
        .spawn(move || {
            let now_secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            let mut snapshot = tc_calc::fx::load_rates_cache(&rates_path);
            let age = now_secs.saturating_sub(snapshot.fetched_at_secs);
            if !force && snapshot.fetched_at_secs > 0 && age < tc_calc::fx::RATES_REFRESH_SECS {
                return;
            }

            let mut updated = false;
            if let Ok(body) = tc_platform_win::https_get(tc_calc::fx::FIAT_FEED_URL, 10_000) {
                if let Some((fiat_map, date)) = tc_calc::fx::parse_open_er_api_json(&body) {
                    snapshot.merge_fiat(fiat_map, date, now_secs);
                    updated = true;
                }
            }
            if let Ok(body) = tc_platform_win::https_get(tc_calc::fx::CRYPTO_FEED_URL, 10_000) {
                if let Some(crypto_map) = tc_calc::fx::parse_coingecko_simple_json(&body) {
                    snapshot.merge_crypto_usd_prices(crypto_map);
                    updated = true;
                }
            }

            if updated {
                let _ = tc_calc::fx::save_rates_cache(&rates_path, &snapshot);
                launcher.set_rates(Arc::new(snapshot));
            }
        });
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
