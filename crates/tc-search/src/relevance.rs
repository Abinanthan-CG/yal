//! How well a query fits an entry.
//!
//! Ported from upstream's
//! `Tinycast/Features/Launcher/Model/SearchRelevance.swift`.
//!
//! An entry can be found by several aliases, and each alias has a role: the
//! name the entry is presented under, or the thing that provides it. The
//! role decides which cell a match lands in, and the cell decides how the
//! entry is ordered.

use crate::fuzzy::{self, Candidate, Query, Tier};

/// What a role means.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    /// What provides the entry rather than what it is: a menu's path, a
    /// window's application.
    Owner,
    /// The name the entry is presented under.
    Name,
}

/// One string an entry can be found by.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SearchAlias {
    pub text: String,
    pub role: Role,
}

impl SearchAlias {
    pub fn name(text: impl Into<String>) -> SearchAlias {
        SearchAlias {
            text: text.into(),
            role: Role::Name,
        }
    }

    pub fn owner(text: impl Into<String>) -> SearchAlias {
        SearchAlias {
            text: text.into(),
            role: Role::Owner,
        }
    }
}

/// The strings an entry can be found by.
///
/// These are never flattened into one string: which alias matched is half of
/// what picks the cell.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SearchFields {
    pub aliases: Vec<SearchAlias>,
}

impl SearchFields {
    pub fn new(aliases: Vec<SearchAlias>) -> SearchFields {
        SearchFields { aliases }
    }

    pub fn push(&mut self, alias: SearchAlias) {
        self.aliases.push(alias);
    }
}

/// How well a query fits a menu item or a window: the strongest field's cell,
/// ordered by shape.
pub struct SearchRelevance;

impl SearchRelevance {
    /// `shape` orders inside one cell and can never leave it.
    pub const SHAPE_SPAN: i64 = 99;

    /// The cell a role and a tier land in, or nothing when the tier is too
    /// weak for the role.
    pub fn cell(role: Role, tier: Tier) -> Option<i64> {
        Some(match (role, tier) {
            (Role::Name, Tier::Exact) => 6_500,
            (Role::Name, Tier::Prefix) => 3_000,
            (Role::Owner, Tier::Exact) => 2_500,
            (Role::Name, Tier::WordStart) => 2_400,
            (Role::Owner, Tier::Prefix) => 2_000,
            (Role::Name, Tier::Substring) => 1_800,
            (Role::Owner, Tier::WordStart) => 1_500,
            (Role::Owner, Tier::Substring) => 1_100,
            (Role::Name, Tier::Subsequence) => 1_000,
            // Every entry an owner provides shares its text, so a subsequence
            // hit would flood.
            (Role::Owner, Tier::Subsequence) => return None,
        })
    }

    /// Orders candidates inside one cell: how much of the name the query
    /// covered, and how early.
    pub fn shape(m: &fuzzy::Match) -> i64 {
        if m.candidate_length == 0 {
            return 0;
        }
        let coverage = (m.query_length as f64 / m.candidate_length as f64).min(1.0);
        // Absolute, not proportional: a hit five characters in is equally deep
        // in any name.
        let positional = if m.tier == Tier::Subsequence {
            (m.spread as f64 / fuzzy::reference_spread(m.query_length) as f64).min(1.0)
        } else {
            1.0 / (1.0 + m.offset as f64 / 4.0)
        };
        (Self::SHAPE_SPAN as f64 * (0.6 * coverage + 0.4 * positional)).round() as i64
    }

    /// The base relevance from the strongest matching alias, or nothing when
    /// none matches.
    pub fn quality(query: &str, fields: &SearchFields) -> Option<i64> {
        Self::quality_folded(&Query::new(query), fields)
    }

    /// The folded form: an index folds one query once, not once per entry.
    pub fn quality_folded(query: &Query, fields: &SearchFields) -> Option<i64> {
        // Every entry is equally relevant to an empty query, so no alias
        // claims a cell.
        if query.is_empty() {
            return Some(0);
        }
        let mut best: Option<i64> = None;
        for alias in &fields.aliases {
            let Some(m) = fuzzy::match_query(query, &Candidate::new(&alias.text)) else {
                continue;
            };
            let Some(cell) = Self::cell(alias.role, m.tier) else {
                continue;
            };
            let value = cell + Self::shape(&m);
            best = Some(best.map_or(value, |current: i64| current.max(value)));
        }
        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_hit_outscores_an_owner_hit() {
        let name = SearchRelevance::cell(Role::Name, Tier::Exact).unwrap();
        let owner = SearchRelevance::cell(Role::Owner, Tier::Exact).unwrap();
        assert!(name > owner);
    }

    #[test]
    fn a_stronger_tier_outscores_a_weaker_one_in_the_same_role() {
        let exact = SearchRelevance::cell(Role::Name, Tier::Exact).unwrap();
        let prefix = SearchRelevance::cell(Role::Name, Tier::Prefix).unwrap();
        let word = SearchRelevance::cell(Role::Name, Tier::WordStart).unwrap();
        let substring = SearchRelevance::cell(Role::Name, Tier::Substring).unwrap();
        let subsequence = SearchRelevance::cell(Role::Name, Tier::Subsequence).unwrap();
        assert!(exact > prefix);
        assert!(prefix > word);
        assert!(word > substring);
        assert!(substring > subsequence);
    }

    #[test]
    fn an_owner_never_takes_a_subsequence_hit() {
        assert_eq!(SearchRelevance::cell(Role::Owner, Tier::Subsequence), None);
    }

    #[test]
    fn a_full_name_match_is_the_strongest_shape() {
        let m = fuzzy::match_query(&Query::new("abcd"), &Candidate::new("abcd")).unwrap();
        assert_eq!(SearchRelevance::shape(&m), 99);
    }

    #[test]
    fn a_later_hit_is_a_weaker_shape() {
        let early = fuzzy::match_query(&Query::new("ab"), &Candidate::new("abcd")).unwrap();
        let late = fuzzy::match_query(&Query::new("cd"), &Candidate::new("abcd")).unwrap();
        assert!(SearchRelevance::shape(&early) > SearchRelevance::shape(&late));
    }

    #[test]
    fn an_empty_query_is_equally_relevant_to_everything() {
        let fields = SearchFields::new(vec![SearchAlias::name("Safari")]);
        assert_eq!(SearchRelevance::quality("", &fields), Some(0));
    }

    #[test]
    fn a_query_that_fits_nothing_has_no_quality() {
        let fields = SearchFields::new(vec![SearchAlias::name("Safari")]);
        assert_eq!(SearchRelevance::quality("xyz", &fields), None);
    }

    #[test]
    fn the_strongest_alias_wins() {
        let fields = SearchFields::new(vec![
            SearchAlias::owner("Some Folder"),
            SearchAlias::name("Safari"),
        ]);
        let quality = SearchRelevance::quality("Safari", &fields).unwrap();
        let name_only = SearchRelevance::quality(
            "Safari",
            &SearchFields::new(vec![SearchAlias::name("Safari")]),
        )
        .unwrap();
        assert!(quality >= name_only);
    }
}
