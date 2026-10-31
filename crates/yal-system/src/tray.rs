//! The system tray icon.
//!
//! The "menu bar extra" is a macOS concept. Its Windows equivalent is
//! a notification-area icon, which is what this module provides: a tray icon
//! with a menu of the launcher's own commands.

use std::sync::mpsc::Sender;

/// The commands the tray icon offers.
///
/// The tray is the launcher's home while its window is parked; the
/// commands answer what a home should: opening, its preferences, and
/// quitting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCommand {
    /// Show the palette.
    ShowPalette,
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
            TrayCommand::OpenSettings => "Preferences…",
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
    ///
    /// The thread is released, not joined: joining would hold whatever is
    /// leaving open until the tray's next poll, and the launcher closes
    /// for its own reasons. What keeps the icon alive is the thread's own
    /// `tray` value, which drops with the thread — on `Quit` at once, or
    /// when the process is otherwise over.
    pub fn stop(&mut self) {
        self.handle.take();
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
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, MSG, PeekMessageW, PM_REMOVE, TranslateMessage,
    };

    let show = MenuItem::new(TrayCommand::ShowPalette.label(), true, None);
    let settings = MenuItem::new(TrayCommand::OpenSettings.label(), true, None);
    let quit = MenuItem::new(TrayCommand::Quit.label(), true, None);
    let separator = PredefinedMenuItem::separator();
    let menu = Menu::new();

    menu.append(&show)?;
    menu.append(&separator)?;
    menu.append(&settings)?;
    menu.append(&separator)?;
    menu.append(&quit)?;

    let mut tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_icon(icon()?)
        .with_tooltip("YAL — the launcher")
        .with_menu_on_left_click(false)
        .build()?;

    // The icon lives by the Win32 message pump of the thread that
    // created it — and that thread is this one. A click on the icon is
    // button-up messages sitting in this thread's message queue, and
    // the menu opens inside the crate's window procedure while they are
    // dispatched; the event channels are filled by that same procedure,
    // not before. The first build slept and polled the channels without
    // ever pumping — verified live: the icon sat in the tray completely
    // inert, eager to be seen and deaf to every click, right or left.
    // So the pump is the loop's first job, before anything is polled.
    let mut message = MSG::default();
    loop {
        unsafe {
            while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }

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

/// The tray icon, drawn rather than borrowed: a slate square with a
/// lighter bar across it — the palette's own row shape. Monochrome by
/// choice: no byte-order surprise on the way to the notification area
/// can turn this launcher red in place of blue.
#[cfg(windows)]
fn icon() -> Result<tray_icon::Icon, Box<dyn std::error::Error>> {
    const SIDE: i32 = 32;
    const RADIUS: i32 = 5;
    /// The square's left, top, right and bottom, in pixels.
    const SQUARE: [i32; 4] = [5, 5, 26, 26];
    /// The bar's vertical and horizontal reach, in pixels.
    const BAR: (std::ops::Range<i32>, std::ops::Range<i32>) = (13..19, 9..23);
    const DARK: [u8; 3] = [46, 46, 52];
    const LIGHT: [u8; 3] = [232, 232, 238];

    let mut rgba = Vec::with_capacity((SIDE * SIDE * 4) as usize);
    for y in 0..SIDE {
        for x in 0..SIDE {
            let inside = x >= SQUARE[0]
                && y >= SQUARE[1]
                && x <= SQUARE[2]
                && y <= SQUARE[3]
                && {
                    // The centre of the nearest corner; its arc IS the
                    // rounding. Every pixel is within one corner's reach
                    // or clear of all of them.
                    let cx = if x < SQUARE[0] + RADIUS {
                        SQUARE[0] + RADIUS
                    } else {
                        SQUARE[2] - RADIUS
                    };
                    let cy = if y < SQUARE[1] + RADIUS {
                        SQUARE[1] + RADIUS
                    } else {
                        SQUARE[3] - RADIUS
                    };
                    let dx = x - cx;
                    let dy = y - cy;
                    dx * dx + dy * dy <= RADIUS * RADIUS
                };
            match inside {
                false => rgba.extend_from_slice(&[0, 0, 0, 0]),
                true => {
                    let in_bar =
                        BAR.0.contains(&y) && BAR.1.contains(&x);
                    let colour = if in_bar { LIGHT } else { DARK };
                    rgba.extend_from_slice(&[colour[0], colour[1], colour[2], 255]);
                }
            }
        }
    }
    Ok(tray_icon::Icon::from_rgba(
        rgba,
        SIDE as u32,
        SIDE as u32,
    )?)
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
            TrayCommand::OpenSettings,
            TrayCommand::Quit,
        ] {
            assert!(!command.label().is_empty());
        }
    }
}
