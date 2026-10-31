//! The calculator: expressions, units, currencies and dates.
//!
//! The calculator answers a query directly and locally: it opens no
//! window, touches no network and reads no user data — a pure function
//! from a query string to a value. Its currency table may carry rows the
//! store fetched by an explicit act (`yal rates-update`); the fetching
//! itself lives elsewhere and the calculator only ever answers from
//! bundled and stored data.
//!
//! ```text
//! 1 + 2 * 3            arithmetic
//! 50%                  percentage
//! 20% of 250           percent-of sugar
//! 2 ^ 10               power
//! 10 km in miles       unit conversion
//! 100 c in f           temperature conversion
//! 100 usd in eur       currency conversion
//! now + 3 days         date arithmetic
//! 10 days from now     natural-language date
//! in 2 weeks           natural-language date
//! next friday          natural-language weekday
//! ```

pub mod currency;
pub mod eval;
pub mod nl;
pub mod parser;
pub mod units;

pub use currency::{CurrencyDef, CurrencyRegistry, RatesTable, RATES_AS_OF};
pub use eval::{
    EvalError, Evaluator, Value, evaluate, evaluate_smart, evaluate_smart_at, evaluate_smart_with,
    format, format_number,
};
pub use parser::{BinOp, Expr, ParseError, UnaryOp, parse};
pub use units::{Dimension, UnitDef, UnitRegistry};
