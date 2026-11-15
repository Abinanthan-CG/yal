//! The native palette: the window the user actually types into.
//!
//! The palette that ships in the browser costs more memory than the whole
//! rest of YAL put together, because a web view brings an engine with it.
//! This one is a Win32 window painted by Direct2D, and it lives on its own
//! thread with its own message loop, because a window with a message loop
//! cannot live on a thread that is already busy with something else.
//!
//! The window knows about the keyboard and the mouse. It does not know what
//! to search for, and it does not know what to do with the row the user
//! chose: those are the app's business, and the palette asks.

use std::sync::{Arc, Mutex};

use tc_platform::FileSystem;
use tc_ui::{
    Color, Icon, Model, Palette, PaletteEvent, PaletteHandle, PaletteReply, Row, Theme, Tokens,
};

use crate::launcher::{
    LauncherState, ResultRow, CALCULATOR_SECTION, COMMAND_SECTION, CUSTOM_COMMAND_SECTION,
    FILE_SECTION, RANKING_FILE, SYSTEM_ACTION_SECTION, WINDOW_COMMAND_SECTION,
};
use crate::open::Runner;

/// The hint at the bottom of the palette.
const FOOTER: &str = "Enter to open  ·  Up and down to choose  ·  Esc to dismiss";

/// The section an application belongs to.
const APPLICATIONS: &str = "Applications";

/// The native palette.
///
/// The window belongs to the thread that made it, so this has to be built
/// and used on one thread. Another thread asks for it by its handle.
pub struct NativePalette {
    /// The palette itself, which owns the window and its message loop.
    palette: Palette,
}

impl NativePalette {
    /// Opens the palette on this thread.
    pub fn open(
        launcher: Arc<LauncherState>,
        runner: Arc<Runner>,
        theme: Theme,
    ) -> Result<NativePalette, String> {
        let palette = build(launcher, runner, theme)?;
        Ok(NativePalette { palette })
    }

    /// Opens the palette on a thread of its own, and returns its handle.
    ///
    /// The handle is what another thread - the one that owns the hotkey -
    /// uses to ask for the palette.
    pub fn spawn(
        launcher: Arc<LauncherState>,
        runner: Arc<Runner>,
        theme: Theme,
    ) -> Result<PaletteHandle, String> {
        let (sender, receiver) = std::sync::mpsc::channel::<PaletteHandle>();

        std::thread::Builder::new()
            .name("yal-palette".to_string())
            .spawn(move || {
                let palette = match build(launcher, runner, theme) {
                    Ok(palette) => palette,
                    Err(error) => {
                        eprintln!("yal: the palette could not be opened: {error}");
                        return;
                    }
                };
                let _ = sender.send(palette.handle());
                palette.run();
            })
            .map_err(|error| format!("the palette's thread could not be started: {error}"))?;

        // The window's handle comes from the thread that made it.
        receiver
            .recv_timeout(std::time::Duration::from_secs(10))
            .map_err(|_| "the palette did not open".to_string())
    }

    /// Shows the palette and gives it the keyboard.
    pub fn show(&self) {
        self.palette.show();
    }

    /// Runs the palette's message loop until the window is closed.
    pub fn run(&self) {
        self.palette.run();
    }
}

/// Builds the palette window.
///
/// What the palette shows and what it does with the row the user chose are
/// the app's business, so the window is given a handler that asks.
fn build(
    launcher: Arc<LauncherState>,
    runner: Arc<Runner>,
    theme: Theme,
) -> Result<Palette, String> {
    // What the palette is showing, and what the user chose from it. The
    // window asks about a position in the list, so the list has to be kept
    // where the answer can find it.
    let shown: Arc<Mutex<Shown>> = Arc::new(Mutex::new(Shown::default()));

    let handler_shown = Arc::clone(&shown);
    let handler_launcher = Arc::clone(&launcher);
    let handler_runner = Arc::clone(&runner);

    let handler = move |event: PaletteEvent| -> PaletteReply {
        match event {
            // The palette starts afresh every time it is shown, so the
            // suggestions go back into it.
            PaletteEvent::Shown => {
                let rows = handler_launcher.suggestions();
                let mut shown = handler_shown.lock().expect("the palette's state");
                shown.query = String::new();
                shown.rows = rows.clone();
                PaletteReply::Update(model_for("", &rows))
            }
            PaletteEvent::QueryChanged(query) => {
                // An empty palette shows what the user opens most; typing
                // narrows it down.
                let rows = if query.trim().is_empty() {
                    handler_launcher.suggestions()
                } else {
                    handler_launcher.search(&query)
                };
                let mut shown = handler_shown.lock().expect("the palette's state");
                shown.query = query.clone();
                shown.rows = rows.clone();
                PaletteReply::Update(model_for(&query, &rows))
            }
            PaletteEvent::Activated(index) => {
                let shown = handler_shown.lock().expect("the palette's state");
                let Some(row) = shown.rows.get(index) else {
                    return PaletteReply::Keep;
                };
                let id = row.id.clone();
                let query = shown.query.clone();
                drop(shown);
                // The row runs whatever it is, and the palette gets out of
                // the way.
                let _ = handler_runner.open(&id, &query, false);
                PaletteReply::Close
            }
            PaletteEvent::Dismissed => PaletteReply::Keep,
        }
    };

    let rows = launcher.suggestions();
    let model = model_for("", &rows);

    Palette::with_handler(theme, model, Box::new(handler))
}

/// What the palette is showing, and what the user chose from it.
#[derive(Default)]
struct Shown {
    /// The query the results were found with.
    query: String,
    /// The results themselves, in the order they were shown.
    rows: Vec<ResultRow>,
}

/// The palette's content for a set of results.
fn model_for(query: &str, rows: &[ResultRow]) -> Model {
    Model {
        query: query.to_string(),
        rows: rows.iter().map(row_of).collect(),
        selected: 0,
        hovered: None,
        footer: FOOTER.to_string(),
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
///
/// The colour says what kind of thing the row is, so the list can be read
/// at a glance: an application, a file, a command, an action.
fn icon_of(result: &ResultRow) -> Icon {
    Icon::new(letter_of(&result.name), color_of(&result.kind))
}

/// The letter in the tile.
///
/// The first letter of the name, when the name has one. A row with no name
/// - which should not happen - gets a dot rather than nothing.
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

/// Runs the native palette on its own.
///
/// This is the whole application, minus the tray icon and the settings
/// screens: the palette, the launcher behind it, and the runner that opens
/// what the user chooses.
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

/// The theme the palette wears.
///
/// A style pack is a design system's tokens in a small file; without one the
/// palette wears the built-in dark appearance.
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
}
