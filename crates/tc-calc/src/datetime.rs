//! Clean-room calendar, date math, workdays, ISO week numbers, holidays,
//! clock spans, human timespans, Unix timestamps, and ISO 8601 evaluator.

use jiff::civil::{Date, ISOWeekDate, Time, Weekday};
use jiff::{Timestamp, ToSpan, Zoned};

use crate::context::CalcContext;
use crate::formatter::Formatter;
use crate::result::{CalcResult, Payload};

/// Evaluate date, calendar, workday, holiday, clock-span, timespan, or
/// timestamp expressions against `ctx`.
pub fn evaluate(query: &str, ctx: &CalcContext) -> Option<CalcResult> {
    let raw = query.trim().trim_end_matches('?').trim();
    if raw.is_empty() || raw.len() > 200 {
        return None;
    }
    let lower = collapse_ws(raw).to_ascii_lowercase();
    let local = ctx.local_now();

    // 1. Single named moments & Unix/ISO conversions of now/today.
    if let Some(card) = eval_named_moment(&lower, raw, &local, ctx) {
        return Some(card);
    }

    // 2. ISO 8601 timestamp string (e.g. `2024-03-15T14:30:00Z`).
    if let Some(card) = eval_iso8601(raw, ctx) {
        return Some(card);
    }

    // 3. Unix timestamp conversions & ranges.
    if let Some(card) = eval_unix_timestamp(&lower, raw, ctx) {
        return Some(card);
    }

    // 4. Clock spans (`10pm to 2am`, `9:30am to 5:15pm`) and clock arithmetic (`3:45pm + 5`).
    if let Some(card) = eval_clock_span(&lower, raw) {
        return Some(card);
    }
    if let Some(card) = eval_clock_math(&lower, raw, ctx) {
        return Some(card);
    }

    // 5. Timespan decomposition (`145 mins to timespan`, `90 seconds to mixed`, `100 months in years and months`).
    if let Some(card) = eval_timespan_phrase(&lower, raw) {
        return Some(card);
    }

    // 6. ISO week lookup (`week 44 2026`, `what week is it`, `current week`).
    if let Some(card) = eval_week_query(&lower, raw, &local) {
        return Some(card);
    }

    // 7. Leap year, days in month/year, workdays/workhours in period, remaining in period.
    if let Some(card) = eval_calendar_facts(&lower, raw, &local) {
        return Some(card);
    }

    // 8. Countdowns & differences (`days until 25 Dec`, `days since march`, `days between ...`).
    if let Some(card) = eval_date_distance(&lower, raw, &local) {
        return Some(card);
    }

    // 9. Holidays (`when is christmas`, `when is thanksgiving 2027`).
    if let Some(card) = eval_when_holiday(&lower, raw, &local) {
        return Some(card);
    }

    // 10. Weekday queries (`what day is 2026-12-25`, `monday in 3 weeks`, `next friday`, `last monday`).
    if let Some(card) = eval_weekday_query(&lower, raw, &local) {
        return Some(card);
    }

    // 11. Relative offsets (`in 3 days`, `35 days ago`, `5 business days from today`).
    if let Some(card) = eval_relative_date(&lower, raw, &local, ctx) {
        return Some(card);
    }

    // 12. Explicit date arithmetic (`August 5 + 5`, `2026-12-25 - 10 days`, `now + 3 hours`, `today + 2 weeks - 3 days`).
    if let Some(card) = eval_date_arithmetic(&lower, raw, &local, ctx) {
        return Some(card);
    }

    None
}

fn unchained(mut card: CalcResult) -> CalcResult {
    card.can_chain = false;
    card
}

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn eval_named_moment(
    lower: &str,
    raw: &str,
    local: &Zoned,
    ctx: &CalcContext,
) -> Option<CalcResult> {
    match lower {
        "now" | "time" => {
            let text = format_time(local.time(), ctx.use_24h_clock);
            Some(unchained(CalcResult::with_badges(
                raw.to_string(),
                "Now",
                "Time",
                Payload::text(&text),
            )))
        }
        "today" => Some(date_card(raw, "Today", local.date())),
        "tomorrow" => Some(date_card(raw, "Tomorrow", local.date().tomorrow().ok()?)),
        "yesterday" => Some(date_card(raw, "Yesterday", local.date().yesterday().ok()?)),
        "end of month" | "end of this month" => {
            let d = local.date();
            let last = Date::new(d.year(), d.month(), d.days_in_month()).ok()?;
            Some(date_card(raw, "End of Month", last))
        }
        "end of year" | "end of this year" => {
            let last = Date::new(local.date().year(), 12, 31).ok()?;
            Some(date_card(raw, "End of Year", last))
        }
        "start of next month" | "next month" => {
            let next = local.date().checked_add(1.months()).ok()?;
            let first = Date::new(next.year(), next.month(), 1).ok()?;
            Some(date_card(raw, "Next Month", first))
        }
        "now as unix" | "now to unix" | "now in unix" | "unix now" | "timestamp now"
        | "unix timestamp" | "today as unix" | "today to unix" => {
            let secs = ctx.now_utc.as_second().to_string();
            Some(unchained(CalcResult::with_badges(
                raw.to_string(),
                "Now",
                "Unix Timestamp",
                Payload::text(&secs),
            )))
        }
        "now as iso" | "now to iso" | "now in iso" | "iso now" => {
            let iso = local.timestamp().to_string();
            Some(unchained(CalcResult::with_badges(
                raw.to_string(),
                "Now",
                "ISO 8601",
                Payload::text(&iso),
            )))
        }
        _ => None,
    }
}

fn eval_iso8601(raw: &str, ctx: &CalcContext) -> Option<CalcResult> {
    if !raw.contains('T') || raw.contains(' ') {
        return None;
    }
    let ts: Timestamp = raw.parse().ok()?;
    let zoned = ts.to_zoned(ctx.local_tz.clone());
    let dt = zoned.datetime();
    let offset_secs = zoned.offset().seconds();
    let sign = if offset_secs >= 0 { '+' } else { '-' };
    let abs = offset_secs.abs();
    let oh = abs / 3600;
    let om = (abs % 3600) / 60;
    let display = format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}{}{:02}:{:02}",
        dt.year(),
        dt.month(),
        dt.day(),
        dt.hour(),
        dt.minute(),
        dt.second(),
        sign,
        oh,
        om
    );
    Some(unchained(CalcResult::with_badges(
        raw.to_string(),
        "ISO 8601",
        "Local Time",
        Payload::text(&display),
    )))
}

fn eval_unix_timestamp(lower: &str, raw: &str, ctx: &CalcContext) -> Option<CalcResult> {
    // Range: `from 1700000000 to 1700003600` or `1700000000 to 1700003600`
    let stripped = lower.strip_prefix("from ").unwrap_or(lower);
    if let Some((a, b)) = stripped.split_once(" to ") {
        if let (Some(t1), Some(t2)) = (parse_unix_secs(a.trim()), parse_unix_secs(b.trim())) {
            let diff = (t2 - t1).abs();
            let text = format_timespan_seconds(diff as f64);
            return Some(unchained(CalcResult::with_badges(
                raw.to_string(),
                "Unix Range",
                "Duration",
                Payload::text(&text),
            )));
        }
    }

    // Single unix timestamp conversion: `unix 1700000000`, `timestamp 1700000000`, `1700000000 to date`
    let candidate = lower
        .strip_prefix("unix ")
        .or_else(|| lower.strip_prefix("timestamp "))
        .or_else(|| lower.strip_suffix(" to date"))
        .or_else(|| lower.strip_suffix(" as date"))
        .or_else(|| lower.strip_suffix(" in date"))
        .or_else(|| lower.strip_suffix(" to datetime"))
        .or_else(|| lower.strip_suffix(" as datetime"))?;

    let secs = parse_unix_secs(candidate.trim())?;
    let ts = Timestamp::from_second(secs).ok()?;
    let zoned = ts.to_zoned(ctx.local_tz.clone());
    let text = format!(
        "{}, {}",
        format_date_full(zoned.date()),
        format_time(zoned.time(), ctx.use_24h_clock)
    );
    Some(unchained(CalcResult::with_badges(
        raw.to_string(),
        "Unix Timestamp",
        weekday_name(zoned.date().weekday()),
        Payload::text(&text),
    )))
}

fn parse_unix_secs(s: &str) -> Option<i64> {
    if !(9..=13).contains(&s.len()) || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let v = s.parse::<i64>().ok()?;
    if s.len() >= 12 {
        Some(v / 1000)
    } else {
        Some(v)
    }
}

fn eval_clock_span(lower: &str, raw: &str) -> Option<CalcResult> {
    let body = lower.strip_prefix("from ").unwrap_or(lower);
    let (lhs, rhs) = body.split_once(" to ").or_else(|| body.split_once(" - "))?;
    let t1 = parse_clock_time(lhs.trim())?;
    let t2 = parse_clock_time(rhs.trim())?;
    let s1 = t1.hour() as i64 * 3600 + t1.minute() as i64 * 60 + t1.second() as i64;
    let s2 = t2.hour() as i64 * 3600 + t2.minute() as i64 * 60 + t2.second() as i64;
    let diff = if s2 > s1 {
        s2 - s1
    } else {
        24 * 3600 - s1 + s2
    };
    let text = format_timespan_seconds(diff as f64);
    Some(unchained(CalcResult::with_badges(
        raw.to_string(),
        "Clock Span",
        "Duration",
        Payload::text(&text),
    )))
}

fn eval_clock_math(lower: &str, raw: &str, ctx: &CalcContext) -> Option<CalcResult> {
    let (lhs, sign, rhs) = split_plus_minus(lower)?;
    let base = parse_clock_time(lhs.trim())?;
    let (amount, unit_secs) = parse_clock_offset(rhs.trim())?;
    let base_secs = base.hour() as i64 * 3600 + base.minute() as i64 * 60 + base.second() as i64;
    let delta = (amount * unit_secs).round() as i64 * sign;
    let wrapped = (base_secs + delta).rem_euclid(24 * 3600);
    let h = (wrapped / 3600) as i8;
    let m = ((wrapped % 3600) / 60) as i8;
    let s = (wrapped % 60) as i8;
    let result = Time::new(h, m, s, 0).ok()?;
    let text = format_time(result, ctx.use_24h_clock);
    Some(unchained(CalcResult::with_badges(
        raw.to_string(),
        "Time",
        "Clock",
        Payload::text(&text),
    )))
}

fn parse_clock_offset(s: &str) -> Option<(f64, f64)> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    match parts.as_slice() {
        // Bare number on a clock time defaults to hours (`3:45pm + 5` -> `8:45 PM`).
        [num] => {
            if let Ok(v) = num.parse::<f64>() {
                return Some((v, 3600.0));
            }
            let (v, u) = split_attached_number_unit(num)?;
            Some((v, time_unit_seconds(u)?))
        }
        [num, unit] => {
            let v = num.parse::<f64>().ok()?;
            Some((v, time_unit_seconds(unit)?))
        }
        _ => None,
    }
}

fn eval_timespan_phrase(lower: &str, raw: &str) -> Option<CalcResult> {
    for connector in [" in ", " to ", " as "] {
        if let Some((lhs, rhs)) = lower.split_once(connector) {
            let rhs = rhs.trim();
            if matches!(rhs, "timespan" | "mixed" | "duration" | "human") {
                let secs = parse_duration_seconds(lhs.trim())?;
                let text = format_timespan_seconds(secs);
                return Some(unchained(CalcResult::with_badges(
                    raw.to_string(),
                    "Duration",
                    "Timespan",
                    Payload::text(&text),
                )));
            }
        }
    }
    None
}

fn eval_week_query(lower: &str, raw: &str, local: &Zoned) -> Option<CalcResult> {
    if matches!(
        lower,
        "what week is it" | "current week" | "week number" | "this week"
    ) {
        let iso = local.date().iso_week_date();
        let text = format!("Week {}, {}", iso.week(), iso.year());
        return Some(unchained(CalcResult::with_badges(
            raw.to_string(),
            "Calendar",
            "ISO Week",
            Payload::text(&text),
        )));
    }

    if let Some(rest) = lower.strip_prefix("week ") {
        let parts: Vec<&str> = rest.split_whitespace().collect();
        let (week, year) = match parts.as_slice() {
            [w] => (w.parse::<i8>().ok()?, local.date().year()),
            [w, y] | [w, "of", y] | [w, "in", y] => (w.parse::<i8>().ok()?, y.parse::<i16>().ok()?),
            _ => return None,
        };
        if !(1..=53).contains(&week) {
            return None;
        }
        let mon = ISOWeekDate::new(year, week, Weekday::Monday).ok()?.date();
        let sun = ISOWeekDate::new(year, week, Weekday::Sunday).ok()?.date();
        let text = format_week_range(mon, sun);
        return Some(unchained(CalcResult::with_badges(
            raw.to_string(),
            "ISO Week",
            &format!("Week {week}"),
            Payload::text(&text),
        )));
    }
    None
}

fn eval_calendar_facts(lower: &str, raw: &str, local: &Zoned) -> Option<CalcResult> {
    let today = local.date();

    // `is 2028 a leap year` / `is 2028 leap year`
    if let Some(rest) = lower.strip_prefix("is ") {
        if let Some(year_str) = rest
            .strip_suffix(" a leap year")
            .or_else(|| rest.strip_suffix(" leap year"))
        {
            let year = year_str.trim().parse::<i16>().ok()?;
            let is_leap = Date::new(year, 1, 1).ok()?.in_leap_year();
            return Some(unchained(CalcResult::with_badges(
                raw.to_string(),
                "Calendar",
                "Boolean",
                Payload::text(if is_leap { "true" } else { "false" }),
            )));
        }
    }

    // `hours left in today` / `hours left today`
    if matches!(lower, "hours left in today" | "hours left today") {
        let t = local.time();
        let secs_elapsed = t.hour() as f64 * 3600.0 + t.minute() as f64 * 60.0 + t.second() as f64;
        let rem_hours = ((86_400.0 - secs_elapsed) / 3600.0).max(0.0);
        let text = format!(
            "{} hours",
            Formatter::display((rem_hours * 100.0).round() / 100.0)
        );
        return Some(unchained(CalcResult::with_badges(
            raw.to_string(),
            "Today",
            "Remaining",
            Payload::text(&text),
        )));
    }

    // `days left in this month/year/week/quarter` and `workdays left in ...`
    for (prefix, workdays_only) in [
        ("days left in ", false),
        ("workdays left in ", true),
        ("business days left in ", true),
    ] {
        if let Some(period) = lower.strip_prefix(prefix) {
            let period = period.trim().strip_prefix("this ").unwrap_or(period.trim());
            let end = match period {
                "month" => Date::new(today.year(), today.month(), today.days_in_month()).ok()?,
                "year" => Date::new(today.year(), 12, 31).ok()?,
                "week" => {
                    let offset = 7 - today.weekday().to_monday_one_offset() as i64;
                    today.checked_add(offset.days()).ok()?
                }
                "quarter" => {
                    let q_end_month = ((today.month() - 1) / 3 + 1) * 3;
                    let d = Date::new(today.year(), q_end_month, 1).ok()?;
                    Date::new(today.year(), q_end_month, d.days_in_month()).ok()?
                }
                _ => return None,
            };
            let count = if workdays_only {
                count_workdays_exclusive_start(today, end)
            } else {
                today.until(end).ok()?.get_days() as i64
            };
            let unit = if workdays_only {
                if count == 1 {
                    "workday"
                } else {
                    "workdays"
                }
            } else if count == 1 {
                "day"
            } else {
                "days"
            };
            let text = format!("{} {unit}", Formatter::grouped(&count.to_string()));
            return Some(unchained(CalcResult::with_badges(
                raw.to_string(),
                "Remaining",
                if workdays_only { "Workdays" } else { "Days" },
                Payload::text(&text),
            )));
        }
    }

    // `workdays in <year|month [year]>`, `workhours in <year|month [year]>`, `days in <year|month [year]>`
    for (prefix, kind) in [
        ("workdays in ", PeriodMetric::Workdays),
        ("business days in ", PeriodMetric::Workdays),
        ("workhours in ", PeriodMetric::Workhours),
        ("work hours in ", PeriodMetric::Workhours),
        ("days in ", PeriodMetric::Days),
    ] {
        if let Some(rest) = lower.strip_prefix(prefix) {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            let (start, end) = match parts.as_slice() {
                [y] => {
                    if let Ok(year) = y.parse::<i16>() {
                        (Date::new(year, 1, 1).ok()?, Date::new(year, 12, 31).ok()?)
                    } else {
                        let month = parse_month(y)?;
                        let first = Date::new(today.year(), month, 1).ok()?;
                        let last = Date::new(today.year(), month, first.days_in_month()).ok()?;
                        (first, last)
                    }
                }
                [m, y] => {
                    let month = parse_month(m)?;
                    let year = y.parse::<i16>().ok()?;
                    let first = Date::new(year, month, 1).ok()?;
                    let last = Date::new(year, month, first.days_in_month()).ok()?;
                    (first, last)
                }
                _ => return None,
            };
            let (count, unit, badge) = match kind {
                PeriodMetric::Days => {
                    let n = start.until(end).ok()?.get_days() as i64 + 1;
                    (n, "days", "Days")
                }
                PeriodMetric::Workdays => {
                    let n = count_workdays_inclusive(start, end);
                    (n, "workdays", "Workdays")
                }
                PeriodMetric::Workhours => {
                    let n = count_workdays_inclusive(start, end) * 8;
                    (n, "hours", "Workhours")
                }
            };
            let text = format!("{} {unit}", Formatter::grouped(&count.to_string()));
            return Some(unchained(CalcResult::with_badges(
                raw.to_string(),
                "Calendar",
                badge,
                Payload::text(&text),
            )));
        }
    }

    None
}

#[derive(Clone, Copy)]
enum PeriodMetric {
    Days,
    Workdays,
    Workhours,
}

fn eval_date_distance(lower: &str, raw: &str, local: &Zoned) -> Option<CalcResult> {
    let today = local.date();

    // `<unit> between <d1> and <d2>`
    for (prefix, unit) in [
        ("days between ", DistUnit::Days),
        ("weeks between ", DistUnit::Weeks),
        ("months between ", DistUnit::Months),
        ("years between ", DistUnit::Years),
        ("workdays between ", DistUnit::Workdays),
        ("business days between ", DistUnit::Workdays),
    ] {
        if let Some(rest) = lower.strip_prefix(prefix) {
            if let Some((a, b)) = rest.split_once(" and ") {
                let d1 = parse_date_expr(a.trim(), today, Bias::Nearest)?;
                let d2 = parse_date_expr(b.trim(), today, Bias::Nearest)?;
                let (lo, hi) = if d1 <= d2 { (d1, d2) } else { (d2, d1) };
                return Some(format_distance_card(raw, lo, hi, unit));
            }
        }
    }

    // Countdowns: `<unit> (until|till|til|'til|to|before|left until|left till|...) <target>`
    // Also supports `how long until/till <target>` and `time until/till <target>`,
    // and clock-time targets like `hours till 6pm` or `minutes until 11:30pm`!
    if let Some((lhs_unit, target_str)) = split_countdown_clause(lower) {
        let unit = parse_dist_unit(lhs_unit)?;

        // Clock-time target (`hours till 6pm`, `minutes until 11:30pm`, `time till 5pm`).
        if let Some(clock) = parse_clock_time(target_str) {
            let now_t = local.time();
            let s1 =
                now_t.hour() as i64 * 3600 + now_t.minute() as i64 * 60 + now_t.second() as i64;
            let s2 =
                clock.hour() as i64 * 3600 + clock.minute() as i64 * 60 + clock.second() as i64;
            let diff_secs = if s2 >= s1 { s2 - s1 } else { 86_400 - s1 + s2 };
            return Some(format_clock_distance_card(raw, diff_secs, unit));
        }

        let target = parse_date_expr(target_str, today, Bias::Future)?;
        return Some(format_distance_card(raw, today, target, unit));
    }

    // Elapsed: `<unit> since <target>`
    if let Some((lhs_unit, target_str)) = lower.split_once(" since ") {
        let unit = parse_dist_unit(lhs_unit.trim())?;
        if let Some(clock) = parse_clock_time(target_str.trim()) {
            let now_t = local.time();
            let s1 =
                clock.hour() as i64 * 3600 + clock.minute() as i64 * 60 + clock.second() as i64;
            let s2 =
                now_t.hour() as i64 * 3600 + now_t.minute() as i64 * 60 + now_t.second() as i64;
            let diff_secs = if s2 >= s1 { s2 - s1 } else { 86_400 - s1 + s2 };
            return Some(format_clock_distance_card(raw, diff_secs, unit));
        }
        let target = parse_date_expr(target_str.trim(), today, Bias::Past)?;
        return Some(format_distance_card(raw, target, today, unit));
    }

    None
}

fn split_countdown_clause(lower: &str) -> Option<(&str, &str)> {
    const CONNECTORS: &[&str] = &[
        " remaining until ",
        " remaining till ",
        " remaining til ",
        " left until ",
        " left till ",
        " left til ",
        " left 'til ",
        " left to ",
        " until ",
        " till ",
        " til ",
        " 'til ",
        " before ",
        " to ",
    ];
    for conn in CONNECTORS {
        if let Some((lhs, rhs)) = lower.split_once(conn) {
            let l = lhs.trim();
            let r = rhs.trim();
            if !l.is_empty() && !r.is_empty() {
                return Some((l, r));
            }
        }
    }
    None
}

fn parse_dist_unit(s: &str) -> Option<DistUnit> {
    Some(match s {
        "day" | "days" | "how long" | "time" | "time left" => DistUnit::Days,
        "week" | "weeks" => DistUnit::Weeks,
        "month" | "months" => DistUnit::Months,
        "year" | "years" => DistUnit::Years,
        "workday" | "workdays" | "business day" | "business days" => DistUnit::Workdays,
        "hour" | "hours" | "hr" | "hrs" => DistUnit::Hours,
        "minute" | "minutes" | "min" | "mins" => DistUnit::Minutes,
        "second" | "seconds" | "sec" | "secs" => DistUnit::Seconds,
        _ => return None,
    })
}

fn format_clock_distance_card(raw: &str, diff_secs: i64, unit: DistUnit) -> CalcResult {
    let (display, badge) = match unit {
        DistUnit::Hours => {
            let h = (diff_secs as f64 / 3600.0 * 100.0).round() / 100.0;
            (format!("{} hours", Formatter::display(h)), "Hours")
        }
        DistUnit::Minutes => {
            let m = (diff_secs as f64 / 60.0 * 100.0).round() / 100.0;
            (format!("{} minutes", Formatter::display(m)), "Minutes")
        }
        DistUnit::Seconds => (
            format!("{} seconds", Formatter::grouped(&diff_secs.to_string())),
            "Seconds",
        ),
        _ => (format_timespan_seconds(diff_secs as f64), "Duration"),
    };
    unchained(CalcResult::with_badges(
        raw.to_string(),
        "Expression",
        badge,
        Payload::text(&display),
    ))
}

#[derive(Clone, Copy)]
enum DistUnit {
    Days,
    Weeks,
    Months,
    Years,
    Workdays,
    Hours,
    Minutes,
    Seconds,
}

fn format_distance_card(raw: &str, start: Date, end: Date, unit: DistUnit) -> CalcResult {
    let days = start.until(end).map(|s| s.get_days() as i64).unwrap_or(0);
    let (display, badge) = match unit {
        DistUnit::Days => (
            format!(
                "{} {}",
                Formatter::grouped(&days.to_string()),
                if days.abs() == 1 { "day" } else { "days" }
            ),
            "Days",
        ),
        DistUnit::Weeks => {
            let w = days as f64 / 7.0;
            (
                format!("{} weeks", Formatter::display((w * 100.0).round() / 100.0)),
                "Weeks",
            )
        }
        DistUnit::Months => {
            let m = days as f64 / 30.436875;
            (
                format!("{} months", Formatter::display((m * 100.0).round() / 100.0)),
                "Months",
            )
        }
        DistUnit::Years => {
            let y = days as f64 / 365.2425;
            (
                format!("{} years", Formatter::display((y * 100.0).round() / 100.0)),
                "Years",
            )
        }
        DistUnit::Workdays => {
            let wd = if start <= end {
                count_workdays_exclusive_start(start, end)
            } else {
                -count_workdays_exclusive_start(end, start)
            };
            (
                format!(
                    "{} {}",
                    Formatter::grouped(&wd.to_string()),
                    if wd.abs() == 1 { "workday" } else { "workdays" }
                ),
                "Workdays",
            )
        }
        DistUnit::Hours => {
            let h = days * 24;
            (
                format!("{} hours", Formatter::grouped(&h.to_string())),
                "Hours",
            )
        }
        DistUnit::Minutes => {
            let m = days * 24 * 60;
            (
                format!("{} minutes", Formatter::grouped(&m.to_string())),
                "Minutes",
            )
        }
        DistUnit::Seconds => {
            let s = days * 86_400;
            (
                format!("{} seconds", Formatter::grouped(&s.to_string())),
                "Seconds",
            )
        }
    };
    unchained(CalcResult::with_badges(
        raw.to_string(),
        "Expression",
        badge,
        Payload::text(&display),
    ))
}

fn eval_when_holiday(lower: &str, raw: &str, local: &Zoned) -> Option<CalcResult> {
    let rest = lower.strip_prefix("when is ")?;
    let parts: Vec<&str> = rest.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }
    let today = local.date();
    let (holiday_str, explicit_year) = if parts.len() >= 2 {
        if let Ok(y) = parts.last()?.parse::<i16>() {
            (parts[..parts.len() - 1].join(" "), Some(y))
        } else {
            (rest.trim().to_string(), None)
        }
    } else {
        (rest.trim().to_string(), None)
    };
    let date = if let Some(y) = explicit_year {
        holiday_date(&holiday_str, y)?
    } else {
        let this_year = holiday_date(&holiday_str, today.year())?;
        if this_year >= today {
            this_year
        } else {
            holiday_date(&holiday_str, today.year() + 1)?
        }
    };
    Some(date_card(raw, "Holiday", date))
}

fn eval_weekday_query(lower: &str, raw: &str, local: &Zoned) -> Option<CalcResult> {
    let today = local.date();

    // `what day is 2026-12-25` / `day of week 2026-12-25` / `weekday of ...`
    for prefix in [
        "what day is ",
        "what weekday is ",
        "day of week ",
        "weekday of ",
    ] {
        if let Some(rest) = lower.strip_prefix(prefix) {
            let date = parse_date_expr(rest.trim(), today, Bias::Nearest)?;
            let name = weekday_name(date.weekday());
            return Some(unchained(CalcResult::with_badges(
                raw.to_string(),
                &format_date_full(date),
                "Weekday",
                Payload::text(name),
            )));
        }
    }

    // `next friday`, `last monday`, `this wednesday`
    if let Some((kw, wd_str)) = lower.split_once(' ') {
        if let Some(target_wd) = parse_weekday(wd_str.trim()) {
            let cur = today.weekday().to_monday_zero_offset() as i64;
            let tgt = target_wd.to_monday_zero_offset() as i64;
            let delta = match kw {
                "next" => {
                    let d = (tgt - cur).rem_euclid(7);
                    if d == 0 {
                        7
                    } else {
                        d
                    }
                }
                "last" => {
                    let d = (cur - tgt).rem_euclid(7);
                    -(if d == 0 { 7 } else { d })
                }
                "this" => (tgt - cur).rem_euclid(7),
                _ => return None,
            };
            let date = today.checked_add(delta.days()).ok()?;
            return Some(date_card(raw, "Date", date));
        }
    }

    // `<weekday> in <n> weeks` (e.g., `monday in 3 weeks` -> Monday of the week `n` weeks ahead)
    if let Some((wd_str, rest)) = lower.split_once(" in ") {
        if let Some(target_wd) = parse_weekday(wd_str.trim()) {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() == 2 && matches!(parts[1], "week" | "weeks" | "wk" | "wks") {
                let n = parts[0].parse::<i64>().ok()?;
                // Anchor on the start (Monday) of the current ISO week, advance `n` weeks,
                // and land on `target_wd`. If `today` is Sunday (end of ISO week) and user
                // asks `monday in 3 weeks`, Monday of the ISO week 3 weeks from Monday-start
                // matches Raycast's calendar week (Sun Nov 22 -> Mon Dec 14 = +22 days = 3w + 1d).
                let cur_offset = today.weekday().to_monday_zero_offset() as i64;
                let tgt_offset = target_wd.to_monday_zero_offset() as i64;
                let next_occ = {
                    let d = (tgt_offset - cur_offset).rem_euclid(7);
                    if d == 0 {
                        7
                    } else {
                        d
                    }
                };
                // If target weekday is already ahead in the upcoming 7 days, n weeks = next_occ + (n - 1)*7 when cur is Sunday?
                // Let's check: Sun Nov 22 + 22 days = Mon Dec 14! Next Monday is Nov 23 (+1 day); + 3 weeks (21d) = Dec 14!
                let delta = if tgt_offset <= cur_offset {
                    next_occ + n * 7
                } else {
                    (tgt_offset - cur_offset) + n * 7
                };
                let date = today.checked_add(delta.days()).ok()?;
                let text = format!(
                    "{}, {}",
                    weekday_name(date.weekday()),
                    format_date_full(date)
                );
                return Some(unchained(CalcResult::with_badges(
                    raw.to_string(),
                    "Date",
                    weekday_name(date.weekday()),
                    Payload::text(&text),
                )));
            }
        }
    }

    None
}

fn eval_relative_date(
    lower: &str,
    raw: &str,
    local: &Zoned,
    ctx: &CalcContext,
) -> Option<CalcResult> {
    let today = local.date();

    // `in <n> <unit>`
    if let Some(rest) = lower.strip_prefix("in ") {
        let (n, unit) = parse_span_qty(rest.trim())?;
        return apply_relative(raw, local, n, unit, ctx);
    }

    // `<n> <unit> ago`
    if let Some(rest) = lower.strip_suffix(" ago") {
        let (n, unit) = parse_span_qty(rest.trim())?;
        return apply_relative(raw, local, -n, unit, ctx);
    }

    // `<n> <unit> from <anchor>`
    if let Some((lhs, anchor)) = lower.split_once(" from ") {
        let (n, unit) = parse_span_qty(lhs.trim())?;
        let anchor_str = anchor.trim();
        if matches!(anchor_str, "now" | "today") {
            return apply_relative(raw, local, n, unit, ctx);
        }
        let base_date = parse_date_expr(anchor_str, today, Bias::Nearest)?;
        let out = add_date_unit(base_date, n, unit)?;
        return Some(date_card(raw, "Date", out));
    }

    None
}

fn apply_relative(
    raw: &str,
    local: &Zoned,
    n: i64,
    unit: RelUnit,
    ctx: &CalcContext,
) -> Option<CalcResult> {
    match unit {
        RelUnit::Seconds | RelUnit::Minutes | RelUnit::Hours => {
            let shifted = match unit {
                RelUnit::Seconds => local.checked_add(n.seconds()).ok()?,
                RelUnit::Minutes => local.checked_add(n.minutes()).ok()?,
                RelUnit::Hours => local.checked_add(n.hours()).ok()?,
                _ => return None,
            };
            let text = if shifted.date() == local.date() {
                format_time(shifted.time(), ctx.use_24h_clock)
            } else {
                format!(
                    "{}, {}",
                    format_date_full(shifted.date()),
                    format_time(shifted.time(), ctx.use_24h_clock)
                )
            };
            Some(unchained(CalcResult::with_badges(
                raw.to_string(),
                "Time",
                weekday_name(shifted.date().weekday()),
                Payload::text(&text),
            )))
        }
        _ => {
            let date = add_date_unit(local.date(), n, unit)?;
            Some(date_card(raw, "Date", date))
        }
    }
}

fn eval_date_arithmetic(
    lower: &str,
    raw: &str,
    local: &Zoned,
    ctx: &CalcContext,
) -> Option<CalcResult> {
    let today = local.date();

    // Parse chained `<date> (+|-) <term> [(+|-) <term> ...]`
    let tokens = split_signed_terms(lower)?;
    if tokens.len() < 2 {
        return None;
    }
    let first = tokens[0].1.trim();
    // Guard against plain arithmetic like `2026 - 1990` or `5 + 5`.
    if first.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
        return None;
    }

    // `now + 3 hours`, `now - 45 min` -> shift live Zoned clock!
    if first == "now" && tokens.len() == 2 {
        let (sign, term) = tokens[1];
        if let Some((n, unit)) = parse_span_qty(term.trim()) {
            if matches!(unit, RelUnit::Seconds | RelUnit::Minutes | RelUnit::Hours) {
                return apply_relative(raw, local, sign * n, unit, ctx);
            }
        }
    }

    let mut current = parse_date_expr(first, today, Bias::Nearest)?;

    for &(sign, term) in &tokens[1..] {
        let trimmed = term.trim();
        let (n, unit) = if let Ok(bare_days) = trimmed.parse::<i64>() {
            // Bare number added to a date means days (`August 5 + 5` -> `August 10`).
            (bare_days, RelUnit::Days)
        } else {
            parse_span_qty(trimmed)?
        };
        current = add_date_unit(current, sign * n, unit)?;
    }

    Some(date_card(raw, "Date", current))
}

#[derive(Clone, Copy)]
enum RelUnit {
    Seconds,
    Minutes,
    Hours,
    Days,
    Workdays,
    Weeks,
    Months,
    Years,
}

fn parse_span_qty(s: &str) -> Option<(i64, RelUnit)> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    match parts.as_slice() {
        [num, u] => Some((num.parse::<i64>().ok()?, parse_rel_unit(u)?)),
        [num, "business", u] if matches!(*u, "day" | "days") => {
            Some((num.parse::<i64>().ok()?, RelUnit::Workdays))
        }
        [attached] => {
            let (v, u) = split_attached_number_unit(attached)?;
            if v.fract() != 0.0 {
                return None;
            }
            Some((v as i64, parse_rel_unit(u)?))
        }
        _ => None,
    }
}

fn parse_rel_unit(u: &str) -> Option<RelUnit> {
    Some(match u {
        "s" | "sec" | "secs" | "second" | "seconds" => RelUnit::Seconds,
        "min" | "mins" | "minute" | "minutes" => RelUnit::Minutes,
        "h" | "hr" | "hrs" | "hour" | "hours" => RelUnit::Hours,
        "d" | "day" | "days" => RelUnit::Days,
        "workday" | "workdays" | "weekday" | "weekdays" => RelUnit::Workdays,
        "w" | "wk" | "wks" | "week" | "weeks" => RelUnit::Weeks,
        "mo" | "mos" | "month" | "months" => RelUnit::Months,
        "y" | "yr" | "yrs" | "year" | "years" => RelUnit::Years,
        _ => return None,
    })
}

fn add_date_unit(base: Date, n: i64, unit: RelUnit) -> Option<Date> {
    match unit {
        RelUnit::Days => base.checked_add(n.days()).ok(),
        RelUnit::Weeks => base.checked_add((n * 7).days()).ok(),
        RelUnit::Months => base.checked_add(n.months()).ok(),
        RelUnit::Years => base.checked_add(n.years()).ok(),
        RelUnit::Workdays => Some(add_workdays(base, n)),
        _ => None,
    }
}

fn add_workdays(mut date: Date, n: i64) -> Date {
    let step = if n >= 0 { 1 } else { -1 };
    let mut remaining = n.abs();
    while remaining > 0 {
        date = if step > 0 {
            date.tomorrow().unwrap_or(date)
        } else {
            date.yesterday().unwrap_or(date)
        };
        if is_workday(date) {
            remaining -= 1;
        }
    }
    date
}

fn is_workday(date: Date) -> bool {
    !matches!(date.weekday(), Weekday::Saturday | Weekday::Sunday)
}

fn count_workdays_inclusive(start: Date, end: Date) -> i64 {
    if start > end {
        return 0;
    }
    let total_days = start.until(end).map(|s| s.get_days() as i64).unwrap_or(0) + 1;
    let full_weeks = total_days / 7;
    let rem = total_days % 7;
    let mut count = full_weeks * 5;
    let mut cur = start.checked_add((full_weeks * 7).days()).unwrap_or(end);
    for _ in 0..rem {
        if is_workday(cur) {
            count += 1;
        }
        cur = cur.tomorrow().unwrap_or(cur);
    }
    count
}

fn count_workdays_exclusive_start(start: Date, end: Date) -> i64 {
    if start >= end {
        return 0;
    }
    let Some(next) = start.tomorrow().ok() else {
        return 0;
    };
    count_workdays_inclusive(next, end)
}

#[derive(Clone, Copy)]
enum Bias {
    Future,
    Past,
    Nearest,
}

fn parse_date_expr(input: &str, today: Date, bias: Bias) -> Option<Date> {
    let s = input.trim().trim_end_matches(',').trim();
    match s {
        "today" | "now" => return Some(today),
        "tomorrow" => return today.tomorrow().ok(),
        "yesterday" => return today.yesterday().ok(),
        _ => {}
    }

    // Named holiday (`christmas`, `thanksgiving`, `easter`, etc.).
    if let Some(d) = holiday_date(s, today.year()) {
        return Some(match bias {
            Bias::Future if d < today => holiday_date(s, today.year() + 1)?,
            Bias::Past if d > today => holiday_date(s, today.year() - 1)?,
            _ => d,
        });
    }

    // Bare weekday (`days till friday`, `days since monday`).
    if let Some(target_wd) = parse_weekday(s) {
        let cur = today.weekday().to_monday_zero_offset() as i64;
        let tgt = target_wd.to_monday_zero_offset() as i64;
        let delta = match bias {
            Bias::Future | Bias::Nearest => {
                let d = (tgt - cur).rem_euclid(7);
                if d == 0 {
                    7
                } else {
                    d
                }
            }
            Bias::Past => {
                let d = (cur - tgt).rem_euclid(7);
                -(if d == 0 { 7 } else { d })
            }
        };
        return today.checked_add(delta.days()).ok();
    }

    // Bare month name (only used in `days until december` / `days since march`).
    if let Some(month) = parse_month(s) {
        let this_yr = Date::new(today.year(), month, 1).ok()?;
        return Some(match bias {
            Bias::Future if this_yr <= today => Date::new(today.year() + 1, month, 1).ok()?,
            Bias::Past if this_yr > today => Date::new(today.year() - 1, month, 1).ok()?,
            _ => this_yr,
        });
    }

    // ISO date `YYYY-MM-DD`, `YYYY/MM/DD`, or `DD-MM-YYYY` / `DD.MM.YYYY` / `DD/MM/YYYY` / `DD/MM`.
    if let Some((a, rest)) = s
        .split_once('-')
        .or_else(|| s.split_once('/'))
        .or_else(|| s.split_once('.'))
    {
        if let Some((b, c)) = rest
            .split_once('-')
            .or_else(|| rest.split_once('/'))
            .or_else(|| rest.split_once('.'))
        {
            if a.len() == 4 {
                return Date::new(a.parse().ok()?, b.parse().ok()?, c.parse().ok()?).ok();
            }
            if c.len() == 4 {
                // Day-first (`29.11.2026`, `29/11/2026`, `29-11-2026`), falling back to month-first if day > 12 on second slot.
                let n1 = a.parse::<i8>().ok()?;
                let n2 = b.parse::<i8>().ok()?;
                let year = c.parse::<i16>().ok()?;
                if (1..=12).contains(&n2) {
                    if let Ok(d) = Date::new(year, n2, n1) {
                        return Some(d);
                    }
                }
                if (1..=12).contains(&n1) {
                    return Date::new(year, n1, n2).ok();
                }
            }
        } else if s.contains('/') || s.contains('.') {
            // Yearless `DD/MM` or `DD.MM` (only reached inside explicit date queries like `days till 29/11`).
            let n1 = a.parse::<i8>().ok()?;
            let n2 = rest.parse::<i8>().ok()?;
            if (1..=12).contains(&n2) && (1..=31).contains(&n1) {
                return resolve_yearless_date(n2, n1, today, bias);
            }
            if (1..=12).contains(&n1) && (1..=31).contains(&n2) {
                return resolve_yearless_date(n1, n2, today, bias);
            }
        }
    }

    // Written month-day or day-month: `August 5`, `25 Dec`, `Dec 25 2026`, `25th of December 2026`.
    let cleaned: Vec<String> = s
        .replace(',', " ")
        .split_whitespace()
        .filter(|w| *w != "of" && *w != "the")
        .map(|w| w.to_string())
        .collect();

    if cleaned.len() == 2 || cleaned.len() == 3 {
        let (month, day, explicit_year) = if let Some(m) = parse_month(&cleaned[0]) {
            let d = parse_ordinal_day(&cleaned[1])?;
            let y = cleaned.get(2).and_then(|s| s.parse::<i16>().ok());
            if cleaned.len() == 3 && y.is_none() {
                return None;
            }
            (m, d, y)
        } else {
            let m = parse_month(&cleaned[1])?;
            let d = parse_ordinal_day(&cleaned[0])?;
            let y = cleaned.get(2).and_then(|s| s.parse::<i16>().ok());
            if cleaned.len() == 3 && y.is_none() {
                return None;
            }
            (m, d, y)
        };

        if let Some(y) = explicit_year {
            return Date::new(y, month, day).ok();
        }
        return resolve_yearless_date(month, day, today, bias);
    }

    None
}

fn resolve_yearless_date(month: i8, day: i8, today: Date, bias: Bias) -> Option<Date> {
    let base_y = today.year();
    let cur = Date::new(base_y, month, day).ok();
    match bias {
        Bias::Future => {
            if let Some(d) = cur {
                if d >= today {
                    return Some(d);
                }
            }
            Date::new(base_y + 1, month, day).ok()
        }
        Bias::Past => {
            if let Some(d) = cur {
                if d <= today {
                    return Some(d);
                }
            }
            Date::new(base_y - 1, month, day).ok()
        }
        Bias::Nearest => cur.or_else(|| Date::new(base_y + 1, month, day).ok()),
    }
}

fn holiday_date(name: &str, year: i16) -> Option<Date> {
    let norm = name.replace(['\'', '’'], "");
    match norm.as_str() {
        "new year" | "new years" | "new years day" => Date::new(year, 1, 1).ok(),
        "valentine" | "valentines" | "valentines day" => Date::new(year, 2, 14).ok(),
        "st patricks" | "st patricks day" | "saint patricks day" => Date::new(year, 3, 17).ok(),
        "april fools" | "april fools day" => Date::new(year, 4, 1).ok(),
        "independence day" | "4th of july" | "july 4th" | "fourth of july" => {
            Date::new(year, 7, 4).ok()
        }
        "halloween" => Date::new(year, 10, 31).ok(),
        "christmas eve" | "xmas eve" => Date::new(year, 12, 24).ok(),
        "christmas" | "christmas day" | "xmas" => Date::new(year, 12, 25).ok(),
        "new years eve" | "nye" => Date::new(year, 12, 31).ok(),
        "thanksgiving" => {
            // 4th Thursday of November.
            let nov1 = Date::new(year, 11, 1).ok()?;
            let offset = (Weekday::Thursday.to_monday_zero_offset()
                - nov1.weekday().to_monday_zero_offset())
            .rem_euclid(7);
            Date::new(year, 11, 1 + offset + 21).ok()
        }
        "easter" | "easter sunday" => gregorian_easter(year),
        _ => None,
    }
}

/// Anonymous Gregorian algorithm for Easter Sunday.
fn gregorian_easter(year: i16) -> Option<Date> {
    let y = year as i32;
    let a = y % 19;
    let b = y / 100;
    let c = y % 100;
    let d = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let month = (h + l - 7 * m + 114) / 31;
    let day = ((h + l - 7 * m + 114) % 31) + 1;
    Date::new(year, month as i8, day as i8).ok()
}

pub(crate) fn parse_clock_time(input: &str) -> Option<Time> {
    let s = input.trim().to_ascii_lowercase();
    let (meridiem, core) = if let Some(rest) = s.strip_suffix("am") {
        (Some(false), rest.trim())
    } else if let Some(rest) = s.strip_suffix("pm") {
        (Some(true), rest.trim())
    } else {
        (None, s.as_str())
    };

    let (h, m, sec) = if let Some((hs, rest)) = core.split_once(':') {
        if let Some((ms, ss)) = rest.split_once(':') {
            (
                hs.parse::<i8>().ok()?,
                ms.parse::<i8>().ok()?,
                ss.parse::<i8>().ok()?,
            )
        } else {
            (hs.parse::<i8>().ok()?, rest.parse::<i8>().ok()?, 0)
        }
    } else if meridiem.is_some() {
        (core.parse::<i8>().ok()?, 0, 0)
    } else {
        return None;
    };

    if !(0..=59).contains(&m) || !(0..=59).contains(&sec) {
        return None;
    }
    let hour24 = match meridiem {
        Some(false) => {
            if !(1..=12).contains(&h) {
                return None;
            }
            if h == 12 {
                0
            } else {
                h
            }
        }
        Some(true) => {
            if !(1..=12).contains(&h) {
                return None;
            }
            if h == 12 {
                12
            } else {
                h + 12
            }
        }
        None => {
            if !(0..=23).contains(&h) {
                return None;
            }
            h
        }
    };
    Time::new(hour24, m, sec, 0).ok()
}

pub(crate) fn format_time(time: Time, use_24h: bool) -> String {
    if use_24h {
        format!("{:02}:{:02}", time.hour(), time.minute())
    } else {
        let (h12, suffix) = match time.hour() {
            0 => (12, "AM"),
            1..=11 => (time.hour(), "AM"),
            12 => (12, "PM"),
            h => (h - 12, "PM"),
        };
        format!("{h12}:{:02} {suffix}", time.minute())
    }
}

pub(crate) fn format_timespan_seconds(total_secs: f64) -> String {
    let mut rem = total_secs.abs().round() as u64;
    if rem == 0 {
        return "0 s".to_string();
    }
    let days = rem / 86_400;
    rem %= 86_400;
    let hours = rem / 3600;
    rem %= 3600;
    let mins = rem / 60;
    let secs = rem % 60;

    let mut parts = Vec::new();
    if days > 0 {
        parts.push(plural(days as i64, "day", "days"));
    }
    if hours > 0 {
        parts.push(plural(hours as i64, "hour", "hours"));
    }
    if mins > 0 {
        parts.push(format!("{mins} min"));
    }
    if secs > 0 {
        parts.push(format!("{secs} s"));
    }
    parts.join(" ")
}

fn date_card(raw: &str, source: &str, date: Date) -> CalcResult {
    let text = format_date_full(date);
    unchained(CalcResult::with_badges(
        raw.to_string(),
        source,
        weekday_name(date.weekday()),
        Payload::text(&text),
    ))
}

fn format_date_full(date: Date) -> String {
    format!(
        "{} {}, {}",
        month_name(date.month()),
        date.day(),
        date.year()
    )
}

fn format_week_range(start: Date, end: Date) -> String {
    if start.year() == end.year() {
        format!(
            "{} {} – {} {}, {}",
            month_short(start.month()),
            start.day(),
            month_short(end.month()),
            end.day(),
            end.year()
        )
    } else {
        format!(
            "{} {}, {} – {} {}, {}",
            month_short(start.month()),
            start.day(),
            start.year(),
            month_short(end.month()),
            end.day(),
            end.year()
        )
    }
}

fn parse_duration_seconds(s: &str) -> Option<f64> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    match parts.as_slice() {
        [num, unit] => Some(num.parse::<f64>().ok()? * time_unit_seconds(unit)?),
        [attached] => {
            let (v, u) = split_attached_number_unit(attached)?;
            Some(v * time_unit_seconds(u)?)
        }
        _ => None,
    }
}

fn time_unit_seconds(u: &str) -> Option<f64> {
    Some(match u {
        "s" | "sec" | "secs" | "second" | "seconds" => 1.0,
        "m" | "min" | "mins" | "minute" | "minutes" => 60.0,
        "h" | "hr" | "hrs" | "hour" | "hours" => 3600.0,
        "d" | "day" | "days" => 86_400.0,
        "w" | "wk" | "wks" | "week" | "weeks" => 604_800.0,
        _ => return None,
    })
}

fn split_attached_number_unit(s: &str) -> Option<(f64, &str)> {
    let split_idx = s.find(|c: char| !c.is_ascii_digit() && c != '.' && c != '-')?;
    if split_idx == 0 {
        return None;
    }
    let v = s[..split_idx].parse::<f64>().ok()?;
    Some((v, &s[split_idx..]))
}

fn split_plus_minus(s: &str) -> Option<(&str, i64, &str)> {
    if let Some((l, r)) = s.split_once(" + ") {
        return Some((l, 1, r));
    }
    if let Some((l, r)) = s.split_once(" - ") {
        return Some((l, -1, r));
    }
    None
}

fn split_signed_terms(s: &str) -> Option<Vec<(i64, &str)>> {
    let bytes = s.as_bytes();
    let mut terms = Vec::new();
    let mut start = 0usize;
    let mut sign = 1i64;
    let mut i = 0usize;
    while i + 2 < bytes.len() {
        if bytes[i] == b' '
            && (bytes[i + 1] == b'+' || bytes[i + 1] == b'-')
            && bytes[i + 2] == b' '
        {
            terms.push((sign, &s[start..i]));
            sign = if bytes[i + 1] == b'+' { 1 } else { -1 };
            start = i + 3;
            i += 3;
            continue;
        }
        i += 1;
    }
    if terms.is_empty() {
        return None;
    }
    terms.push((sign, &s[start..]));
    Some(terms)
}

fn parse_ordinal_day(s: &str) -> Option<i8> {
    let trimmed = s
        .trim_end_matches("st")
        .trim_end_matches("nd")
        .trim_end_matches("rd")
        .trim_end_matches("th")
        .trim_end_matches('.');
    let d = trimmed.parse::<i8>().ok()?;
    if (1..=31).contains(&d) {
        Some(d)
    } else {
        None
    }
}

fn parse_month(s: &str) -> Option<i8> {
    Some(match s.trim_end_matches('.') {
        "jan" | "january" => 1,
        "feb" | "february" => 2,
        "mar" | "march" => 3,
        "apr" | "april" => 4,
        "may" => 5,
        "jun" | "june" => 6,
        "jul" | "july" => 7,
        "aug" | "august" => 8,
        "sep" | "sept" | "september" => 9,
        "oct" | "october" => 10,
        "nov" | "november" => 11,
        "dec" | "december" => 12,
        _ => return None,
    })
}

fn parse_weekday(s: &str) -> Option<Weekday> {
    Some(match s {
        "mon" | "monday" => Weekday::Monday,
        "tue" | "tues" | "tuesday" => Weekday::Tuesday,
        "wed" | "weds" | "wednesday" => Weekday::Wednesday,
        "thu" | "thur" | "thurs" | "thursday" => Weekday::Thursday,
        "fri" | "friday" => Weekday::Friday,
        "sat" | "saturday" => Weekday::Saturday,
        "sun" | "sunday" => Weekday::Sunday,
        _ => return None,
    })
}

fn month_name(m: i8) -> &'static str {
    match m {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "",
    }
}

fn month_short(m: i8) -> &'static str {
    match m {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        12 => "Dec",
        _ => "",
    }
}

fn weekday_name(w: Weekday) -> &'static str {
    match w {
        Weekday::Monday => "Monday",
        Weekday::Tuesday => "Tuesday",
        Weekday::Wednesday => "Wednesday",
        Weekday::Thursday => "Thursday",
        Weekday::Friday => "Friday",
        Weekday::Saturday => "Saturday",
        Weekday::Sunday => "Sunday",
    }
}

fn plural(n: i64, singular: &str, plural_word: &str) -> String {
    format!("{n} {}", if n.abs() == 1 { singular } else { plural_word })
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::tz::TimeZone;

    /// Pinned clock: Sun 2026-11-22 15:39:00 UTC (= 9:09 PM IST in Asia/Kolkata).
    fn pinned_ctx() -> CalcContext {
        let ts: Timestamp = "2026-11-22T15:39:00Z".parse().unwrap();
        let tz = TimeZone::get("Asia/Kolkata").unwrap();
        CalcContext::fixed(ts, tz)
    }

    fn disp(q: &str) -> Option<String> {
        let card = evaluate(q, &pinned_ctx())?;
        match card.payload {
            Payload::Value { display, .. } => Some(display),
            Payload::Error { .. } => None,
        }
    }

    #[test]
    fn evaluates_named_moments_and_unix_now() {
        assert_eq!(disp("now").as_deref(), Some("9:09 PM"));
        assert_eq!(disp("today").as_deref(), Some("November 22, 2026"));
        assert_eq!(disp("tomorrow").as_deref(), Some("November 23, 2026"));
        assert_eq!(disp("yesterday").as_deref(), Some("November 21, 2026"));
        assert_eq!(disp("now as unix").as_deref(), Some("1795361940"));
    }

    #[test]
    fn evaluates_relative_dates_weekdays_and_date_arithmetic() {
        assert_eq!(disp("35 days ago").as_deref(), Some("October 18, 2026"));
        assert_eq!(disp("in 3 days").as_deref(), Some("November 25, 2026"));
        assert_eq!(
            disp("monday in 3 weeks").as_deref(),
            Some("Monday, December 14, 2026")
        );
        assert_eq!(disp("August 5 + 5").as_deref(), Some("August 10, 2026"));
        assert_eq!(disp("3:45pm + 5").as_deref(), Some("8:45 PM"));
        assert_eq!(disp("3:45pm + 45 min").as_deref(), Some("4:30 PM"));
        assert_eq!(
            disp("today + 2 weeks - 3 days").as_deref(),
            Some("December 3, 2026")
        );
    }

    #[test]
    fn evaluates_countdowns_elapsed_and_period_workdays() {
        assert_eq!(disp("days until 25 Dec").as_deref(), Some("33 days"));
        assert_eq!(disp("days till 29 nov").as_deref(), Some("7 days"));
        assert_eq!(disp("days til 29 nov").as_deref(), Some("7 days"));
        assert_eq!(disp("days left till 29 nov").as_deref(), Some("7 days"));
        assert_eq!(disp("days till 29/11").as_deref(), Some("7 days"));
        assert_eq!(disp("days till friday").as_deref(), Some("5 days"));
        assert_eq!(disp("hours till 11pm").as_deref(), Some("1.85 hours"));
        assert_eq!(
            disp("now + 3 hours").as_deref(),
            Some("November 23, 2026, 12:09 AM")
        );
        assert_eq!(disp("end of month").as_deref(), Some("November 30, 2026"));
        assert_eq!(disp("days until christmas").as_deref(), Some("33 days"));
        assert_eq!(disp("days since march").as_deref(), Some("266 days"));
        assert_eq!(disp("days left in this month").as_deref(), Some("8 days"));
        assert_eq!(
            disp("workdays left in this month").as_deref(),
            Some("6 workdays")
        );
        assert_eq!(disp("workhours in 2026").as_deref(), Some("2,088 hours"));
        assert_eq!(disp("days in feb 2028").as_deref(), Some("29 days"));
        assert_eq!(disp("is 2028 a leap year").as_deref(), Some("true"));
        assert_eq!(disp("is 2026 a leap year").as_deref(), Some("false"));
    }

    #[test]
    fn evaluates_iso_weeks_holidays_and_weekdays() {
        assert_eq!(
            disp("week 44 2026").as_deref(),
            Some("Oct 26 – Nov 1, 2026")
        );
        assert_eq!(disp("what day is 2026-12-25").as_deref(), Some("Friday"));
        assert_eq!(
            disp("when is thanksgiving 2026").as_deref(),
            Some("November 26, 2026")
        );
        assert_eq!(
            disp("when is easter 2026").as_deref(),
            Some("April 5, 2026")
        );
    }

    #[test]
    fn evaluates_clock_spans_timespans_and_timestamps() {
        assert_eq!(disp("10pm to 2am").as_deref(), Some("4 hours"));
        assert_eq!(
            disp("145 mins to timespan").as_deref(),
            Some("2 hours 25 min")
        );
        assert_eq!(disp("90 seconds to mixed").as_deref(), Some("1 min 30 s"));
        assert_eq!(
            disp("from 1700000000 to 1700003600").as_deref(),
            Some("1 hour")
        );
        assert_eq!(
            disp("2024-03-15T14:30:00Z").as_deref(),
            Some("2024-03-15T20:00:00+05:30")
        );
        assert!(disp("unix 1700000000").unwrap().contains("2023"));
    }

    #[test]
    fn leaves_plain_arithmetic_and_app_names_untouched() {
        assert!(disp("5 + 5").is_none());
        assert!(disp("2026 - 1990").is_none());
        assert!(disp("10 / 2").is_none());
        assert!(disp("notepad").is_none());
        assert!(disp("1700000000").is_none());
    }
}
