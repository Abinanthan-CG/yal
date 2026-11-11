//! The scorer the launcher's root search runs on.
//!
//! Ported from upstream's
//! `Tinycast/Features/Launcher/Model/LauncherMatch.swift`.
//!
//! A query is aligned against a name, and the best alignment wins. A word
//! start outscores any other letter, so `s` finds Safari before it finds
//! Clipboard History.

use crate::fuzzy::normalized;
use crate::romanization;

/// A text folded once, so the scorer does not fold it on every keystroke.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchText {
    /// UTF-16, because the scorer walks it by index on every keystroke.
    units: Vec<u16>,
    /// Word starts no separator marks, such as a capital after a lowercase letter.
    humps: Vec<usize>,
}

impl SearchText {
    /// Fold a text.
    ///
    /// With `transliterated`, a name in another script is read through its
    /// Latin reading, so a name typed in its own script is still an exact hit.
    pub fn new(raw: &str, transliterated: bool) -> SearchText {
        let latin = if transliterated {
            romanization::latin(raw)
        } else {
            None
        };
        let units = latin
            .clone()
            .unwrap_or_else(|| normalized(raw))
            .encode_utf16()
            .collect();
        // A name read in another script has no Latin humps, because the
        // reading is a different text.
        let humps = if latin.is_none() {
            humps_in(raw)
        } else {
            Vec::new()
        };
        SearchText { units, humps }
    }

    /// A text built from its units, for an entry that holds no name yet.
    pub fn from_units(units: Vec<u16>) -> SearchText {
        SearchText {
            units,
            humps: Vec::new(),
        }
    }

    /// The folded text.
    pub fn string(&self) -> String {
        String::from_utf16_lossy(&self.units)
    }

    /// The UTF-16 units the scorer walks.
    pub fn units(&self) -> &[u16] {
        &self.units
    }

    /// The word starts.
    pub fn humps(&self) -> &[usize] {
        &self.humps
    }

    pub fn is_empty(&self) -> bool {
        self.units.is_empty()
    }

    /// Two texts as one phrase, so `brew search` reaches `Search` under `Brew`.
    pub fn joined_with(&self, other: &SearchText) -> SearchText {
        let mut units = self.units.clone();
        units.push(SPACE);
        units.extend_from_slice(&other.units);
        let offset = self.units.len() + 1;
        let humps = self
            .humps
            .iter()
            .copied()
            .chain(other.humps.iter().map(|hump| hump + offset))
            .collect();
        SearchText { units, humps }
    }
}

/// The space between two words.
pub const SPACE: u16 = 0x20;

/// Below this many letters the pre-check costs more than the alignment it
/// would skip.
const PRECHECK_THRESHOLD: usize = 2;

/// The word starts of a name.
///
/// ASCII only: its fold keeps every index, where a non-ASCII fold can shift
/// them.
fn humps_in(raw: &str) -> Vec<usize> {
    let mut humps = Vec::new();
    let bytes = raw.as_bytes();
    let mut before_previous: u8 = 0;
    let mut previous: u8 = 0;

    for (offset, byte) in bytes.iter().copied().enumerate() {
        if byte >= 0x80 {
            return Vec::new();
        }
        let is_lowercase = byte.is_ascii_lowercase();
        let is_uppercase = byte.is_ascii_uppercase();
        let previous_is_lowercase = previous.is_ascii_lowercase();
        let previous_is_uppercase = previous.is_ascii_uppercase();
        let previous_is_digit = previous.is_ascii_digit();
        let before_is_uppercase = before_previous.is_ascii_uppercase();

        if (previous_is_lowercase && is_uppercase)
            || (previous_is_digit && is_lowercase)
            || (previous_is_digit && is_uppercase)
        {
            humps.push(offset);
        } else if before_is_uppercase && previous_is_uppercase && is_lowercase {
            // The capital before this lowercase letter ends an acronym and
            // starts a word.
            humps.push(offset - 1);
        }
        before_previous = previous;
        previous = byte;
    }
    humps
}

/// The best alignment of a query over a text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The whole text, exactly.
    Exact,
    /// `skipped` counts query separators with nothing to land on.
    Scored { score: i64, skipped: usize },
}

impl Outcome {
    /// The score, or the largest possible for an exact hit.
    pub fn value(&self) -> i64 {
        match self {
            Outcome::Exact => i64::MAX,
            Outcome::Scored { score, .. } => *score,
        }
    }
}

/// Whether a unit separates two words.
pub fn is_separator(unit: u16) -> bool {
    matches!(
        unit,
        0x09 | 0x0A | 0x20 | 0x28 | 0x29 | 0x2D | 0x2E | 0x2F | 0x5B | 0x5D
    )
}

/// The best alignment of a query over a text, where a word start outscores any
/// other letter.
pub fn match_text(query: &SearchText, target: &SearchText) -> Option<Outcome> {
    let q = &query.units;
    let t = &target.units;
    if q.is_empty() {
        return None;
    }
    if q == t {
        return Some(Outcome::Exact);
    }
    let letters = q.iter().filter(|unit| !is_separator(**unit)).count();
    if letters > t.len() {
        return None;
    }
    if letters > PRECHECK_THRESHOLD && !is_rough_subsequence(q, t) {
        return None;
    }
    align(q, t, &target.humps, letters)
}

/// A query separator may skip ahead, so this only rules out a letter the text
/// lacks.
fn is_rough_subsequence(q: &[u16], t: &[u16]) -> bool {
    let mut position = 0usize;
    for unit in q {
        if is_separator(*unit) {
            if position < t.len() && is_separator(t[position]) {
                position += 1;
            }
            continue;
        }
        let found = t[position..].iter().position(|candidate| candidate == unit);
        match found {
            Some(offset) => position += offset + 1,
            None => return false,
        }
    }
    true
}

/// One row per query character; a running maximum keeps a row linear in the
/// text.
fn align(q: &[u16], t: &[u16], humps: &[usize], letters: usize) -> Option<Outcome> {
    let width = t.len();
    let mut previous = vec![i64::MIN; width];
    let mut current = vec![i64::MIN; width];
    // The first column the last kept row matched at; the next row starts
    // after it.
    let mut anchor: i64 = -1;
    let mut row_start = 0usize;
    let mut row_end = 0usize;
    let mut matched = 0usize;
    let mut skipped = 0usize;

    for unit in q {
        let unit_is_separator = is_separator(*unit);
        let lower = (anchor + 1) as usize;
        // Matched separators count against letters still owed, so the bound
        // can pass the end. The arithmetic is signed, because the bound can
        // go below zero.
        let bound = width as i64 - (letters as i64 - 1 - matched as i64);
        let upper = bound.clamp(0, width as i64) as usize;
        let mut first: i64 = -1;
        let mut gap_best = i64::MIN;

        if lower < upper {
            for column in lower..upper {
                if anchor >= 0 && column as i64 - 2 >= anchor {
                    gap_best = gap_best.max(previous[column - 2]);
                }
                let candidate = t[column];
                let same = candidate == *unit;
                let both_separators = !same && unit_is_separator && is_separator(candidate);
                if !(same || both_separators) {
                    current[column] = i64::MIN;
                    continue;
                }
                let points = if both_separators {
                    1
                } else if anchor < 0 && column == 0 {
                    4
                } else {
                    word_points(t, column, humps)
                };
                if anchor < 0 {
                    current[column] = points;
                } else {
                    let mut best = i64::MIN;
                    let adjacent = previous[column - 1];
                    if adjacent != i64::MIN {
                        best = adjacent + points;
                    }
                    if gap_best != i64::MIN {
                        best = best.max(gap_best + points - 1);
                    }
                    current[column] = best;
                }
                if first < 0 {
                    first = column as i64;
                }
            }
        }

        if first >= 0 {
            anchor = first;
            matched += 1;
            row_start = lower;
            row_end = upper;
            std::mem::swap(&mut previous, &mut current);
        } else if unit_is_separator {
            skipped += 1;
        } else {
            return None;
        }
    }

    if anchor < 0 {
        return None;
    }
    let best = previous[row_start..row_end]
        .iter()
        .copied()
        .max()
        .unwrap_or(i64::MIN);
    if best == i64::MIN {
        None
    } else {
        Some(Outcome::Scored {
            score: best,
            skipped,
        })
    }
}

/// The points a match at a column is worth.
fn word_points(t: &[u16], column: usize, humps: &[usize]) -> i64 {
    if (column > 0 && is_separator(t[column - 1]) && !is_separator(t[column]))
        || humps.contains(&column)
    {
        3
    } else {
        2
    }
}

/// How loose a fuzzy hit may be and still show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchSensitivity {
    Low,
    #[default]
    Medium,
    High,
}

impl SearchSensitivity {
    /// Whether an outcome is worth showing at this sensitivity.
    ///
    /// `query_length` is in the units the outcome was scored against.
    pub fn accepts(&self, outcome: &Outcome, query_length: usize) -> bool {
        let Outcome::Scored { score, skipped } = outcome else {
            return true;
        };
        let length = query_length.saturating_sub(*skipped) as i64;
        match self {
            SearchSensitivity::Low => true,
            SearchSensitivity::Medium => *score as f64 >= 1.5 * (length - 2) as f64 + 4.0,
            SearchSensitivity::High => *score > 2 * length,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(query: &str, target: &str) -> Option<Outcome> {
        match_text(
            &SearchText::new(query, true),
            &SearchText::new(target, true),
        )
    }

    fn score(query: &str, target: &str) -> Option<i64> {
        match outcome(query, target) {
            Some(Outcome::Scored { score, .. }) => Some(score),
            _ => None,
        }
    }

    fn passes(query: &str, target: &str, sensitivity: SearchSensitivity) -> bool {
        let text = SearchText::new(query, true);
        match outcome(query, target) {
            Some(outcome) => sensitivity.accepts(&outcome, text.units().len()),
            None => false,
        }
    }

    /// The scores upstream pins, case for case.
    const PINNED: [(&str, &str, i64); 19] = [
        ("s", "Safari", 4),
        ("s", "Clipboard History", 2),
        ("sa", "Safari", 6),
        ("sa", "Slack", 5),
        ("vsc", "Visual Studio Code", 8),
        ("gc", "Google Chrome", 6),
        ("ss", "System Settings", 6),
        ("settings", "System Settings", 17),
        ("settings", "Tinycast Settings", 17),
        ("sett", "System Settings", 9),
        ("chrome", "Google Chrome", 13),
        ("chrome", "Chrome Remote Desktop", 14),
        ("olu", "Set Volume", 6),
        ("code", "Xcode", 8),
        ("code", "Visual Studio Code", 9),
        ("sfr", "Safari", 6),
        ("notes", "Search Notes", 11),
        ("google chrome", "Google-Chrome", 28),
        ("vs code", "Visual Studio Code", 16),
    ];

    #[test]
    fn the_pinned_scores_are_the_scores() {
        for (query, target, expected) in PINNED {
            assert_eq!(
                score(query, target),
                Some(expected),
                "'{query}' on {target}"
            );
        }
    }

    #[test]
    fn an_equal_name_is_exact() {
        assert_eq!(outcome("brew", "Brew"), Some(Outcome::Exact));
    }

    #[test]
    fn a_missing_letter_never_matches() {
        assert_eq!(outcome("xyz", "Safari"), None);
    }

    #[test]
    fn a_query_longer_than_the_name_never_matches() {
        assert_eq!(outcome("safarix", "Safari"), None);
    }

    #[test]
    fn an_empty_query_matches_nothing() {
        assert_eq!(outcome("", "Safari"), None);
    }

    #[test]
    fn a_query_separator_with_nothing_to_land_on_is_skipped() {
        assert_eq!(
            outcome("vs-code", "vscode"),
            Some(Outcome::Scored {
                score: 14,
                skipped: 1
            })
        );
    }

    #[test]
    fn matched_separators_never_push_a_row_past_the_name() {
        assert_eq!(
            outcome("a--", "a---"),
            Some(Outcome::Scored {
                score: 8,
                skipped: 0
            })
        );
    }

    #[test]
    fn a_camel_case_hump_starts_a_word() {
        assert_eq!(score("p", "TablePlus"), Some(3));
    }

    #[test]
    fn a_letter_after_a_digit_starts_a_word() {
        assert_eq!(score("p", "1Password"), Some(3));
    }

    #[test]
    fn an_acronyms_last_capital_starts_a_word() {
        assert_eq!(score("e", "HTMLEditor"), Some(3));
    }

    #[test]
    fn but_never_the_capital_before_it() {
        assert_eq!(score("l", "HTMLEditor"), Some(2));
    }

    #[test]
    fn a_hump_survives_a_join() {
        let joined =
            SearchText::new("Search", true).joined_with(&SearchText::new("OrbStack", true));
        assert_eq!(joined.humps(), [10]);
    }

    #[test]
    fn but_not_in_a_name_the_fold_can_shift() {
        assert_eq!(score("s", "CaféStack"), Some(2));
    }

    #[test]
    fn high_turns_away_letter_soup() {
        assert!(!passes("olu", "Set Volume", SearchSensitivity::High));
    }

    #[test]
    fn which_medium_lets_through() {
        assert!(passes("olu", "Set Volume", SearchSensitivity::Medium));
    }

    #[test]
    fn high_turns_away_a_mid_word_hit() {
        assert!(!passes("code", "Xcode", SearchSensitivity::High));
        assert!(passes("code", "Xcode", SearchSensitivity::Medium));
    }

    #[test]
    fn and_so_does_the_default() {
        assert!(passes(
            "pec",
            "Accessibility Inspector",
            SearchSensitivity::default()
        ));
    }

    #[test]
    fn high_still_finds_initials() {
        assert!(passes("vsc", "Visual Studio Code", SearchSensitivity::High));
    }

    #[test]
    fn high_still_finds_a_later_word() {
        assert!(passes("chrome", "Google Chrome", SearchSensitivity::High));
    }

    #[test]
    fn and_a_camel_case_one() {
        assert!(passes("stack", "OrbStack", SearchSensitivity::High));
    }

    #[test]
    fn one_after_a_digit() {
        assert!(passes("password", "1Password", SearchSensitivity::High));
    }

    #[test]
    fn and_one_after_an_acronym() {
        assert!(passes("edit", "BBEdit", SearchSensitivity::High));
    }

    #[test]
    fn one_letter_must_start_a_word() {
        assert!(!passes("s", "Clipboard History", SearchSensitivity::Medium));
    }

    #[test]
    fn and_does_when_it_does() {
        assert!(passes("s", "Clipboard History", SearchSensitivity::Low));
    }

    #[test]
    fn an_exact_hit_passes_every_level() {
        assert!(SearchSensitivity::High.accepts(&Outcome::Exact, 99));
    }

    #[test]
    fn low_accepts_any_alignment() {
        assert!(passes("sfr", "Safari", SearchSensitivity::Low));
    }

    #[test]
    fn han_reads_as_spaced_pinyin() {
        assert_eq!(SearchText::new("微信", true).string(), "wei xin");
    }

    #[test]
    fn so_its_initials_hit_word_starts() {
        assert!(passes("wx", "微信", SearchSensitivity::High));
    }

    #[test]
    fn and_the_joined_reading_hits_too() {
        assert!(passes("weixin", "微信", SearchSensitivity::High));
    }

    #[test]
    fn and_the_name_typed_in_chinese_is_exact() {
        assert_eq!(outcome("微信", "微信"), Some(Outcome::Exact));
    }

    #[test]
    fn a_longer_reading_keeps_its_syllables() {
        assert_eq!(
            SearchText::new("网易云音乐", true).string(),
            "wang yi yun yin le"
        );
    }

    #[test]
    fn mixed_script_keeps_the_latin_word() {
        assert_eq!(
            SearchText::new("Safari浏览器", true).string(),
            "safari liu lan qi"
        );
    }

    #[test]
    fn cyrillic_reads_the_way_users_type_it() {
        assert_eq!(SearchText::new("Телеграм", true).string(), "telegram");
    }

    #[test]
    fn the_kana_romanize_and_the_kanji_drop() {
        assert_eq!(SearchText::new("メモ帳", true).string(), "memo");
    }

    #[test]
    fn an_accent_folds_away() {
        assert_eq!(SearchText::new("Café Noir", true).string(), "cafe noir");
    }

    #[test]
    fn full_width_input_folds() {
        assert_eq!(SearchText::new("ｃａｆｅ", true).string(), "cafe");
    }

    #[test]
    fn latin_text_is_left_as_folded() {
        assert_eq!(
            SearchText::new("Visual Studio Code", true).string(),
            "visual studio code"
        );
    }

    #[test]
    fn invisible_format_scalars_never_reach_the_scorer() {
        assert_eq!(SearchText::new("\u{200E}Safari", true).string(), "safari");
    }

    #[test]
    fn scoring_is_deterministic() {
        let names = [
            "Safari",
            "System Settings",
            "Visual Studio Code",
            "Google Chrome",
            "Clipboard History",
            "Search Emoji & Symbols",
            "Tinycast Settings",
            "Activity Monitor",
            "Wi-Fi",
            "Date & Time",
            "Move to Next Display",
            "1Password 7",
            "Set Volume to 25%",
            "微信",
            "Телеграм",
        ];
        let alphabet: Vec<char> = "abcdefghijklmnopqrstuvwxyz -.".chars().collect();
        let mut state: u64 = 0x5EED_1234_ABCD_0001;
        for _ in 0..2_000 {
            let mut next = || {
                state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
                let mut z = state;
                z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
                z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
                z ^ (z >> 31)
            };
            let name = names[(next() % names.len() as u64) as usize];
            let length = 1 + (next() % 6) as usize;
            let mut query = String::new();
            for _ in 0..length {
                query.push(alphabet[(next() % alphabet.len() as u64) as usize]);
            }
            let first = outcome(&query, name);
            assert_eq!(first, outcome(&query, name), "'{query}' on {name}");

            // Typing more never widens a match, once it holds a letter.
            if first.is_none() && query.chars().any(|c| c.is_alphabetic()) {
                assert!(
                    outcome(&format!("{query}a"), name).is_none(),
                    "'{query}' widened on {name}"
                );
            }
        }
    }

    #[test]
    fn a_score_stays_within_its_alignment_bounds() {
        let names = ["Safari", "System Settings", "Visual Studio Code", "微信"];
        let alphabet: Vec<char> = "abcdefghijklmnopqrstuvwxyz -.".chars().collect();
        let mut state: u64 = 0x1234_5678_9ABC_DEF0;
        for _ in 0..2_000 {
            let mut next = || {
                state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
                let mut z = state;
                z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
                z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
                z ^ (z >> 31)
            };
            let name = names[(next() % names.len() as u64) as usize];
            let length = 1 + (next() % 6) as usize;
            let mut query = String::new();
            for _ in 0..length {
                query.push(alphabet[(next() % alphabet.len() as u64) as usize]);
            }
            let text = SearchText::new(&query, true);
            if let Some(Outcome::Scored { score, .. }) = outcome(&query, name) {
                let ceiling = 4 + 3 * (text.units().len() as i64 - 1);
                assert!(
                    score >= 1 && score <= ceiling,
                    "'{query}' on {name} scored {score}, outside 1..{ceiling}"
                );
            }
        }
    }
}
