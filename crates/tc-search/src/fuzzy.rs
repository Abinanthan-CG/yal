//! The fuzzy match that every search shares.
//!
//! Ported from upstream's `Tinycast/Features/Launcher/Model/SearchRelevance.swift`.
//!
//! Upstream folds a string once and then compares it: an exact hit beats a
//! prefix, a prefix beats a word start, a word start beats a substring, and a
//! substring beats a subsequence. The subsequence walk is what lets `npx`
//! find `npm`.

/// How a query hits a candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier {
    /// The whole candidate, exactly.
    Exact,
    /// The start of the candidate.
    Prefix,
    /// The start of a word inside the candidate.
    WordStart,
    /// Somewhere inside the candidate.
    Substring,
    /// The query's letters, in order but not together.
    Subsequence,
}

/// One match, with the geometry the ranking reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Match {
    pub tier: Tier,
    /// Characters into the candidate the hit begins; 0 for both anchored tiers.
    pub offset: usize,
    pub query_length: usize,
    pub candidate_length: usize,
    /// The subsequence walk's raw bonus total, and 0 for every literal tier.
    pub spread: i64,
}

/// A query folded once, so ranking does not re-fold it for every candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    text: String,
    characters: Vec<char>,
}

impl Query {
    pub fn new(raw: &str) -> Query {
        let text = normalized(raw);
        let characters = text.chars().collect();
        Query { text, characters }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}

/// A candidate folded once, for an index that matches the same text against
/// every query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    text: String,
    length: usize,
}

impl Candidate {
    pub fn new(raw: &str) -> Candidate {
        let text = normalized(raw);
        let length = text.chars().count();
        Candidate { text, length }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn length(&self) -> usize {
        self.length
    }
}

/// The tier a query hits a candidate at.
pub fn match_query(query: &Query, candidate: &Candidate) -> Option<Match> {
    let q = query.text.as_str();
    let c = candidate.text.as_str();
    let length = candidate.length;

    if query.text.is_empty() {
        return Some(Match {
            tier: Tier::Exact,
            offset: 0,
            query_length: 0,
            candidate_length: length,
            spread: 0,
        });
    }
    if c == q {
        return Some(Match {
            tier: Tier::Exact,
            offset: 0,
            query_length: query.characters.len(),
            candidate_length: length,
            spread: 0,
        });
    }
    if let Some(rest) = c.strip_prefix(q) {
        let _ = rest;
        return Some(Match {
            tier: Tier::Prefix,
            offset: 0,
            query_length: query.characters.len(),
            candidate_length: length,
            spread: 0,
        });
    }
    if let Some(offset) = c.find(q) {
        let characters: Vec<char> = c.chars().collect();
        let index = c[..offset].chars().count();
        let tier = if is_word_start(&characters, index) {
            Tier::WordStart
        } else {
            Tier::Substring
        };
        return Some(Match {
            tier,
            offset: index,
            query_length: query.characters.len(),
            candidate_length: length,
            spread: 0,
        });
    }
    let spread = subsequence_score(&query.characters, c)?;
    Some(Match {
        tier: Tier::Subsequence,
        offset: 0,
        query_length: query.characters.len(),
        candidate_length: length,
        spread,
    })
}

/// The score a single field's match is worth, without any role to weigh.
pub fn raw_score(m: &Match) -> i64 {
    match m.tier {
        Tier::Exact => 100_000,
        Tier::Prefix => 90_000 - m.candidate_length as i64,
        Tier::WordStart => 80_000 - m.candidate_length as i64,
        Tier::Substring => 70_000 - m.candidate_length as i64,
        Tier::Subsequence => m.spread,
    }
}

/// The score of a match, or nothing when the query does not fit.
pub fn score(query: &Query, candidate: &Candidate) -> Option<i64> {
    match_query(query, candidate).map(|m| raw_score(&m))
}

/// The one fold every search shares: matching, the learned-ranking keys and
/// the deduplication all call it.
pub fn normalized(value: &str) -> String {
    use unicode_normalization::UnicodeNormalization;

    if !value.chars().any(|c| c >= '\u{00AD}') {
        return value.to_lowercase();
    }
    // Composed first, so a Korean or Vietnamese IME's decomposed input folds
    // like typed text.
    let composed: String = value.nfc().collect();
    let scalars: String = composed.chars().filter(|c| !is_format(*c)).collect();
    fold_width(&fold_diacritics(&scalars.to_lowercase()))
}

/// What a contiguous run from index 0 would score, so `spread` normalizes to
/// a fraction.
pub fn reference_spread(query_length: usize) -> i64 {
    if query_length == 0 {
        return 1;
    }
    let n = query_length as i64;
    13 + (n - 1) + 3 * n * (n - 1) / 2
}

/// Whether the character before the index starts a word.
fn is_word_start(text: &[char], index: usize) -> bool {
    if index == 0 {
        return true;
    }
    let before = text[index - 1];
    !before.is_alphabetic() && !before.is_numeric()
}

/// Walks the candidate in place, carrying the previous character.
fn subsequence_score(query: &[char], candidate: &str) -> Option<i64> {
    let mut query_index = 0usize;
    let mut score: i64 = 0;
    let mut run: i64 = 0;
    let mut previous_index: i64 = -2;
    let mut previous: Option<char> = None;

    for (index, ch) in candidate.chars().enumerate() {
        let candidate_index = index as i64;
        if query_index < query.len() && ch == query[query_index] {
            let mut bonus: i64 = 1;
            if candidate_index == previous_index + 1 {
                run += 1;
                bonus += run * 3;
            } else {
                run = 0;
            }
            if candidate_index == 0 {
                bonus += 12;
            } else if let Some(previous) = previous {
                if !previous.is_alphabetic() && !previous.is_numeric() {
                    bonus += 8;
                }
            }
            score += bonus;
            previous_index = candidate_index;
            query_index += 1;
            if query_index == query.len() {
                break;
            }
        }
        previous = Some(ch);
    }

    if query_index == query.len() {
        Some(score)
    } else {
        None
    }
}

/// Whether a character is in a format category, which is dropped by the fold.
fn is_format(c: char) -> bool {
    matches!(c,
        '\u{00AD}'
        | '\u{061C}'
        | '\u{200B}'..='\u{200F}'
        | '\u{202A}'..='\u{202E}'
        | '\u{2060}'..='\u{2064}'
        | '\u{2066}'..='\u{206F}'
        | '\u{FEFF}'
        | '\u{FFF9}'..='\u{FFFB}')
}

/// Drops the combining marks, so a diacritic does not change the match.
fn fold_diacritics(value: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    value.nfd().filter(|c| !is_mark(*c)).nfd().collect()
}

/// Whether a character is a combining mark.
fn is_mark(c: char) -> bool {
    use unicode_normalization::char::canonical_combining_class;
    canonical_combining_class(c) != 0
}

/// Folds the full-width forms onto their ASCII equivalents.
fn fold_width(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '\u{3000}' => out.push(' '),
            '\u{FF01}'..='\u{FF5E}' => {
                let folded = (c as u32 - 0xFEE0) as u8 as char;
                out.push(folded);
            }
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tier_of(query: &str, candidate: &str) -> Option<Tier> {
        match_query(&Query::new(query), &Candidate::new(candidate)).map(|m| m.tier)
    }

    #[test]
    fn an_exact_hit_is_the_strongest_tier() {
        assert_eq!(tier_of("abc", "abc"), Some(Tier::Exact));
    }

    #[test]
    fn a_prefix_beats_a_word_start() {
        assert_eq!(tier_of("ab", "abcd"), Some(Tier::Prefix));
        assert_eq!(tier_of("cd", "ab cd"), Some(Tier::WordStart));
    }

    #[test]
    fn a_word_start_beats_a_substring() {
        assert_eq!(tier_of("cd", "abcd"), Some(Tier::Substring));
    }

    #[test]
    fn letters_in_order_but_not_together_still_match() {
        assert_eq!(tier_of("ac", "abc"), Some(Tier::Subsequence));
    }

    #[test]
    fn a_query_that_does_not_fit_does_not_match() {
        assert_eq!(tier_of("xyz", "abc"), None);
    }

    #[test]
    fn an_empty_query_matches_everything_exactly() {
        let m =
            match_query(&Query::new(""), &Candidate::new("abc")).expect("an empty query matches");
        assert_eq!(m.tier, Tier::Exact);
        assert_eq!(m.query_length, 0);
    }

    #[test]
    fn the_tiers_score_in_order() {
        let exact = raw_score(&match_query(&Query::new("abc"), &Candidate::new("abc")).unwrap());
        let prefix = raw_score(&match_query(&Query::new("abc"), &Candidate::new("abcd")).unwrap());
        let word = raw_score(&match_query(&Query::new("cd"), &Candidate::new("ab cd")).unwrap());
        let substring =
            raw_score(&match_query(&Query::new("bc"), &Candidate::new("abcd")).unwrap());
        assert!(exact > prefix);
        assert!(prefix > word);
        assert!(word > substring);
    }

    #[test]
    fn a_shorter_candidate_scores_higher_at_the_same_tier() {
        let short = raw_score(&match_query(&Query::new("ab"), &Candidate::new("abcd")).unwrap());
        let long = raw_score(&match_query(&Query::new("ab"), &Candidate::new("abcdefgh")).unwrap());
        assert!(short > long);
    }

    #[test]
    fn the_subsequence_walk_scores_a_contiguous_run_higher() {
        let m = match_query(&Query::new("ac"), &Candidate::new("abc")).unwrap();
        // 'a' at the start is worth 13, 'c' after 'b' is worth 1.
        assert_eq!(m.spread, 14);
    }

    #[test]
    fn the_reference_spread_is_what_a_contiguous_run_scores() {
        assert_eq!(reference_spread(0), 1);
        assert_eq!(reference_spread(1), 13);
        assert_eq!(reference_spread(2), 17);
        assert_eq!(reference_spread(3), 24);
    }

    #[test]
    fn the_fold_lowercases() {
        assert_eq!(normalized("ABC"), "abc");
    }

    #[test]
    fn the_fold_drops_diacritics() {
        assert_eq!(normalized("Café"), "cafe");
        assert_eq!(normalized("Ünïcödé"), "unicode");
    }

    #[test]
    fn the_fold_makes_full_width_ascii_ascii() {
        assert_eq!(normalized("ＡＢＣ"), "abc");
        assert_eq!(normalized("a　b"), "a b");
    }

    #[test]
    fn the_fold_drops_format_characters() {
        assert_eq!(normalized("a\u{200B}b"), "ab");
    }

    #[test]
    fn a_folded_query_matches_a_folded_candidate() {
        assert_eq!(tier_of("cafe", "Café"), Some(Tier::Exact));
    }

    #[test]
    fn the_offset_is_where_the_hit_begins() {
        let m = match_query(&Query::new("bc"), &Candidate::new("abcd")).unwrap();
        assert_eq!(m.offset, 1);
    }

    #[test]
    fn the_candidate_length_is_counted_in_characters() {
        let m = match_query(&Query::new("bc"), &Candidate::new("abcd")).unwrap();
        assert_eq!(m.candidate_length, 4);
    }
}
