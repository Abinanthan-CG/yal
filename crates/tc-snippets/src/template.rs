//! The template engine: the placeholders a snippet's text may contain.
//!
//! Mirrors upstream `Features/Snippets/Model/SnippetTemplateEngine.swift`.
//!
//! A template is plain text with `{…}` placeholders in it. A placeholder
//! either names a built-in value (`{clipboard}`, `{selection}`, `{date}`,
//! `{time}`, `{datetime}`, `{day}`, `{uuid}`, `{cursor}`), asks the user for
//! a value (`{argument name="…"}`), or pulls in another snippet
//! (`{snippet:Name}`). A placeholder that is malformed, or that names
//! something unknown, stays in the text as it was written.

use std::collections::{BTreeMap, HashSet};

use chrono::{DateTime, Datelike, FixedOffset, NaiveDate, Timelike};
use unicode_segmentation::UnicodeSegmentation;

use crate::locale::{for_identifier, LocaleData};
use crate::snippet::StoredSnippet;

/// How deep a `{snippet:…}` reference may nest before it stops expanding.
///
/// Mirrors upstream's `maximumReferenceDepth`.
pub const MAXIMUM_REFERENCE_DEPTH: usize = 5;

/// Everything a template needs to know about the world it is expanded in.
///
/// Mirrors upstream's `SnippetTemplateEngine.ExpansionContext`.
pub struct ExpansionContext {
    /// The clipboard history, most recent first: `{clipboard}` is offset 0.
    pub clipboard_history: Vec<String>,
    /// The text the user has selected.
    pub selection: String,
    /// The moment the expansion happens.
    pub now: DateTime<FixedOffset>,
    /// The locale the date and time tokens format in.
    pub locale: String,
    /// The time zone the date and time tokens format in.
    pub time_zone: FixedOffset,
    /// Where `{uuid}` gets its value; injected so it is reproducible in tests.
    uuid_factory: Box<dyn Fn() -> String + Send + Sync>,
}

impl std::fmt::Debug for ExpansionContext {
    /// Shows the context without the closure it holds.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExpansionContext")
            .field("clipboard_history", &self.clipboard_history)
            .field("selection", &self.selection)
            .field("now", &self.now)
            .field("locale", &self.locale)
            .field("time_zone", &self.time_zone)
            .finish()
    }
}

impl ExpansionContext {
    /// A context with an injected clock, locale, time zone and clipboard.
    ///
    /// Mirrors upstream's `ExpansionContext.init(clipboardHistory:selection:now:calendar:locale:timeZone:makeUUID:)`.
    pub fn new(
        clipboard_history: Vec<String>,
        selection: impl Into<String>,
        now: DateTime<FixedOffset>,
        locale: impl Into<String>,
        time_zone: FixedOffset,
    ) -> Self {
        Self {
            clipboard_history,
            selection: selection.into(),
            now,
            locale: locale.into(),
            time_zone,
            uuid_factory: Box::new(default_uuid),
        }
    }

    /// A context that only knows the current clipboard.
    ///
    /// Mirrors upstream's convenience initializer.
    pub fn with_clipboard(
        clipboard: impl Into<String>,
        selection: impl Into<String>,
        now: DateTime<FixedOffset>,
        locale: impl Into<String>,
        time_zone: FixedOffset,
    ) -> Self {
        Self::new(vec![clipboard.into()], selection, now, locale, time_zone)
    }

    /// Replace where `{uuid}` gets its value.
    ///
    /// Mirrors upstream's injected `makeUUID`.
    pub fn with_uuid_factory<F>(mut self, factory: F) -> Self
    where
        F: Fn() -> String + Send + Sync + 'static,
    {
        self.uuid_factory = Box::new(factory);
        self
    }
}

/// An `{argument}` still waiting for a value, with the options it offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingArgument {
    /// The argument's name.
    pub name: String,
    /// The choices the argument offers, if it declared any.
    pub options: Vec<String>,
}

/// An `{argument}` a form has to ask for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredArgument {
    /// The argument's name.
    pub name: String,
    /// The choices the argument offers, if it declared any.
    pub options: Vec<String>,
    /// Whether the argument still resolves when it is left empty.
    pub is_optional: bool,
}

/// The result of expanding a template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpansionResult {
    /// The expanded text.
    pub text: String,
    /// Where the cursor goes, counted back from the end of the text.
    pub cursor_offset_from_end: Option<usize>,
    /// The arguments that still need a value.
    pub missing_arguments: Vec<MissingArgument>,
    /// Whether the template reads the user's selection, nested snippets included.
    pub reads_selection: bool,
}

/// How a value that is substituted into a template is formatted.
///
/// Mirrors upstream's `SnippetTemplateEngine.ValueEncoding`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ValueEncoding {
    /// The value is inserted as it is.
    #[default]
    None,
    /// The value is percent-encoded, for a value that goes into a URL.
    PercentEncoding,
}

/// Expands snippet templates.
///
/// Mirrors upstream's `SnippetTemplateEngine`.
pub struct SnippetTemplateEngine;

impl SnippetTemplateEngine {
    /// Expand a stored snippet's template.
    ///
    /// Mirrors upstream's `expand(_:snippets:context:userArguments:)`.
    pub fn expand(
        record: &StoredSnippet,
        snippets: &[StoredSnippet],
        context: &ExpansionContext,
        user_arguments: &BTreeMap<String, String>,
    ) -> ExpansionResult {
        let mut sorted: Vec<&StoredSnippet> = snippets.iter().collect();
        sorted.sort_by(|lhs, rhs| lhs.id().cmp(&rhs.id()));

        let expansion = expand_text(
            &record.snippet.text,
            &sorted,
            context,
            user_arguments,
            ValueEncoding::None,
            0,
            &mut HashSet::from([record.id()]),
        );
        result_of(expansion)
    }

    /// Expand a template that is not a snippet.
    ///
    /// Mirrors upstream's `expand(text:context:userArguments:encoding:)`: a
    /// `{snippet:…}` reference has nothing to resolve against and stays as text.
    pub fn expand_text(
        text: &str,
        context: &ExpansionContext,
        user_arguments: &BTreeMap<String, String>,
        encoding: ValueEncoding,
    ) -> ExpansionResult {
        let expansion = expand_text(
            text,
            &[],
            context,
            user_arguments,
            encoding,
            0,
            &mut HashSet::new(),
        );
        result_of(expansion)
    }

    /// The `{argument}`s a template declares, in the order they are written.
    ///
    /// Mirrors upstream's `declaredArguments(in:)`.
    pub fn declared_arguments(text: &str) -> Vec<DeclaredArgument> {
        let tokens: Vec<ArgumentToken> = parse_segments(text)
            .into_iter()
            .filter_map(|segment| match segment {
                Segment::Argument { token, .. } => Some(token),
                _ => None,
            })
            .collect();

        // Expansion falls back per occurrence, so one without a `default=` leaves the field owed.
        let owed: HashSet<&str> = tokens
            .iter()
            .filter(|token| token.default_value.is_none())
            .map(|token| token.name.as_str())
            .collect();

        let mut seen: HashSet<&str> = HashSet::new();
        tokens
            .into_iter()
            .filter(|token| seen.insert(token.name.as_str()))
            .map(|token| DeclaredArgument {
                is_optional: !owed.contains(token.name.as_str()),
                name: token.name,
                options: token.options,
            })
            .collect()
    }
}

/// The work-in-progress of an expansion.
struct Expansion {
    text: String,
    cursor_character_offset: Option<usize>,
    missing_arguments: Vec<MissingArgument>,
    missing_argument_names: HashSet<String>,
    reads_selection: bool,
}

impl Expansion {
    /// Append literal text.
    fn append(&mut self, value: &str) {
        self.text.push_str(value);
    }

    /// Append another expansion, carrying its cursor and its missing arguments.
    fn append_nested(&mut self, nested: Expansion) {
        let insertion_offset = grapheme_count(&self.text);
        if self.cursor_character_offset.is_none() {
            if let Some(nested_cursor) = nested.cursor_character_offset {
                self.cursor_character_offset = Some(insertion_offset + nested_cursor);
            }
        }
        self.text.push_str(&nested.text);
        self.reads_selection = self.reads_selection || nested.reads_selection;
        for argument in nested.missing_arguments {
            self.add_missing_argument(argument);
        }
    }

    /// Remember where the cursor goes, if it is not already known.
    fn mark_cursor(&mut self) {
        if self.cursor_character_offset.is_none() {
            self.cursor_character_offset = Some(grapheme_count(&self.text));
        }
    }

    /// Remember an argument that still needs a value.
    fn add_missing_argument(&mut self, argument: MissingArgument) {
        if self.missing_argument_names.insert(argument.name.clone()) {
            self.missing_arguments.push(argument);
        }
    }
}

/// Turn an expansion into the result a caller sees.
fn result_of(expansion: Expansion) -> ExpansionResult {
    ExpansionResult {
        cursor_offset_from_end: expansion
            .cursor_character_offset
            .map(|offset| grapheme_count(&expansion.text) - offset),
        text: expansion.text,
        missing_arguments: expansion.missing_arguments,
        reads_selection: expansion.reads_selection,
    }
}

/// One piece of a template.
enum Segment {
    Literal(String),
    Clipboard {
        offset: usize,
        modifiers: Vec<Modifier>,
    },
    Selection {
        modifiers: Vec<Modifier>,
    },
    DateTime {
        token: DateTimeToken,
        modifiers: Vec<Modifier>,
    },
    Uuid {
        modifiers: Vec<Modifier>,
    },
    Argument {
        token: ArgumentToken,
        source: String,
        modifiers: Vec<Modifier>,
    },
    Cursor,
    SnippetReference {
        key: String,
        source: String,
    },
}

/// A date or time token and the offsets it asks for.
struct DateTimeToken {
    kind: DateTimeKind,
    offsets: Vec<Offset>,
    format: Option<String>,
    locale_identifier: Option<String>,
}

/// Which date or time a token renders.
enum DateTimeKind {
    Date,
    Time,
    DateTime,
    Weekday,
}

/// A signed calendar offset.
struct Offset {
    component: OffsetComponent,
    value: i64,
}

/// The calendar component an offset moves.
enum OffsetComponent {
    Minute,
    Hour,
    Day,
    Month,
    Year,
}

/// An `{argument}` token.
struct ArgumentToken {
    name: String,
    options: Vec<String>,
    default_value: Option<String>,
}

/// Post-processing applied to a placeholder's value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Modifier {
    Uppercase,
    Lowercase,
    Trim,
    PercentEncode,
    JsonStringify,
    Raw,
}

impl Modifier {
    /// The modifier a raw name names.
    fn parse(raw: &str) -> Option<Self> {
        match raw {
            "uppercase" => Some(Self::Uppercase),
            "lowercase" => Some(Self::Lowercase),
            "trim" => Some(Self::Trim),
            "percent-encode" => Some(Self::PercentEncode),
            "json-stringify" => Some(Self::JsonStringify),
            "raw" => Some(Self::Raw),
            _ => None,
        }
    }
}

/// Expand a template's text.
fn expand_text(
    text: &str,
    snippets: &[&StoredSnippet],
    context: &ExpansionContext,
    user_arguments: &BTreeMap<String, String>,
    encoding: ValueEncoding,
    depth: usize,
    visited_ids: &mut HashSet<String>,
) -> Expansion {
    let mut expansion = Expansion {
        text: String::new(),
        cursor_character_offset: None,
        missing_arguments: Vec::new(),
        missing_argument_names: HashSet::new(),
        reads_selection: false,
    };

    for segment in parse_segments(text) {
        match segment {
            Segment::Literal(value) => expansion.append(&value),
            Segment::Clipboard { offset, modifiers } => {
                let value = context
                    .clipboard_history
                    .get(offset)
                    .cloned()
                    .unwrap_or_default();
                expansion.append(&apply_modifiers(&modifiers, &value, encoding));
            }
            Segment::Selection { modifiers } => {
                expansion.reads_selection = true;
                expansion.append(&apply_modifiers(&modifiers, &context.selection, encoding));
            }
            Segment::DateTime { token, modifiers } => {
                expansion.append(&apply_modifiers(&modifiers, &format_token(&token, context), encoding));
            }
            Segment::Uuid { modifiers } => {
                expansion.append(&apply_modifiers(&modifiers, &(context.uuid_factory)(), encoding));
            }
            Segment::Argument {
                token,
                source,
                modifiers,
            } => match user_arguments.get(&token.name).or(token.default_value.as_ref()) {
                Some(value) => expansion.append(&apply_modifiers(&modifiers, value, encoding)),
                None => {
                    expansion.append(&source);
                    expansion.add_missing_argument(MissingArgument {
                        name: token.name,
                        options: token.options,
                    });
                }
            },
            Segment::Cursor => expansion.mark_cursor(),
            Segment::SnippetReference { key, source } => {
                let resolved = if depth < MAXIMUM_REFERENCE_DEPTH {
                    resolve_reference(&key, snippets)
                        .filter(|target| !visited_ids.contains(&target.id()))
                } else {
                    None
                };
                match resolved {
                    Some(target) => {
                        let mut nested_visited = visited_ids.clone();
                        nested_visited.insert(target.id());
                        let nested = expand_text(
                            &target.snippet.text,
                            snippets,
                            context,
                            user_arguments,
                            encoding,
                            depth + 1,
                            &mut nested_visited,
                        );
                        expansion.append_nested(nested);
                    }
                    None => expansion.append(&source),
                }
            }
        }
    }
    expansion
}

/// The snippet a `{snippet:…}` reference names.
///
/// Mirrors upstream's `resolveReference(_:snippets:)`: a disabled snippet is
/// not expandable, and a name is matched before a keyword.
fn resolve_reference<'a>(
    key: &str,
    snippets: &[&'a StoredSnippet],
) -> Option<&'a StoredSnippet> {
    let normalized_key = key.to_lowercase();
    let candidates: Vec<&&StoredSnippet> =
        snippets.iter().filter(|s| s.snippet.is_enabled).collect();

    if let Some(name_match) = candidates
        .iter()
        .find(|s| s.snippet.name.to_lowercase() == normalized_key)
    {
        return Some(name_match);
    }
    candidates.iter().find(|s| {
        s.snippet
            .keyword
            .as_ref()
            .is_some_and(|keyword| keyword.to_lowercase() == normalized_key)
    })
}

/// Apply a placeholder's modifiers to its value.
fn apply_modifiers(modifiers: &[Modifier], value: &str, encoding: ValueEncoding) -> String {
    let mut modified = value.to_string();
    for modifier in modifiers {
        modified = match modifier {
            Modifier::Uppercase => modified.to_uppercase(),
            Modifier::Lowercase => modified.to_lowercase(),
            Modifier::Trim => modified.trim().to_string(),
            Modifier::PercentEncode => percent_encoded(&modified),
            Modifier::JsonStringify => json_escaped(&modified),
            Modifier::Raw => modified,
        };
    }
    // Last, so `| uppercase` can't rewrite the `%xx`; skipped when the template spoke first.
    if encoding == ValueEncoding::PercentEncoding
        && !modifiers.contains(&Modifier::Raw)
        && !modifiers.contains(&Modifier::PercentEncode)
    {
        return percent_encoded(&modified);
    }
    modified
}

/// Percent-encode everything outside RFC 3986's unreserved set.
fn percent_encoded(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        let character = *byte as char;
        if character.is_ascii_alphanumeric() || "-._~".contains(character) {
            encoded.push(character);
        } else {
            encoded.push('%');
            encoded.push_str(&format!("{byte:02X}"));
        }
    }
    encoded
}

/// Escape a value for use inside a JSON string.
fn json_escaped(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            other if (other as u32) < 0x20 => {
                escaped.push_str(&format!("\\u{:04x}", other as u32));
            }
            other => escaped.push(other),
        }
    }
    escaped
}

/// Format a date or time token.
///
/// Mirrors upstream's `format(_:context:)`.
fn format_token(token: &DateTimeToken, context: &ExpansionContext) -> String {
    let locale = for_identifier(
        token
            .locale_identifier
            .as_deref()
            .unwrap_or(context.locale.as_str()),
    );
    let date = apply_offsets(context.now.with_timezone(&context.time_zone), &token.offsets);

    let pattern = match token.format.as_deref() {
        Some(format) => format,
        None => match token.kind {
            DateTimeKind::Date => locale.medium_date,
            DateTimeKind::Time => locale.short_time,
            DateTimeKind::DateTime => locale.medium_datetime,
            DateTimeKind::Weekday => "EEEE",
        },
    };
    format_pattern(pattern, &date, locale)
}

/// Move a date by the offsets a token asks for, in the order they are written.
fn apply_offsets(date: DateTime<FixedOffset>, offsets: &[Offset]) -> DateTime<FixedOffset> {
    let mut date = date;
    for offset in offsets {
        date = match offset.component {
            OffsetComponent::Minute => date + chrono::Duration::minutes(offset.value),
            OffsetComponent::Hour => date + chrono::Duration::hours(offset.value),
            OffsetComponent::Day => {
                let naive = date.naive_local();
                let shifted = naive.date().checked_add_days(chrono::Days::new(offset.value));
                match shifted {
                    Some(day) => day
                        .and_time(naive.time())
                        .and_local_timezone(date.offset().fix())
                        .single()
                        .unwrap_or(date),
                    None => date,
                }
            }
            OffsetComponent::Month | OffsetComponent::Year => {
                let naive = date.naive_local();
                let shifted = shift_months(naive.date(), offset);
                match shifted {
                    Some(day) => day
                        .and_time(naive.time())
                        .and_local_timezone(date.offset().fix())
                        .single()
                        .unwrap_or(date),
                    None => date,
                }
            }
        };
    }
    date
}

/// Move a date by whole months or years, clamping the day to the month.
fn shift_months(date: NaiveDate, offset: &Offset) -> Option<NaiveDate> {
    let (months, years) = match offset.component {
        OffsetComponent::Year => (0, offset.value),
        OffsetComponent::Month => (offset.value, 0),
        _ => return Some(date),
    };

    let total_months = i64::from(date.year()) * 12 + i64::from(date.month0()) + months;
    let year = total_months.div_euclid(12);
    let month = total_months.rem_euclid(12) as u32 + 1;
    let year = year + years;
    if !(1..=9999).contains(&year) {
        return None;
    }
    let year = year as i32;
    let day = date.day().min(days_in_month(year, month));
    NaiveDate::from_ymd_opt(year, month, day)
}

/// The number of days in a month.
fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

/// Format a date with a date pattern.
///
/// The pattern is the subset of the Unicode date pattern language the port
/// supports: the year, month, day, weekday, hour, minute and second fields,
/// the AM/PM marker, and text in single quotes.
fn format_pattern(pattern: &str, date: &DateTime<FixedOffset>, locale: &LocaleData) -> String {
    let characters: Vec<char> = pattern.chars().collect();
    let mut formatted = String::new();
    let mut index = 0usize;

    while index < characters.len() {
        let character = characters[index];
        if character == '\'' {
            index += 1;
            while index < characters.len() {
                if characters[index] == '\'' {
                    if characters.get(index + 1) == Some(&'\'') {
                        formatted.push('\'');
                        index += 2;
                        continue;
                    }
                    index += 1;
                    break;
                }
                formatted.push(characters[index]);
                index += 1;
            }
            continue;
        }

        if !character.is_ascii_alphabetic() {
            formatted.push(character);
            index += 1;
            continue;
        }

        let mut count = 0usize;
        while index + count < characters.len() && characters[index + count] == character {
            count += 1;
        }
        index += count;
        formatted.push_str(&format_field(character, count, date, locale));
    }
    formatted
}

/// Format one field of a date pattern.
fn format_field(
    field: char,
    count: usize,
    date: &DateTime<FixedOffset>,
    locale: &LocaleData,
) -> String {
    match field {
        'y' => match count {
            1 => date.year().to_string(),
            2 => format!("{:02}", date.year().rem_euclid(100)),
            _ => format!("{:04}", date.year()),
        },
        'M' => match count {
            1 => date.month().to_string(),
            2 => format!("{:02}", date.month()),
            3 => locale.months_abbreviated[(date.month() - 1) as usize].to_string(),
            _ => locale.months_full[(date.month() - 1) as usize].to_string(),
        },
        'd' => match count {
            1 => date.day().to_string(),
            _ => format!("{:02}", date.day()),
        },
        'E' => {
            let weekday = date.weekday().num_days_from_sunday() as usize;
            match count {
                1..=3 => locale.weekdays_abbreviated[weekday].to_string(),
                _ => locale.weekdays_full[weekday].to_string(),
            }
        }
        'H' => match count {
            1 => date.hour().to_string(),
            _ => format!("{:02}", date.hour()),
        },
        'h' => {
            let hour = date.hour() % 12;
            let hour = if hour == 0 { 12 } else { hour };
            match count {
                1 => hour.to_string(),
                _ => format!("{:02}", hour),
            }
        }
        'm' => match count {
            1 => date.minute().to_string(),
            _ => format!("{:02}", date.minute()),
        },
        's' => match count {
            1 => date.second().to_string(),
            _ => format!("{:02}", date.second()),
        },
        'a' => {
            if date.hour() < 12 {
                locale.am.to_string()
            } else {
                locale.pm.to_string()
            }
        }
        _ => character.to_string().repeat(count),
    }
}

/// Split a template into its pieces.
///
/// Mirrors upstream's `parseSegments(_:)`: a `{` without a `}` and everything
/// after it is literal text.
fn parse_segments(source: &str) -> Vec<Segment> {
    let characters: Vec<char> = source.chars().collect();
    let mut segments: Vec<Segment> = Vec::new();
    let mut position = 0usize;

    while position < characters.len() {
        let Some(opening) = characters[position..].iter().position(|c| *c == '{') else {
            break;
        };
        let opening = position + opening;
        if position < opening {
            segments.push(Segment::Literal(characters[position..opening].iter().collect()));
        }

        let Some(closing) = characters[opening + 1..].iter().position(|c| *c == '}') else {
            segments.push(Segment::Literal(characters[opening..].iter().collect()));
            return segments;
        };
        let closing = opening + 1 + closing;
        let raw_token: String = characters[opening..=closing].iter().collect();
        let body: String = characters[opening + 1..closing].iter().collect();

        match segment_for(&body, &raw_token) {
            Some(segment) => {
                segments.push(segment);
                position = closing + 1;
            }
            None => {
                segments.push(Segment::Literal("{".to_string()));
                position = opening + 1;
            }
        }
    }

    if position < characters.len() {
        segments.push(Segment::Literal(characters[position..].iter().collect()));
    }
    segments
}

/// The piece a token body names, if it names one at all.
fn segment_for(body: &str, source: &str) -> Option<Segment> {
    let parts = split_on_pipes(body)?;
    let head = parts.first()?;
    let mut modifiers = Vec::new();
    for raw in &parts[1..] {
        modifiers.push(Modifier::parse(raw)?);
    }

    // Structural tokens produce no text, so a modifier on one is malformed.
    if head == "cursor" {
        return if modifiers.is_empty() {
            Some(Segment::Cursor)
        } else {
            None
        };
    }
    if let Some(key) = head.strip_prefix("snippet:") {
        let key = key.trim_matches(|c: char| c.is_whitespace());
        if key.is_empty() || !modifiers.is_empty() {
            return None;
        }
        return Some(Segment::SnippetReference {
            key: key.to_string(),
            source: source.to_string(),
        });
    }

    let token = parse_token(head)?;
    match token.command.as_str() {
        "clipboard" => {
            let offset = int_parameter(&token, "offset", 0)?;
            if offset < 0 || !token.has_only(&["offset"]) {
                return None;
            }
            Some(Segment::Clipboard {
                offset: offset as usize,
                modifiers,
            })
        }
        // `selectedText` is Raycast's spelling, accepted on the way in but never written out.
        "selection" | "selectedtext" => {
            if !token.parameters.is_empty() {
                return None;
            }
            Some(Segment::Selection { modifiers })
        }
        "uuid" => {
            if !token.parameters.is_empty() {
                return None;
            }
            Some(Segment::Uuid { modifiers })
        }
        "date" | "time" | "datetime" | "day" => {
            let date_time = parse_date_time(&token)?;
            Some(Segment::DateTime {
                token: date_time,
                modifiers,
            })
        }
        // `query` is Raycast's spelling of the same token.
        "argument" | "query" => {
            let argument = parse_argument(&token)?;
            Some(Segment::Argument {
                token: argument,
                source: source.to_string(),
                modifiers,
            })
        }
        "snippet" => {
            let name = token
                .parameters
                .iter()
                .find(|(key, _)| key == "name")
                .map(|(_, value)| value.trim_matches(|c: char| c.is_whitespace()).to_string())?;
            if name.is_empty() || !token.has_only(&["name"]) || !modifiers.is_empty() {
                return None;
            }
            Some(Segment::SnippetReference {
                key: name,
                source: source.to_string(),
            })
        }
        _ => None,
    }
}

/// A token's command and its parameters.
struct ParsedToken {
    command: String,
    parameters: Vec<(String, String)>,
}

impl ParsedToken {
    /// Whether the token only carries the parameters it is allowed to carry.
    fn has_only(&self, allowed: &[&str]) -> bool {
        self.parameters
            .iter()
            .all(|(key, _)| allowed.contains(&key.as_str()))
    }
}

/// Read a token body: a command and its parameters.
fn parse_token(head: &str) -> Option<ParsedToken> {
    let characters: Vec<char> = head.chars().collect();
    let mut index = 0usize;
    while index < characters.len() && characters[index].is_whitespace() {
        index += 1;
    }
    let command_start = index;
    while index < characters.len() && !characters[index].is_whitespace() && characters[index] != '='
    {
        index += 1;
    }
    if index == command_start {
        return None;
    }
    let command: String = characters[command_start..index].iter().collect();
    let command = command.to_lowercase();

    let mut parameters: Vec<(String, String)> = Vec::new();
    loop {
        while index < characters.len() && characters[index].is_whitespace() {
            index += 1;
        }
        if index >= characters.len() {
            break;
        }

        let key_start = index;
        while index < characters.len() && !characters[index].is_whitespace() && characters[index] != '='
        {
            index += 1;
        }
        if index == key_start {
            return None;
        }
        let key: String = characters[key_start..index].iter().collect();

        while index < characters.len() && characters[index].is_whitespace() {
            index += 1;
        }
        if index >= characters.len() || characters[index] != '=' {
            return None;
        }
        index += 1;
        while index < characters.len() && characters[index].is_whitespace() {
            index += 1;
        }

        let value = if characters.get(index) == Some(&'"') {
            index += 1;
            let (decoded, consumed) = decode_quoted(&characters[index..])?;
            index += consumed;
            decoded
        } else {
            let (value, consumed) = take_bare_value(&characters[index..])?;
            index += consumed;
            value
        };

        if parameters.iter().any(|(existing, _)| existing == &key) {
            return None;
        }
        parameters.push((key, value));
    }

    Some(ParsedToken { command, parameters })
}

/// Read an integer parameter, or the fallback when it is absent.
fn int_parameter(token: &ParsedToken, key: &str, fallback: i64) -> Option<i64> {
    match token.parameters.iter().find(|(name, _)| name == key) {
        Some((_, raw)) => raw.parse::<i64>().ok(),
        None => Some(fallback),
    }
}

/// Split a token body on the pipes that separate its modifiers.
fn split_on_pipes(body: &str) -> Option<Vec<String>> {
    let characters: Vec<char> = body.chars().collect();
    let mut parts: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut escaped = false;

    for character in characters {
        if escaped {
            current.push(character);
            escaped = false;
            continue;
        }
        match character {
            '\\' if in_quotes => {
                current.push(character);
                escaped = true;
            }
            '"' => {
                in_quotes = !in_quotes;
                current.push(character);
            }
            '|' if !in_quotes => {
                parts.push(current.trim_matches(is_whitespace).to_string());
                current.clear();
            }
            _ => current.push(character),
        }
    }
    if in_quotes || escaped {
        return None;
    }
    parts.push(current.trim_matches(is_whitespace).to_string());
    if parts.iter().any(|part| part.is_empty()) {
        return None;
    }
    Some(parts)
}

/// Read a quoted parameter value.
fn decode_quoted(characters: &[char]) -> Option<(String, usize)> {
    let mut decoded = String::new();
    let mut escaped = false;
    let mut consumed = 0usize;

    for character in characters {
        consumed += 1;
        if escaped {
            match character {
                '\\' => decoded.push('\\'),
                '"' => decoded.push('"'),
                'n' => decoded.push('\n'),
                'r' => decoded.push('\r'),
                't' => decoded.push('\t'),
                _ => return None,
            }
            escaped = false;
            continue;
        }
        match character {
            '\\' => escaped = true,
            '"' => return Some((decoded, consumed)),
            other => decoded.push(other),
        }
    }
    None
}

/// Read an unquoted parameter value, which runs up to the next parameter.
fn take_bare_value(characters: &[char]) -> Option<(String, usize)> {
    let mut index = 0usize;
    let mut end = 0usize;
    while index < characters.len() {
        if characters[index].is_whitespace() {
            while index < characters.len() && characters[index].is_whitespace() {
                index += 1;
            }
            if starts_parameter(&characters[index..]) {
                break;
            }
        } else {
            index += 1;
            end = index;
        }
    }
    if end == 0 {
        return None;
    }
    Some((characters[..end].iter().collect(), end))
}

/// Whether the text starts a `key=` parameter.
fn starts_parameter(characters: &[char]) -> bool {
    let mut index = 0usize;
    while index < characters.len() && !characters[index].is_whitespace() && characters[index] != '='
    {
        index += 1;
    }
    if index == 0 {
        return false;
    }
    let mut rest = index;
    while rest < characters.len() && characters[rest].is_whitespace() {
        rest += 1;
    }
    characters.get(rest) == Some(&'=')
}

/// Read a date or time token's parameters.
fn parse_date_time(token: &ParsedToken) -> Option<DateTimeToken> {
    if !token.has_only(&["offset", "format", "locale"]) {
        return None;
    }
    let format = token
        .parameters
        .iter()
        .find(|(key, _)| key == "format")
        .map(|(_, value)| value.clone());
    let locale_identifier = token
        .parameters
        .iter()
        .find(|(key, _)| key == "locale")
        .map(|(_, value)| value.clone());

    // These are mutually exclusive; a template asking for both is ambiguous.
    if format.is_some() && locale_identifier.is_some() {
        return None;
    }
    if format.as_ref().is_some_and(|value| value.is_empty()) {
        return None;
    }
    if locale_identifier
        .as_ref()
        .is_some_and(|value| value.is_empty())
    {
        return None;
    }

    let mut offsets = Vec::new();
    if let Some((_, raw)) = token.parameters.iter().find(|(key, _)| key == "offset") {
        offsets = parse_offsets(raw)?;
    }

    let kind = match token.command.as_str() {
        "date" => DateTimeKind::Date,
        "time" => DateTimeKind::Time,
        "datetime" => DateTimeKind::DateTime,
        _ => DateTimeKind::Weekday,
    };

    Some(DateTimeToken {
        kind,
        offsets,
        format,
        locale_identifier,
    })
}

/// Read the offsets of a date token: `"+2y +5M"`, `"-3d"`.
fn parse_offsets(raw: &str) -> Option<Vec<Offset>> {
    let pieces: Vec<&str> = raw.split_whitespace().collect();
    if pieces.is_empty() {
        return None;
    }

    let mut offsets = Vec::new();
    for piece in pieces {
        let unit = piece.chars().last()?;
        let component = match unit {
            'm' => OffsetComponent::Minute,
            'h' => OffsetComponent::Hour,
            'd' => OffsetComponent::Day,
            'M' => OffsetComponent::Month,
            'y' => OffsetComponent::Year,
            _ => return None,
        };
        let amount = &piece[..piece.len() - unit.len_utf8()];
        if amount.is_empty() {
            return None;
        }
        let value = amount.parse::<i64>().ok()?;
        offsets.push(Offset { component, value });
    }
    Some(offsets)
}

/// Read an argument token's parameters.
fn parse_argument(token: &ParsedToken) -> Option<ArgumentToken> {
    if !token.has_only(&["name", "default", "options"]) {
        return None;
    }
    let name = token
        .parameters
        .iter()
        .find(|(key, _)| key == "name")
        .map(|(_, value)| value.trim_matches(is_whitespace).to_string())
        .unwrap_or_else(|| "Argument".to_string());
    if name.is_empty() {
        return None;
    }

    let options: Vec<String> = token
        .parameters
        .iter()
        .find(|(key, _)| key == "options")
        .map(|(_, raw)| {
            raw.split(',')
                .map(|option| option.trim_matches(is_whitespace).to_string())
                .filter(|option| !option.is_empty())
                .collect()
        })
        .unwrap_or_default();
    if token.parameters.iter().any(|(key, _)| key == "options") && options.is_empty() {
        return None;
    }

    let default_value = token
        .parameters
        .iter()
        .find(|(key, _)| key == "default")
        .map(|(_, value)| value.clone());

    Some(ArgumentToken {
        name,
        options,
        default_value,
    })
}

/// Whether a character counts as whitespace inside a token body.
fn is_whitespace(character: char) -> bool {
    character.is_whitespace() && character != '\n' && character != '\r'
}

/// How many user-perceived characters a string has.
fn grapheme_count(text: &str) -> usize {
    text.graphemes(true).count()
}

/// The default source of `{uuid}` values.
fn default_uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}
