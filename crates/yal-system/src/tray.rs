//! The system tray icon.
//!
//! The "menu bar extra" is a macOS concept. Its Windows equivalent is
//! a notification-area icon, which is what this module provides: a tray icon
//! with a menu of the launcher's own commands.

use std::sync::mpsc::Sender;

/// The commands the tray icon offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCommand {
    /// Show the palette.
    ShowPalette,
    /// Copy the clipboard history.
    CopyClipboardHistory,
    /// Open the settings.
    OpenSettings,
    /// Quit the launcher.
    Quit,
}

impl TrayCommand {
    /// The label shown in the menu.
    pub fn label(self) -> &'static str {
        match self {
            TrayCommand::ShowPalette => "Open Launcher",
            TrayCommand::CopyClipboardHistory => "Clipboard History",
            TrayCommand::OpenSettings => "Settings…",
            TrayCommand::Quit => "Quit",
        }
    }
}

/// A running tray icon.
pub struct Tray {
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Tray {
    /// Start the tray icon.
    ///
    /// `sender` receives the chosen command. The tray runs on its own thread
    /// because the icon toolkit owns the thread it was created on.
    pub fn start(sender: Sender<TrayCommand>) -> Option<Self> {
        let handle = std::thread::Builder::new()
            .name("tray".into())
            .spawn(move || {
                let _ = run(sender);
            })
            .ok()?;
        Some(Self { handle: Some(handle) })
    }

    /// Stop the tray icon.
    pub fn stop(&mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Run the tray icon on the current thread.
#[cfg(windows)]
fn run(sender: Sender<TrayCommand>) -> Result<(), Box<dyn std::error::Error>> {
    use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use tray_icon::{TrayIconBuilder, TrayIconEvent};

    let show = MenuItem::new(TrayCommand::ShowPalette.label(), true, None);
    let clipboard = MenuItem::new(TrayCommand::CopyClipboardHistory.label(), true, None);
    let settings = MenuItem::new(TrayCommand::OpenSettings.label(), true, None);
    let quit = MenuItem::new(TrayCommand::Quit.label(), true, None);
    let separator = PredefinedMenuItem::separator();
    let menu = Menu::new();

    menu.append(&show)?;
    menu.append(&clipboard)?;
    menu.append(&separator)?;
    menu.append(&settings)?;
    menu.append(&separator)?;
    menu.append(&quit)?;

    let mut tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Launcher")
        .with_menu_on_left_click(false)
        .build()?;

    loop {
        std::thread::sleep(Duration::from_millis(120));

        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
            if let TrayIconEvent::Click {
                button: tray_icon::MouseButton::Left,
                ..
            } = event
            {
                let _ = sender.send(TrayCommand::ShowPalette);
            }
        }

        while let Ok(event) = MenuEvent::receiver().try_recv() {
            let command = if event.id == show.id() {
                TrayCommand::ShowPalette
            } else if event.id == clipboard.id() {
                TrayCommand::CopyClipboardHistory
            } else if event.id == settings.id() {
                TrayCommand::OpenSettings
            } else if event.id == quit.id() {
                TrayCommand::Quit
            } else {
                continue;
            };
            let _ = sender.send(command);
            if command == TrayCommand::Quit {
                return Ok(());
            }
        }

        // Keep the icon alive.
        let _ = &mut tray;
    }
}

/// The tray icon is not available outside Windows.
#[cfg(not(windows))]
fn run(_sender: Sender<TrayCommand>) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}

use std::time::Duration;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_command_has_a_label() {
        for command in [
            TrayCommand::ShowPalette,
            TrayCommand::CopyClipboardHistory,
            TrayCommand::OpenSettings,
            TrayCommand::Quit,
        ] {
            assert!(!command.label().is_empty());
        }
    }
}
