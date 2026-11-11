//! The launcher's fallbacks: the rows offered under "Use "<query>" with…".
//!
//! Upstream calls this `Fallback`. A fallback is a destination that takes the
//! typed query as its input, so it is offered whatever the query says.
//!
//! The id of a fallback *is* the row's entry id, which is what lets a stored
//! order name a row that is still live.

use crate::command::CommandId;
use crate::entry::PREFIX_QUICKLINK;
use crate::ids::Uuid;

/// The shipped fallbacks, in the order a fresh install offers them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FallbackBuiltin {
    /// Asks the AI provider.
    QuickAi,
    /// Opens file search with the query typed in.
    SearchFiles,
    /// Runs the query in a shell.
    RunShellCommand,
    /// Looks the word up in the dictionary.
    Define,
}

impl FallbackBuiltin {
    /// Every built-in, in the order a fresh install offers them.
    pub const ALL: [FallbackBuiltin; 4] = [
        FallbackBuiltin::QuickAi,
        FallbackBuiltin::SearchFiles,
        FallbackBuiltin::RunShellCommand,
        FallbackBuiltin::Define,
    ];

    /// The command the fallback runs.
    pub fn command(self) -> CommandId {
        match self {
            FallbackBuiltin::QuickAi => CommandId::QuickAi,
            FallbackBuiltin::SearchFiles => CommandId::SearchFiles,
            FallbackBuiltin::RunShellCommand => CommandId::RunShellCommand,
            FallbackBuiltin::Define => CommandId::Define,
        }
    }

    /// The verb the row shows, so the row reads like the command it runs.
    pub fn open_verb(self) -> &'static str {
        match self {
            FallbackBuiltin::QuickAi => "Ask Quick AI",
            FallbackBuiltin::SearchFiles => "Search Files",
            FallbackBuiltin::RunShellCommand => "Run Shell Command",
            FallbackBuiltin::Define => "Define Word",
        }
    }
}

/// A destination that takes the typed query as its input.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Fallback {
    /// One of the shipped fallbacks.
    Builtin(FallbackBuiltin),
    /// A quicklink that takes an argument.
    Quicklink(Uuid),
}

impl Fallback {
    /// The row's entry id, so a stored order outlives a rename and survives a
    /// reinstall.
    pub fn id(&self) -> String {
        match self {
            Fallback::Builtin(builtin) => builtin.command().entry_id().to_string(),
            Fallback::Quicklink(id) => format!("{PREFIX_QUICKLINK}{id}"),
        }
    }

    /// Read a fallback back from its id.
    ///
    /// A command with no fallback of its own is not a fallback, and neither is
    /// a bare UUID: only the four built-ins and a quicklink resolve.
    pub fn from_id(id: &str) -> Option<Fallback> {
        if let Some(command) = CommandId::from_entry_id(id) {
            return FallbackBuiltin::ALL
                .iter()
                .find(|builtin| builtin.command() == command)
                .map(|builtin| Fallback::Builtin(*builtin));
        }
        let rest = id.strip_prefix(PREFIX_QUICKLINK)?;
        let uuid = Uuid::parse(rest)?;
        Some(Fallback::Quicklink(uuid))
    }

    /// The verb the row shows.
    pub fn open_verb(&self) -> &'static str {
        match self {
            Fallback::Builtin(builtin) => builtin.open_verb(),
            Fallback::Quicklink(_) => "Open Quicklink",
        }
    }

    /// The fallbacks in the order they are offered: the stored order first, then
    /// anything it has never seen.
    ///
    /// A quicklink added today lands at the end rather than vanishing, and a
    /// stored id whose quicklink has been deleted is skipped without shifting
    /// its neighbours.
    pub fn ordered(available: &[Fallback], stored_ids: &[String]) -> Vec<Fallback> {
        let mut known: Vec<Fallback> = Vec::new();
        let mut used: Vec<String> = Vec::new();
        for id in stored_ids {
            if used.iter().any(|seen| seen == id) {
                continue;
            }
            if let Some(fallback) = available.iter().find(|fallback| fallback.id() == *id) {
                used.push(id.clone());
                known.push(fallback.clone());
            }
        }
        let rest: Vec<Fallback> = available
            .iter()
            .filter(|fallback| !used.contains(&fallback.id()))
            .cloned()
            .collect();
        known.into_iter().chain(rest).collect()
    }

    /// The section header above the fallback rows.
    ///
    /// A long query is elided in the middle, so "with…" always survives.
    pub fn section_title(query: &str, limit: usize) -> String {
        let characters: Vec<char> = query.chars().collect();
        if characters.len() <= limit {
            return format!("Use \u{201c}{query}\u{201d} with\u{2026}");
        }
        let head: String = characters[..limit / 2].iter().collect();
        let tail_length = limit - limit / 2 - 1;
        let tail: String = characters[characters.len() - tail_length..]
            .iter()
            .collect();
        format!("Use \u{201c}{head}\u{2026}{tail}\u{201d} with\u{2026}")
    }

    /// The section header, with upstream's shipped limit.
    pub fn default_section_title(query: &str) -> String {
        Self::section_title(query, 72)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quicklink() -> Fallback {
        Fallback::Quicklink(
            Uuid::parse("3f2504e0-4f89-11d3-9a0c-0305e82c3301").expect("a canonical uuid"),
        )
    }

    /// Upstream: "The id *is* the row's entry id, which is what lets a stored
    /// order name a live row."
    #[test]
    fn a_built_in_is_its_commands_id() {
        for builtin in FallbackBuiltin::ALL {
            let fallback = Fallback::Builtin(builtin);
            assert_eq!(fallback.id(), builtin.command().entry_id());
            assert_eq!(Fallback::from_id(&fallback.id()), Some(fallback));
        }
    }

    #[test]
    fn a_quicklink_is_its_entry_id() {
        let fallback = quicklink();
        assert_eq!(
            fallback.id(),
            "quicklink:3f2504e0-4f89-11d3-9a0c-0305e82c3301"
        );
        assert_eq!(Fallback::from_id(&fallback.id()), Some(fallback));
    }

    /// Upstream: "Every built-in's id must be distinct, or one would silently
    /// take another's stored slot."
    #[test]
    fn the_built_in_ids_are_distinct() {
        let mut ids: Vec<String> = FallbackBuiltin::ALL
            .iter()
            .map(|builtin| Fallback::Builtin(*builtin).id())
            .collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), FallbackBuiltin::ALL.len());
    }

    /// Upstream: "A command with no fallback of its own must not resolve to one."
    #[test]
    fn something_that_is_not_a_fallback_does_not_resolve() {
        assert_eq!(Fallback::from_id(CommandId::About.entry_id()), None);
        assert_eq!(Fallback::from_id("banana"), None);
        // A bare UUID is not a quicklink: the prefix is what makes it one.
        assert_eq!(
            Fallback::from_id("3f2504e0-4f89-11d3-9a0c-0305e82c3301"),
            None
        );
    }

    #[test]
    fn no_stored_order_keeps_the_offered_order() {
        let ai = Fallback::Builtin(FallbackBuiltin::QuickAi);
        let files = Fallback::Builtin(FallbackBuiltin::SearchFiles);
        let shell = Fallback::Builtin(FallbackBuiltin::RunShellCommand);
        assert_eq!(
            Fallback::ordered(&[ai.clone(), files.clone(), shell.clone()], &[]),
            vec![ai, files, shell]
        );
    }

    #[test]
    fn a_stored_order_is_honoured() {
        let ai = Fallback::Builtin(FallbackBuiltin::QuickAi);
        let files = Fallback::Builtin(FallbackBuiltin::SearchFiles);
        let shell = Fallback::Builtin(FallbackBuiltin::RunShellCommand);
        let stored = vec![shell.id(), ai.id(), files.id()];
        assert_eq!(
            Fallback::ordered(&[ai.clone(), files.clone(), shell.clone()], &stored),
            vec![shell, ai, files]
        );
    }

    /// Upstream: "A quicklink created after the last reorder must land at the
    /// end, not vanish."
    #[test]
    fn an_unseen_fallback_lands_last() {
        let ai = Fallback::Builtin(FallbackBuiltin::QuickAi);
        let files = Fallback::Builtin(FallbackBuiltin::SearchFiles);
        let link = quicklink();
        let stored = vec![files.id(), ai.id()];
        assert_eq!(
            Fallback::ordered(&[ai.clone(), link.clone(), files.clone()], &stored),
            vec![files, ai, link]
        );
    }

    /// Upstream: "A deleted quicklink's id is still stored; it must not
    /// resurrect or shift its neighbours."
    #[test]
    fn a_stored_id_with_nothing_behind_it_is_skipped() {
        let ai = Fallback::Builtin(FallbackBuiltin::QuickAi);
        let files = Fallback::Builtin(FallbackBuiltin::SearchFiles);
        let link = quicklink();
        let stored = vec![link.id(), files.id(), ai.id()];
        assert_eq!(
            Fallback::ordered(&[ai.clone(), files.clone()], &stored),
            vec![files, ai]
        );
    }

    #[test]
    fn nothing_available_is_nothing_offered() {
        let ai = Fallback::Builtin(FallbackBuiltin::QuickAi);
        assert!(Fallback::ordered(&[], &[ai.id()]).is_empty());
    }

    #[test]
    fn a_short_query_is_quoted_whole() {
        assert_eq!(
            Fallback::section_title("todo", 72),
            "Use \u{201c}todo\u{201d} with\u{2026}"
        );
    }

    /// Upstream: "The point of the elision: the header must still say what the
    /// section is for."
    #[test]
    fn a_long_query_is_elided_in_the_middle() {
        let long = "a".repeat(200);
        let title = Fallback::section_title(&long, 20);
        assert!(title.ends_with("\u{201d} with\u{2026}"), "got {title}");
        assert!(title.contains("\u{2026}a"), "got {title}");
        assert!(title.chars().count() <= 20 + 13, "got {title}");
    }

    /// Upstream: "The boundary is exact equality, so a query at the limit is
    /// never touched."
    #[test]
    fn a_query_at_the_limit_is_kept_whole() {
        let exact = "b".repeat(20);
        assert_eq!(
            Fallback::section_title(&exact, 20),
            format!("Use \u{201c}{exact}\u{201d} with\u{2026}")
        );
    }

    /// Upstream: "The shipped limit has to clear a real URL, which is the query
    /// this was drawn against."
    #[test]
    fn the_default_limit_keeps_a_repository_url_whole() {
        let url = "https://github.com/vgnshiyer/py-apple-books.git";
        assert_eq!(
            Fallback::default_section_title(url),
            format!("Use \u{201c}{url}\u{201d} with\u{2026}")
        );
    }

    #[test]
    fn every_fallback_names_its_own_action() {
        let mut verbs: Vec<&str> = FallbackBuiltin::ALL
            .iter()
            .map(|builtin| Fallback::Builtin(*builtin).open_verb())
            .collect();
        verbs.push(
            Fallback::Quicklink(
                Uuid::parse("3f2504e0-4f89-11d3-9a0c-0305e82c3301").expect("a canonical uuid"),
            )
            .open_verb(),
        );
        assert!(verbs.iter().all(|verb| !verb.is_empty()));
        let mut distinct = verbs.clone();
        distinct.sort_unstable();
        distinct.dedup();
        assert_eq!(distinct.len(), verbs.len());
    }
}
