//! The programs the launcher can open.
//!
//! This crate holds the model and the rules; the reading of Windows is in
//! `tc-platform-win`, so the rules can be tested without a desktop.
//!
//! Upstream's macOS port of this is `Tinycast/Features/Launcher/Service/AppIndex.swift`,
//! which walks LaunchServices for `.app` bundles. Windows has no bundle, so the
//! same three questions are asked of three places instead:
//!
//! - the **Start Menu**, which holds the shortcut Windows shows the user;
//! - the registry's **uninstall** key, which names every installed program;
//! - the registry's **App Paths**, which says how Windows starts one.

use tc_search::entry_naming::{EntryNaming, SearchProfile, Sources};
use tc_search::fuzzy::normalized;

/// Where the launcher learned about a program.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Source {
    /// A shortcut in the Start Menu: the name Windows shows the user.
    StartMenu,
    /// The registry's uninstall key: an installed program.
    Registry,
    /// The registry's App Paths key: an executable Windows can start.
    AppPath,
}

impl Source {
    /// What the launcher calls this kind of entry.
    pub fn label(&self) -> &'static str {
        match self {
            Source::StartMenu => "Start menu",
            Source::Registry => "Installed program",
            Source::AppPath => "Application path",
        }
    }
}

/// One program the launcher can open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    /// A stable identifier: the same program always has the same one.
    pub id: String,
    /// The name it is shown under.
    pub name: String,
    /// The other names it is known by.
    pub alternates: Vec<String>,
    /// The words it is found by, but never ranked by.
    pub keywords: Vec<String>,
    /// Who publishes it.
    pub publisher: Option<String>,
    /// What the launcher opens: an executable, or a shortcut to one.
    pub target: String,
    /// Where the launcher learned about it.
    pub source: Source,
}

impl Program {
    /// A program, with nothing but its name and what to open.
    pub fn new(name: impl Into<String>, target: impl Into<String>, source: Source) -> Program {
        let name = name.into();
        Program {
            id: identity(&name),
            name,
            alternates: Vec::new(),
            keywords: Vec::new(),
            publisher: None,
            target: target.into(),
            source,
        }
    }

    pub fn with_alternates(mut self, alternates: Vec<String>) -> Program {
        self.alternates = alternates;
        self
    }

    pub fn with_keywords(mut self, keywords: Vec<String>) -> Program {
        self.keywords = keywords;
        self
    }

    pub fn with_publisher(mut self, publisher: Option<String>) -> Program {
        self.publisher = publisher;
        self
    }

    /// The profile the search matches against.
    ///
    /// The publisher is a keyword, not an alternate name: the launcher finds a
    /// program by who made it, but never ranks it above the program itself.
    pub fn profile(&self) -> SearchProfile {
        let mut sources = Sources::new(&self.name);
        sources.alternate_titles = self.alternates.clone();
        let mut keywords = self.keywords.clone();
        if let Some(publisher) = &self.publisher {
            keywords.push(publisher.clone());
        }
        sources.keywords = keywords;
        EntryNaming::profile(&sources)
    }
}

/// The identity of a program: its name, folded.
///
/// Two entries with the same folded name are the same program as far as the
/// launcher is concerned, whichever place they were found in.
pub fn identity(name: &str) -> String {
    normalized(name)
}

/// Keep one entry per program, and fold the rest into it.
///
/// The Start Menu wins, because its name is the one Windows shows the user.
/// The registry's name, the executable's name and the publisher become
/// alternates and keywords of the entry that survives.
pub fn deduplicate(programs: Vec<Program>) -> Vec<Program> {
    let mut best: Vec<Program> = Vec::new();
    // The sources are ordered by how much they are trusted, so the first
    // program with a given identity is the one that is kept.
    let mut ordered = programs;
    ordered.sort_by_key(|program| program.source);

    for program in ordered {
        let id = program.id.clone();
        match best.iter_mut().find(|kept| kept.id == id) {
            Some(kept) => {
                for alternate in program.alternates {
                    if !kept.alternates.contains(&alternate) && kept.name != alternate {
                        kept.alternates.push(alternate);
                    }
                }
                for keyword in program.keywords {
                    if !kept.keywords.contains(&keyword) {
                        kept.keywords.push(keyword);
                    }
                }
                if kept.publisher.is_none() {
                    kept.publisher = program.publisher;
                }
            }
            None => best.push(program),
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_program_is_found_by_its_name() {
        let program = Program::new("Visual Studio Code", "code.exe", Source::Registry);
        let profile = program.profile();
        assert_eq!(profile.title.string(), "visual studio code");
    }

    #[test]
    fn the_publisher_is_found_by_but_never_ranked_by() {
        let program = Program::new("Notepad", "notepad.exe", Source::Registry)
            .with_publisher(Some("Microsoft Corporation".to_string()));
        let profile = program.profile();
        assert_eq!(profile.keywords.len(), 1);
        assert_eq!(profile.keywords[0].string(), "microsoft corporation");
    }

    #[test]
    fn an_alternate_name_is_folded_but_not_transliterated() {
        let program = Program::new("系统设置", "settings.exe", Source::Registry)
            .with_alternates(vec!["System Settings".to_string()]);
        let profile = program.profile();
        assert_eq!(profile.alternate_titles[0].string(), "system settings");
    }

    #[test]
    fn the_start_menu_wins_when_the_same_program_is_listed_twice() {
        let programs = vec![
            Program::new("Notepad", "notepad.exe", Source::Registry)
                .with_publisher(Some("Microsoft".to_string())),
            Program::new("notepad", "notepad.lnk", Source::StartMenu),
        ];
        let kept = deduplicate(programs);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].source, Source::StartMenu);
        // The publisher the registry knew about is not lost.
        assert_eq!(kept[0].publisher.as_deref(), Some("Microsoft"));
    }

    #[test]
    fn two_different_programs_are_kept_apart() {
        let programs = vec![
            Program::new("Notepad", "notepad.exe", Source::Registry),
            Program::new("WordPad", "wordpad.exe", Source::Registry),
        ];
        assert_eq!(deduplicate(programs).len(), 2);
    }

    #[test]
    fn the_identity_folds_case_and_diacritics() {
        assert_eq!(identity("Café"), identity("cafe"));
        assert_ne!(identity("Notepad"), identity("WordPad"));
    }
}
