//! The parsed catalog and the search over it.
//!
//! Mirrors upstream `Tinycast/Features/Emoji/Service/EmojiIndex.swift`.
//!
//! The search folds every entry once when the catalog loads, so a keystroke
//! folds only the query.

use std::collections::HashMap;

use tc_search::fuzzy::{self, Candidate, Query, Tier};

use crate::catalog::{EmojiCatalog, EmojiCategory, EmojiEntry};
use crate::stores::FrequentEmojiStore;

/// An entry's text folded once at load.
#[derive(Debug)]
struct FoldedEntry {
    name: Candidate,
    keywords: Candidate,
    keyword_list: Vec<Candidate>,
}

impl FoldedEntry {
    fn new(entry: &EmojiEntry) -> Self {
        Self {
            name: Candidate::new(&entry.name),
            keywords: Candidate::new(&entry.keywords),
            keyword_list: entry
                .keyword_list()
                .into_iter()
                .map(Candidate::new)
                .collect(),
        }
    }
}

/// The parsed catalog: the sections in display order, and the search.
#[derive(Debug, Default)]
pub struct EmojiIndex {
    entries: Vec<EmojiEntry>,
    category_sections: Vec<(EmojiCategory, Vec<EmojiEntry>)>,
    by_glyph: HashMap<String, EmojiEntry>,
    folded_entries: Vec<FoldedEntry>,
}

impl EmojiIndex {
    /// Loads the catalog that ships with the crate.
    pub fn load() -> EmojiIndex {
        Self::load_raw(crate::data::RAW)
    }

    /// Loads a catalog from a table in upstream's format.
    pub fn load_raw(raw: &str) -> EmojiIndex {
        let entries = EmojiCatalog::parse(raw);
        let folded_entries = entries.iter().map(FoldedEntry::new).collect();
        let mut grouped: Vec<(EmojiCategory, Vec<EmojiEntry>)> = Vec::new();
        for entry in &entries {
            match grouped
                .iter_mut()
                .find(|(category, _)| *category == entry.category)
            {
                Some((_, items)) => items.push(entry.clone()),
                None => grouped.push((entry.category, vec![entry.clone()])),
            }
        }
        // The sections are shown in the catalog's order, not in the order the
        // entries happen to arrive in.
        let mut category_sections: Vec<(EmojiCategory, Vec<EmojiEntry>)> = Vec::new();
        for category in EmojiCategory::ALL {
            if let Some(section) = grouped.iter().find(|(name, _)| *name == category) {
                category_sections.push(section.clone());
            }
        }
        let mut by_glyph: HashMap<String, EmojiEntry> = HashMap::with_capacity(entries.len());
        for entry in &entries {
            by_glyph
                .entry(entry.glyph.clone())
                .or_insert_with(|| entry.clone());
        }
        EmojiIndex {
            entries,
            category_sections,
            by_glyph,
            folded_entries,
        }
    }

    /// Every entry, in the catalog's order.
    pub fn entries(&self) -> &[EmojiEntry] {
        &self.entries
    }

    /// The sections, in display order.
    pub fn category_sections(&self) -> &[(EmojiCategory, Vec<EmojiEntry>)] {
        &self.category_sections
    }

    /// The entry a glyph names, if the catalog has it.
    pub fn entry(&self, glyph: &str) -> Option<&EmojiEntry> {
        self.by_glyph.get(glyph)
    }

    /// The ranked matches over the names and the keywords.
    ///
    /// An empty query matches nothing.
    pub fn search(
        &self,
        query: &str,
        frequent: &FrequentEmojiStore,
        limit: usize,
    ) -> Vec<&EmojiEntry> {
        let trimmed = fuzzy::normalized(query);
        let trimmed = trimmed.trim();
        let trimmed =
            if trimmed.chars().count() > 2 && trimmed.starts_with(':') && trimmed.ends_with(':') {
                let inner: String = trimmed
                    .chars()
                    .skip(1)
                    .take(trimmed.chars().count() - 2)
                    .collect();
                inner
            } else {
                trimmed.to_string()
            };
        let words: Vec<String> = trimmed.split_whitespace().map(str::to_string).collect();
        let joined = words.join(" ");
        if joined.is_empty() || limit == 0 {
            return Vec::new();
        }
        let query = Query::new(&joined);
        let terms: Vec<String> = if words.len() > 1 { words } else { Vec::new() };

        let frequent_glyphs = frequent.top(FREQUENCY_LIMIT);
        let frecency: HashMap<&str, i64> = frequent_glyphs
            .iter()
            .enumerate()
            .map(|(index, glyph)| (glyph.as_str(), (FREQUENCY_LIMIT - index) as i64))
            .collect();

        let mut scored: Vec<(i64, usize, &EmojiEntry)> = Vec::new();
        for (order, entry) in self.entries.iter().enumerate() {
            let Some(text_score) = text_score(&query, &terms, &self.folded_entries[order]) else {
                continue;
            };
            let score = text_score + frecency.get(entry.glyph.as_str()).copied().unwrap_or(0);
            scored.push((score, order, entry));
        }
        scored.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
        scored
            .into_iter()
            .take(limit)
            .map(|(_, _, entry)| entry)
            .collect()
    }
}

/// Just under half a tier, so an equal-quality name match always wins.
const KEYWORD_PENALTY: i64 = 500;
/// How many of the most-used characters weigh on the ranking.
const FREQUENCY_LIMIT: usize = 100;
/// A complete leading name word: above an exact keyword, below the exact name.
const LEADING_WORD_SCORE: i64 = 95_000;
/// Scattered query words rank below every literal phrase, name-only words first.
const NAME_WORDS_SCORE: i64 = 60_000;
/// The same, for a query that reached into the keywords as well.
const MIXED_WORDS_SCORE: i64 = 50_000;

/// The score of one entry for one query.
fn text_score(query: &Query, terms: &[String], folded: &FoldedEntry) -> Option<i64> {
    let mut name_only = true;
    for term in terms {
        if contains_word_start(term, folded.name.text()) {
            continue;
        }
        if term.contains(',') || !contains_word_start(term, folded.keywords.text()) {
            return None;
        }
        name_only = false;
    }

    let name_match = fuzzy::match_query(query, &folded.name);
    if let Some(matched) = &name_match {
        if matched.tier == Tier::Exact {
            return Some(fuzzy::raw_score(matched));
        }
    }
    let mut best = name_match.as_ref().map(fuzzy::raw_score);
    if let Some(matched) = &name_match {
        if matched.tier == Tier::Prefix {
            let next = folded.name.text().chars().nth(matched.query_length);
            if let Some(next) = next {
                if !next.is_alphabetic() && !next.is_numeric() {
                    best = Some(LEADING_WORD_SCORE - matched.candidate_length as i64);
                }
            }
        }
    }
    if !terms.is_empty() {
        let ordered = match &name_match {
            Some(matched) if matched.tier == Tier::Subsequence => fuzzy::raw_score(matched),
            _ => 0,
        };
        let base = if name_only {
            NAME_WORDS_SCORE
        } else {
            MIXED_WORDS_SCORE
        };
        best = Some(best.unwrap_or(i64::MIN).max(base + ordered));
    }
    if folded.keyword_list.is_empty() || fuzzy::score(query, &folded.keywords).is_none() {
        return best;
    }
    for keyword in &folded.keyword_list {
        let Some(matched) = fuzzy::match_query(query, keyword) else {
            continue;
        };
        let score = fuzzy::raw_score(&matched);
        best = Some(
            best.unwrap_or(i64::MIN)
                .max(score.min(LEADING_WORD_SCORE) - KEYWORD_PENALTY),
        );
        if matched.tier == Tier::Exact {
            break;
        }
    }
    best
}

/// Whether the term begins a word inside the candidate.
fn contains_word_start(term: &str, candidate: &str) -> bool {
    let mut start = 0usize;
    while let Some(found) = candidate[start..].find(term) {
        let at = start + found;
        if at == 0 {
            return true;
        }
        let previous = candidate[..at].chars().next_back().unwrap_or(' ');
        if !previous.is_alphabetic() && !previous.is_numeric() {
            return true;
        }
        start = at + 1;
        if start > candidate.len() {
            break;
        }
    }
    false
}
