//! The application shell: the palette window, the tray icon, the global
//! shortcut and the single-instance lock.
//!
//! The palette window is created once and hidden, so the hotkey shows it
//! instantly instead of building a window on every press.

use tauri::Emitter;
use tauri::Manager;

/// The palette's label, so the window can be found again.
const PALETTE: &str = "palette";

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
        .plugin(tauri_plugin_global_shortcut::Builder::new().with_handler(
            move |app, shortcut, event| {
                if event.state() != tauri_plugin_global_shortcut::ShortcutState::Pressed {
                    return;
                }
                let _ = shortcut;
                toggle_palette(app);
            },
        ).build())
        .setup(move |app| {
            let handle = app.handle().clone();
            // The installer asks a running instance to quit before it replaces
            // the program files, so the app has to be listening for that.
            let quitting = handle.clone();
            let _ = tc_platform_win::QuitWatcher::spawn(move || {
                quitting.exit(0);
            });
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
        .run(tauri::generate_context!())
        .map_err(|error| format!("the application could not start: {error}"))
}

/// Show the palette when it is hidden, hide it when it is shown.
fn toggle_palette(app: &tauri::AppHandle) {
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
