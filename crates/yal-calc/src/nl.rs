//! The natural-language pre-parser.
//!
//! A small set of English shapes becomes ordinary expression trees the
//! grammar already evaluates, so asking costs no second engine to render
//! or reconcile — and the phrase layer stays cheap enough to mind the
//! keystroke. A shape that does not match falls through to the ordinary
//! grammar, so plain math is never in the way.
//!
//! The phrase shapes are adapted from [Numen], the built-in calculator
//! backend of [Vicinae]. Numen is BSD-3-Clause, whose notice rides along
//! because this file ports its idea in more than spirit:
//!
//! > Copyright (c) 2026, Aurélien Brabant. Redistribution and use in
//! > source and binary forms, with or without modification, are
//! > permitted provided that the conditions of that license are met.
//!
//! [Numen]: https://github.com/vicinaehq/numen
//! [Vicinae]: https://github.com/vicinaehq/vicinae
//!
//! ## The surface
//!
//! ```text
//! 10 days from now        now + 10 days
//! in 2 weeks              now + 2 weeks
//! 90 minutes ago          now − 90 minutes
//! tomorrow / yesterday    now ± 1 day
//! next friday             the next Friday, never today
//! last monday             the previous Monday, never today
//! 20 percent of 250       20% of 250 (the grammar's own sugar)
//! ```
//!
//! Months and years are read as **calendar** spans (`add_month`,
//! `add_year`), not fixed factors — one month past the 31st of January
//! is the last day of February. Singular and plural both resolve, and a
//! final full stop is ignored. A unit that is not a duration ("10 km
//! from now") does not match, so the ordinary grammar's error answers.

use crate::parser::{BinOp, Expr};
use crate::units::{Dimension, UnitDef, UnitRegistry};

/// Turn an English phrase into an expression, or fall through when the
/// words do not form one of the shapes above.
pub fn parse(input: &str) -> Option<Expr> {
    let words: Vec<&str> = input
        .split_whitespace()
        .map(|word| word.trim_end_matches(['.', ',', '!', '?']))
        .filter(|word| !word.is_empty())
        .collect();

    // `<N> percent of <expr>` is spelled for the grammar's sugar.
    if let [count, percent, "of", rest @ ..] = words.as_slice() {
        if *percent == "percent"
            && count.parse::<f64>().is_ok()
            && !rest.is_empty()
        {
            let text = format!("{count}% of {}", rest.join(" "));
            return crate::parser::parse(&text).ok();
        }
    }

    match words.as_slice() {
        ["tomorrow"] => span("1", "day", true),
        ["yesterday"] => span("1", "day", false),
        [count, word, "from", "now"] => span(count, word, true),
        [count, word, "ago"] => span(count, word, false),
        ["in", count, word] => span(count, word, true),
        ["next", weekday] => weekday_shift(weekday, true),
        ["last", weekday] => weekday_shift(weekday, false),
        _ => None,
    }
}

/// `<N> <unit>` relative to now, forwards or backwards.
///
/// The count parses as a number and the word resolves as a *duration*
/// — anything else leaves the phrase unmatched, and the ordinary
/// grammar gets its turn. Months and years are the calendar spans.
fn span(count: &str, word: &str, ahead: bool) -> Option<Expr> {
    let amount = count.parse::<f64>().ok()?;
    let now = Expr::Ident("now".into());
    let unit = resolve_time_unit(word)?;

    let signed = if ahead { amount } else { -amount };
    Some(match unit.id {
        "mo" | "yr" => Expr::Call {
            name: if unit.id == "mo" { "add_month".into() } else { "add_year".into() },
            args: vec![now, Expr::Number(signed)],
        },
        _ => {
            let step = Expr::Quantity { amount, unit };
            Expr::Binary {
                op: if ahead { BinOp::Add } else { BinOp::Sub },
                lhs: Box::new(now),
                rhs: Box::new(step),
            }
        }
    })
}

/// Resolve the unit word as a duration, tolerating a plural the alias
/// table missed ("yrs").
fn resolve_time_unit(word: &str) -> Option<&'static UnitDef> {
    let registry = UnitRegistry::new();
    let unit = registry
        .resolve(word)
        .or_else(|| registry.resolve(word.strip_suffix('s')?))?;
    (unit.dimension == Dimension::Time).then_some(unit)
}

/// `next <weekday>` / `last <weekday>`, as the evaluator reads them:
/// the function takes the weekday's ISO index, spelled.
fn weekday_shift(weekday: &str, ahead: bool) -> Option<Expr> {
    const DAYS: [&str; 7] = [
        "monday",
        "tuesday",
        "wednesday",
        "thursday",
        "friday",
        "saturday",
        "sunday",
    ];
    if !DAYS.contains(&weekday) {
        return None;
    }
    Some(Expr::Call {
        name: if ahead { "next" } else { "last" }.into(),
        args: vec![Expr::Ident(weekday.into())],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse as parse_expr;

    /// The phrase and its expression form must shape the same tree.
    fn same_as(phrase: &str, expression: &str) {
        assert_eq!(
            parse(phrase),
            parse_expr(expression).ok(),
            "`{phrase}` should shape as `{expression}`"
        );
    }

    #[test]
    fn shapes_the_duration_phrases() {
        same_as("10 days from now", "now + 10 days");
        same_as("in 2 weeks", "now + 2 weeks");
        same_as("90 minutes ago", "now - 90 minutes");
        same_as("3 hours ago", "now - 3 hours");
    }

    #[test]
    fn shapes_the_day_phrases() {
        same_as("tomorrow", "now + 1 day");
        same_as("yesterday", "now - 1 day");
    }

    #[test]
    fn months_and_years_move_the_calendar() {
        match parse("in 2 months").unwrap() {
            Expr::Call { name, args } => {
                assert_eq!(name, "add_month");
                assert!(matches!(args[1], Expr::Number(n) if (n - 2.0).abs() < 1e-9));
            }
            other => panic!("expected a calendar call, got {other:?}"),
        }
        match parse("1 year ago").unwrap() {
            Expr::Call { name, args } => {
                assert_eq!(name, "add_year");
                assert!(matches!(args[1], Expr::Number(n) if (n + 1.0).abs() < 1e-9));
            }
            other => panic!("expected a calendar call, got {other:?}"),
        }
    }

    #[test]
    fn shapes_the_weekday_phrases() {
        match parse("next friday").unwrap() {
            Expr::Call { name, args } => {
                assert_eq!(name, "next");
                assert!(matches!(args[0], Expr::Ident(ref word) if word == "friday"));
            }
            other => panic!("expected a weekday call, got {other:?}"),
        }
        match parse("last monday").unwrap() {
            Expr::Call { name, .. } => assert_eq!(name, "last"),
            other => panic!("expected a weekday call, got {other:?}"),
        }
    }

    #[test]
    fn percent_of_spells_for_the_sugar() {
        same_as("20 percent of 250", "20% of 250");
    }

    #[test]
    fn tolerates_a_final_full_stop() {
        same_as("tomorrow.", "now + 1 day");
    }

    #[test]
    fn a_non_time_unit_does_not_form_a_date() {
        // Not a date phrase, so the ordinary grammar gets its turn —
        // and its turn will say what was actually wrong.
        assert!(parse("10 km from now").is_none());
    }

    #[test]
    fn ordinary_words_fall_through() {
        assert!(parse("hello there").is_none());
        assert!(parse("10 things from now").is_none());
        assert!(parse("next snack break").is_none());
    }
}
