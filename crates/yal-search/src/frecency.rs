//! Frecency: ranking a candidate by how often and how recently it was used.
//!
//! Frecency is deliberately a *modifier*, not a score of its own. Text match
//! quality decides which candidates are plausible; frecency decides which
//! plausible candidate the user most likely meant. A never-used item is
//! neither penalised nor boosted relative to a match of the same quality.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

/// How a candidate has been used historically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Frecency {
    /// Number of times the candidate has been selected.
    pub count: u32,
    /// When it was last selected, if ever.
    pub last_used: Option<DateTime<Utc>>,
}

impl Frecency {
    /// A candidate that has never been used.
    pub fn never() -> Self {
        Self::default()
    }

    /// A candidate used `count` times, most recently at `last_used`.
    pub fn new(count: u32, last_used: Option<DateTime<Utc>>) -> Self {
        Self { count, last_used }
    }

    /// Whether the candidate has any recorded usage.
    pub fn is_used(&self) -> bool {
        self.count > 0
    }
}

/// The decay model applied to usage history.
#[derive(Debug, Clone, Copy)]
pub struct FrecencyModel {
    /// How long it takes for a usage to lose half of its weight.
    pub half_life: Duration,
    /// The largest multiplier frecency may apply.
    pub max_multiplier: f64,
}

impl Default for FrecencyModel {
    fn default() -> Self {
        Self {
            half_life: Duration::hours(24),
            max_multiplier: 3.0,
        }
    }
}

impl FrecencyModel {
    /// The multiplier applied to a candidate's match score.
    ///
    /// Returns `1.0` for a candidate with no history, so frecency never
    /// penalises an unranked candidate relative to another unranked one.
    pub fn multiplier(&self, entry: Frecency, now: DateTime<Utc>) -> f64 {
        if !entry.is_used() {
            return 1.0;
        }

        let age = entry
            .last_used
            .map(|at| now.signed_duration_since(at))
            // A timestamp in the future is treated as "just used" rather than
            // producing a multiplier above the cap.
            .unwrap_or(Duration::zero())
            .num_seconds()
            .max(0) as f64;

        let half_life_seconds = self.half_life.num_seconds().max(1) as f64;
        let recency = 0.5f64.powf(age / half_life_seconds);

        // Logarithmic in the count so that one hundred uses is not a hundred
        // times the pull of a single use.
        let frequency = (1.0 + entry.count as f64).ln();

        (1.0 + recency * frequency).min(self.max_multiplier)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Utc> {
        DateTime::from_timestamp(1_700_000_000, 0).unwrap()
    }

    #[test]
    fn unused_candidate_is_unmodified() {
        let model = FrecencyModel::default();
        assert_eq!(model.multiplier(Frecency::never(), now()), 1.0);
    }

    #[test]
    fn recent_use_beats_old_use() {
        let model = FrecencyModel::default();
        let recent = Frecency::new(5, Some(now() - Duration::minutes(5)));
        let old = Frecency::new(5, Some(now() - Duration::days(30)));
        assert!(
            model.multiplier(recent, now()) > model.multiplier(old, now()),
            "recent usage must outrank old usage"
        );
    }

    #[test]
    fn frequent_use_beats_infrequent_use_at_equal_age() {
        let model = FrecencyModel::default();
        let many = Frecency::new(50, Some(now() - Duration::hours(1)));
        let few = Frecency::new(1, Some(now() - Duration::hours(1)));
        assert!(model.multiplier(many, now()) > model.multiplier(few, now()));
    }

    #[test]
    fn multiplier_is_capped() {
        let model = FrecencyModel::default();
        let heavy = Frecency::new(100_000, Some(now()));
        assert!(model.multiplier(heavy, now()) <= model.max_multiplier);
    }

    #[test]
    fn future_timestamp_does_not_exceed_cap() {
        let model = FrecencyModel::default();
        let future = Frecency::new(10, Some(now() + Duration::days(1)));
        assert!(model.multiplier(future, now()) <= model.max_multiplier);
    }

    #[test]
    fn half_life_halves_the_weight() {
        let model = FrecencyModel {
            half_life: Duration::hours(24),
            max_multiplier: 100.0,
        };
        let fresh = Frecency::new(1, Some(now()));
        let aged = Frecency::new(1, Some(now() - Duration::hours(24)));
        let fresh_weight = model.multiplier(fresh, now()) - 1.0;
        let aged_weight = model.multiplier(aged, now()) - 1.0;
        assert!((fresh_weight - aged_weight * 2.0).abs() < 1e-9);
    }
}
