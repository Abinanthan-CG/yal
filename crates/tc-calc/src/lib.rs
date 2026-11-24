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
//! Upstream's calculator also carries currencies, dates, percentages, finance and more.
//! Those are ported separately, so this crate asks a [`catalog::Catalog`] about the
//! currencies it meets and answers [`engine::CalcOutcome::Unsupported`] for a query that
//! needs one of them. It never answers a made-up number.
//!
//! The **units** are here: [`units::UnitCatalog`] holds upstream's whole unit table, and
//! the engine answers the conversions between two of them.
//!
//! # Example
//!
//! ```
//! use tc_calc::{evaluate, CalcOutcome, UnitCatalog};
//!
//! let outcome = evaluate("2^3^2", &UnitCatalog::new());
//! assert_eq!(outcome.display(), Some("512"));
//! assert!(matches!(outcome, CalcOutcome::Card(_)));
//! ```

pub mod catalog;
pub mod color;
pub mod context;
pub mod currency;
pub mod datetime;
pub mod duration;
pub mod engine;
pub mod finance;
pub mod formatter;
pub mod fx;
pub mod growth;
pub mod math;
pub mod measure;
pub mod mixed_units;
pub mod parser;
pub mod percent;
pub mod result;
pub mod rounding;
pub mod specialty;
pub mod split;
pub mod spoken;
pub mod statistics;
pub mod timezone;
pub mod token;
pub mod tokenizer;
pub mod units;
pub mod value;

#[cfg(test)]
mod measure_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod units_tests;

#[cfg(test)]
mod currency_tests;

#[cfg(test)]
mod split_tests;

#[cfg(test)]
mod finance_tests;

#[cfg(test)]
mod growth_tests;

#[cfg(test)]
mod statistics_tests;

pub use catalog::{Catalog, CurrencyInfo, FullCatalog, NameCatalog, UnitInfo};
pub use context::{CalcContext, ExchangeRates};
pub use currency::{CurrencyCatalog, CurrencyDef};
pub use engine::{evaluate, evaluate_with_context, swap_units_query, CalcOutcome};
pub use formatter::Formatter;
pub use math::Math;
pub use measure::Measurement;
pub use parser::Parser;
pub use result::{CalcResult, Payload};
pub use token::{NumberBase, Operator, Token};
pub use tokenizer::tokenize;
pub use units::{UnitCatalog, UnitCategory, UnitDef};
pub use value::{ParseError, UnsupportedArea, Value};
