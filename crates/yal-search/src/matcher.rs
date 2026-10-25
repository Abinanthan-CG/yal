//! Fuzzy matching.
//!
//! The matcher answers one question: given a query and a candidate string, how
//! well do they match, and where?
//!
//! The algorithm is tiered. A match is first classified into a *kind* — exact,
//! prefix, word-boundary, substring, acronym or subsequence — and the kind
//! dominates the score. Within a kind, quality bonuses are added and penalties
//! subtracted. Because the tier weights are separated by wide gaps, a stronger
//! tier always outranks a weaker one regardless of within-tier quality.
//!
//! Multi-term queries use AND semantics: every whitespace-separated term must
//! match, and the combined score is the sum of the per-term scores.

use yal_core::ranking::{Match, MatchKind};

/// Characters that separate words in a candidate string.
const SEPARATORS: [char; 15] = [
    ' ', '\t', '-', '_', '.', '/', '\\', ':', ',', '(', ')', '[', ']', '\'', '"',
];

/// Tuning constants for the within-tier bonuses.
///
/// These are gathered here so the behaviour of the matcher can be reasoned
/// about as a whole rather than scattered through the scoring code.
#[derive(Debug, Clone, Copy)]
pub struct Weights {
    /// Added for each matched character that begins a word.
    pub boundary: f64,
    /// Added for each pair of adjacent matched characters.
    pub consecutive: f64,
    /// Added for each camelCase transition that the query lands on.
    pub camel: f64,
    /// Multiplied by the number of unmatched characters inside the match span.
    pub gap: f64,
    /// Multiplied by the number of candidate characters beyond the query.
    pub length: f64,
    /// Multiplied by the offset of the first matched character.
    pub position: f64,
}

impl Default for Weights {
    fn default() -> Self {
        Self {
            boundary: 15.0,
            consecutive: 12.0,
            camel: 8.0,
            gap: 3.0,
            length: 1.5,
            position: 1.0,
        }
    }
}

/// The fuzzy matcher.
#[derive(Debug, Clone)]
pub struct Matcher {
    weights: Weights,
}

impl Default for Matcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Matcher {
    /// A matcher with the default weights.
    pub fn new() -> Self {
        Self {
            weights: Weights::default(),
        }
    }

    /// A matcher with custom weights.
    pub fn with_weights(weights: Weights) -> Self {
        Self { weights }
    }

    /// Match a query against a candidate.
    ///
    /// Returns a [`Match`] whose `kind` is [`MatchKind::None`] when the query
    /// does not match at all.
    pub fn match_str(&self, query: &str, candidate: &str) -> Match {
        let terms: Vec<&str> = query.split_whitespace().collect();
        if terms.is_empty() {
            return Match::none();
        }
        if terms.len() == 1 {
            return self.match_term(terms[0], candidate);
        }
        self.match_terms(&terms, candidate)
    }

    /// Match every term of a multi-term query.
    ///
    /// All terms must match. The combined kind is the weakest individual
    /// kind, because a single weak term is enough to make the whole result
    /// feel weak.
    fn match_terms(&self, terms: &[&str], candidate: &str) -> Match {
        let mut total = 0.0;
        let mut kind = MatchKind::Exact;
        let mut positions = Vec::new();

        for term in terms {
            let m = self.match_term(term, candidate);
            if !m.is_some() {
                return Match::none();
            }
            total += m.score;
            kind = kind.min(m.kind);
            positions.extend(m.positions);
        }

        positions.sort_unstable();
        positions.dedup();

        Match {
            kind,
            score: total,
            ranges: Match::ranges_from_positions(&positions),
            positions,
        }
    }

    /// Match a single term against a candidate.
    fn match_term(&self, term: &str, candidate: &str) -> Match {
        let needle: Vec<char> = term.chars().filter(|c| !c.is_whitespace()).collect();
        if needle.is_empty() {
            return Match::none();
        }
        let haystack: Vec<char> = candidate.chars().collect();

        // Lowercase both sides for comparison while keeping the original
        // haystack for case-sensitive bonuses.
        let lower_needle: Vec<char> = needle.iter().flat_map(|c| c.to_lowercase()).collect();
        let lower_haystack: Vec<char> = haystack.iter().flat_map(|c| c.to_lowercase()).collect();

        if lower_haystack.len() < lower_needle.len() {
            return Match::none();
        }

        // Tier 1-4: contiguous matches.
        if let Some(offset) = find_subsequence_contiguous(&lower_needle, &lower_haystack) {
            let positions: Vec<usize> = (offset..offset + lower_needle.len()).collect();
            let kind = if offset == 0 {
                if lower_haystack.len() == lower_needle.len() {
                    MatchKind::Exact
                } else {
                    MatchKind::Prefix
                }
            } else if starts_word(&haystack, offset) {
                MatchKind::WordBoundary
            } else {
                MatchKind::Substring
            };
            let score = self.score_contiguous(
                &needle,
                &haystack,
                &positions,
                kind,
                lower_needle.len(),
            );
            return Match {
                kind,
                score,
                ranges: Match::ranges_from_positions(&positions),
                positions,
            };
        }

        // Tier 5: camelCase acronym, e.g. "vscode" -> "Visual Studio Code".
        if let Some(positions) = match_acronym(&needle, &haystack) {
            let score = self.score_contiguous(
                &needle,
                &haystack,
                &positions,
                MatchKind::Acronym,
                lower_needle.len(),
            );
            return Match {
                kind: MatchKind::Acronym,
                score,
                ranges: Match::ranges_from_positions(&positions),
                positions,
            };
        }

        // Tier 6: scattered subsequence.
        if let Some(positions) = match_subsequence(&lower_needle, &lower_haystack) {
            let score = self.score_scattered(&needle, &haystack, &positions);
            return Match {
                kind: MatchKind::Subsequence,
                score,
                ranges: Match::ranges_from_positions(&positions),
                positions,
            };
        }

        Match::none()
    }

    /// Score a contiguous match.
    fn score_contiguous(
        &self,
        needle: &[char],
        haystack: &[char],
        positions: &[usize],
        kind: MatchKind,
        needle_len: usize,
    ) -> f64 {
        let mut score = kind.weight();
        score += self.quality_bonuses(needle, haystack, positions);
        score -= self.gap_penalty(positions);
        score -= self.length_penalty(haystack.len(), needle_len);
        score -= self.position_penalty(positions.first().copied());
        score
    }

    /// Score a scattered (subsequence) match.
    fn score_scattered(&self, needle: &[char], haystack: &[char], positions: &[usize]) -> f64 {
        let mut score = MatchKind::Subsequence.weight();
        score += self.quality_bonuses(needle, haystack, positions);
        score -= self.gap_penalty(positions);
        score -= self.length_penalty(haystack.len(), needle.len());
        score -= self.position_penalty(positions.first().copied());
        score
    }

    /// Bonuses that reward where the match landed.
    fn quality_bonuses(&self, needle: &[char], haystack: &[char], positions: &[usize]) -> f64 {
        let mut bonus = 0.0;

        for &pos in positions {
            if starts_word(haystack, pos) {
                bonus += self.weights.boundary;
            }
            // A camelCase transition is a stronger signal than a plain
            // separator, so it earns both the boundary and the camel bonus.
            if pos > 0 && is_camel_transition(haystack, pos) {
                bonus += self.weights.camel;
            }
        }

        // Consecutive runs.
        for window in positions.windows(2) {
            if window[1] == window[0] + 1 {
                bonus += self.weights.consecutive;
            }
        }

        let _ = needle;
        bonus
    }

    /// Penalty proportional to the unmatched characters inside the match span.
    fn gap_penalty(&self, positions: &[usize]) -> f64 {
        if positions.len() < 2 {
            return 0.0;
        }
        let span = positions[positions.len() - 1] - positions[0] + 1;
        let gaps = span.saturating_sub(positions.len());
        self.weights.gap * gaps as f64
    }

    /// Penalty for candidate characters beyond the query length.
    fn length_penalty(&self, haystack_len: usize, needle_len: usize) -> f64 {
        haystack_len
            .saturating_sub(needle_len)
            .saturating_sub(1) as f64
            * self.weights.length
    }

    /// Penalty for starting the match late in the candidate.
    fn position_penalty(&self, first: Option<usize>) -> f64 {
        first.unwrap_or(0) as f64 * self.weights.position
    }
}

/// Find a contiguous occurrence of `needle` in `haystack`.
fn find_subsequence_contiguous(needle: &[char], haystack: &[char]) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    let first = needle[0];
    for start in 0..=(haystack.len() - needle.len()) {
        if haystack[start] != first {
            continue;
        }
        if haystack[start..start + needle.len()] == *needle {
            return Some(start);
        }
    }
    None
}

/// Match `needle` against the initials of the candidate's words.
///
/// "vsc" matches "Visual Studio Code" at positions 0, 7, 13.
fn match_acronym(needle: &[char], haystack: &[char]) -> Option<Vec<usize>> {
    let mut initials = Vec::new();
    for (index, &ch) in haystack.iter().enumerate() {
        if index == 0 || starts_word(haystack, index) {
            initials.push((index, ch));
        }
    }
    if initials.len() < needle.len() {
        return None;
    }

    let initials_lower: Vec<char> = initials
        .iter()
        .flat_map(|(_, ch)| ch.to_lowercase())
        .collect();
    let needle_lower: Vec<char> = needle.iter().flat_map(|c| c.to_lowercase()).collect();

    // Greedy forward scan finds the earliest consistent assignment.
    let mut positions = Vec::with_capacity(needle.len());
    let mut cursor = 0usize;
    for &wanted in &needle_lower {
        let mut found = None;
        while cursor < initials_lower.len() {
            if initials_lower[cursor] == wanted {
                found = Some(cursor);
                cursor += 1;
                break;
            }
            cursor += 1;
        }
        positions.push(found?);
    }

    Some(positions.into_iter().map(|i| initials[i].0).collect())
}

/// Match `needle` as a scattered subsequence of `haystack`.
///
/// Greedy left-to-right matching is used, but with a lookahead that prefers a
/// match at a word boundary when one is available at the same or a nearby
/// position. This keeps "vscode" matching "Visual Studio Code" rather than
/// scattering across the whole string.
fn match_subsequence(needle: &[char], haystack: &[char]) -> Option<Vec<usize>> {
    let mut positions = Vec::with_capacity(needle.len());
    let mut cursor = 0usize;

    for &wanted in needle {
        let mut chosen = None;
        let mut scan = cursor;
        // Look a short distance ahead for a word-boundary occurrence.
        let lookahead = (scan + 16).min(haystack.len());
        while scan < lookahead {
            if haystack[scan] == wanted {
                if starts_word(haystack, scan) {
                    chosen = Some(scan);
                    break;
                }
                if chosen.is_none() {
                    chosen = Some(scan);
                }
            }
            scan += 1;
        }
        if chosen.is_none() {
            scan = cursor;
            while scan < haystack.len() {
                if haystack[scan] == wanted {
                    chosen = Some(scan);
                    break;
                }
                scan += 1;
            }
        }
        let pos = chosen?;
        positions.push(pos);
        cursor = pos + 1;
    }

    Some(positions)
}

/// Whether the character at `index` begins a new word.
pub fn starts_word(haystack: &[char], index: usize) -> bool {
    if index == 0 {
        return true;
    }
    let previous = haystack[index - 1];
    let current = haystack[index];
    if SEPARATORS.contains(&previous) {
        return true;
    }
    // camelCase: a lowercase or digit followed by an uppercase letter.
    (previous.is_lowercase() || previous.is_numeric()) && current.is_uppercase()
}

/// Whether `index` sits on a camelCase transition.
pub fn is_camel_transition(haystack: &[char], index: usize) -> bool {
    if index == 0 {
        return false;
    }
    let previous = haystack[index - 1];
    let current = haystack[index];
    (previous.is_lowercase() || previous.is_numeric()) && current.is_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(query: &str, candidate: &str) -> MatchKind {
        Matcher::new().match_str(query, candidate).kind
    }

    #[test]
    fn classifies_exact_prefix_boundary_substring() {
        assert_eq!(kind("code", "code"), MatchKind::Exact);
        assert_eq!(kind("cod", "code editor"), MatchKind::Prefix);
        assert_eq!(kind("edit", "code editor"), MatchKind::WordBoundary);
        // A substring that begins a word is still a word-boundary match, so
        // the mid-word case has to start inside a word to qualify.
        assert_eq!(kind("od", "code editor"), MatchKind::Substring);
    }

    #[test]
    fn word_initial_substring_is_a_word_boundary() {
        assert_eq!(kind("ed", "code editor"), MatchKind::WordBoundary);
    }

    #[test]
    fn matches_scattered_subsequence() {
        // "vde" is not an acronym of "visual studio code" and is not
        // contiguous, so it can only match as a scattered subsequence.
        assert_eq!(kind("vde", "visual studio code"), MatchKind::Subsequence);
        assert_eq!(kind("zzz", "visual studio code"), MatchKind::None);
    }

    #[test]
    fn acronym_outranks_scattered_for_the_same_query() {
        let m = Matcher::new();
        let acronym = m.match_str("vsc", "visual studio code");
        let scattered = m.match_str("vde", "visual studio code");
        assert_eq!(acronym.kind, MatchKind::Acronym);
        assert_eq!(scattered.kind, MatchKind::Subsequence);
        assert!(acronym.score > scattered.score);
    }

    #[test]
    fn matches_acronym_before_subsequence() {
        assert_eq!(kind("vsc", "VisualStudioCode"), MatchKind::Acronym);
    }

    #[test]
    fn case_insensitive() {
        assert_eq!(kind("CODE", "code"), MatchKind::Exact);
        assert_eq!(kind("code", "CODE"), MatchKind::Exact);
    }

    #[test]
    fn all_terms_must_match() {
        let m = Matcher::new();
        assert!(m.match_str("code editor", "code editor").is_some());
        assert!(!m.match_str("code missing", "code editor").is_some());
    }

    #[test]
    fn stronger_tier_always_wins() {
        let m = Matcher::new();
        let exact = m.match_str("code", "code");
        let scattered = m.match_str("code", "c_o_d_e");
        assert!(exact.score > scattered.score);
    }

    #[test]
    fn shorter_candidate_ranks_higher_at_equal_tier() {
        let m = Matcher::new();
        let short = m.match_str("code", "code");
        let long = m.match_str("code", "code editor advanced");
        assert!(short.score > long.score);
    }

    #[test]
    fn word_boundary_outranks_mid_word_substring() {
        let m = Matcher::new();
        let boundary = m.match_str("edit", "code editor");
        let midword = m.match_str("dit", "credit");
        assert!(boundary.score > midword.score);
    }

    #[test]
    fn positions_are_sorted_and_deduplicated() {
        let m = Matcher::new();
        let matched = m.match_str("vs code", "visual studio code");
        let mut sorted = matched.positions.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(matched.positions, sorted);
    }

    #[test]
    fn ranges_cover_positions() {
        let m = Matcher::new();
        let matched = m.match_str("code", "code");
        assert_eq!(matched.ranges, vec![0..4]);
    }

    #[test]
    fn empty_query_matches_nothing() {
        assert!(!Matcher::new().match_str("", "anything").is_some());
        assert!(!Matcher::new().match_str("   ", "anything").is_some());
    }

    #[test]
    fn unicode_does_not_split_characters() {
        let m = Matcher::new();
        let matched = m.match_str("日本", "日本語 editor");
        assert!(matched.is_some());
    }

    #[test]
    fn needle_longer_than_candidate_fails() {
        assert!(!Matcher::new().match_str("abcdef", "abc").is_some());
    }
}
