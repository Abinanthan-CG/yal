//! What the user opens, and by which query.
//!
//! Ported from upstream's
//! `Tinycast/Features/Launcher/Model/LauncherRankingStore.swift`.
//!
//! The ranking is bounded, on-device frecency data: each visit adds 100 to a
//! score that halves every ten days and never drops below 1, and the terms a
//! user typed to reach an entry steer the ranking for a while afterwards.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::matcher::SearchText;

/// Each visit adds 100 to a score that halves every ten days.
pub const HALF_LIFE_SECONDS: f64 = 10.0 * 86_400.0;

/// The weight one visit adds.
pub const VISIT_WEIGHT: f64 = 100.0;

/// Past this since the last open, an entry's search terms stop steering the
/// ranking.
pub const TERM_WINDOW_SECONDS: f64 = 408.0 * 3_600.0;

/// How many terms an entry keeps.
pub const TERM_LIMIT: usize = 3;

/// `exp` overflows a `f64` just past this.
pub const EXPONENT_CEILING: f64 = 709.78;

/// Past this many characters, a pasted query is not stored as a term.
pub const QUERY_LIMIT: usize = 64;

/// The decay, so a score halves every half-life.
fn decay() -> f64 {
    2.0f64.ln() / HALF_LIFE_SECONDS
}

/// One entry's usage: a decaying score, and the terms it was last opened by.
///
/// The field names are upstream's, so the file reads the same as the one
/// upstream writes. The times are Unix seconds here, where upstream counts
/// from 2001.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LauncherVisit {
    /// When the decayed score falls back to 1, so a later anchor is always the
    /// higher score.
    pub anchor: f64,
    #[serde(rename = "openedAt")]
    pub opened_at: f64,
    /// Distinct and folded, most recent last, at most `TERM_LIMIT`.
    #[serde(rename = "searchTerms")]
    pub search_terms: Vec<String>,
}

/// What the ranking reads about one entry.
#[derive(Debug, Clone, PartialEq)]
pub struct LauncherUsage {
    /// 1 for an entry never opened, or opened too long ago to count.
    pub frecency: f64,
    /// Empty unless the entry was opened recently enough for its terms to
    /// steer a query.
    pub search_terms: Vec<String>,
}

impl LauncherUsage {
    /// An entry that was never opened.
    pub fn unused() -> LauncherUsage {
        LauncherUsage {
            frecency: 1.0,
            search_terms: Vec::new(),
        }
    }
}

/// The form a query is matched in, so a term is stored as the ranking will
/// compare it.
pub fn normalize(query: &str) -> String {
    SearchText::new(query.trim(), true).string()
}

/// The decayed score of an anchor at a moment.
pub fn frecency(anchor: f64, at: f64) -> f64 {
    let exponent = decay() * (anchor - at);
    (exponent.min(EXPONENT_CEILING)).exp().max(1.0)
}

/// The anchor that puts the score at `score + VISIT_WEIGHT` right now.
pub fn anchor(visited_with: f64, at: f64) -> f64 {
    at + (visited_with + VISIT_WEIGHT).ln() / decay()
}

/// What the ranking reads about a visit at a moment.
pub fn usage_of(visit: Option<&LauncherVisit>, at: f64) -> LauncherUsage {
    let Some(visit) = visit else {
        return LauncherUsage::unused();
    };
    let frecency = frecency(visit.anchor, at);
    let recent = frecency > 1.0 && (at - visit.opened_at) < TERM_WINDOW_SECONDS;
    LauncherUsage {
        frecency,
        search_terms: if recent {
            visit.search_terms.clone()
        } else {
            Vec::new()
        },
    }
}

/// Learns what the user opens, and by which query.
#[derive(Debug, Clone, Default)]
pub struct LauncherRankingStore {
    file: Option<PathBuf>,
    visits: BTreeMap<String, LauncherVisit>,
    revision: u64,
}

impl LauncherRankingStore {
    /// A store that keeps its table in memory only.
    pub fn in_memory() -> LauncherRankingStore {
        LauncherRankingStore::default()
    }

    /// A store backed by a JSON file, loaded from it when it is there.
    pub fn open(file: PathBuf, now: f64) -> LauncherRankingStore {
        let decoded: BTreeMap<String, LauncherVisit> = std::fs::read_to_string(&file)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        LauncherRankingStore {
            file: Some(file),
            visits: live(decoded, now),
            revision: 0,
        }
    }

    /// The table.
    pub fn visits(&self) -> &BTreeMap<String, LauncherVisit> {
        &self.visits
    }

    /// Whether the table is empty.
    pub fn is_empty(&self) -> bool {
        self.visits.is_empty()
    }

    /// Part of the index's cache key, invalidating a result after a visit or
    /// a reset.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Record a visit.
    ///
    /// `query` is nothing when the text was no search for the entry, like a
    /// category word.
    pub fn visit(&mut self, item_key: &str, query: Option<&str>, at: f64) {
        if item_key.is_empty() {
            return;
        }
        let previous = self.visits.get(item_key);
        let score = previous
            .map(|visit| frecency(visit.anchor, at))
            .unwrap_or(1.0);
        let mut terms = previous
            .map(|visit| visit.search_terms.clone())
            .unwrap_or_default();
        if let Some(term) = query.map(normalize).filter(|term| !term.is_empty()) {
            if term.chars().count() <= QUERY_LIMIT {
                terms.retain(|existing| existing != &term);
                terms.push(term);
                if terms.len() > TERM_LIMIT {
                    let overflow = terms.len() - TERM_LIMIT;
                    terms.drain(0..overflow);
                }
            }
        }
        self.visits.insert(
            item_key.to_string(),
            LauncherVisit {
                anchor: anchor(score, at),
                opened_at: at,
                search_terms: terms,
            },
        );
        self.did_mutate();
    }

    /// Everything one ranking pass reads, with the clock read once for the
    /// whole pass.
    pub fn usage(&self, item_key: &str, at: f64) -> LauncherUsage {
        usage_of(self.visits.get(item_key), at)
    }

    /// Whether an entry has a ranking.
    pub fn has_ranking(&self, item_key: &str) -> bool {
        self.visits.contains_key(item_key)
    }

    /// Forget one entry.
    pub fn reset(&mut self, item_key: &str) {
        if self.visits.remove(item_key).is_some() {
            self.did_mutate();
        }
    }

    /// Forget everything.
    pub fn reset_all(&mut self) {
        if self.visits.is_empty() {
            return;
        }
        self.visits.clear();
        self.did_mutate();
    }

    /// Replace the table wholesale from a backup, dropping what the
    /// initialiser would.
    pub fn replace(&mut self, imported: BTreeMap<String, LauncherVisit>, at: f64) {
        self.visits = live(imported, at);
        self.did_mutate();
    }

    /// Write the table to its file.
    pub fn save(&self) -> Result<(), String> {
        let Some(file) = &self.file else {
            return Ok(());
        };
        write_visits(file, &self.visits)
    }

    fn did_mutate(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }
}

/// The entries that still count, at a moment.
///
/// A passed anchor scores 1 with its terms gated off, which is exactly an
/// entry never opened.
fn live(table: BTreeMap<String, LauncherVisit>, at: f64) -> BTreeMap<String, LauncherVisit> {
    table
        .into_iter()
        .filter(|(key, visit)| !key.is_empty() && visit.anchor > at)
        .collect()
}

/// Write a table to a file.
pub fn write_visits(file: &Path, visits: &BTreeMap<String, LauncherVisit>) -> Result<(), String> {
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("{}: {error}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(visits)
        .map_err(|error| format!("the ranking could not be encoded: {error}"))?;
    std::fs::write(file, text).map_err(|error| format!("{}: {error}", file.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: f64 = 86_400.0;

    /// The upstream test's own clock.
    fn clock() -> f64 {
        2_000_000_000.0
    }

    fn near(value: f64, expected: f64) -> bool {
        (value - expected).abs() < 0.001
    }

    #[test]
    fn a_term_trims_surrounding_whitespace() {
        assert_eq!(normalize(" wha \n"), "wha");
    }

    #[test]
    fn a_term_folds_case() {
        assert_eq!(normalize("WhA"), "wha");
    }

    #[test]
    fn a_term_folds_diacritics() {
        assert_eq!(normalize("Café"), "cafe");
    }

    #[test]
    fn a_term_folds_full_width_input() {
        assert_eq!(normalize("ｃａｆｅ"), "cafe");
    }

    #[test]
    fn a_term_is_stored_as_the_ranking_reads_it() {
        assert_eq!(normalize("微信"), "wei xin");
    }

    #[test]
    fn a_term_folds_without_a_locale() {
        assert_eq!(normalize("I"), "i");
    }

    #[test]
    fn an_unvisited_entry_reads_as_never_used() {
        let store = LauncherRankingStore::in_memory();
        assert_eq!(
            store.usage("com.apple.Safari", clock()),
            LauncherUsage::unused()
        );
    }

    #[test]
    fn an_empty_store_says_so() {
        assert!(LauncherRankingStore::in_memory().is_empty());
    }

    #[test]
    fn one_visit_scores_101() {
        let mut store = LauncherRankingStore::in_memory();
        store.visit("com.apple.Safari", Some("saf"), clock());
        assert!(near(
            store.usage("com.apple.Safari", clock()).frecency,
            101.0
        ));
    }

    #[test]
    fn and_is_remembered() {
        let mut store = LauncherRankingStore::in_memory();
        store.visit("com.apple.Safari", Some("saf"), clock());
        assert!(store.has_ranking("com.apple.Safari"));
        assert!(!store.is_empty());
    }

    #[test]
    fn ten_days_halve_it() {
        let mut store = LauncherRankingStore::in_memory();
        store.visit("com.apple.Safari", Some("saf"), clock());
        let later = clock() + 10.0 * DAY;
        assert!(near(store.usage("com.apple.Safari", later).frecency, 50.5));
    }

    #[test]
    fn a_visit_adds_100_to_what_is_left() {
        let mut store = LauncherRankingStore::in_memory();
        store.visit("com.apple.Safari", Some("saf"), clock());
        let later = clock() + 10.0 * DAY;
        store.visit("com.apple.Safari", None, later);
        assert!(near(store.usage("com.apple.Safari", later).frecency, 150.5));
    }

    #[test]
    fn a_long_silence_floors_it_at_1() {
        let mut store = LauncherRankingStore::in_memory();
        store.visit("com.apple.Safari", Some("saf"), clock());
        let later = clock() + 200.0 * DAY;
        assert_eq!(store.usage("com.apple.Safari", later).frecency, 1.0);
    }

    #[test]
    fn a_later_anchor_stays_ahead_until_both_floor() {
        let at = clock();
        let early = LauncherVisit {
            anchor: at + 5.0 * DAY,
            opened_at: at,
            search_terms: Vec::new(),
        };
        let late = LauncherVisit {
            anchor: at + 6.0 * DAY,
            opened_at: at,
            search_terms: Vec::new(),
        };
        for offset in [0.0, 3.0, 30.0] {
            let moment = at + offset * DAY;
            let a = usage_of(Some(&early), moment).frecency;
            let b = usage_of(Some(&late), moment).frecency;
            if offset < 5.0 {
                assert!(b > a, "at {offset} days the later anchor lost");
            } else {
                assert_eq!(a, b, "at {offset} days the scores should have floored");
            }
        }
    }

    #[test]
    fn a_visit_keeps_its_folded_term() {
        let mut store = LauncherRankingStore::in_memory();
        store.visit("com.tinyspeck.slackmacgap", Some(" Sa "), clock());
        assert_eq!(
            store
                .usage("com.tinyspeck.slackmacgap", clock())
                .search_terms,
            vec!["sa".to_string()]
        );
    }

    #[test]
    fn only_the_newest_three_distinct_terms_stay() {
        let mut store = LauncherRankingStore::in_memory();
        store.visit("com.tinyspeck.slackmacgap", Some(" Sa "), clock());
        for term in ["sl", "slack", "sa", "s"] {
            store.visit("com.tinyspeck.slackmacgap", Some(term), clock());
        }
        assert_eq!(
            store
                .usage("com.tinyspeck.slackmacgap", clock())
                .search_terms,
            vec!["slack".to_string(), "sa".to_string(), "s".to_string()]
        );
    }

    #[test]
    fn an_empty_or_pasted_query_leaves_the_terms_alone() {
        let mut store = LauncherRankingStore::in_memory();
        store.visit("com.tinyspeck.slackmacgap", Some(" Sa "), clock());
        for term in ["sl", "slack", "sa", "s"] {
            store.visit("com.tinyspeck.slackmacgap", Some(term), clock());
        }
        store.visit("com.tinyspeck.slackmacgap", Some(""), clock());
        store.visit("com.tinyspeck.slackmacgap", Some(&"x".repeat(65)), clock());
        assert_eq!(
            store
                .usage("com.tinyspeck.slackmacgap", clock())
                .search_terms,
            vec!["slack".to_string(), "sa".to_string(), "s".to_string()]
        );
    }

    #[test]
    fn terms_stop_counting_after_seventeen_days() {
        let mut store = LauncherRankingStore::in_memory();
        store.visit("com.tinyspeck.slackmacgap", Some("sl"), clock());
        let later = clock() + 18.0 * DAY;
        let usage = store.usage("com.tinyspeck.slackmacgap", later);
        assert!(usage.search_terms.is_empty());
        assert!(usage.frecency > 1.0);
    }

    #[test]
    fn a_fresh_open_brings_the_terms_back() {
        // Upstream's own sequence: the three terms the entry learned, a
        // silence long enough for them to stop counting, and then a fresh open.
        let mut store = LauncherRankingStore::in_memory();
        store.visit("com.tinyspeck.slackmacgap", Some(" Sa "), clock());
        for term in ["sl", "slack", "sa", "s"] {
            store.visit("com.tinyspeck.slackmacgap", Some(term), clock());
        }
        store.visit("com.tinyspeck.slackmacgap", Some(""), clock());
        store.visit("com.tinyspeck.slackmacgap", Some(&"x".repeat(65)), clock());
        let later = clock() + 18.0 * DAY;
        assert!(store
            .usage("com.tinyspeck.slackmacgap", later)
            .search_terms
            .is_empty());

        store.visit("com.tinyspeck.slackmacgap", Some("sl"), later);
        assert_eq!(
            store.usage("com.tinyspeck.slackmacgap", later).search_terms,
            vec!["sa".to_string(), "s".to_string(), "sl".to_string()]
        );
    }

    #[test]
    fn a_visit_survives_a_relaunch() {
        let file = std::env::temp_dir().join("yal-ranking-test.json");
        let _ = std::fs::remove_file(&file);
        let mut store = LauncherRankingStore::open(file.clone(), clock());
        store.visit("com.tinyspeck.slackmacgap", Some("sl"), clock());
        store.save().expect("the ranking should be written");

        let reloaded = LauncherRankingStore::open(file.clone(), clock());
        assert_eq!(
            reloaded.visits().get("com.tinyspeck.slackmacgap"),
            store.visits().get("com.tinyspeck.slackmacgap")
        );
        let _ = std::fs::remove_file(&file);
    }

    #[test]
    fn an_entry_whose_score_has_floored_is_pruned_on_load() {
        let file = std::env::temp_dir().join("yal-ranking-prune-test.json");
        let _ = std::fs::remove_file(&file);
        let mut store = LauncherRankingStore::open(file.clone(), clock());
        store.visit("com.apple.Safari", Some("saf"), clock());
        store.save().expect("the ranking should be written");

        let later = clock() + 200.0 * DAY;
        let reloaded = LauncherRankingStore::open(file.clone(), later);
        assert!(!reloaded.has_ranking("com.apple.Safari"));
        let _ = std::fs::remove_file(&file);
    }

    #[test]
    fn the_saved_file_reads_the_way_upstream_writes_it() {
        let file = std::env::temp_dir().join("yal-ranking-shape-test.json");
        let _ = std::fs::remove_file(&file);
        let mut store = LauncherRankingStore::open(file.clone(), clock());
        store.visit("com.apple.Safari", Some("saf"), clock());
        store.save().expect("the ranking should be written");

        let text = std::fs::read_to_string(&file).expect("the ranking should be readable");
        assert!(text.contains("\"anchor\""), "{text}");
        assert!(text.contains("\"openedAt\""), "{text}");
        assert!(text.contains("\"searchTerms\""), "{text}");
        let _ = std::fs::remove_file(&file);
    }

    #[test]
    fn a_reset_forgets_one_entry() {
        let mut store = LauncherRankingStore::in_memory();
        store.visit("com.tinyspeck.slackmacgap", Some("sl"), clock());
        let revision = store.revision();
        store.reset("com.tinyspeck.slackmacgap");
        assert!(!store.has_ranking("com.tinyspeck.slackmacgap"));
        assert_ne!(store.revision(), revision);
    }

    #[test]
    fn resetting_nothing_leaves_the_revision() {
        let mut store = LauncherRankingStore::in_memory();
        store.visit("a", None, clock());
        let revision = store.revision();
        store.reset("missing");
        assert_eq!(store.revision(), revision);
    }

    #[test]
    fn reset_all_empties_the_table() {
        let mut store = LauncherRankingStore::in_memory();
        store.visit("a", None, clock());
        store.visit("b", None, clock());
        store.reset_all();
        assert!(store.is_empty());
    }

    #[test]
    fn an_import_keeps_only_live_keyed_entries() {
        let mut store = LauncherRankingStore::in_memory();
        let at = clock();
        let mut imported = BTreeMap::new();
        imported.insert(
            "kept".to_string(),
            LauncherVisit {
                anchor: at + DAY,
                opened_at: at,
                search_terms: vec!["k".to_string()],
            },
        );
        imported.insert(
            "stale".to_string(),
            LauncherVisit {
                anchor: at - DAY,
                opened_at: at - 90.0 * DAY,
                search_terms: Vec::new(),
            },
        );
        imported.insert(
            String::new(),
            LauncherVisit {
                anchor: at + DAY,
                opened_at: at,
                search_terms: Vec::new(),
            },
        );
        store.replace(imported, at);
        assert_eq!(
            store.visits().keys().cloned().collect::<Vec<_>>(),
            vec!["kept".to_string()]
        );
    }
}
