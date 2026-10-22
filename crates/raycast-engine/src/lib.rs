//! The application layer.
//!
//! The engine turns a query into a ranked list of results. It owns the
//! decision of *which* commands answer a query and *how* their candidates
//! compete, but it owns no presentation and no I/O beyond the store and the
//! index it is given.

use chrono::{DateTime, Utc};
use yal_core::command::CommandKind;
use yal_core::item::Item;
use yal_index::Discovered;
use yal_search::{Candidate, Frecency, FrecencyModel, Matcher, Scored, rank};
use yal_store::Store;

pub mod sources;

pub use sources::Source;

/// A failure while producing results.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("store error: {0}")]
    Store(#[from] yal_store::StoreError),

    #[error("the query could not be interpreted: {0}")]
    Query(String),
}

/// Result alias for engine operations.
pub type EngineResult<T> = Result<T, EngineError>;

/// The maximum number of results the engine returns.
pub const DEFAULT_LIMIT: usize = 50;

/// The engine.
pub struct Engine {
    matcher: Matcher,
    frecency_model: FrecencyModel,
    limit: usize,
    /// The last window list, and when it was taken.
    window_cache: Option<(DateTime<Utc>, Vec<yal_system::Window>)>,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    /// An engine with the default ranking configuration.
    pub fn new() -> Self {
        Self {
            matcher: Matcher::new(),
            frecency_model: FrecencyModel::default(),
            limit: DEFAULT_LIMIT,
            window_cache: None,
        }
    }

    /// Change how many results are returned.
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }

    /// The frecency model in use.
    pub fn frecency_model(&self) -> &FrecencyModel {
        &self.frecency_model
    }

    /// Rank a set of candidates against a query.
    ///
    /// Frecency is supplied per candidate so the engine stays free of I/O.
    pub fn rank_candidates(
        &self,
        candidates: &[Candidate],
        query: &str,
        now: DateTime<Utc>,
    ) -> Vec<Scored> {
        rank(
            candidates,
            query,
            &self.matcher,
            &self.frecency_model,
            now,
            self.limit,
        )
        .results
    }

    /// Build a candidate from an item and a usage record.
    pub fn candidate(item: Item, kind: CommandKind, frecency: Frecency) -> Candidate {
        Candidate {
            item,
            kind,
            frecency,
        }
    }

    /// Answer a query.
    ///
    /// Candidates are gathered from every source, then ranked together. Usage
    /// history is read once for all candidates, so the number of queries
    /// against the store does not grow with the number of results.
    pub fn search(
        &mut self,
        store: &Store,
        applications: &[Discovered],
        query: &str,
        now: DateTime<Utc>,
    ) -> EngineResult<Vec<Scored>> {
        let mut candidates = sources::applications(applications);
        candidates.extend(sources::snippets(store)?);
        candidates.extend(sources::quicklinks(store)?);
        candidates.extend(sources::bookmarks(store)?);
        candidates.extend(sources::clipboard(store, 100)?);

        let trimmed = query.trim();
        if !trimmed.is_empty() {
            candidates.extend(sources::emoji(trimmed, 20));
            if let Some(calculator) = sources::calculator(trimmed) {
                candidates.push(calculator);
            }
            if let Some(expansion) = sources::snippet_expansion(store, trimmed)? {
                candidates.push(expansion);
            }
            if let Some(code) = sources::qr_code(trimmed) {
                candidates.push(code);
            }
            if let Some(code) = sources::totp(trimmed) {
                candidates.push(code);
            }
            if let Some(matched) = sources::regex(trimmed, "") {
                candidates.push(matched);
            }
            if let Some(search) = sources::web_search(trimmed) {
                candidates.push(search);
            }
            if trimmed.chars().count() >= 3 {
                let roots = yal_index::default_search_roots();
                let found = yal_index::search_files(&roots, trimmed, 30, 2_000);
                candidates.extend(sources::files(&found));
            }
        }

        candidates.extend(sources::windows(&self.open_windows(now)));

        // Read usage history for every candidate in one query.
        let keys: Vec<String> = candidates
            .iter()
            .map(|candidate| candidate.item.id.0.clone())
            .collect();
        let usage = store.usage_for(&keys)?;

        let enriched: Vec<Candidate> = candidates
            .into_iter()
            .map(|mut candidate| {
                if let Some(record) = usage.get(&candidate.item.id.0) {
                    candidate.frecency = record.frecency();
                }
                candidate
            })
            .collect();

        // An empty query matches nothing, which would leave the palette
        // blank. The default view is the set of things the user reaches for
        // most, so an empty query ranks by usage instead of by text.
        if query.trim().is_empty() {
            return Ok(self.rank_by_usage(enriched, now));
        }

        Ok(self.rank_candidates(&enriched, query, now))
    }

    /// The open windows, refreshed at most every couple of seconds.
    ///
    /// Enumerating windows is a system call per window, so doing it on every
    /// keystroke would cost more than the result is worth.
    fn open_windows(&mut self, now: DateTime<Utc>) -> Vec<yal_system::Window> {
        const TTL_SECONDS: i64 = 2;
        if let Some((at, windows)) = &self.window_cache {
            if (now - *at).num_seconds() < TTL_SECONDS {
                return windows.clone();
            }
        }
        let windows = yal_system::list_windows().unwrap_or_default();
        self.window_cache = Some((now, windows.clone()));
        windows
    }

    /// Rank candidates by how often they are used, then by title.
    fn rank_by_usage(&self, candidates: Vec<Candidate>, now: DateTime<Utc>) -> Vec<Scored> {        let mut scored: Vec<Scored> = candidates
            .into_iter()
            .map(|candidate| {
                let score = self.frecency_model.multiplier(candidate.frecency, now);
                Scored {
                    item: candidate.item,
                    kind: candidate.kind,
                    matched: yal_core::ranking::Match::none(),
                    score,
                    frecency: candidate.frecency,
                }
            })
            .collect();

        scored.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.item.title.cmp(&b.item.title))
        });
        scored.truncate(self.limit);
        scored
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranks_candidates_by_query() {
        let engine = Engine::new();
        let candidates = vec![
            Engine::candidate(Item::new("Calculator"), CommandKind::Calculator, Frecency::never()),
            Engine::candidate(Item::new("Notepad"), CommandKind::Applications, Frecency::never()),
        ];
        let results = engine.rank_candidates(&candidates, "calc", Utc::now());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].item.title, "Calculator");
    }

    #[test]
    fn limit_is_respected() {
        let engine = Engine::new().with_limit(2);
        let candidates: Vec<Candidate> = (0..10)
            .map(|index| {
                Engine::candidate(
                    Item::new(format!("item {index}")),
                    CommandKind::Applications,
                    Frecency::never(),
                )
            })
            .collect();
        let results = engine.rank_candidates(&candidates, "item", Utc::now());
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn an_empty_query_still_returns_candidates() {
        // An empty query matches no text, so the engine falls back to ranking
        // by usage rather than showing nothing at all.
        let engine = Engine::new();
        let candidates = vec![
            Engine::candidate(Item::new("Calculator"), CommandKind::Calculator, Frecency::never()),
            Engine::candidate(Item::new("Notepad"), CommandKind::Applications, Frecency::never()),
        ];
        let ranked = engine.rank_by_usage(candidates, Utc::now());
        assert_eq!(ranked.len(), 2);
    }

    #[test]
    fn an_empty_query_ranks_used_items_first() {
        let engine = Engine::new();
        let now = Utc::now();
        let candidates = vec![
            Engine::candidate(
                Item::new("Never used"),
                CommandKind::Applications,
                Frecency::never(),
            ),
            Engine::candidate(
                Item::new("Often used"),
                CommandKind::Applications,
                Frecency::new(50, Some(now - chrono::Duration::hours(1))),
            ),
        ];
        let ranked = engine.rank_by_usage(candidates, now);
        assert_eq!(ranked[0].item.title, "Often used");
    }
}
