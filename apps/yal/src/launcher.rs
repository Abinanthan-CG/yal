//! The launcher: the programs it knows, how it ranks them, and how it opens
//! one.
//!
//! Upstream's `LauncherOrder` does the ranking; this holds the index it ranks
//! and keeps the learned ranking in the app's own folder.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tc_apps::Program;
use tc_calc::{evaluate, CalcOutcome, FullCatalog};
use tc_commands::catalog::{pane_of, CommandSettings};
use tc_commands::model::CustomCommand;
use tc_core::{CommandId, SystemActionId};
use tc_files::index::Entry as FileEntry;
use tc_files::store::IndexStore;
use tc_palette::FileType;
use tc_search::entry_naming::SearchProfile;
use tc_search::matcher::SearchSensitivity;
use tc_search::order::{LauncherOrder, Query, Rankable, Signals};
use tc_search::ranking::LauncherRankingStore;

/// How many results the palette shows at once.
const LIMIT: usize = 50;

/// The name of the file the learned ranking is kept in.
///
/// Upstream calls it `launcher-ranking.json` and keeps it in the app's own
/// folder, so a backup moves between systems.
pub const RANKING_FILE: &str = "launcher-ranking.json";

/// The prefix a system action's row id carries.
///
/// A program is opened by its target; a system action is run by its identifier,
/// and the two are told apart by this prefix.
pub const SYSTEM_ACTION_PREFIX: &str = "system-action:";

/// The section the system actions are listed under.
const SYSTEM_ACTION_SECTION: &str = "System Actions";

/// The prefix a calculator row's id carries.
///
/// A calculator row is not opened; its answer is copied.
pub const CALCULATOR_PREFIX: &str = "calculator:";
/// The prefix the launcher opens a window command by.
pub const WINDOW_COMMAND_PREFIX: &str = "window-command:";
/// The section the window commands are listed under.
pub const WINDOW_COMMAND_SECTION: &str = "Window Commands";

/// The section the calculator's answer is listed under.
const CALCULATOR_SECTION: &str = "Calculator";
/// The section the built-in commands are listed under.
pub const COMMAND_SECTION: &str = "Commands";
/// The section the user's own commands are listed under.
pub const CUSTOM_COMMAND_SECTION: &str = "Custom Commands";

/// The section the files are listed under.
///
/// "It sits below the apps and commands, so return still opens the top
/// result."
pub const FILE_SECTION: &str = "Files";

/// The prefix a file row's id carries.
pub const FILE_PREFIX: &str = "file:";

/// The built-in commands this build can run.
///
/// The catalogue holds every built-in command; the launcher offers only the
/// ones whose feature is in the build, so a row is never a dead end. The rest
/// arrive with their features.
const AVAILABLE_COMMANDS: [CommandId; 2] = [CommandId::Quit, CommandId::Support];

/// One value a command asks for before it runs.
#[derive(Debug, Clone, Serialize)]
pub struct CommandArgument {
    /// The name shown beside the field.
    pub name: String,
    /// Whether the field may be left empty.
    pub is_optional: bool,
}

/// One row in the results list.
#[derive(Debug, Clone, Serialize)]
pub struct ResultRow {
    /// The key the launcher opens the row by.
    pub id: String,
    /// The name the row is shown under.
    pub name: String,
    /// The line under the name: who publishes it, or where it came from.
    pub detail: String,
    /// The section the row belongs to.
    pub kind: String,
    /// What the launcher opens.
    pub target: String,
    /// Whether the launcher has to ask before it does it.
    pub needs_confirm: bool,
    /// The values the command asks for first.
    #[serde(default)]
    pub arguments: Vec<CommandArgument>,
    /// Whether the row is a folder. Only the file rows say.
    #[serde(default)]
    pub is_dir: Option<bool>,
}

/// A program, with the names the search matches against.
#[derive(Clone)]
struct Entry {
    program: Program,
    profile: SearchProfile,
    signals: Signals,
}

/// The file search: the index, the home folder it ranks against, and the
/// files the user has made a favorite of.
///
/// "Turn it on in Settings → File Search. It's off by default. While it's
/// off, there's no command and no index on disk."
pub struct FileSearch {
    /// The index the rows come from.
    ///
    /// The lock is there because a SQLite connection cannot be shared between
    /// threads on its own, and the launcher is read from more than one.
    store: Mutex<IndexStore>,
    /// The user's home folder, which the ranking prefers.
    home: PathBuf,
    /// The files the user has made a favorite of.
    favorites: Vec<String>,
}

impl FileSearch {
    /// A file search over an index.
    pub fn new(store: IndexStore, home: PathBuf, favorites: Vec<String>) -> FileSearch {
        FileSearch {
            store: Mutex::new(store),
            home,
            favorites,
        }
    }

    /// The whole index, read into memory.
    fn entries(&self) -> Vec<FileEntry> {
        let store = self.store.lock().expect("the file index lock");
        store.entries().unwrap_or_default()
    }
}

impl Rankable for Entry {
    fn profile(&self) -> &SearchProfile {
        &self.profile
    }

    fn signals(&self) -> &Signals {
        &self.signals
    }
}

/// The launcher: the index, and what it has learned.
pub struct LauncherState {
    entries: Vec<Entry>,
    ranking: Mutex<LauncherRankingStore>,
    sensitivity: SearchSensitivity,
    /// Whether the window commands are listed in the results. This is the
    /// "Show in launcher" switch, and window management is off until it is
    /// turned on - upstream's own default.
    show_window_commands: bool,
    /// The built-in commands, and the switches that decide which of them exist.
    commands: CommandSettings,
    /// The user's own shell commands.
    custom_commands: Vec<CustomCommand>,
    /// The file search, when it is on.
    files: Option<FileSearch>,
    /// Whether the files are listed in the launcher. This is the
    /// "Show in launcher" switch under the File Search one.
    show_files: bool,
}

impl LauncherState {
    /// The index, built from the programs Windows knows about.
    pub fn new(ranking_file: PathBuf) -> LauncherState {
        LauncherState::from_programs(tc_platform_win::programs(), ranking_file)
    }

    /// The index, from a list of programs.
    pub fn from_programs(programs: Vec<Program>, ranking_file: PathBuf) -> LauncherState {
        let entries = programs
            .into_iter()
            .map(|program| {
                let profile = program.profile();
                let signals = Signals {
                    alias: None,
                    usage: tc_search::ranking::LauncherUsage::unused(),
                    // Applications sit above the launcher's own commands.
                    priority: 4,
                    title: program.name.clone(),
                    boosted_terms: Default::default(),
                };
                Entry {
                    program,
                    profile,
                    signals,
                }
            })
            .collect();
        LauncherState {
            entries,
            ranking: Mutex::new(LauncherRankingStore::open(ranking_file, now_seconds())),
            sensitivity: SearchSensitivity::Medium,
            show_window_commands: false,
            commands: CommandSettings::new(),
            custom_commands: Vec::new(),
            files: None,
            show_files: false,
        }
    }

    /// Show or hide the files in the launcher.
    pub fn set_show_files(&mut self, show: bool) {
        self.show_files = show;
    }

    /// Show or hide the window commands in the results.
    pub fn set_show_window_commands(&mut self, show: bool) {
        self.show_window_commands = show;
    }

    /// The built-in commands, and the switches that decide which of them exist.
    pub fn set_command_settings(&mut self, settings: CommandSettings) {
        self.commands = settings;
    }

    /// The user's own shell commands.
    pub fn set_custom_commands(&mut self, commands: Vec<CustomCommand>) {
        self.custom_commands = commands;
    }

    /// Turn the file search on, with its index.
    pub fn set_file_search(&mut self, files: FileSearch) {
        self.files = Some(files);
    }

    /// The rows of the file-search screen.
    ///
    /// With nothing typed the list is the recent files; with something typed
    /// it is the results. The filter narrows the search as it runs.
    pub fn file_screen_rows(&self, query: &str, filter: FileType) -> Vec<ResultRow> {
        let Some(files) = &self.files else {
            return Vec::new();
        };
        tc_palette::screen_rows(
            &files.entries(),
            query,
            &files.home,
            filter,
            tc_files::search::RESULT_LIMIT,
        )
        .into_iter()
        .map(file_row)
        .collect()
    }

    /// The files the launcher lists under its own rows.
    ///
    /// "When your search has three or more characters, a Files section lists
    /// up to five matches."
    pub fn launcher_file_rows(&self, query: &str) -> Vec<ResultRow> {
        let Some(files) = &self.files else {
            return Vec::new();
        };
        tc_palette::launcher_rows(&files.entries(), query, &files.home, &files.favorites)
            .into_iter()
            .map(file_row)
            .collect()
    }

    /// The built-in commands whose name the query finds.
    ///
    /// "Commands are actions you can run by name from the launcher." A command
    /// whose feature is off, or whose row checkbox is cleared, is not offered.
    pub fn command_rows(&self, query: &str) -> Vec<ResultRow> {
        let wanted = query.trim().to_lowercase();
        if wanted.is_empty() {
            return Vec::new();
        }
        CommandId::ALL
            .into_iter()
            .filter(|command| AVAILABLE_COMMANDS.contains(command))
            .filter(|command| self.commands.switches.is_listed(*command))
            .filter(|command| {
                self.commands
                    .name(*command)
                    .to_lowercase()
                    .contains(&wanted)
            })
            .map(|command| ResultRow {
                id: command.entry_id().to_string(),
                name: self.commands.name(command),
                detail: pane_of(command)
                    .map(|pane| pane.label().to_string())
                    .unwrap_or_else(|| COMMAND_SECTION.to_string()),
                kind: COMMAND_SECTION.to_string(),
                target: command.entry_id().to_string(),
                needs_confirm: false,
                is_dir: None,
                arguments: Vec::new(),
            })
            .collect()
    }

    /// The user's own commands whose name the query finds.
    ///
    /// "Only the command's name is searchable, not the command itself." A
    /// command that is switched off, or taken out of the main search, is not
    /// offered.
    pub fn custom_command_rows(&self, query: &str) -> Vec<ResultRow> {
        let wanted = query.trim().to_lowercase();
        if wanted.is_empty() {
            return Vec::new();
        }
        self.custom_commands
            .iter()
            .filter(|command| command.is_enabled && command.shows_in_root_search)
            .filter(|command| command.name.to_lowercase().contains(&wanted))
            .map(|command| ResultRow {
                id: command.entry_id(),
                name: command.name.clone(),
                detail: command.command.clone(),
                kind: CUSTOM_COMMAND_SECTION.to_string(),
                target: command.entry_id(),
                needs_confirm: command.requires_confirmation,
                is_dir: None,
                arguments: command
                    .arguments
                    .iter()
                    .map(|argument| CommandArgument {
                        name: argument.name.clone(),
                        is_optional: argument.is_optional,
                    })
                    .collect(),
            })
            .collect()
    }

    /// The custom command a row id names.
    pub fn custom_command_of(&self, id: &str) -> Option<&CustomCommand> {
        let uuid = CustomCommand::id_from_entry_id(id)?;
        self.custom_commands
            .iter()
            .find(|command| command.id == uuid)
    }

    /// The built-in command a row id names.
    pub fn command_of(id: &str) -> Option<CommandId> {
        CommandId::from_entry_id(id)
    }

    /// The programs the launcher knows about.
    pub fn program_count(&self) -> usize {
        self.entries.len()
    }

    /// The rows that match the query, best first.
    pub fn search(&self, query: &str) -> Vec<ResultRow> {
        let ranking = self.ranking.lock().expect("the ranking lock");
        let at = now_seconds();
        // The usage is read once for the whole pass, so every row is compared
        // against the same moment.
        let entries: Vec<Entry> = self
            .entries
            .iter()
            .map(|entry| {
                let mut entry = entry.clone();
                entry.signals.usage = ranking.usage(&entry.program.id, at);
                entry
            })
            .collect();
        drop(ranking);

        let search = Query::new(query);
        let mut rows: Vec<ResultRow> =
            LauncherOrder::ranked(&entries, &search, self.sensitivity, LIMIT)
                .into_iter()
                .map(|entry| ResultRow {
                    id: entry.program.id.clone(),
                    name: entry.program.name.clone(),
                    detail: entry
                        .program
                        .publisher
                        .clone()
                        .unwrap_or_else(|| entry.program.source.label().to_string()),
                    kind: "Applications".to_string(),
                    target: entry.program.target.clone(),
                    needs_confirm: false,
                    is_dir: None,
                    arguments: Vec::new(),
                })
                .collect();
        // The calculator's answer comes first: a query that is a calculation is
        // asking for the answer, not for a program.
        let mut rows = match calculator_row(query) {
            Some(row) => {
                rows.insert(0, row);
                rows
            }
            None => rows,
        };
        // The system actions are matched by their name, and they sit under the
        // applications: a program is what the user is usually looking for.
        rows.extend(self.system_actions_matching(query));
        // The window commands come last of all.
        rows.extend(self.window_commands_matching(query));
        // The commands come after the applications, and the user's own commands
        // after the built-in ones.
        rows.extend(self.command_rows(query));
        rows.extend(self.custom_command_rows(query));
        // The files come last of all: "It sits below the apps and commands,
        // so return still opens the top result."
        if self.show_files {
            rows.extend(self.launcher_file_rows(query));
        }
        rows
    }

    /// The window commands whose name the query finds.
    ///
    /// The query has to appear in the name as a whole, the way the system
    /// actions are found: "left" finds "Left Half" and "Top Left Quarter",
    /// and "lhf" finds nothing at all.
    pub fn window_commands_matching(&self, query: &str) -> Vec<ResultRow> {
        if !self.show_window_commands {
            return Vec::new();
        }
        let wanted = query.trim().to_lowercase();
        if wanted.is_empty() {
            return Vec::new();
        }
        tc_windows::command::WindowCommandCatalog::all()
            .into_iter()
            .filter(|command| command.name.to_lowercase().contains(&wanted))
            .map(|command| ResultRow {
                id: format!("{WINDOW_COMMAND_PREFIX}{}", command.id.as_str()),
                name: command.name.to_string(),
                detail: command.group.label().to_string(),
                kind: WINDOW_COMMAND_SECTION.to_string(),
                target: command.id.as_str().to_string(),
                needs_confirm: false,
                is_dir: None,
                arguments: Vec::new(),
            })
            .collect()
    }

    /// The window command a row id names, if it names one.
    pub fn window_command_of(id: &str) -> Option<tc_windows::command::WindowCommandId> {
        let rest = id.strip_prefix(WINDOW_COMMAND_PREFIX)?;
        tc_windows::command::WindowCommandId::parse(rest)
    }

    /// The system actions whose name the query finds.
    ///
    /// The query has to appear in the name as a whole, so a system action is
    /// found by a word of its name and not by scattered letters: "lock" finds
    /// "Lock Screen", and "vsc" finds no action at all.
    pub fn system_actions_matching(&self, query: &str) -> Vec<ResultRow> {
        let wanted = query.trim().to_lowercase();
        if wanted.is_empty() {
            return Vec::new();
        }
        SystemActionId::ALL
            .into_iter()
            .filter(|action| action.english_name().to_lowercase().contains(&wanted))
            .map(|action| ResultRow {
                id: format!("{SYSTEM_ACTION_PREFIX}{}", action.as_str()),
                name: action.english_name().to_string(),
                detail: action.category().label().to_string(),
                kind: SYSTEM_ACTION_SECTION.to_string(),
                target: action.as_str().to_string(),
                needs_confirm: action.confirmation() != tc_core::SystemActionConfirmation::None,
                is_dir: None,
                arguments: Vec::new(),
            })
            .collect()
    }

    /// The system action a row id names, if it names one.
    pub fn system_action_of(id: &str) -> Option<SystemActionId> {
        let rest = id.strip_prefix(SYSTEM_ACTION_PREFIX)?;
        SystemActionId::parse(rest)
    }

    /// The row's target, and the key the ranking learns under.
    pub fn target_of(&self, id: &str) -> Option<(String, String)> {
        self.entries
            .iter()
            .find(|entry| entry.program.id == id)
            .map(|entry| (entry.program.target.clone(), entry.program.name.clone()))
    }

    /// Learn that the user opened an entry by a query.
    pub fn remember(&self, id: &str, query: &str) {
        let mut ranking = self.ranking.lock().expect("the ranking lock");
        ranking.visit(id, Some(query), now_seconds());
        let _ = ranking.save();
    }

    /// The rows with nothing typed: what the user opens most, then the rest.
    pub fn suggestions(&self) -> Vec<ResultRow> {
        let ranking = self.ranking.lock().expect("the ranking lock");
        let at = now_seconds();
        let entries: Vec<Entry> = self
            .entries
            .iter()
            .map(|entry| {
                let mut entry = entry.clone();
                entry.signals.usage = ranking.usage(&entry.program.id, at);
                entry
            })
            .collect();
        drop(ranking);

        LauncherOrder::by_usage(&entries)
            .into_iter()
            .take(LIMIT)
            .map(|entry| ResultRow {
                id: entry.program.id.clone(),
                name: entry.program.name.clone(),
                detail: entry
                    .program
                    .publisher
                    .clone()
                    .unwrap_or_else(|| entry.program.source.label().to_string()),
                kind: "Applications".to_string(),
                target: entry.program.target.clone(),
                needs_confirm: false,
                is_dir: None,
                arguments: Vec::new(),
            })
            .collect()
    }
}

/// A file, as a row.
///
/// "Each row shows the name, the folder it's in, and whether it's a file or
/// a folder."
fn file_row(entry: FileEntry) -> ResultRow {
    ResultRow {
        id: format!("{FILE_PREFIX}{}", entry.path),
        name: entry.name,
        detail: entry.parent,
        kind: FILE_SECTION.to_string(),
        target: entry.path,
        needs_confirm: false,
        is_dir: Some(entry.is_dir),
        arguments: Vec::new(),
    }
}

/// The clock the ranking reads, in seconds since the epoch.
pub fn now_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// The calculator's answer to the query, as a row.
///
/// The answer is copied when the row is opened, so the row's target is the text
/// that lands on the clipboard.
fn calculator_row(query: &str) -> Option<ResultRow> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return None;
    }
    let CalcOutcome::Card(card) = evaluate(trimmed, &FullCatalog) else {
        return None;
    };
    let tc_calc::Payload::Value { display, copy_text } = &card.payload else {
        return None;
    };
    // A query that only repeats itself is not a calculation.
    if display == trimmed {
        return None;
    }
    Some(ResultRow {
        id: format!("{CALCULATOR_PREFIX}{copy_text}"),
        name: display.clone(),
        detail: card.expression.clone(),
        kind: CALCULATOR_SECTION.to_string(),
        target: copy_text.clone(),
        needs_confirm: false,
        is_dir: None,
        arguments: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tc_apps::Source;

    fn program(name: &str) -> Program {
        Program::new(name, format!("C:\\{name}.exe"), Source::Registry)
    }

    fn launcher(programs: Vec<Program>) -> LauncherState {
        LauncherState::from_programs(programs, PathBuf::from("unused-ranking.json"))
    }

    /// An index over a handful of files, in memory.
    fn file_search(entries: Vec<FileEntry>) -> FileSearch {
        let mut store = tc_files::store::IndexStore::open_in_memory().expect("an in-memory index");
        tc_files::index::Backing::rewrite_entries(&mut store, "C:\\Users\\dabin", &entries);
        FileSearch::new(store, PathBuf::from("C:\\Users\\dabin"), Vec::new())
    }

    fn file(path: &str, name: &str, modified_ns: i64, is_dir: bool) -> FileEntry {
        FileEntry {
            path: path.to_string(),
            name: name.to_string(),
            parent: path
                .rsplit_once('\\')
                .map(|(parent, _)| parent)
                .unwrap_or("")
                .to_string(),
            size: 10,
            modified_ns,
            is_dir,
        }
    }

    fn sample_files() -> Vec<FileEntry> {
        vec![
            file(
                "C:\\Users\\dabin\\Documents\\annual report.pdf",
                "annual report.pdf",
                300,
                false,
            ),
            file(
                "C:\\Users\\dabin\\Documents\\notes.txt",
                "notes.txt",
                400,
                false,
            ),
            file(
                "C:\\Users\\dabin\\Documents\\annual report",
                "annual report",
                800,
                true,
            ),
        ]
    }

    #[test]
    fn there_are_no_files_while_the_file_search_is_off() {
        let launcher = launcher(vec![]);
        assert!(launcher.launcher_file_rows("annual").is_empty());
        assert!(launcher
            .file_screen_rows("annual", FileType::All)
            .is_empty());
    }

    #[test]
    fn the_files_are_listed_under_their_own_section() {
        let mut launcher = launcher(vec![]);
        launcher.set_file_search(file_search(sample_files()));
        launcher.set_show_files(true);
        let rows = launcher.search("annual");
        let files: Vec<&ResultRow> = rows.iter().filter(|row| row.kind == FILE_SECTION).collect();
        assert_eq!(files.len(), 2);
        assert!(files[0].id.starts_with(FILE_PREFIX));
    }

    #[test]
    fn the_files_sit_below_the_programs() {
        let mut launcher = launcher(vec![program("Annual")]);
        launcher.set_file_search(file_search(sample_files()));
        launcher.set_show_files(true);
        let rows = launcher.search("annual");
        assert_eq!(rows[0].kind, "Applications");
        assert_eq!(rows[rows.len() - 1].kind, FILE_SECTION);
    }

    #[test]
    fn the_files_can_be_kept_out_of_the_launcher() {
        let mut launcher = launcher(vec![]);
        launcher.set_file_search(file_search(sample_files()));
        launcher.set_show_files(false);
        let rows = launcher.search("annual");
        assert!(rows.iter().all(|row| row.kind != FILE_SECTION));
    }

    #[test]
    fn the_file_screen_shows_the_recent_files_with_nothing_typed() {
        let mut launcher = launcher(vec![]);
        launcher.set_file_search(file_search(sample_files()));
        let rows = launcher.file_screen_rows("", FileType::All);
        assert_eq!(rows[0].name, "notes.txt", "the newest own file");
    }

    #[test]
    fn the_file_screen_shows_the_results_with_something_typed() {
        let mut launcher = launcher(vec![]);
        launcher.set_file_search(file_search(sample_files()));
        let rows = launcher.file_screen_rows("annual", FileType::All);
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn the_file_screen_narrows_to_the_folders() {
        let mut launcher = launcher(vec![]);
        launcher.set_file_search(file_search(sample_files()));
        let rows = launcher.file_screen_rows("annual", FileType::Folders);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "annual report");
        assert_eq!(rows[0].is_dir, Some(true));
    }

    #[test]
    fn a_file_row_says_where_the_file_is() {
        let mut launcher = launcher(vec![]);
        launcher.set_file_search(file_search(sample_files()));
        let rows = launcher.file_screen_rows("notes", FileType::All);
        assert_eq!(rows[0].detail, "C:\\Users\\dabin\\Documents");
        assert_eq!(rows[0].is_dir, Some(false));
    }

    #[test]
    fn a_program_is_found_by_its_name() {
        let launcher = launcher(vec![program("Notepad"), program("WordPad")]);
        let rows = launcher.search("note");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "Notepad");
    }

    #[test]
    fn scattered_letters_still_find_the_program() {
        let launcher = launcher(vec![program("Visual Studio Code")]);
        let rows = launcher.search("vsc");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "Visual Studio Code");
    }

    #[test]
    fn a_query_that_fits_nothing_finds_nothing() {
        let launcher = launcher(vec![program("Notepad")]);
        assert!(launcher.search("qqqqzzz").is_empty());
    }

    #[test]
    fn an_empty_query_finds_nothing() {
        let launcher = launcher(vec![program("Notepad")]);
        assert!(launcher.search("").is_empty());
    }

    #[test]
    fn a_window_command_is_found_by_its_name() {
        let mut launcher = launcher(vec![]);
        launcher.set_show_window_commands(true);
        let rows = launcher.search("left half");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "Left Half");
        assert_eq!(rows[0].kind, "Window Commands");
        assert_eq!(rows[0].id, "window-command:left-half");
    }

    #[test]
    fn the_window_commands_are_listed_under_their_own_section() {
        let mut launcher = launcher(vec![]);
        launcher.set_show_window_commands(true);
        let rows = launcher.search("quarter");
        assert_eq!(rows.len(), 4, "the four quarters");
        for row in &rows {
            assert_eq!(row.kind, "Window Commands");
        }
    }

    #[test]
    fn the_window_commands_can_be_hidden_from_the_launcher() {
        let mut launcher = launcher(vec![]);
        // Window management is off until it is turned on, so the commands are
        // not in the results to begin with.
        assert!(launcher.search("left half").is_empty());
        launcher.set_show_window_commands(true);
        assert!(!launcher.search("left half").is_empty());
        launcher.set_show_window_commands(false);
        assert!(launcher.search("left half").is_empty());
    }

    #[test]
    fn a_window_command_row_names_the_command_it_runs() {
        let command = LauncherState::window_command_of("window-command:top-left-quarter");
        assert_eq!(
            command,
            Some(tc_windows::command::WindowCommandId::TopLeftQuarter)
        );
        assert!(LauncherState::window_command_of("left-half").is_none());
        assert!(LauncherState::window_command_of("window-command:nope").is_none());
    }

    #[test]
    fn a_window_command_is_found_by_a_word_of_its_name() {
        let mut launcher = launcher(vec![]);
        launcher.set_show_window_commands(true);
        // "half" finds the halves, the quarters are not halves.
        let rows = launcher.search("half");
        assert!(rows.iter().any(|row| row.name == "Left Half"));
        assert!(rows.iter().all(|row| row.name.contains("Half")));
    }

    #[test]
    fn the_suggestions_are_the_programs_the_machine_has() {
        let launcher = launcher(vec![program("Notepad"), program("WordPad")]);
        assert_eq!(launcher.suggestions().len(), 2);
    }

    #[test]
    fn a_row_carries_what_to_open() {
        let launcher = launcher(vec![program("Notepad")]);
        let rows = launcher.search("notepad");
        assert_eq!(rows[0].target, "C:\\Notepad.exe");
        assert_eq!(rows[0].kind, "Applications");
        let (target, _) = launcher.target_of(&rows[0].id).expect("the row is known");
        assert_eq!(target, "C:\\Notepad.exe");
    }

    #[test]
    fn a_system_action_is_found_by_its_name() {
        let launcher = launcher(vec![]);
        let rows = launcher.search("lock");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "Lock Screen");
        assert_eq!(rows[0].kind, "System Actions");
        assert_eq!(rows[0].id, "system-action:lock-screen");
    }

    #[test]
    fn a_system_action_that_needs_asking_says_so() {
        let launcher = launcher(vec![]);
        let rows = launcher.search("empty");
        assert_eq!(rows.len(), 1);
        assert!(
            rows[0].needs_confirm,
            "emptying the bin has to be asked for"
        );
        let quiet = launcher.search("sleep displays");
        assert!(
            !quiet[0].needs_confirm,
            "sleeping the display needs no asking"
        );
    }

    #[test]
    fn a_system_action_row_names_the_action_it_runs() {
        let launcher = launcher(vec![]);
        let rows = launcher.search("volume up");
        assert_eq!(rows[0].target, "volume-up");
        assert_eq!(rows[0].detail, "Volume");
    }

    #[test]
    fn a_row_that_is_not_a_system_action_is_not_one() {
        assert_eq!(LauncherState::system_action_of("app:notepad"), None);
        assert_eq!(
            LauncherState::system_action_of("system-action:banana"),
            None
        );
        assert_eq!(
            LauncherState::system_action_of("system-action:lock-screen"),
            Some(SystemActionId::LockScreen)
        );
    }

    #[test]
    fn an_empty_query_finds_no_system_action() {
        let launcher = launcher(vec![]);
        assert!(launcher.search("").is_empty());
    }

    // MARK: - The calculator's row

    #[test]
    fn a_calculation_is_a_row_of_its_own() {
        let row = calculator_row("10 km to m").expect("the query is a conversion");
        assert_eq!(row.name, "10,000 m");
        assert_eq!(row.detail, "10 km");
        assert_eq!(row.kind, CALCULATOR_SECTION);
        assert_eq!(row.target, "10000 m");
        assert!(row.id.starts_with(CALCULATOR_PREFIX));
        assert!(!row.needs_confirm);
    }

    #[test]
    fn the_calculator_row_comes_first() {
        let launcher = launcher(vec![program("Notepad")]);
        let rows = launcher.search("2+2");
        assert_eq!(rows.first().expect("a row").kind, CALCULATOR_SECTION);
        assert_eq!(rows.first().expect("a row").name, "4");
    }

    #[test]
    fn a_number_that_only_repeats_itself_is_not_a_calculation() {
        assert!(calculator_row("5").is_none());
        assert!(calculator_row("").is_none());
        assert!(calculator_row("notepad").is_none());
    }

    #[test]
    fn a_percentage_is_a_row_of_its_own() {
        let row = calculator_row("20% off 500").expect("the query is a discount");
        assert_eq!(row.name, "400");
        assert_eq!(row.detail, "20% off 500");
        assert_eq!(row.kind, CALCULATOR_SECTION);
    }

    #[test]
    fn an_amount_of_money_is_a_row_of_its_own() {
        let row = calculator_row("20% discount off $100").expect("the query is a money discount");
        assert_eq!(row.name, "80.00 USD");
        assert_eq!(row.kind, CALCULATOR_SECTION);
    }

    // MARK: - The commands

    #[test]
    fn a_built_in_command_is_found_by_its_name() {
        let launcher = launcher(vec![]);
        let rows = launcher.search("quit yal");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "Quit YAL");
        assert_eq!(rows[0].kind, COMMAND_SECTION);
        assert_eq!(rows[0].id, "command:quit");
    }

    #[test]
    fn a_built_in_command_names_the_pane_it_belongs_to() {
        let launcher = launcher(vec![]);
        let rows = launcher.search("support yal");
        assert_eq!(rows[0].detail, "Commands");
    }

    #[test]
    fn a_built_in_command_that_is_switched_off_is_not_offered() {
        let mut launcher = launcher(vec![]);
        let mut settings = CommandSettings::new();
        settings.switches.hidden.insert(CommandId::Quit);
        launcher.set_command_settings(settings);
        assert!(launcher.search("quit yal").is_empty());
    }

    #[test]
    fn a_built_in_command_can_be_given_an_alias() {
        let mut launcher = launcher(vec![]);
        let mut settings = CommandSettings::new();
        settings.set_alias(CommandId::Quit, Some("Leave".to_string()));
        launcher.set_command_settings(settings);
        let rows = launcher.search("leave");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "Leave");
    }

    #[test]
    fn a_custom_command_is_found_by_its_name() {
        let mut launcher = launcher(vec![]);
        launcher.set_custom_commands(vec![CustomCommand::new("Nap", "Start-Sleep 5")]);
        let rows = launcher.search("nap");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "Nap");
        assert_eq!(rows[0].kind, CUSTOM_COMMAND_SECTION);
        assert_eq!(rows[0].detail, "Start-Sleep 5");
        assert!(rows[0].id.starts_with("custom-command:"));
    }

    #[test]
    fn a_custom_command_that_is_switched_off_is_not_offered() {
        let mut launcher = launcher(vec![]);
        let mut command = CustomCommand::new("Nap", "Start-Sleep 5");
        command.is_enabled = false;
        launcher.set_custom_commands(vec![command]);
        assert!(launcher.search("nap").is_empty());
    }

    #[test]
    fn a_custom_command_taken_out_of_the_main_search_is_not_offered() {
        let mut launcher = launcher(vec![]);
        let mut command = CustomCommand::new("Nap", "Start-Sleep 5");
        command.shows_in_root_search = false;
        launcher.set_custom_commands(vec![command]);
        assert!(launcher.search("nap").is_empty());
    }

    #[test]
    fn a_custom_command_carries_the_values_it_asks_for_and_asks_first() {
        let mut launcher = launcher(vec![]);
        let mut command = CustomCommand::new("Move", "Move-Item $1 $2");
        command.arguments = vec![
            tc_commands::model::CustomCommandArgument::new("From", false),
            tc_commands::model::CustomCommandArgument::new("To", true),
        ];
        let id = command.entry_id();
        launcher.set_custom_commands(vec![command]);
        let rows = launcher.search("move");
        assert_eq!(rows[0].arguments.len(), 2);
        assert_eq!(rows[0].arguments[0].name, "From");
        assert!(rows[0].needs_confirm, "a command asks before it runs");
        assert!(launcher.custom_command_of(&id).is_some());
    }

    #[test]
    fn the_custom_command_names_the_values_it_asks_for() {
        let mut launcher = launcher(vec![]);
        let mut command = CustomCommand::new("Move", "Move-Item $1");
        command.arguments = vec![tc_commands::model::CustomCommandArgument::new(
            "From", false,
        )];
        launcher.set_custom_commands(vec![command]);
        let rows = launcher.search("move");
        assert_eq!(rows[0].arguments.len(), 1);
        assert!(!rows[0].arguments[0].is_optional);
    }
}
