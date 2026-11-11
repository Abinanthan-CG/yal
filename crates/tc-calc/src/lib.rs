//! `tc-calc` - the calculator core of the YAL port of Tinycast.
//!
//! This crate is the arithmetic heart of the calculator: the tokenizer, the expression
//! parser, the evaluation engine and the number formatting. It is a faithful port of
//! upstream Tinycast's `Features/Calculator/Model` files `CalcToken`, `CalcOperator`,
//! `CalcNumberBase`, `CalcTokenizer`, `CalcExpressionParser`, `CalcEngine`, `CalcMath`
//! and `CalcFormatter`.
//!
//! # What is not here
//!
//! Upstream's calculator also carries units, currencies, dates, percentages, finance
//! and more. Those are ported separately, so this crate asks a
//! [`catalog::Catalog`] about the units and the currencies it meets and answers
//! [`engine::CalcOutcome::Unsupported`] for a query that needs one of them. It never
//! answers a made-up number.
//!
//! # Example
//!
//! ```
//! use tc_calc::{evaluate, CalcOutcome, NameCatalog};
//!
//! let outcome = evaluate("2^3^2", &NameCatalog::new());
//! assert_eq!(outcome.display(), Some("512"));
//! assert!(matches!(outcome, CalcOutcome::Card(_)));
//! ```

pub mod catalog;
pub mod engine;
pub mod formatter;
pub mod math;
pub mod parser;
pub mod result;
pub mod rounding;
pub mod token;
pub mod tokenizer;
pub mod value;

#[cfg(test)]
mod tests;

pub use catalog::{Catalog, CurrencyInfo, NameCatalog, UnitInfo};
pub use engine::{evaluate, CalcOutcome};
pub use formatter::Formatter;
pub use math::Math;
pub use parser::Parser;
pub use result::{CalcResult, Payload};
pub use token::{NumberBase, Operator, Token};
pub use tokenizer::tokenize;
pub use value::{ParseError, UnsupportedArea, Value};
