//! The result of matching a query against a candidate string.

use std::ops::Range;

/// How the query matched the candidate.
///
/// The tier is the dominant term in the final score. A command that matches
/// exactly should always outrank one that merely contains the query as a
/// scattered subsequence, no matter how many extra keywords it has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MatchKind {
    /// The candidate equals the query.
    Exact,

    /// The candidate starts with the query.
    Prefix,

    /// The candidate starts with the query on a word boundary.
    WordBoundary,

    /// The candidate contains the query.
    Substring,

    /// The query's characters appear in order, but scattered.
    Subsequence,

    /// The query's characters appear in order and form a camelCase acronym.
    Acronym,

    /// No match.
    None,
}

impl MatchKind {
    /// The score multiplier for this tier.
    ///
    /// Tiers are separated by wide gaps so that a tier difference always
    /// dominates any amount of within-tier quality difference.
    pub fn weight(self) -> f64 {
        match self {
            MatchKind::Exact => 1000.0,
            MatchKind::Prefix => 700.0,
            MatchKind::WordBoundary => 550.0,
            MatchKind::Substring => 400.0,
            MatchKind::Acronym => 250.0,
            MatchKind::Subsequence => 150.0,
            MatchKind::None => 0.0,
        }
    }
}

/// A scored match.
#[derive(Debug, Clone, PartialEq)]
pub struct Match {
    /// The tier of the match.
    pub kind: MatchKind,
    /// The final score, higher is better.
    pub score: f64,
    /// The character positions in the candidate that matched, in order.
    pub positions: Vec<usize>,
    /// Contiguous runs within `positions`, for highlighting.
    pub ranges: Vec<Range<usize>>,
}

impl Match {
    /// A match that did not succeed.
    pub fn none() -> Self {
        Self {
            kind: MatchKind::None,
            score: 0.0,
            positions: Vec::new(),
            ranges: Vec::new(),
        }
    }

    /// Whether the match succeeded.
    pub fn is_some(&self) -> bool {
        self.kind != MatchKind::None
    }

    /// Collapse sorted positions into contiguous ranges.
    pub fn ranges_from_positions(positions: &[usize]) -> Vec<Range<usize>> {
        let mut ranges: Vec<Range<usize>> = Vec::new();
        let mut iter = positions.iter().peekable();
        while let Some(&start) = iter.next() {
            let mut end = start + 1;
            while let Some(&next) = iter.peek() {
                if *next == end {
                    end += 1;
                    iter.next();
                } else {
                    break;
                }
            }
            ranges.push(start..end);
        }
        ranges
    }
}
