//! Reminders: reading a reminder's text and due time out of words.
//!
//! The words a person writes in one sweep — `remind me to check the oven
//! in 40m`, `remind standup at 9:30`, `remind pay rent tomorrow at 9` —
//! are parsed here, in pure code, so the engine and the CLI share the one
//! interpretation. The accepted shapes are narrow and documented rather
//! than pretending to full natural language.

use chrono::{DateTime, Duration, Local, NaiveTime, TimeZone, Utc};

/// A reminder the words describe: the text and when it comes due.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReminderSpec {
    /// The reminder's text.
    pub text: String,
    /// When the reminder comes due.
    pub due_at: DateTime<Utc>,
}

/// The largest span a reminder may ask for: a season, not a decade.
const MAX_SPAN_SECONDS: u64 = 90 * 24 * 3_600;

/// Parse the words into a reminder.
///
/// The words take the shape `remind [me] [to|about] <text> <when>`, where
/// `<when>` is a suffix of the words:
///
/// - `in <duration>` — duration tokens such as `40m`, `1h 30m`, `90`, up
///   to ninety days; a bare number counts as minutes, so `in 20` is
///   twenty minutes the way it reads;
/// - `[tomorrow] [at] <time>` — a clock time `9`, `9:30`, `9:30 am`,
///   today, or tomorrow when the day is named or the time is past.
///
/// The when-phrase must end the words, so a text that merely mentions
/// `in` or `at` in the middle is not cut short. `None` when the words are
/// not a reminder, the text is empty, or nothing in them names a time.
pub fn parse(text: &str, now: DateTime<Utc>) -> Option<ReminderSpec> {
    let rest = text.trim().strip_prefix("remind")?;
    // `reminders` and similar words are not this reminder; the prefix
    // must be the word on its own.
    if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let rest = rest.trim();
    // `remind me to` and `remind me about` read naturally; the connector
    // is not part of the text.
    let rest = rest.strip_prefix("me ").unwrap_or(rest);
    let rest = match rest.strip_prefix("to ") {
        Some(without) => without,
        None => rest.strip_prefix("about ").unwrap_or(rest),
    };
    let rest = rest.trim();
    if rest.is_empty() {
        return None;
    }

    let tokens: Vec<&str> = rest.split_whitespace().collect();
    let (due_at, text_end) = when_from_tokens(&tokens, now)?;

    let reminder_text = tokens[..text_end].join(" ");
    if reminder_text.is_empty() {
        return None;
    }
    Some(ReminderSpec {
        text: reminder_text,
        due_at,
    })
}

/// Work out the when-phrase from the end of the words.
///
/// Returns the due time and the number of tokens the text keeps —
/// everything before the when-phrase.
fn when_from_tokens(tokens: &[&str], now: DateTime<Utc>) -> Option<(DateTime<Utc>, usize)> {
    // A duration block: the rightmost `in` with only durations after it.
    for anchor in (0..tokens.len()).rev() {
        if tokens[anchor] != "in" || anchor + 1 >= tokens.len() {
            continue;
        }
        let Some(total) = all_duration_tokens(&tokens[anchor + 1..]) else {
            continue;
        };
        let due_at = now + Duration::seconds(total as i64);
        return Some((due_at, anchor));
    }

    // A clock time at the end: `[tomorrow] [at] <time>`.
    time_of_day_tokens(tokens, now)
}

/// Whether every token is a duration, and how long that is together.
fn all_duration_tokens(tokens: &[&str]) -> Option<u64> {
    let mut total: u64 = 0;
    for token in tokens {
        if token.chars().all(|ch| ch.is_ascii_digit()) {
            // A bare number counts as minutes — `in 20` reads as twenty
            // minutes, not twenty seconds.
            total += token.parse::<u64>().ok()?.saturating_mul(60);
            continue;
        }
        let split = token
            .find(|ch: char| !ch.is_ascii_digit())
            .unwrap_or(token.len());
        let (digits, unit) = token.split_at(split);
        let multiplier = match unit.to_lowercase().as_str() {
            "s" => 1,
            "m" => 60,
            "h" => 3_600,
            "d" => 86_400,
            _ => return None,
        };
        total += digits.parse::<u64>().ok()?.saturating_mul(multiplier);
    }
    if total == 0 || total > MAX_SPAN_SECONDS {
        return None;
    }
    Some(total)
}

/// Read a clock time and its optional day from the word tail.
///
/// The tail has at most three tokens: `[tomorrow] [at] <time>`, where the
/// time is `9`, `9:30`, `9:30:15`, with an optional `am` or `pm` ending.
/// The tail must end the words; anything else does not parse. Returns the
/// due time and the number of tokens the text keeps.
fn time_of_day_tokens(tokens: &[&str], now: DateTime<Utc>) -> Option<(DateTime<Utc>, usize)> {
    let (time_token, rest) = tokens.split_last()?;
    let time = clock_time(time_token)?;

    // The words may end `[tomorrow] [at] <time>`; the words before the
    // time are the text, minus the connector and the day if named.
    let mut text_end = rest.len();
    let mut tomorrow = false;
    let words = match rest.last() {
        Some(&"at") => {
            text_end -= 1;
            &rest[..rest.len() - 1]
        }
        _ => rest,
    };
    if words.last() == Some(&"tomorrow") {
        tomorrow = true;
        text_end -= 1;
    }

    let today = Local.from_utc_datetime(&now.naive_utc()).date_naive();
    // Today's slot, compared against the words' own now: a time behind is
    // never set into the past, so it moves to tomorrow, as does an
    // outright `tomorrow`. For a slot the day's clock shift skipped, the
    // parse refuses rather than guessing.
    let today_slot = Local
        .from_local_datetime(&today.and_time(time))
        .single()
        .map(|local| local.with_timezone(&Utc));
    let day = match tomorrow || today_slot.map(|slot| slot <= now) == Some(true) {
        true => today.succ_opt()?,
        false => today,
    };
    let due = Local
        .from_local_datetime(&day.and_time(time))
        .single()
        .map(|local| local.with_timezone(&Utc))?;
    Some((due, text_end))
}

/// One clock time: `9`, `9:30`, `9:30:15`, any with `am` or `pm`.
fn clock_time(token: &str) -> Option<NaiveTime> {
    let (body, afternoon) = match token.strip_suffix("am") {
        Some(body) => (body, false),
        None => match token.strip_suffix("pm") {
            Some(body) => (body, true),
            None => (token, false),
        },
    };
    let mut parts = body.split(':');
    let hours: u32 = parts.next()?.parse().ok()?;
    let minutes: u32 = match parts.next() {
        Some(minutes) => minutes.parse().ok()?,
        None => 0,
    };
    let seconds: u32 = match parts.next() {
        Some(seconds) => seconds.parse().ok()?,
        None => 0,
    };
    if parts.next().is_some() || hours > 23 || minutes > 59 || seconds > 59 {
        return None;
    }
    let hours = match (afternoon, hours) {
        // 12 am folds to midnight; 12 pm stays noon.
        (false, 12) => 0,
        (true, 12) => 12,
        (true, hours) if hours < 12 => hours + 12,
        (_, hours) => hours,
    };
    NaiveTime::from_hms_opt(hours, minutes, seconds)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Timelike;

    fn local_at(hour: u32, minute: u32, month: u32, day: u32) -> DateTime<Utc> {
        Local
            .with_ymd_and_hms(2026, month, day, hour, minute, 0)
            .single()
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn a_duration_is_read_from_its_tail() {
        let spec = parse("remind check the oven in 40m", local_at(11, 30, 10, 7)).unwrap();
        assert_eq!(spec.text, "check the oven");
        assert_eq!(
            spec.due_at,
            local_at(12, 10, 10, 7),
            "forty minutes after the words, not after a clock hour"
        );
    }

    #[test]
    fn durations_add_up() {
        let spec = parse("remind me to stretch in 1h 30m", local_at(7, 0, 10, 7)).unwrap();
        assert_eq!(spec.text, "stretch");
        assert_eq!(spec.due_at, local_at(8, 30, 10, 7));
    }

    #[test]
    fn a_bare_number_counts_as_minutes() {
        let spec = parse("remind breathe in 20", local_at(7, 0, 10, 7)).unwrap();
        assert_eq!(spec.due_at, local_at(7, 20, 10, 7));
    }

    #[test]
    fn a_clock_time_today_is_kept_when_it_still_comes() {
        let spec = parse("remind standup at 14:00", local_at(9, 0, 10, 7)).unwrap();
        assert_eq!(spec.text, "standup");
        assert_eq!(spec.due_at, local_at(14, 0, 10, 7));
    }

    #[test]
    fn a_clock_time_already_past_moves_to_tomorrow() {
        let spec = parse("remind standup at 8:00", local_at(9, 0, 10, 7)).unwrap();
        assert_eq!(spec.due_at, local_at(8, 0, 10, 8));
    }

    #[test]
    fn tomorrow_names_the_next_day() {
        let spec = parse("remind pay rent tomorrow at 9:30", local_at(9, 0, 10, 7)).unwrap();
        assert_eq!(spec.text, "pay rent");
        assert_eq!(spec.due_at, local_at(9, 30, 10, 8));
    }

    #[test]
    fn tomorrow_alone_before_the_time() {
        let spec = parse("remind pay rent tomorrow 9:30", local_at(9, 0, 10, 7)).unwrap();
        assert_eq!(spec.due_at, local_at(9, 30, 10, 8));
    }

    #[test]
    fn afternoon_hours_shift_twelve() {
        let spec = parse("remind call home at 5pm", local_at(9, 0, 10, 7)).unwrap();
        assert_eq!(spec.due_at, local_at(17, 0, 10, 7));
    }

    #[test]
    fn twelve_am_is_midnight() {
        let spec = parse("remind flip the backup at 12:30am", local_at(9, 0, 10, 7)).unwrap();
        assert_eq!(spec.due_at, local_at(0, 30, 10, 8));
    }

    #[test]
    fn words_are_not_cut_by_a_stray_in() {
        // The duration block is the tail; the earlier `in` is text.
        let spec = parse("remind turn in homework in 2h", local_at(7, 0, 10, 7)).unwrap();
        assert_eq!(spec.text, "turn in homework");
        assert_eq!(spec.due_at, local_at(9, 0, 10, 7));
    }

    #[test]
    fn an_at_in_the_middle_of_text_offers_nothing() {
        assert!(parse("remind cookies at Nana's house", local_at(9, 0, 10, 7)).is_none());
    }

    #[test]
    fn another_word_starting_with_remind_is_not_a_reminder() {
        assert!(parse("reminders sweep at 5", local_at(9, 0, 10, 7)).is_none());
    }

    #[test]
    fn without_remind_there_is_no_reminder() {
        assert!(parse("check the oven in 40m", local_at(9, 0, 10, 7)).is_none());
    }

    #[test]
    fn with_no_time_there_is_no_reminder() {
        assert!(parse("remind call the dentist", local_at(9, 0, 10, 7)).is_none());
    }

    #[test]
    fn with_no_text_there_is_no_reminder() {
        assert!(parse("remind in 40m", local_at(9, 0, 10, 7)).is_none());
    }

    #[test]
    fn no_time_at_all_is_refused() {
        assert!(parse("remind", local_at(9, 0, 10, 7)).is_none());
    }

    #[test]
    fn spans_over_ninety_days_are_refused() {
        assert!(parse("remind retire in 100d", local_at(9, 0, 10, 7)).is_none());
    }

    #[test]
    fn a_time_of_day_keeps_its_hours() {
        let time = clock_time("14:05").unwrap();
        assert_eq!(time.hour(), 14);
        assert_eq!(time.minute(), 5);
    }
}
