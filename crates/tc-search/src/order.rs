//! The launcher's ordering: which entry comes first.
//!
//! Ported from upstream's
//! `Tinycast/Features/Launcher/Model/LauncherOrder.swift`.
//!
//! Two orderings live here. The first ranks the entries that match a query.
//! The second is the empty list: what the launcher shows before anything is
//! typed. Both are pure, so the shipped code can be ranked by a harness
//! without a window.

use std::cmp::Ordering;
use std::collections::BTreeSet;

use crate::entry_naming::SearchProfile;
use crate::fuzzy::{self, Tier};
use crate::matcher::{Outcome, SearchSensitivity, SearchText};
use crate::ranking::LauncherUsage;

/// A query, folded once, in both the forms the fields compare against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    /// Titles, subtitles, keywords and search terms compare against the Latin
    /// reading.
    latin: SearchText,
    /// Alternate titles and aliases compare against the text as typed.
    typed: SearchText,
    /// The folded text, which is what a learned term is stored as.
    term: String,
}

impl Query {
    pub fn new(raw: &str) -> Query {
        let trimmed = raw.trim();
        let typed = SearchText::new(trimmed, false);
        Query {
            latin: SearchText::new(trimmed, true),
            term: typed.string(),
            typed,
        }
    }

    /// The query as the Latin reading.
    pub fn latin(&self) -> &SearchText {
        &self.latin
    }

    /// The query as it was typed.
    pub fn typed(&self) -> &SearchText {
        &self.typed
    }

    /// The folded query, which is what a learned term is compared against.
    pub fn term(&self) -> &str {
        &self.term
    }

    pub fn is_empty(&self) -> bool {
        self.typed.is_empty()
    }
}

/// A query that names a whole category rather than an entry.
#[derive(Debug, Clone)]
pub struct CategoryQuery {
    /// The canonical name of the category.
    pub name: String,
    query: fuzzy::Query,
}

impl CategoryQuery {
    /// A prefix or a word start of at least three letters names a category.
    const PREFIX_LENGTH: usize = 3;

    pub fn new(raw: &str) -> CategoryQuery {
        let name = fuzzy::normalized(&raw.split_whitespace().collect::<Vec<_>>().join(" "));
        CategoryQuery {
            query: fuzzy::Query::new(&name),
            name,
        }
    }

    /// Whether the query names the candidate's category.
    pub fn matches(&self, candidate: &fuzzy::Candidate) -> bool {
        if self.query.is_empty() {
            return false;
        }
        let Some(m) = fuzzy::match_query(&self.query, candidate) else {
            return false;
        };
        match m.tier {
            Tier::Exact => true,
            Tier::Prefix | Tier::WordStart => m.query_length >= Self::PREFIX_LENGTH,
            Tier::Substring | Tier::Subsequence => false,
        }
    }
}

/// Everything the ordering reads about an entry besides its names.
#[derive(Debug, Clone, PartialEq)]
pub struct Signals {
    /// The alias the user set, if any.
    pub alias: Option<SearchText>,
    /// How often and how recently the entry is opened.
    pub usage: LauncherUsage,
    /// Which kind wins a full tie; higher first.
    pub priority: i32,
    /// The name the entry is presented under, for the last comparison.
    pub title: String,
    /// Queries this entry wins until the user opens a rival more.
    pub boosted_terms: BTreeSet<String>,
}

impl Default for Signals {
    fn default() -> Signals {
        Signals {
            alias: None,
            usage: LauncherUsage::unused(),
            priority: 0,
            title: String::new(),
            boosted_terms: BTreeSet::new(),
        }
    }
}

/// An entry the ordering can rank.
pub trait Rankable {
    /// The names the entry is found by.
    fn profile(&self) -> &SearchProfile;
    /// Everything else the ordering reads.
    fn signals(&self) -> &Signals;
}

/// An entry, with its names and its signals together.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub profile: SearchProfile,
    pub signals: Signals,
}

impl Entry {
    pub fn new(profile: SearchProfile, signals: Signals) -> Entry {
        Entry { profile, signals }
    }
}

impl Rankable for Entry {
    fn profile(&self) -> &SearchProfile {
        &self.profile
    }

    fn signals(&self) -> &Signals {
        &self.signals
    }
}

/// The launcher's two orderings.
pub struct LauncherOrder;

impl LauncherOrder {
    /// The entries that match the query, best first.
    pub fn ranked<'a, T: Rankable>(
        items: &'a [T],
        query: &Query,
        sensitivity: SearchSensitivity,
        limit: usize,
    ) -> Vec<&'a T> {
        if query.is_empty() {
            return Vec::new();
        }
        let mut scored: Vec<(usize, &'a T, Scored)> = items
            .iter()
            .enumerate()
            .filter_map(|(position, item)| {
                let signals = item.signals().clone();
                let facts = Facts::new(item.profile(), &signals, query, sensitivity)?;
                Some((
                    position,
                    item,
                    Scored {
                        facts: Some(facts),
                        signals,
                    },
                ))
            })
            .collect();

        let length = query.latin.units().len();
        scored.sort_by(|a, b| {
            let order = compare(&a.2, &b.2, length);
            if order != 0 {
                return order.cmp(&0);
            }
            a.0.cmp(&b.0)
        });

        scored
            .into_iter()
            .take(limit)
            .map(|(_, item, _)| item)
            .collect()
    }

    /// The empty list: most frecent first, then aliased entries, then by kind
    /// and name.
    pub fn by_usage<'a, T: Rankable>(items: &'a [T]) -> Vec<&'a T> {
        let mut scored: Vec<(usize, &'a T, Scored)> = items
            .iter()
            .enumerate()
            .map(|(position, item)| {
                (
                    position,
                    item,
                    Scored {
                        facts: None,
                        signals: item.signals().clone(),
                    },
                )
            })
            .collect();

        scored.sort_by(|a, b| {
            let order = tiebreak(&a.2, &b.2);
            if order != 0 {
                return order.cmp(&0);
            }
            a.0.cmp(&b.0)
        });

        scored.into_iter().map(|(_, item, _)| item).collect()
    }
}

/// One entry's match.
struct Scored {
    facts: Option<Facts>,
    signals: Signals,
}

/// How the alias matched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AliasHit {
    None,
    Prefix,
    Exact,
}

/// How a past search term relates to the query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TermHit {
    None,
    Exact { length: usize },
    Prefix { length: usize },
    Overbounds { length: usize },
}

impl TermHit {
    fn strength(&self) -> i32 {
        match self {
            TermHit::None => 0,
            TermHit::Overbounds { .. } => 1,
            TermHit::Prefix { .. } => 2,
            TermHit::Exact { .. } => 3,
        }
    }

    fn is_exact(&self) -> bool {
        matches!(self, TermHit::Exact { .. })
    }

    fn is_prefix(&self) -> bool {
        matches!(self, TermHit::Prefix { .. })
    }

    /// A stored term this long or longer is specific enough to reorder a
    /// longer query.
    fn is_long_overbounds(&self) -> bool {
        matches!(self, TermHit::Overbounds { length } if *length >= OVERBOUNDS_FLOOR)
    }
}

/// Everything the ordering knows about one entry's match.
struct Facts {
    alias: AliasHit,
    is_boosted: bool,
    title_exact: bool,
    /// The best of the title and alternate titles; an exact hit is `i64::MAX`.
    title: i64,
    title_prefix: bool,
    subtitle_exact: bool,
    subtitle: i64,
    term: TermHit,
}

impl Facts {
    /// Nothing at all when the entry does not match.
    fn new(
        profile: &SearchProfile,
        signals: &Signals,
        query: &Query,
        sensitivity: SearchSensitivity,
    ) -> Option<Facts> {
        let latin_length = query.latin.units().len();
        let typed_length = query.typed.units().len();

        let alias = signals
            .alias
            .as_ref()
            .map(|text| alias_hit(text, &query.typed))
            .unwrap_or(AliasHit::None);
        let is_boosted = signals.boosted_terms.contains(&query.term);

        let title_match = crate::matcher::match_text(&query.latin, &profile.title);
        let mut alternate_titles = profile.alternate_titles.clone();
        if alias == AliasHit::None {
            if let Some(text) = &signals.alias {
                alternate_titles.push(text.clone());
            }
        }
        let alternates: Vec<Option<Outcome>> = alternate_titles
            .iter()
            .map(|title| crate::matcher::match_text(&query.typed, title))
            .collect();
        let subtitle_match = profile
            .subtitle
            .as_ref()
            .and_then(|subtitle| crate::matcher::match_text(&query.latin, subtitle));

        let passes = |outcome: Option<Outcome>, length: usize| {
            outcome
                .map(|outcome| sensitivity.accepts(&outcome, length))
                .unwrap_or(false)
        };

        let is_matching = alias != AliasHit::None
            || passes(title_match, latin_length)
            || alternates
                .iter()
                .any(|outcome| passes(*outcome, typed_length))
            || passes(subtitle_match, latin_length)
            || profile.keywords.iter().any(|keyword| {
                passes(
                    crate::matcher::match_text(&query.latin, keyword),
                    latin_length,
                )
            });
        if !is_matching {
            return None;
        }

        let title_exact =
            title_match == Some(Outcome::Exact) || alternates.contains(&Some(Outcome::Exact));
        let title = alternates.iter().fold(value(title_match), |best, outcome| {
            best.max(value(*outcome))
        });
        let title_prefix = profile.title.units().starts_with(query.latin.units())
            || alternate_titles
                .iter()
                .any(|title| title.units().starts_with(query.typed.units()));

        Some(Facts {
            alias,
            is_boosted,
            title_exact,
            title,
            title_prefix,
            subtitle_exact: subtitle_match == Some(Outcome::Exact),
            subtitle: value(subtitle_match),
            term: term_hit(&signals.usage.search_terms, query.latin.units()),
        })
    }
}

/// How many characters a query may run past a stored term and still be read as
/// it.
const OVERBOUNDS_REACH: usize = 3;

/// A stored term of at least this length reorders a longer query.
const OVERBOUNDS_FLOOR: usize = 3;

/// A miss sorts below every alignment.
fn value(outcome: Option<Outcome>) -> i64 {
    outcome.map(|outcome| outcome.value()).unwrap_or(i64::MIN)
}

/// A prefix hit must leave some of the alias still untyped.
fn alias_hit(alias: &SearchText, query: &SearchText) -> AliasHit {
    let a = alias.units();
    let q = query.units();
    if a.len() > q.len() {
        if a.starts_with(q) {
            AliasHit::Prefix
        } else {
            AliasHit::None
        }
    } else if a == q {
        AliasHit::Exact
    } else {
        AliasHit::None
    }
}

/// Newest term first: an exact one wins outright, then the newest prefix.
fn term_hit(terms: &[String], query: &[u16]) -> TermHit {
    let mut best = TermHit::None;
    for term in terms.iter().rev() {
        let stored: Vec<u16> = term.encode_utf16().collect();
        if stored.is_empty() {
            continue;
        }
        if stored.len() > query.len() {
            if !stored.starts_with(query) {
                continue;
            }
            if !best.is_prefix() {
                best = TermHit::Prefix {
                    length: stored.len(),
                };
            }
        } else if stored.len() == query.len() {
            if stored == query {
                return TermHit::Exact {
                    length: stored.len(),
                };
            }
        } else if query.starts_with(&stored) {
            let extra = query.len() - stored.len();
            if extra > OVERBOUNDS_REACH {
                continue;
            }
            best = match best {
                TermHit::None => TermHit::Overbounds {
                    length: stored.len(),
                },
                TermHit::Overbounds { length } if stored.len() > length => TermHit::Overbounds {
                    length: stored.len(),
                },
                other => other,
            };
        }
    }
    best
}

/// Negative puts `a` first; the first rule that separates the two decides.
fn compare(a: &Scored, b: &Scored, length: usize) -> i32 {
    let (Some(x), Some(y)) = (&a.facts, &b.facts) else {
        return tiebreak(a, b);
    };

    if x.alias != y.alias && (x.alias == AliasHit::Exact || y.alias == AliasHit::Exact) {
        return if x.alias == AliasHit::Exact { -1 } else { 1 };
    }

    if x.is_boosted != y.is_boosted {
        let (boosted, other) = if x.is_boosted { (a, b) } else { (b, a) };
        let used = other.signals.usage.frecency;
        if !(used > 1.0 && used > boosted.signals.usage.frecency) {
            return if x.is_boosted { -1 } else { 1 };
        }
    }

    if length > 3 && (x.title_exact || y.title_exact) {
        if !(x.title_exact && y.title_exact) {
            return if x.title_exact { -1 } else { 1 };
        }
        return first([term_strength(x.term, y.term), frecency(a, b)])
            .unwrap_or_else(|| tiebreak(a, b));
    }

    if x.term.is_exact() || y.term.is_exact() {
        if !(x.term.is_exact() && y.term.is_exact()) {
            return if x.term.is_exact() { -1 } else { 1 };
        }
        return first([frecency(a, b)]).unwrap_or_else(|| tiebreak(a, b));
    }

    if x.subtitle_exact || y.subtitle_exact {
        if !(x.subtitle_exact && y.subtitle_exact) {
            return if x.subtitle_exact { -1 } else { 1 };
        }
        return first([frecency(a, b), descending(x.title, y.title)])
            .unwrap_or_else(|| tiebreak(a, b));
    }

    if (x.alias == AliasHit::Prefix) != (y.alias == AliasHit::Prefix) {
        return if x.alias == AliasHit::Prefix { -1 } else { 1 };
    }

    if x.term.is_prefix() != y.term.is_prefix() {
        return if x.term.is_prefix() { -1 } else { 1 };
    }

    if x.term != TermHit::None && y.term != TermHit::None {
        if x.term.is_prefix() && y.term.is_prefix() {
            if let Some(order) = first([frecency(a, b)]) {
                return order;
            }
        }
        if let (TermHit::Overbounds { length: left }, TermHit::Overbounds { length: right }) =
            (x.term, y.term)
        {
            if left != right && (left >= OVERBOUNDS_FLOOR || right >= OVERBOUNDS_FLOOR) {
                return descending(left as i64, right as i64);
            }
        }
        if x.term.is_long_overbounds() != y.term.is_long_overbounds() {
            return if x.term.is_long_overbounds() { -1 } else { 1 };
        }
    }

    if x.term.is_long_overbounds() && y.term == TermHit::None {
        return -1;
    }
    if y.term.is_long_overbounds() && x.term == TermHit::None {
        return 1;
    }

    first([
        descending(x.title.max(x.subtitle), y.title.max(y.subtitle)),
        frecency(a, b),
        descending(x.title, y.title),
        descending(x.title_prefix, y.title_prefix),
        descending(a.signals.priority, b.signals.priority),
    ])
    .unwrap_or_else(|| collate(a, b))
}

/// What decides two entries the query cannot tell apart.
fn tiebreak(a: &Scored, b: &Scored) -> i32 {
    let aliased = descending(a.signals.alias.is_some(), b.signals.alias.is_some());
    first([
        frecency(a, b),
        aliased,
        descending(a.signals.priority, b.signals.priority),
    ])
    .unwrap_or_else(|| collate(a, b))
}

fn term_strength(x: TermHit, y: TermHit) -> i32 {
    if x.strength() != y.strength() {
        return descending(x.strength(), y.strength());
    }
    if let (TermHit::Overbounds { length: left }, TermHit::Overbounds { length: right }) = (x, y) {
        return descending(left as i64, right as i64);
    }
    0
}

fn frecency(a: &Scored, b: &Scored) -> i32 {
    descending_f64(a.signals.usage.frecency, b.signals.usage.frecency)
}

/// Numeric and case-blind: `Item 2` before `Item 10`.
fn collate(a: &Scored, b: &Scored) -> i32 {
    match natural_compare(&a.signals.title, &b.signals.title) {
        Ordering::Less => -1,
        Ordering::Greater => 1,
        Ordering::Equal => 0,
    }
}

/// The first order that is not a tie.
fn first<const N: usize>(orders: [i32; N]) -> Option<i32> {
    orders.into_iter().find(|order| *order != 0)
}

/// The higher value first.
fn descending<Value: PartialOrd>(left: Value, right: Value) -> i32 {
    if left == right {
        0
    } else if left > right {
        -1
    } else {
        1
    }
}

/// The higher value first, for a score that may be equal but not ordered.
fn descending_f64(left: f64, right: f64) -> i32 {
    if left == right {
        0
    } else if left > right {
        -1
    } else {
        1
    }
}

/// Compare two names the way a file list does: digits count as numbers, and
/// case does not count at all.
fn natural_compare(left: &str, right: &str) -> Ordering {
    let mut left = Chunks::new(left);
    let mut right = Chunks::new(right);
    loop {
        match (left.next(), right.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(a), Some(b)) => {
                let order = match (a, b) {
                    (Chunk::Digits(x), Chunk::Digits(y)) => compare_digits(x, y),
                    (Chunk::Digits(x), Chunk::Text(y)) => compare_digits(x, y),
                    (Chunk::Text(x), Chunk::Digits(y)) => compare_digits(y, x).reverse(),
                    (Chunk::Text(x), Chunk::Text(y)) => x.to_lowercase().cmp(&y.to_lowercase()),
                };
                if order != Ordering::Equal {
                    return order;
                }
            }
        }
    }
}

/// Compare two runs of digits as numbers.
fn compare_digits(left: &str, right: &str) -> Ordering {
    let left = left.trim_start_matches('0');
    let right = right.trim_start_matches('0');
    left.len().cmp(&right.len()).then_with(|| left.cmp(right))
}

/// A run of a name: digits, or anything else.
enum Chunk<'a> {
    Digits(&'a str),
    Text(&'a str),
}

/// A name, split into runs.
struct Chunks<'a> {
    rest: &'a str,
}

impl<'a> Chunks<'a> {
    fn new(text: &'a str) -> Chunks<'a> {
        Chunks { rest: text }
    }
}

impl<'a> Iterator for Chunks<'a> {
    type Item = Chunk<'a>;

    fn next(&mut self) -> Option<Chunk<'a>> {
        let rest = self.rest.trim_start();
        if rest.is_empty() {
            self.rest = "";
            return None;
        }
        let digits = rest.chars().take_while(char::is_ascii_digit).count();
        let (chunk, tail) = if digits > 0 {
            rest.split_at(digits)
        } else {
            let letters = rest.chars().take_while(|c| !c.is_ascii_digit()).count();
            rest.split_at(letters)
        };
        self.rest = tail;
        Some(if digits > 0 {
            Chunk::Digits(chunk)
        } else {
            Chunk::Text(chunk)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry_naming::{EntryNaming, Sources};
    use crate::fuzzy::Candidate;
    use crate::matcher::SearchSensitivity;

    /// An entry, as the ordering harness builds one.
    struct Item {
        name: String,
        alternates: Vec<String>,
        subtitle: Option<String>,
        keywords: Vec<String>,
        alias: Option<String>,
        frecency: f64,
        /// Most recent last, as the store keeps them.
        terms: Vec<String>,
        priority: i32,
        boosted: BTreeSet<String>,
    }

    impl Item {
        fn new(name: &str) -> Item {
            Item {
                name: name.to_string(),
                alternates: Vec::new(),
                subtitle: None,
                keywords: Vec::new(),
                alias: None,
                frecency: 1.0,
                terms: Vec::new(),
                priority: 3,
                boosted: BTreeSet::new(),
            }
        }

        fn alternate(mut self, name: &str) -> Item {
            self.alternates.push(name.to_string());
            self
        }

        fn subtitle(mut self, name: &str) -> Item {
            self.subtitle = Some(name.to_string());
            self
        }

        fn keyword(mut self, name: &str) -> Item {
            self.keywords.push(name.to_string());
            self
        }

        fn alias(mut self, name: &str) -> Item {
            self.alias = Some(name.to_string());
            self
        }

        fn frecency(mut self, value: f64) -> Item {
            self.frecency = value;
            self
        }

        fn terms(mut self, terms: &[&str]) -> Item {
            self.terms = terms.iter().map(|t| t.to_string()).collect();
            self
        }

        fn priority(mut self, value: i32) -> Item {
            self.priority = value;
            self
        }

        fn boosted(mut self, terms: &[&str]) -> Item {
            self.boosted = terms.iter().map(|t| t.to_string()).collect();
            self
        }

        fn profile(&self) -> SearchProfile {
            let mut sources = Sources::new(&self.name);
            sources.alternate_titles = self.alternates.clone();
            sources.subtitle = self.subtitle.clone();
            sources.keywords = self.keywords.clone();
            EntryNaming::profile(&sources)
        }

        fn signals(&self) -> Signals {
            Signals {
                alias: self
                    .alias
                    .as_ref()
                    .map(|alias| SearchText::new(alias, false)),
                usage: LauncherUsage {
                    frecency: self.frecency,
                    search_terms: self.terms.clone(),
                },
                priority: self.priority,
                title: self.name.clone(),
                boosted_terms: self.boosted.clone(),
            }
        }
    }

    impl Rankable for Harness {
        fn profile(&self) -> &SearchProfile {
            &self.profile
        }

        fn signals(&self) -> &Signals {
            &self.signals
        }
    }

    /// An item, folded into what the ordering reads.
    struct Harness {
        profile: SearchProfile,
        signals: Signals,
    }

    impl Harness {
        fn of(item: &Item) -> Harness {
            Harness {
                profile: item.profile(),
                signals: item.signals(),
            }
        }
    }

    fn rank(query: &str, items: &[Item]) -> Vec<String> {
        let entries: Vec<Harness> = items.iter().map(Harness::of).collect();
        LauncherOrder::ranked(&entries, &Query::new(query), SearchSensitivity::High, 200)
            .into_iter()
            .map(|entry| entry.signals.title.clone())
            .collect()
    }

    fn first(query: &str, items: &[Item]) -> Option<String> {
        rank(query, items).into_iter().next()
    }

    #[test]
    fn an_exact_alias_beats_an_exact_title() {
        let items = [Item::new("Fig"), Item::new("Figma").alias("fig")];
        assert_eq!(first("fig", &items).as_deref(), Some("Figma"));
    }

    #[test]
    fn a_boosted_term_beats_a_stronger_alignment() {
        let items = [
            Item::new("ChatGPT"),
            Item::new("AI Chat").boosted(&["ai", "chat"]),
        ];
        assert_eq!(first("chat", &items).as_deref(), Some("AI Chat"));
    }

    #[test]
    fn until_the_other_entry_is_the_one_the_user_opens_more() {
        let items = [
            Item::new("ChatGPT").frecency(300.0),
            Item::new("AI Chat").frecency(101.0).boosted(&["chat"]),
        ];
        assert_eq!(first("chat", &items).as_deref(), Some("ChatGPT"));
    }

    #[test]
    fn past_three_letters_an_exact_title_beats_any_habit() {
        let items = [
            Item::new("Search Notes").frecency(900.0).terms(&["notes"]),
            Item::new("Notes"),
        ];
        assert_eq!(first("notes", &items).as_deref(), Some("Notes"));
    }

    #[test]
    fn at_three_letters_an_exact_search_term_beats_an_exact_title() {
        let items = [
            Item::new("Not"),
            Item::new("Notes").frecency(200.0).terms(&["not"]),
        ];
        assert_eq!(first("not", &items).as_deref(), Some("Notes"));
    }

    #[test]
    fn an_exact_search_term_beats_a_stronger_alignment() {
        let items = [
            Item::new("Safari"),
            Item::new("Slack").frecency(150.0).terms(&["sa"]),
        ];
        assert_eq!(first("sa", &items).as_deref(), Some("Slack"));
    }

    #[test]
    fn an_exact_subtitle_lists_an_extensions_commands_first() {
        let items = [Item::new("Brewer"), Item::new("Search").subtitle("Brew")];
        assert_eq!(first("brew", &items).as_deref(), Some("Search"));
    }

    #[test]
    fn an_alias_prefix_beats_a_stronger_alignment() {
        let items = [Item::new("Spotify"), Item::new("Arc").alias("spaces")];
        assert_eq!(first("sp", &items).as_deref(), Some("Arc"));
    }

    #[test]
    fn a_later_word_of_an_alias_finds_its_entry() {
        let appearance = Item::new("Toggle System Appearance").alias("toggle light / dark");
        let items = [Item::new("Safari"), appearance];
        assert_eq!(
            rank("dark", &items),
            vec!["Toggle System Appearance".to_string()]
        );
    }

    #[test]
    fn but_ranked_by_its_score_so_a_title_prefix_still_wins() {
        let appearance = Item::new("Toggle System Appearance").alias("toggle light / dark");
        let items = [appearance, Item::new("Darkroom")];
        assert_eq!(
            rank("dark", &items),
            vec![
                "Darkroom".to_string(),
                "Toggle System Appearance".to_string()
            ]
        );
    }

    #[test]
    fn a_mid_word_alias_hit_stays_under_the_sensitivity() {
        let items = [Item::new("Ghostty").alias("myterm")];
        assert!(rank("term", &items).is_empty());
    }

    #[test]
    fn a_term_the_query_prefixes_reaches_back_to_shorter_queries() {
        let items = [
            Item::new("Slack").frecency(400.0),
            Item::new("Safari").frecency(101.0).terms(&["safari"]),
        ];
        assert_eq!(first("s", &items).as_deref(), Some("Safari"));
    }

    #[test]
    fn a_term_a_longer_query_runs_past_still_counts_three_letters_or_more() {
        let fantastical = Item::new("Fantastical")
            .keyword("calendar")
            .frecency(150.0)
            .terms(&["cal"]);
        let items = [Item::new("Calendar"), fantastical];
        assert_eq!(first("cale", &items).as_deref(), Some("Fantastical"));
    }

    #[test]
    fn but_not_from_two() {
        let short = Item::new("Fantastical")
            .keyword("calendar")
            .frecency(150.0)
            .terms(&["ca"]);
        let items = [Item::new("Calendar"), short];
        assert_eq!(first("cale", &items).as_deref(), Some("Calendar"));
    }

    #[test]
    fn the_stronger_alignment_wins_before_usage() {
        let items = [
            Item::new("Google Chrome").frecency(800.0),
            Item::new("Chrome Remote Desktop"),
        ];
        assert_eq!(
            first("chrome", &items).as_deref(),
            Some("Chrome Remote Desktop")
        );
    }

    #[test]
    fn frecency_breaks_an_equal_alignment() {
        let items = [Item::new("Safari"), Item::new("Slack").frecency(200.0)];
        assert_eq!(first("s", &items).as_deref(), Some("Slack"));
    }

    #[test]
    fn a_title_hit_beats_the_same_score_on_a_subtitle() {
        let items = [Item::new("Search").subtitle("Maps"), Item::new("Maps")];
        assert_eq!(first("ma", &items).as_deref(), Some("Maps"));
    }

    #[test]
    fn a_title_the_query_starts_beats_the_same_score_skipping_to_a_later_word() {
        let items = [Item::new("AirPort Utility"), Item::new("App Store")];
        assert_eq!(first("ap", &items).as_deref(), Some("App Store"));
    }

    #[test]
    fn but_ahead_of_kind_priority() {
        let items = [
            Item::new("Desk View").priority(4),
            Item::new("Device Hub").priority(1),
        ];
        assert_eq!(first("dev", &items).as_deref(), Some("Device Hub"));
    }

    #[test]
    fn but_behind_frecency() {
        let items = [
            Item::new("App Store"),
            Item::new("AirPort Utility").frecency(200.0),
        ];
        assert_eq!(first("ap", &items).as_deref(), Some("AirPort Utility"));
    }

    #[test]
    fn an_app_wins_the_tie_a_command_ties_it_on() {
        let items = [
            Item::new("Tinycast Settings").priority(3),
            Item::new("System Settings").priority(4),
        ];
        assert_eq!(
            first("settings", &items).as_deref(),
            Some("System Settings")
        );
    }

    #[test]
    fn names_compare_numerically_last() {
        let items = [Item::new("Item 10"), Item::new("Item 2")];
        assert_eq!(
            rank("item", &items),
            vec!["Item 2".to_string(), "Item 10".to_string()]
        );
    }

    #[test]
    fn a_keyword_finds_an_entry_without_ranking_it() {
        let items = [
            Item::new("Calendar").keyword("iCal"),
            Item::new("iCal Import"),
        ];
        assert_eq!(
            rank("ical", &items),
            vec!["iCal Import".to_string(), "Calendar".to_string()]
        );
    }

    #[test]
    fn an_alternate_title_ranks_where_a_keyword_only_finds() {
        let items = [
            Item::new("Utility").keyword("sysadmin"),
            Item::new("系统设置").alternate("System Settings"),
        ];
        assert_eq!(
            rank("sys", &items),
            vec!["系统设置".to_string(), "Utility".to_string()]
        );
    }

    #[test]
    fn the_empty_list_reads_frecency_then_aliases_then_kind_then_name() {
        let items = [
            Item::new("Zed"),
            Item::new("Arc").alias("a"),
            Item::new("Maps").frecency(300.0),
            Item::new("Mail").priority(4),
        ];
        let entries: Vec<Harness> = items.iter().map(Harness::of).collect();
        let names: Vec<String> = LauncherOrder::by_usage(&entries)
            .into_iter()
            .map(|entry| entry.signals.title.clone())
            .collect();
        assert_eq!(
            names,
            vec![
                "Maps".to_string(),
                "Arc".to_string(),
                "Mail".to_string(),
                "Zed".to_string()
            ]
        );
    }

    #[test]
    fn an_empty_query_matches_nothing() {
        let items = [Item::new("Safari")];
        assert!(rank("", &items).is_empty());
    }

    // MARK: - Category queries

    fn category_names() -> Vec<Candidate> {
        [
            "Window Management",
            "Window Command",
            "Window Layouts",
            "Window Layout",
        ]
        .into_iter()
        .map(Candidate::new)
        .collect()
    }

    fn matching(raw: &str) -> Vec<String> {
        let query = CategoryQuery::new(raw);
        category_names()
            .iter()
            .filter(|candidate| query.matches(candidate))
            .map(|candidate| candidate.text().to_string())
            .collect()
    }

    #[test]
    fn a_category_word_finds_every_window_category() {
        for query in [
            "win",
            "window",
            "WINDOW",
            "ｗｉｎｄｏｗ",
            "wíndow",
            " \twindow\n",
        ] {
            assert_eq!(
                matching(query).len(),
                4,
                "'{query}' found {:?}",
                matching(query)
            );
        }
    }

    #[test]
    fn a_partial_category_phrase_narrows_to_management() {
        assert_eq!(matching("window man"), vec!["window management"]);
    }

    #[test]
    fn category_whitespace_is_folded_once() {
        assert_eq!(matching("window \t\n man"), vec!["window management"]);
    }

    #[test]
    fn a_later_word_finds_its_category() {
        assert_eq!(matching("manage"), vec!["window management"]);
    }

    #[test]
    fn a_label_finds_the_same_category() {
        assert_eq!(matching("window com"), vec!["window command"]);
    }

    #[test]
    fn a_later_layout_word_stays_specific() {
        assert_eq!(matching("lay"), vec!["window layouts", "window layout"]);
    }

    #[test]
    fn a_partial_word_expands_no_categories() {
        for query in [
            "",
            " \t\n",
            "w",
            "wi",
            "ind",
            "wnd",
            "windowx",
            "window manager",
        ] {
            assert!(matching(query).is_empty(), "'{query}' found something");
        }
    }

    #[test]
    fn an_exact_short_category_is_still_valid() {
        assert!(CategoryQuery::new("AI").matches(&Candidate::new("AI")));
    }

    #[test]
    fn canonical_category_names_share_the_launcher_fold() {
        assert_eq!(
            CategoryQuery::new(" \nＳＹＳＴＥＭ\tSettings ").name,
            "system settings"
        );
    }
}
