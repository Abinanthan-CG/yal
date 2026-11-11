//! The one place an entry's searchable names are decided, for every kind alike.
//!
//! Ported from upstream's
//! `Tinycast/Features/Launcher/Model/EntryNaming.swift`.
//!
//! An entry is found by more than one name: the one it is presented under, the
//! ones it is also known by, the thing it comes from, and the words it should
//! match without ranking above anything. This decides which of those survive
//! and how each is folded.

use crate::fuzzy::normalized;
use crate::matcher::SearchText;

/// The names an entry is known by, before they are folded.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sources {
    /// The name the entry is presented under.
    pub name: String,
    /// Ranked like the title: a translation, a renamed file, a snippet keyword.
    pub alternate_titles: Vec<String>,
    /// What the entry comes from, shown beside it: an extension's title.
    pub subtitle: Option<String>,
    /// Found by, never ranked by: a declared name, an extension's keywords.
    pub keywords: Vec<String>,
}

impl Sources {
    pub fn new(name: impl Into<String>) -> Sources {
        Sources {
            name: name.into(),
            ..Sources::default()
        }
    }

    pub fn with_alternate_titles(mut self, titles: impl IntoIterator<Item = String>) -> Sources {
        self.alternate_titles = titles.into_iter().collect();
        self
    }

    pub fn with_subtitle(mut self, subtitle: impl Into<String>) -> Sources {
        self.subtitle = Some(subtitle.into());
        self
    }

    pub fn with_keywords(mut self, keywords: impl IntoIterator<Item = String>) -> Sources {
        self.keywords = keywords.into_iter().collect();
        self
    }
}

/// An entry's searchable names, folded once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchProfile {
    /// The name the entry is presented under.
    pub title: SearchText,
    /// The other names it is known by.
    pub alternate_titles: Vec<SearchText>,
    /// What the entry comes from.
    pub subtitle: Option<SearchText>,
    /// Match to appear, never to rank.
    pub keywords: Vec<SearchText>,
}

impl SearchProfile {
    /// What an entry holds before its first index pass: nothing a query can
    /// reach.
    pub fn unnamed() -> SearchProfile {
        SearchProfile {
            title: SearchText::from_units(Vec::new()),
            alternate_titles: Vec::new(),
            subtitle: None,
            keywords: Vec::new(),
        }
    }
}

/// The names an entry is found by.
pub struct EntryNaming;

impl EntryNaming {
    /// The profile for a set of names.
    pub fn profile(sources: &Sources) -> SearchProfile {
        let title = SearchText::new(&sources.name, true);
        // Folded, never transliterated: these compare against the query as typed.
        let alternates = Self::usable(
            &sources.alternate_titles,
            std::slice::from_ref(&sources.name),
        )
        .into_iter()
        .map(|name| SearchText::new(&name, false))
        .collect::<Vec<_>>();

        let subtitle = sources
            .subtitle
            .as_ref()
            .map(|subtitle| SearchText::new(subtitle, true))
            .filter(|subtitle| !subtitle.is_empty() && subtitle.units() != title.units());

        let mut rejecting = vec![sources.name.clone()];
        rejecting.extend(sources.alternate_titles.iter().cloned());
        let mut keywords = Self::usable(&sources.keywords, &rejecting)
            .into_iter()
            .map(|name| SearchText::new(&name, true))
            .collect::<Vec<_>>();

        if let Some(subtitle) = &subtitle {
            keywords.push(title.joined_with(subtitle));
            keywords.push(subtitle.joined_with(&title));
        }

        SearchProfile {
            title,
            alternate_titles: alternates,
            subtitle,
            keywords,
        }
    }

    /// The name without the bundle extension.
    ///
    /// Upstream's bundle is a `.app`; a Windows executable carries its own
    /// extension, and the name that reaches this function has already lost it.
    pub fn stripping_app_extension(name: &str) -> String {
        match name.strip_suffix(".app") {
            Some(stripped) => stripped.to_string(),
            None => name.to_string(),
        }
    }

    /// The names worth keeping: not empty, not a placeholder, and not one the
    /// entry is already known by.
    ///
    /// Info.plist lists repeat the name and ship `ALTERNATE_NAME_1`
    /// placeholders.
    pub fn usable(raw: &[String], rejecting: &[String]) -> Vec<String> {
        let mut seen: Vec<String> = rejecting
            .iter()
            .map(|name| normalized(&Self::stripping_app_extension(name)))
            .collect();
        let mut out = Vec::new();
        for candidate in raw {
            let name = candidate.trim().to_string();
            if name.is_empty() || Self::is_placeholder(&name) {
                continue;
            }
            let key = normalized(&Self::stripping_app_extension(&name));
            if key.is_empty() || seen.contains(&key) {
                continue;
            }
            seen.push(key);
            out.push(name);
        }
        out
    }

    /// A lone SCREAMING_SNAKE token is an untranslated placeholder, and several
    /// ship.
    fn is_placeholder(name: &str) -> bool {
        name.contains('_') && !name.chars().any(|c| c.is_lowercase() || c.is_whitespace())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(
        name: &str,
        alternates: &[&str],
        subtitle: Option<&str>,
        keywords: &[&str],
    ) -> SearchProfile {
        let mut sources = Sources::new(name);
        sources.alternate_titles = alternates.iter().map(|s| s.to_string()).collect();
        sources.subtitle = subtitle.map(|s| s.to_string());
        sources.keywords = keywords.iter().map(|s| s.to_string()).collect();
        EntryNaming::profile(&sources)
    }

    fn strings(texts: &[SearchText]) -> Vec<String> {
        texts.iter().map(|t| t.string()).collect()
    }

    #[test]
    fn an_alternate_repeating_the_title_is_dropped() {
        let settings = profile(
            "System Settings",
            &["System Settings", "系统设置"],
            None,
            &["ALTERNATE_NAME_1", "System Settings", "Preferences"],
        );
        assert_eq!(settings.alternate_titles.len(), 1);
    }

    #[test]
    fn an_alternate_title_is_folded_never_transliterated() {
        let settings = profile("System Settings", &["系统设置"], None, &[]);
        assert_eq!(settings.alternate_titles[0].string(), "系统设置");
    }

    #[test]
    fn a_placeholder_and_a_repeated_name_leave_the_keywords() {
        let settings = profile(
            "System Settings",
            &["System Settings", "系统设置"],
            None,
            &["ALTERNATE_NAME_1", "System Settings", "Preferences"],
        );
        assert_eq!(strings(&settings.keywords), vec!["preferences"]);
    }

    #[test]
    fn a_subtitle_rides_along() {
        let command = profile("Search", &[], Some("Brew"), &[]);
        assert_eq!(command.subtitle.unwrap().string(), "brew");
    }

    #[test]
    fn the_title_and_the_subtitle_join_both_ways_as_keywords() {
        let command = profile("Search", &[], Some("Brew"), &[]);
        assert_eq!(
            strings(&command.keywords),
            vec!["search brew", "brew search"]
        );
    }

    #[test]
    fn a_subtitle_equal_to_the_title_is_dropped() {
        assert!(profile("Zed", &[], Some("zed"), &[]).subtitle.is_none());
    }

    #[test]
    fn an_unbuilt_entry_matches_nothing() {
        assert!(SearchProfile::unnamed().title.is_empty());
    }

    #[test]
    fn the_bundle_extension_is_stripped_before_the_fold() {
        assert_eq!(EntryNaming::stripping_app_extension("Safari.app"), "Safari");
        assert_eq!(EntryNaming::stripping_app_extension("Safari"), "Safari");
    }

    #[test]
    fn a_repeated_alternate_keeps_only_the_first() {
        let usable = EntryNaming::usable(
            &[
                "Photos".to_string(),
                "photos".to_string(),
                "Finder".to_string(),
            ],
            &[],
        );
        assert_eq!(usable, vec!["Photos", "Finder"]);
    }

    #[test]
    fn a_placeholder_is_a_lone_screaming_snake_token() {
        let usable = EntryNaming::usable(
            &[
                "ALTERNATE_NAME_1".to_string(),
                "ALTERNATE NAME 2".to_string(),
                "PREMIUM_APP".to_string(),
            ],
            &[],
        );
        assert_eq!(usable, vec!["ALTERNATE NAME 2"]);
    }

    #[test]
    fn an_empty_name_is_never_usable() {
        let usable = EntryNaming::usable(&["   ".to_string(), "Finder".to_string()], &[]);
        assert_eq!(usable, vec!["Finder"]);
    }

    #[test]
    fn a_name_the_entry_already_has_is_never_usable() {
        let usable = EntryNaming::usable(&["Finder".to_string()], &["finder".to_string()]);
        assert!(usable.is_empty());
    }
}
