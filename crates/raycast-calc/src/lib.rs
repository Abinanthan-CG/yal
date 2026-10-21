//! The calculator: expressions, units, currencies and dates.
//!
//! The calculator answers a query directly. It never opens a window, never
//! touches the network and never reads user data, so it is a pure function
//! from a query string to a value.
//!
//! ```text
//! 1 + 2 * 3            arithmetic
//! 50%                  percentage
//! 2 ^ 10               power
//! 10 km in miles       unit conversion
//! 100 c in f           temperature conversion
//! 100 usd in eur       currency conversion
//! now + 3 days         date arithmetic
//! ```

pub mod currency;
pub mod eval;
pub mod parser;
pub mod units;

pub use currency::{CurrencyDef, CurrencyRegistry, RATES_AS_OF};
pub use eval::{EvalError, Evaluator, Value, evaluate, format, format_number};
pub use parser::{BinOp, Expr, ParseError, UnaryOp, parse};
pub use units::{Dimension, UnitDef, UnitRegistry};
