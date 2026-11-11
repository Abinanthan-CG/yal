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
use tc_core::SystemActionId;
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
}

/// A program, with the names the search matches against.
#[derive(Clone)]
struct Entry {
    program: Program,
    profile: SearchProfile,
    signals: Signals,
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
        }
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
                })
                .collect();
        // The system actions are matched by their name, and they sit under the
        // applications: a program is what the user is usually looking for.
        rows.extend(self.system_actions_matching(query));
        rows
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
            })
            .collect()
    }
}

/// The clock the ranking reads, in seconds since the epoch.
pub fn now_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
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
}
