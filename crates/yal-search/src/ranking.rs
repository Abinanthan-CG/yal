//! Ranking candidates against a query.
//!
//! A candidate exposes several searchable fields. Each field is matched
//! independently and the *best* field match wins, with a per-field weight so
//! that a title match always outranks an equally good subtitle match. The
//! winning match is then scaled by the candidate's frecency multiplier.

use chrono::Utc;
use yal_core::command::CommandKind;
use yal_core::item::Item;
use yal_core::ranking::Match;

use crate::frecency::{Frecency, FrecencyModel};
use crate::matcher::Matcher;

/// How much each field contributes relative to the title.
pub struct FieldWeights {
    pub title: f64,
    pub subtitle: f64,
    pub keyword: f64,
}

impl Default for FieldWeights {
    fn default() -> Self {
        Self {
            title: 1.0,
            subtitle: 0.55,
            keyword: 0.75,
        }
    }
}

/// A candidate to be ranked.
#[derive(Debug, Clone)]
pub struct Candidate {
    /// The item to return if this candidate wins.
    pub item: Item,
    /// The command that produced the item, used for grouping.
    pub kind: CommandKind,
    /// Usage history.
    pub frecency: Frecency,
}

impl Candidate {
    /// A candidate with no recorded usage.
    pub fn new(item: Item, kind: CommandKind) -> Self {
        Self {
            item,
            kind,
            frecency: Frecency::never(),
        }
    }

    /// Attach usage history.
    pub fn with_frecency(mut self, frecency: Frecency) -> Self {
        self.frecency = frecency;
        self
    }
}

/// A candidate together with the score that placed it.
#[derive(Debug, Clone)]
pub struct Scored {
    /// The matched item.
    pub item: Item,
    /// The command that produced the item.
    pub kind: CommandKind,
    /// The winning field match.
    pub matched: Match,
    /// The final score, after frecency.
    pub score: f64,
    /// The usage history that contributed.
    pub frecency: Frecency,
}

/// The ranked output of a search.
#[derive(Debug, Clone, Default)]
pub struct Ranked {
    /// Results in descending score order.
    pub results: Vec<Scored>,
    /// How many candidates were considered.
    pub considered: usize,
    /// How many candidates matched at all.
    pub matched: usize,
}

/// Rank candidates against a query.
///
/// Results are sorted by descending score. Ties are broken by candidate
/// insertion order, which keeps the output stable when several items score
/// identically.
pub fn rank(
    candidates: &[Candidate],
    query: &str,
    matcher: &Matcher,
    model: &FrecencyModel,
    now: chrono::DateTime<Utc>,
    limit: usize,
) -> Ranked {
    let weights = FieldWeights::default();
    let mut scored: Vec<Scored> = Vec::new();
    let mut matched = 0usize;

    for candidate in candidates {
        let Some(best) = best_field_match(candidate, query, matcher, &weights) else {
            continue;
        };
        matched += 1;
        let multiplier = model.multiplier(candidate.frecency, now);
        scored.push(Scored {
            item: candidate.item.clone(),
            kind: candidate.kind,
            matched: best.0,
            score: best.1 * multiplier,
            frecency: candidate.frecency,
        });
    }

    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.item.title.cmp(&b.item.title))
    });
    scored.truncate(limit);

    Ranked {
        results: scored,
        considered: candidates.len(),
        matched,
    }
}

/// Find the best matching field of a candidate.
///
/// Returns the match and its weighted score, or `None` when no field matched.
fn best_field_match(
    candidate: &Candidate,
    query: &str,
    matcher: &Matcher,
    weights: &FieldWeights,
) -> Option<(Match, f64)> {
    let mut best: Option<(Match, f64)> = None;

    let mut consider = |matched: Match, weight: f64| {
        if !matched.is_some() {
            return;
        }
        let weighted = matched.score * weight;
        let better = match &best {
            None => true,
            Some((current, current_score)) => {
                weighted > *current_score
                    || (weighted == *current_score && current.kind > matched.kind)
            }
        };
        if better {
            best = Some((matched, weighted));
        }
    };

    consider(matcher.match_str(query, &candidate.item.title), weights.title);

    if let Some(subtitle) = candidate.item.subtitle.as_deref() {
        consider(matcher.match_str(query, subtitle), weights.subtitle);
    }

    for keyword in candidate.item.search_terms() {
        consider(matcher.match_str(query, keyword), weights.keyword);
    }

    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use yal_core::ranking::MatchKind;

    fn candidate(title: &str, subtitle: Option<&str>) -> Candidate {
        let mut item = Item::new(title);
        if let Some(subtitle) = subtitle {
            item = item.with_subtitle(subtitle);
        }
        Candidate::new(item, CommandKind::Applications)
    }

    fn score_of(ranked: &Ranked, title: &str) -> f64 {
        ranked
            .results
            .iter()
            .find(|r| r.item.title == title)
            .map(|r| r.score)
            .unwrap_or(f64::NEG_INFINITY)
    }

    #[test]
    fn ranks_exact_title_above_subtitle_match() {
        let candidates = vec![
            candidate("Visual Studio Code", Some("code editor")),
            candidate("Notepad", Some("visual studio code")),
        ];
        let ranked = rank(
            &candidates,
            "code",
            &Matcher::new(),
            &FrecencyModel::default(),
            Utc::now(),
            10,
        );
        assert_eq!(ranked.results[0].item.title, "Visual Studio Code");
    }

    #[test]
    fn non_matching_candidates_are_dropped() {
        let candidates = vec![candidate("Calculator", None), candidate("Paint", None)];
        let ranked = rank(
            &candidates,
            "calculator",
            &Matcher::new(),
            &FrecencyModel::default(),
            Utc::now(),
            10,
        );
        assert_eq!(ranked.results.len(), 1);
        assert_eq!(ranked.considered, 2);
        assert_eq!(ranked.matched, 1);
    }

    #[test]
    fn frecency_breaks_ties_between_equal_matches() {
        let now = Utc::now();
        let plain = candidate("Editor", None);
        let used = candidate("Editor", None).with_frecency(Frecency::new(
            20,
            Some(now - Duration::hours(1)),
        ));
        let candidates = vec![plain, used];
        let ranked = rank(
            &candidates,
            "editor",
            &Matcher::new(),
            &FrecencyModel::default(),
            now,
            10,
        );
        assert_eq!(ranked.results.len(), 2);
        assert!(score_of(&ranked, "Editor") > 0.0);
        // The two candidates share a title, so the used one must lead.
        assert!(ranked.results[0].frecency.is_used());
    }

    #[test]
    fn limit_truncates_results() {
        let candidates: Vec<Candidate> = (0..10)
            .map(|i| candidate(&format!("item {i}"), None))
            .collect();
        let ranked = rank(
            &candidates,
            "item",
            &Matcher::new(),
            &FrecencyModel::default(),
            Utc::now(),
            3,
        );
        assert_eq!(ranked.results.len(), 3);
        assert_eq!(ranked.matched, 10);
    }

    #[test]
    fn keyword_matches_but_scores_below_title() {
        let titled = candidate("Visual Studio Code", None);
        let keyworded = {
            let mut item = Item::new("Something Else");
            item = item.with_keyword("visual studio code");
            Candidate::new(item, CommandKind::Applications)
        };
        let ranked = rank(
            &[titled, keyworded],
            "visual studio code",
            &Matcher::new(),
            &FrecencyModel::default(),
            Utc::now(),
            10,
        );
        assert_eq!(ranked.results[0].item.title, "Visual Studio Code");
        assert_eq!(ranked.results[1].matched.kind, MatchKind::Exact);
    }
}
