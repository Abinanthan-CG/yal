//! Clean-room DST-aware world clocks and time zone conversions backed by
//! `jiff`'s bundled IANA time zone database.
//!
//! Covers major world cities (with accent folding), countries, IATA airport
//! codes, standard abbreviations (`PT`, `EST`, `UTC`, `IST`, `CET`, etc.),
//! fixed UTC offsets (`UTC+5:30`), and direct IANA names.

use jiff::tz::{Offset, TimeZone};
use jiff::{ToSpan, Zoned};

use crate::context::CalcContext;
use crate::datetime::{format_time, parse_clock_time};
use crate::formatter::Formatter;
use crate::result::{CalcResult, Payload};

/// Evaluate world clock, zone conversion, or time-difference queries.
pub fn evaluate(query: &str, ctx: &CalcContext) -> Option<CalcResult> {
    let raw = query.trim().trim_end_matches('?').trim();
    if raw.is_empty() || raw.len() > 200 {
        return None;
    }
    let folded = fold_ascii(raw);

    // 1. Time difference (`time diff Paris`, `diff Tokyo`, `time difference between London and Tokyo`).
    if let Some(card) = eval_time_diff(&folded, raw, ctx) {
        return Some(card);
    }

    // 2. Relative future time (`time in 4 hours`, `time in 4 hours in San Francisco`).
    if let Some(card) = eval_time_in_delay(&folded, raw, ctx) {
        return Some(card);
    }

    // 3. Current time at a place (with optional `+ 4 hours` / `+ 4` shift):
    //    `time in tokyo`, `time in brisbane + 4 hours`, `time in JFK`, `time in São Paulo`, `tokyo time`.
    if let Some(card) = eval_time_at_place(&folded, raw, ctx) {
        return Some(card);
    }

    // 4. Clock conversion between zones (`5pm ldn in sf`, `4:00 PT in UTC`, `14:00 UTC in India`, `5pm in tokyo`).
    if let Some(card) = eval_clock_conversion(&folded, raw, ctx) {
        return Some(card);
    }

    None
}

fn unchained(mut card: CalcResult) -> CalcResult {
    card.can_chain = false;
    card
}

fn fold_ascii(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        let mapped = match ch {
            'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' | 'Á' | 'À' | 'Â' | 'Ä' | 'Ã' | 'Å' => {
                'a'
            }
            'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Ì' | 'Î' | 'Ï' => 'i',
            'ó' | 'ò' | 'ô' | 'ö' | 'õ' | 'ø' | 'Ó' | 'Ò' | 'Ô' | 'Ö' | 'Õ' | 'Ø' => {
                'o'
            }
            'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' => 'u',
            'ñ' | 'Ñ' => 'n',
            'ç' | 'Ç' => 'c',
            'ß' => 's',
            other => other.to_ascii_lowercase(),
        };
        out.push(mapped);
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn eval_time_diff(folded: &str, raw: &str, ctx: &CalcContext) -> Option<CalcResult> {
    let rest = folded
        .strip_prefix("time difference between ")
        .or_else(|| folded.strip_prefix("time diff between "))
        .or_else(|| folded.strip_prefix("time difference "))
        .or_else(|| folded.strip_prefix("time diff "))
        .or_else(|| folded.strip_prefix("diff "))?;
    let rest = rest.trim();
    if rest.is_empty() {
        return None;
    }

    let (label_a, tz_a, label_b, tz_b) = if let Some((a, b)) = rest.split_once(" and ") {
        let pa = resolve_place(a.trim())?;
        let pb = resolve_place(b.trim())?;
        (pa.label, pa.tz, pb.label, pb.tz)
    } else if let Some((a, b)) = rest.split_once(" to ") {
        let pa = resolve_place(a.trim())?;
        let pb = resolve_place(b.trim())?;
        (pa.label, pa.tz, pb.label, pb.tz)
    } else {
        let target = resolve_place(rest)?;
        (
            "Local".to_string(),
            ctx.local_tz.clone(),
            target.label,
            target.tz,
        )
    };

    let off_a = ctx.now_utc.to_zoned(tz_a).offset().seconds();
    let off_b = ctx.now_utc.to_zoned(tz_b).offset().seconds();
    let diff_hours = (off_b - off_a) as f64 / 3600.0;
    let sign = if diff_hours > 0.0 { "+" } else { "" };
    let unit = if (diff_hours.abs() - 1.0).abs() < 1e-6 {
        "hour"
    } else {
        "hours"
    };
    let display = format!("{sign}{} {unit}", Formatter::display(diff_hours));
    Some(unchained(CalcResult::with_badges(
        raw.to_string(),
        &label_a,
        &label_b,
        Payload::text(&display),
    )))
}

fn eval_time_in_delay(folded: &str, raw: &str, ctx: &CalcContext) -> Option<CalcResult> {
    let rest = folded.strip_prefix("time in ")?;
    let (delay_part, place_opt) = if let Some((l, r)) = rest.split_once(" in ") {
        (l.trim(), Some(r.trim()))
    } else {
        (rest.trim(), None)
    };
    let (secs, is_duration) = parse_delay_seconds(delay_part)?;
    if !is_duration {
        return None;
    }
    let shifted_ts = ctx.now_utc.checked_add(secs.seconds()).ok()?;
    let (tz, badge) = match place_opt {
        Some(p) => {
            let place = resolve_place(p)?;
            (place.tz, place.label)
        }
        None => (ctx.local_tz.clone(), "Local Time".to_string()),
    };
    let zoned = shifted_ts.to_zoned(tz);
    let display = format_time(zoned.time(), ctx.use_24h_clock);
    Some(unchained(CalcResult::with_badges(
        raw.to_string(),
        "Time",
        &badge,
        Payload::text(&display),
    )))
}

fn eval_time_at_place(folded: &str, raw: &str, ctx: &CalcContext) -> Option<CalcResult> {
    let body = folded
        .strip_prefix("what time is it in ")
        .or_else(|| folded.strip_prefix("current time in "))
        .or_else(|| folded.strip_prefix("time in "))
        .or_else(|| folded.strip_prefix("time at "))
        .or_else(|| folded.strip_suffix(" time"));
    let body = body?.trim();
    if body.is_empty() {
        return None;
    }

    // Optional trailing offset: `brisbane + 4 hours`, `brisbane + 4`, `tokyo - 30 min`.
    let (place_str, offset_secs) = if let Some((p, r)) = body.split_once(" + ") {
        let (s, _) = parse_delay_seconds_default_hours(r.trim())?;
        (p.trim(), s)
    } else if let Some((p, r)) = body.split_once(" - ") {
        let (s, _) = parse_delay_seconds_default_hours(r.trim())?;
        (p.trim(), -s)
    } else {
        (body, 0)
    };

    let place = resolve_place(place_str)?;
    let zoned = ctx.now_utc.to_zoned(place.tz);
    let shifted = if offset_secs != 0 {
        zoned.checked_add(offset_secs.seconds()).ok()?
    } else {
        zoned
    };
    let display = format_time(shifted.time(), ctx.use_24h_clock);
    let badge = format!("{} ({})", place.label, format_offset(shifted.offset()));
    Some(unchained(CalcResult::with_badges(
        raw.to_string(),
        "World Clock",
        &badge,
        Payload::text(&display),
    )))
}

fn eval_clock_conversion(folded: &str, raw: &str, ctx: &CalcContext) -> Option<CalcResult> {
    let (lhs, rhs) = folded
        .rsplit_once(" in ")
        .or_else(|| folded.rsplit_once(" to "))?;
    let target = resolve_place(rhs.trim())?;

    let lhs = lhs.trim();
    // Try `<clock> <source_place>` first, then bare `<clock>` (local timezone).
    let (clock, source_label, source_tz) = if let Some(clock) = parse_clock_time(lhs) {
        (clock, "Local".to_string(), ctx.local_tz.clone())
    } else {
        // Split clock prefix (either 1 token like `5pm` / `14:00` or 2 tokens like `4:00 pm`).
        let parts: Vec<&str> = lhs.split_whitespace().collect();
        if parts.len() >= 3 {
            let two = format!("{} {}", parts[0], parts[1]);
            if let Some(clock) = parse_clock_time(&two) {
                let src = resolve_place(&parts[2..].join(" "))?;
                (clock, src.label, src.tz)
            } else {
                let clock = parse_clock_time(parts[0])?;
                let src = resolve_place(&parts[1..].join(" "))?;
                (clock, src.label, src.tz)
            }
        } else if parts.len() == 2 {
            let clock = parse_clock_time(parts[0])?;
            let src = resolve_place(parts[1])?;
            (clock, src.label, src.tz)
        } else {
            return None;
        }
    };

    // Anchor on today's date in the source time zone so DST matches the current season.
    let src_now = ctx.now_utc.to_zoned(source_tz.clone());
    let src_zoned: Zoned = src_now
        .date()
        .at(clock.hour(), clock.minute(), clock.second(), 0)
        .to_zoned(source_tz)
        .ok()?;
    let converted = src_zoned.with_time_zone(target.tz);

    let day_diff = src_zoned
        .date()
        .until(converted.date())
        .map(|s| s.get_days())
        .unwrap_or(0);
    let time_str = format_time(converted.time(), ctx.use_24h_clock);
    let display = match day_diff {
        0 => time_str,
        d if d > 0 => format!("{time_str} (+{d}d)"),
        d => format!("{time_str} ({d}d)"),
    };

    Some(unchained(CalcResult::with_badges(
        raw.to_string(),
        &source_label,
        &target.label,
        Payload::text(&display),
    )))
}

fn parse_delay_seconds(s: &str) -> Option<(i64, bool)> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() != 2 {
        return None;
    }
    let val = parts[0].parse::<f64>().ok()?;
    let mult = match parts[1] {
        "s" | "sec" | "secs" | "second" | "seconds" => 1.0,
        "m" | "min" | "mins" | "minute" | "minutes" => 60.0,
        "h" | "hr" | "hrs" | "hour" | "hours" => 3600.0,
        "d" | "day" | "days" => 86_400.0,
        _ => return None,
    };
    Some(((val * mult).round() as i64, true))
}

fn parse_delay_seconds_default_hours(s: &str) -> Option<(i64, bool)> {
    if let Some(res) = parse_delay_seconds(s) {
        return Some(res);
    }
    let hours = s.parse::<f64>().ok()?;
    Some(((hours * 3600.0).round() as i64, true))
}

fn format_offset(offset: Offset) -> String {
    let secs = offset.seconds();
    if secs == 0 {
        return "UTC".to_string();
    }
    let sign = if secs > 0 { '+' } else { '-' };
    let abs = secs.abs();
    let h = abs / 3600;
    let m = (abs % 3600) / 60;
    if m == 0 {
        format!("UTC{sign}{h}")
    } else {
        format!("UTC{sign}{h}:{m:02}")
    }
}

struct Place {
    label: String,
    tz: TimeZone,
}

fn resolve_place(key: &str) -> Option<Place> {
    let k = key.trim();
    if k.is_empty() {
        return None;
    }

    // 1. Explicit `utc+5:30`, `gmt-8`, `utc`, `gmt`.
    if let Some(p) = parse_utc_offset_place(k) {
        return Some(p);
    }

    // 2. Fixed-offset standard abbreviations.
    if let Some((label, offset_mins)) = fixed_abbrev(k) {
        let tz = TimeZone::fixed(Offset::from_seconds(offset_mins * 60).ok()?);
        return Some(Place {
            label: label.to_string(),
            tz,
        });
    }

    // 3. Curated cities, countries, airports, and DST-aware regional abbreviations.
    if let Some((label, iana)) = lookup_iana_place(k) {
        let tz = TimeZone::get(iana).ok()?;
        return Some(Place {
            label: label.to_string(),
            tz,
        });
    }

    // 4. Direct IANA identifier (e.g., `America/New_York`, `Asia/Tokyo`).
    if k.contains('/') {
        // Restore casing best-effort by trying common capitalization.
        let iana_guess: String = k
            .split('/')
            .map(|seg| {
                seg.split('_')
                    .map(|w| {
                        let mut c = w.chars();
                        match c.next() {
                            Some(first) => format!("{}{}", first.to_ascii_uppercase(), c.as_str()),
                            None => String::new(),
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("_")
            })
            .collect::<Vec<_>>()
            .join("/");
        if let Ok(tz) = TimeZone::get(&iana_guess) {
            return Some(Place {
                label: iana_guess,
                tz,
            });
        }
    }

    None
}

fn parse_utc_offset_place(k: &str) -> Option<Place> {
    if k == "utc" || k == "z" || k == "zulu" {
        return Some(Place {
            label: "UTC".to_string(),
            tz: TimeZone::UTC,
        });
    }
    if k == "gmt" {
        return Some(Place {
            label: "GMT".to_string(),
            tz: TimeZone::UTC,
        });
    }
    let rest = k.strip_prefix("utc").or_else(|| k.strip_prefix("gmt"))?;
    let sign = if rest.starts_with('+') {
        1
    } else if rest.starts_with('-') {
        -1
    } else {
        return None;
    };
    let digits = &rest[1..];
    let (h, m) = if let Some((hs, ms)) = digits.split_once(':') {
        (hs.parse::<i32>().ok()?, ms.parse::<i32>().ok()?)
    } else {
        (digits.parse::<i32>().ok()?, 0)
    };
    if !(0..=14).contains(&h) || !(0..=59).contains(&m) {
        return None;
    }
    let offset = Offset::from_seconds(sign * (h * 3600 + m * 60)).ok()?;
    Some(Place {
        label: format_offset(offset),
        tz: TimeZone::fixed(offset),
    })
}

fn fixed_abbrev(k: &str) -> Option<(&'static str, i32)> {
    Some(match k {
        "pst" => ("PST", -8 * 60),
        "pdt" => ("PDT", -7 * 60),
        "mst" => ("MST", -7 * 60),
        "mdt" => ("MDT", -6 * 60),
        "cst" => ("CST", -6 * 60),
        "cdt" => ("CDT", -5 * 60),
        "est" => ("EST", -5 * 60),
        "edt" => ("EDT", -4 * 60),
        "akst" => ("AKST", -9 * 60),
        "akdt" => ("AKDT", -8 * 60),
        "hst" => ("HST", -10 * 60),
        "bst" => ("BST", 60),
        "cest" => ("CEST", 2 * 60),
        "eest" => ("EEST", 3 * 60),
        "ist" => ("IST", 5 * 60 + 30),
        "gst" => ("GST", 4 * 60),
        "sgt" => ("SGT", 8 * 60),
        "hkt" => ("HKT", 8 * 60),
        "jst" => ("JST", 9 * 60),
        "kst" => ("KST", 9 * 60),
        "awst" => ("AWST", 8 * 60),
        "aest" => ("AEST", 10 * 60),
        "aedt" => ("AEDT", 11 * 60),
        "nzst" => ("NZST", 12 * 60),
        "nzdt" => ("NZDT", 13 * 60),
        _ => return None,
    })
}

fn lookup_iana_place(k: &str) -> Option<(&'static str, &'static str)> {
    Some(match k {
        // DST-aware regional abbreviations
        "pt" | "pacific" => ("Pacific Time", "America/Los_Angeles"),
        "mt" | "mountain" => ("Mountain Time", "America/Denver"),
        "ct" | "central" => ("Central Time", "America/Chicago"),
        "et" | "eastern" => ("Eastern Time", "America/New_York"),
        "wet" => ("London", "Europe/London"),
        "cet" => ("Central European", "Europe/Paris"),
        "eet" => ("Eastern European", "Europe/Athens"),

        // Americas — Cities & Airports
        "san francisco" | "sf" | "sfo" => ("San Francisco", "America/Los_Angeles"),
        "los angeles" | "la" | "lax" => ("Los Angeles", "America/Los_Angeles"),
        "seattle" | "sea" => ("Seattle", "America/Los_Angeles"),
        "portland" | "san diego" | "las vegas" | "vancouver" | "yvr" => {
            ("Pacific", "America/Los_Angeles")
        }
        "denver" | "den" | "salt lake city" | "calgary" => ("Denver", "America/Denver"),
        "phoenix" | "phx" => ("Phoenix", "America/Phoenix"),
        "chicago" | "chi" | "ord" => ("Chicago", "America/Chicago"),
        "austin" | "dallas" | "dfw" | "houston" | "minneapolis" => ("Chicago", "America/Chicago"),
        "mexico city" | "mexico" | "mex" => ("Mexico City", "America/Mexico_City"),
        "new york" | "nyc" | "jfk" | "ewr" | "lga" => ("New York", "America/New_York"),
        "boston" | "bos" => ("Boston", "America/New_York"),
        "miami" | "mia" | "atlanta" | "atl" | "washington" | "dc" | "philadelphia" | "detroit" => {
            ("New York", "America/New_York")
        }
        "toronto" | "yyz" | "montreal" | "ottawa" | "canada" => ("Toronto", "America/Toronto"),
        "honolulu" | "hawaii" | "hnl" => ("Honolulu", "Pacific/Honolulu"),
        "anchorage" | "alaska" => ("Anchorage", "America/Anchorage"),
        "sao paulo" | "rio de janeiro" | "rio" | "brazil" | "gru" => {
            ("São Paulo", "America/Sao_Paulo")
        }
        "buenos aires" | "argentina" => ("Buenos Aires", "America/Argentina/Buenos_Aires"),
        "santiago" | "chile" => ("Santiago", "America/Santiago"),
        "lima" | "peru" => ("Lima", "America/Lima"),
        "bogota" | "colombia" => ("Bogotá", "America/Bogota"),

        // Europe — Cities, Countries & Airports
        "london" | "ldn" | "lhr" | "lgw" | "uk" | "united kingdom" | "britain" | "england" => {
            ("London", "Europe/London")
        }
        "dublin" | "ireland" => ("Dublin", "Europe/Dublin"),
        "lisbon" | "portugal" => ("Lisbon", "Europe/Lisbon"),
        "reykjavik" | "iceland" => ("Reykjavik", "Atlantic/Reykjavik"),
        "paris" | "cdg" | "france" | "chamonix" => ("Paris", "Europe/Paris"),
        "berlin" | "munich" | "munchen" | "frankfurt" | "fra" | "germany" => {
            ("Berlin", "Europe/Berlin")
        }
        "amsterdam" | "ams" | "netherlands" | "holland" => ("Amsterdam", "Europe/Amsterdam"),
        "brussels" | "belgium" => ("Brussels", "Europe/Brussels"),
        "madrid" | "barcelona" | "spain" => ("Madrid", "Europe/Madrid"),
        "rome" | "milan" | "italy" => ("Rome", "Europe/Rome"),
        "zurich" | "geneva" | "switzerland" => ("Zurich", "Europe/Zurich"),
        "vienna" | "austria" => ("Vienna", "Europe/Vienna"),
        "prague" | "czechia" | "czech republic" => ("Prague", "Europe/Prague"),
        "warsaw" | "poland" => ("Warsaw", "Europe/Warsaw"),
        "stockholm" | "sweden" => ("Stockholm", "Europe/Stockholm"),
        "oslo" | "norway" => ("Oslo", "Europe/Oslo"),
        "copenhagen" | "denmark" => ("Copenhagen", "Europe/Copenhagen"),
        "budapest" | "hungary" => ("Budapest", "Europe/Budapest"),
        "helsinki" | "finland" => ("Helsinki", "Europe/Helsinki"),
        "athens" | "greece" => ("Athens", "Europe/Athens"),
        "bucharest" | "romania" => ("Bucharest", "Europe/Bucharest"),
        "kyiv" | "kiev" | "ukraine" => ("Kyiv", "Europe/Kyiv"),
        "istanbul" | "ankara" | "turkey" => ("Istanbul", "Europe/Istanbul"),
        "moscow" | "russia" => ("Moscow", "Europe/Moscow"),

        // Middle East & Africa
        "dubai" | "dxb" | "abu dhabi" | "uae" => ("Dubai", "Asia/Dubai"),
        "riyadh" | "jeddah" | "saudi arabia" => ("Riyadh", "Asia/Riyadh"),
        "doha" | "qatar" => ("Doha", "Asia/Qatar"),
        "tel aviv" | "jerusalem" | "israel" => ("Jerusalem", "Asia/Jerusalem"),
        "cairo" | "egypt" => ("Cairo", "Africa/Cairo"),
        "johannesburg" | "cape town" | "south africa" => ("Johannesburg", "Africa/Johannesburg"),
        "lagos" | "nigeria" => ("Lagos", "Africa/Lagos"),
        "nairobi" | "kenya" => ("Nairobi", "Africa/Nairobi"),
        "casablanca" | "morocco" => ("Casablanca", "Africa/Casablanca"),

        // Asia
        "india" | "mumbai" | "bombay" | "bom" | "delhi" | "new delhi" | "del" | "bengaluru"
        | "bangalore" | "blr" | "chennai" | "maa" | "kochi" | "cochin" | "cok" | "kolkata"
        | "hyderabad" | "pune" | "ahmedabad" => ("India", "Asia/Kolkata"),
        "colombo" | "sri lanka" => ("Colombo", "Asia/Colombo"),
        "kathmandu" | "nepal" => ("Kathmandu", "Asia/Kathmandu"),
        "karachi" | "lahore" | "islamabad" | "pakistan" => ("Karachi", "Asia/Karachi"),
        "dhaka" | "bangladesh" => ("Dhaka", "Asia/Dhaka"),
        "bangkok" | "thailand" => ("Bangkok", "Asia/Bangkok"),
        "ho chi minh" | "hanoi" | "vietnam" => ("Ho Chi Minh", "Asia/Ho_Chi_Minh"),
        "jakarta" | "indonesia" => ("Jakarta", "Asia/Jakarta"),
        "singapore" | "sin" => ("Singapore", "Asia/Singapore"),
        "kuala lumpur" | "malaysia" => ("Kuala Lumpur", "Asia/Kuala_Lumpur"),
        "manila" | "philippines" => ("Manila", "Asia/Manila"),
        "hong kong" | "hkg" => ("Hong Kong", "Asia/Hong_Kong"),
        "taipei" | "taiwan" => ("Taipei", "Asia/Taipei"),
        "shanghai" | "beijing" | "shenzhen" | "guangzhou" | "china" => {
            ("Shanghai", "Asia/Shanghai")
        }
        "seoul" | "icn" | "south korea" | "korea" => ("Seoul", "Asia/Seoul"),
        "tokyo" | "osaka" | "kyoto" | "nrt" | "hnd" | "japan" => ("Tokyo", "Asia/Tokyo"),

        // Oceania
        "perth" => ("Perth", "Australia/Perth"),
        "adelaide" => ("Adelaide", "Australia/Adelaide"),
        "brisbane" | "bne" => ("Brisbane", "Australia/Brisbane"),
        "sydney" | "syd" | "canberra" | "australia" => ("Sydney", "Australia/Sydney"),
        "melbourne" | "mel" => ("Melbourne", "Australia/Melbourne"),
        "auckland" | "wellington" | "new zealand" => ("Auckland", "Pacific/Auckland"),
        "usa" | "us" | "united states" => ("New York", "America/New_York"),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::Timestamp;

    /// Pinned clock: Sun 2026-11-22 15:39:00 UTC (winter standard time in Northern Hemisphere).
    fn winter_ctx() -> CalcContext {
        let ts: Timestamp = "2026-11-22T15:39:00Z".parse().unwrap();
        let tz = TimeZone::get("Asia/Kolkata").unwrap();
        CalcContext::fixed(ts, tz)
    }

    /// Pinned clock: Wed 2026-07-15 15:00:00 UTC (summer DST active in US/EU).
    fn summer_ctx() -> CalcContext {
        let ts: Timestamp = "2026-07-15T15:00:00Z".parse().unwrap();
        let tz = TimeZone::UTC;
        CalcContext::fixed(ts, tz)
    }

    fn disp(q: &str, ctx: &CalcContext) -> Option<String> {
        let card = evaluate(q, ctx)?;
        match card.payload {
            Payload::Value { display, .. } => Some(display),
            Payload::Error { .. } => None,
        }
    }

    #[test]
    fn resolves_world_clocks_airports_and_accented_cities() {
        let ctx = winter_ctx();
        // 15:39 UTC + 9h = 00:39 AM Tokyo
        assert_eq!(disp("time in tokyo", &ctx).as_deref(), Some("12:39 AM"));
        // 15:39 UTC - 5h = 10:39 AM New York (JFK)
        assert_eq!(disp("time in JFK", &ctx).as_deref(), Some("10:39 AM"));
        // 15:39 UTC - 3h = 12:39 PM São Paulo
        assert_eq!(disp("time in São Paulo", &ctx).as_deref(), Some("12:39 PM"));
        // 15:39 UTC + 10h = 01:39 AM Brisbane; + 4 hours = 5:39 AM
        assert_eq!(
            disp("time in brisbane + 4 hours", &ctx).as_deref(),
            Some("5:39 AM")
        );
        assert_eq!(
            disp("time in brisbane + 4", &ctx).as_deref(),
            Some("5:39 AM")
        );
    }

    #[test]
    fn converts_clocks_between_zones_with_real_dst() {
        let winter = winter_ctx();
        // Winter: London UTC+0 -> SF UTC-8: 5pm ldn = 9:00 AM sf
        assert_eq!(disp("5pm ldn in sf", &winter).as_deref(), Some("9:00 AM"));
        // Winter: 4:00 PT (PST = UTC-8) -> UTC = 12:00 PM
        assert_eq!(disp("4:00 PT in UTC", &winter).as_deref(), Some("12:00 PM"));

        let summer = summer_ctx();
        // Summer: 4:00 PT (PDT = UTC-7) -> UTC = 11:00 AM
        assert_eq!(disp("4:00 PT in UTC", &summer).as_deref(), Some("11:00 AM"));
    }

    #[test]
    fn computes_time_diffs_and_delays() {
        let ctx = winter_ctx();
        // Kolkata UTC+5:30 to Paris UTC+1 = -4.5 hours
        assert_eq!(disp("time diff Paris", &ctx).as_deref(), Some("-4.5 hours"));
        assert_eq!(disp("diff Paris", &ctx).as_deref(), Some("-4.5 hours"));

        // 15:39 UTC + 4h = 19:39 UTC -> SF (UTC-8) = 11:39 AM
        assert_eq!(
            disp("time in 4 hours in San Francisco", &ctx).as_deref(),
            Some("11:39 AM")
        );
        // Local (UTC+5:30): 9:09 PM + 4h = 1:09 AM
        assert_eq!(disp("time in 4 hours", &ctx).as_deref(), Some("1:09 AM"));
    }

    #[test]
    fn ignores_non_timezone_queries() {
        let ctx = winter_ctx();
        assert!(disp("10ft in m", &ctx).is_none());
        assert!(disp("100 usd in inr", &ctx).is_none());
        assert!(disp("notepad", &ctx).is_none());
    }
}
