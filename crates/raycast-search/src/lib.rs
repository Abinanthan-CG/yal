//! Search: fuzzy matching, ranking and frecency.
//!
//! This crate answers "which of these candidates best answers this query".
//! It is deliberately free of I/O so it can be benchmarked and tested in
//! isolation; the store and index crates feed it candidates, and the UI crate
//! renders what it returns.

pub mod frecency;
pub mod matcher;
pub mod ranking;

pub use frecency::{Frecency, FrecencyModel};
pub use matcher::{Matcher, Weights};
pub use ranking::{Candidate, Ranked, Scored, rank};
