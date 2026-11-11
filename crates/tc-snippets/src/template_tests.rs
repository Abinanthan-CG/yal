//! The tests of the template engine.
//!
//! Every expectation in this file is read out of upstream Tinycast's own test
//! suite, `upstream/Tests/snippets-test.swift`, in the section that tests the
//! dynamic placeholders. The clock, the locale, the clipboard and the UUIDs are
//! the ones upstream injects, so the expected values are upstream's own.

use std::collections::BTreeMap;

use chrono::{DateTime, FixedOffset, TimeZone};

use crate::snippet::{Snippet, StoredSnippet};
use crate::template::{ExpansionContext, SnippetTemplateEngine, ValueEncoding};

/// The moment upstream's tests expand at: 24 July 2026, 13:05 UTC.
fn now() -> DateTime<FixedOffset> {
    FixedOffset::east_opt(0)
        .expect("UTC is a fixed offset")
        .with_ymd_and_hms(2026, 7, 24, 13, 5, 0)
        .single()
        .expect("the moment is a real one")
}

/// The context upstream's dynamic-placeholder tests use.
fn context() -> ExpansionContext {
    let uuids = std::sync::atomic::AtomicU32::new(0);
    ExpansionContext::new(
        vec![
            "  newest  ".to_string(),
            "older".to_string(),
            "oldest".to_string(),
        ],
        "picked",
        now(),
        "en_US_POSIX",
        FixedOffset::east_opt(0).expect("UTC is a fixed offset"),
    )
    .with_uuid_factory(move || {
        let number = uuids.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        format!("uuid-{number}")
    })
}

/// Expand a template the way upstream's helper does.
fn expand(text: &str) -> crate::template::ExpansionResult {
    expand_with(text, &BTreeMap::new(), ValueEncoding::None)
}

/// Expand a template with arguments.
fn expand_with(
    text: &str,
    arguments: &BTreeMap<String, String>,
    encoding: ValueEncoding,
) -> crate::template::ExpansionResult {
    SnippetTemplateEngine::expand_text(text, &context(), arguments, encoding)
}

/// Upstream: the date and the time styles, and the weekday name.
#[test]
fn the_date_and_the_time_tokens_render_their_styles() {
    assert_eq!(expand("{date}").text, "Jul 24, 2026");
    assert_eq!(expand("{time}").text, "1:05\u{202F}PM");
    assert_eq!(expand("{datetime}").text, "Jul 24, 2026 at 1:05\u{202F}PM");
    assert_eq!(expand("{day}").text, "Friday");
}

/// Upstream: a single signed offset shifts the date.
#[test]
fn an_offset_shifts_the_date() {
    assert_eq!(expand("{date offset=\"+1d\"}").text, "Jul 25, 2026");
    // A bare value needs no quotes.
    assert_eq!(expand("{day offset=-3d}").text, "Tuesday");
}

/// Upstream: several offsets apply in the order they are written.
#[test]
fn several_offsets_apply_in_order() {
    assert_eq!(expand("{date offset=\"+2y +5M\"}").text, "Dec 24, 2028");
    assert_eq!(expand("{time offset=\"+3h +30m\"}").text, "4:35\u{202F}PM");
}

/// Upstream: an offset that cannot be read leaves the token as it was.
#[test]
fn an_offset_that_cannot_be_read_leaves_the_token_literal() {
    assert_eq!(
        expand("{date offset=\"+1w\"}").text,
        "{date offset=\"+1w\"}"
    );
    assert_eq!(expand("{date offset=\"d\"}").text, "{date offset=\"d\"}");
}

/// Upstream: the format parameter, with and without quotes.
#[test]
fn the_format_parameter_picks_the_pattern() {
    assert_eq!(expand("{date format=\"yyyy-MM-dd\"}").text, "2026-07-24");
    assert_eq!(expand("{date format=yyyy-MM-dd}").text, "2026-07-24");
    // A bare format keeps its spaces.
    assert_eq!(expand("{date format=MMMM d, yyyy}").text, "July 24, 2026");
    // A format still applies with an offset.
    assert_eq!(
        expand("{date offset=\"-1d\" format=\"yyyy-MM-dd\"}").text,
        "2026-07-23"
    );
    // A bare value ends at the next parameter.
    assert_eq!(expand("{date format=MMM d offset=+1d}").text, "Jul 25");
    assert_eq!(expand("{date offset=+1d format=MMM d}").text, "Jul 25");
}

/// Upstream: a locale the token cannot take leaves it literal.
#[test]
fn a_locale_with_a_format_leaves_the_token_literal() {
    assert_eq!(
        expand("{date format=\"yyyy\" locale=\"fr-FR\"}").text,
        "{date format=\"yyyy\" locale=\"fr-FR\"}"
    );
}

/// Upstream: each `{uuid}` draws a fresh value.
#[test]
fn each_uuid_token_draws_a_fresh_value() {
    assert_eq!(expand("{uuid}|{uuid}").text, "uuid-1|uuid-2");
}

/// Upstream: the clipboard token reaches back through the history.
#[test]
fn the_clipboard_token_reaches_back_through_the_history() {
    assert_eq!(
        expand("{clipboard offset=1}|{clipboard offset=2}").text,
        "older|oldest"
    );
    // An offset past the end expands to nothing.
    assert!(expand("{clipboard offset=9}").text.is_empty());
    // A negative offset leaves the token literal.
    assert_eq!(
        expand("{clipboard offset=-1}").text,
        "{clipboard offset=-1}"
    );
}

/// Upstream: the modifiers chain, from left to right.
#[test]
fn the_modifiers_chain_from_left_to_right() {
    assert_eq!(expand("{clipboard | trim | uppercase}").text, "NEWEST");
}

/// Upstream: percent-encode escapes everything outside the unreserved set.
#[test]
fn percent_encode_escapes_everything_outside_the_unreserved_set() {
    let mut arguments = BTreeMap::new();
    arguments.insert("U".to_string(), "a b/c?d&e=f~g-h".to_string());
    assert_eq!(
        expand_with(
            "{argument name=\"U\" | percent-encode}",
            &arguments,
            ValueEncoding::None
        )
        .text,
        "a%20b%2Fc%3Fd%26e%3Df~g-h"
    );
}

/// Upstream: json-stringify escapes without adding quotes.
#[test]
fn json_stringify_escapes_without_adding_quotes() {
    let mut arguments = BTreeMap::new();
    arguments.insert("J".to_string(), "a\"b\\c\nd".to_string());
    assert_eq!(
        expand_with(
            "{argument name=\"J\" | json-stringify}",
            &arguments,
            ValueEncoding::None
        )
        .text,
        "a\\\"b\\\\c\\nd"
    );
}

/// Upstream: a value that goes into a URL is encoded, unless the template says
/// otherwise, and never twice.
#[test]
fn an_encoded_value_is_not_encoded_twice() {
    let context = ExpansionContext::new(
        vec!["a b&c".to_string()],
        "a b&c",
        now(),
        "en_US_POSIX",
        FixedOffset::east_opt(0).expect("UTC is a fixed offset"),
    );
    let encoded = SnippetTemplateEngine::expand_text(
        "{clipboard}",
        &context,
        &BTreeMap::new(),
        ValueEncoding::PercentEncoding,
    );
    assert_eq!(encoded.text, "a%20b%26c");

    let raw = SnippetTemplateEngine::expand_text(
        "{clipboard | raw}",
        &context,
        &BTreeMap::new(),
        ValueEncoding::PercentEncoding,
    );
    assert_eq!(raw.text, "a b&c");

    let explicit = SnippetTemplateEngine::expand_text(
        "{clipboard | percent-encode}",
        &context,
        &BTreeMap::new(),
        ValueEncoding::PercentEncoding,
    );
    assert_eq!(explicit.text, "a%20b%26c");
}

/// Upstream: a duplicated parameter leaves the token literal.
#[test]
fn a_duplicated_parameter_leaves_the_token_literal() {
    assert_eq!(
        expand("{date offset=\"+1d\" offset=\"+2d\"}").text,
        "{date offset=\"+1d\" offset=\"+2d\"}"
    );
}

/// Upstream: a parameter on a token that takes none leaves it literal.
#[test]
fn a_parameter_on_a_token_that_takes_none_leaves_it_literal() {
    assert_eq!(expand("{uuid offset=1}").text, "{uuid offset=1}");
}

/// Upstream: the alias of the selection token rejects parameters, exactly as
/// the selection token does.
#[test]
fn the_selection_alias_rejects_parameters() {
    assert_eq!(
        expand("{selectedText offset=1}").text,
        "{selectedText offset=1}"
    );
}

/// Upstream: a malformed token does not count as reading the selection.
#[test]
fn a_malformed_token_does_not_count_as_reading_the_selection() {
    assert!(!expand("{selection offset=1}").reads_selection);
    assert!(expand("{selection}").reads_selection);
}

/// Upstream: the selection is inserted as it is.
#[test]
fn the_selection_is_inserted_as_it_is() {
    assert_eq!(expand("{selection}").text, "picked");
}

/// Upstream: the clipboard is inserted as it is, spaces and all.
#[test]
fn the_clipboard_is_inserted_as_it_is() {
    assert_eq!(expand("{clipboard}").text, "  newest  ");
}

/// Upstream: a token that names nothing stays in the text as it was written.
#[test]
fn an_unknown_token_stays_literal() {
    assert_eq!(expand("{weather}").text, "{weather}");
    assert_eq!(expand("a {b} c").text, "a {b} c");
}

/// Upstream: the arguments a template declares, and the ones still owed.
#[test]
fn the_declared_arguments_are_the_ones_the_template_asks_for() {
    let declared = SnippetTemplateEngine::declared_arguments(
        "{argument name=\"A\"} and {argument name=\"B\" default=\"x\"}",
    );
    assert_eq!(declared.len(), 2);
    assert_eq!(declared[0].name, "A");
    assert!(!declared[0].is_optional, "A has no default, so it is owed");
    assert_eq!(declared[1].name, "B");
    assert!(
        declared[1].is_optional,
        "B has a default, so it is not owed"
    );
}

/// Upstream: an argument that was not supplied is owed, and its name is
/// reported.
#[test]
fn an_argument_that_was_not_supplied_is_owed() {
    let result = expand("{argument name=\"A\"}");
    assert_eq!(result.missing_arguments.len(), 1);
    assert_eq!(result.missing_arguments[0].name, "A");
}

/// Upstream: an argument with a default expands without asking.
#[test]
fn an_argument_with_a_default_expands_without_asking() {
    let result = expand("{argument name=\"A\" default=\"fallback\"}");
    assert_eq!(result.text, "fallback");
    assert!(result.missing_arguments.is_empty());
}

/// Upstream: a `{snippet:Name}` reference expands the other snippet's text.
#[test]
fn a_snippet_reference_expands_the_other_snippet() {
    let child = StoredSnippet::for_content(
        "/tmp/child.md",
        Snippet::new("Child", "the child text"),
        "the child text",
    );
    let parent = StoredSnippet::for_content(
        "/tmp/parent.md",
        Snippet::new("Parent", "[{snippet:Child}]"),
        "[{snippet:Child}]",
    );
    let result = SnippetTemplateEngine::expand(
        &parent,
        &[child, parent.clone()],
        &context(),
        &BTreeMap::new(),
    );
    assert_eq!(result.text, "[the child text]");
}

/// Upstream: the `{cursor}` token says where the cursor goes afterward.
#[test]
fn the_cursor_token_says_where_the_cursor_goes() {
    let result = expand("a{cursor}b");
    assert_eq!(result.text, "ab");
    assert_eq!(result.cursor_offset_from_end, Some(1));
}

/// Upstream: the `uppercase` and `lowercase` modifiers.
#[test]
fn the_case_modifiers_change_the_case() {
    assert_eq!(expand("{selection | uppercase}").text, "PICKED");
    assert_eq!(expand("{selection | lowercase}").text, "picked");
}

/// Upstream: another locale renders the date in that locale.
#[test]
fn another_locale_renders_the_date_in_that_locale() {
    let french = ExpansionContext::new(
        vec!["copied".to_string()],
        "picked",
        now(),
        "fr-FR",
        FixedOffset::east_opt(0).expect("UTC is a fixed offset"),
    );
    let result = SnippetTemplateEngine::expand_text(
        "{date}",
        &french,
        &BTreeMap::new(),
        ValueEncoding::None,
    );
    assert_eq!(result.text, "24 juil. 2026");
}

/// Upstream: an argument may offer the choices it accepts.
#[test]
fn an_argument_may_offer_its_choices() {
    let declared =
        SnippetTemplateEngine::declared_arguments("{argument name=\"A\" options=\"a, b, c\"}");
    assert_eq!(declared.len(), 1);
    assert_eq!(declared[0].name, "A");
    assert_eq!(declared[0].options, vec!["a", "b", "c"]);
}

/// Upstream: a reference to a snippet that does not exist stays literal.
#[test]
fn a_reference_to_a_missing_snippet_stays_literal() {
    let parent = StoredSnippet::for_content(
        "/tmp/parent.md",
        Snippet::new("Parent", "[{snippet:Nope}]"),
        "[{snippet:Nope}]",
    );
    let result = SnippetTemplateEngine::expand(
        &parent,
        std::slice::from_ref(&parent),
        &context(),
        &BTreeMap::new(),
    );
    assert_eq!(result.text, "[{snippet:Nope}]");
}

/// Upstream: a disabled snippet cannot be reached.
#[test]
fn a_disabled_snippet_cannot_be_reached() {
    let child = StoredSnippet::for_content(
        "/tmp/child.md",
        Snippet::with_options("Child", "the child text", None, false, false),
        "the child text",
    );
    let parent = StoredSnippet::for_content(
        "/tmp/parent.md",
        Snippet::new("Parent", "[{snippet:Child}]"),
        "[{snippet:Child}]",
    );
    let result = SnippetTemplateEngine::expand(
        &parent,
        &[child, parent.clone()],
        &context(),
        &BTreeMap::new(),
    );
    assert_eq!(result.text, "[{snippet:Child}]");
}
