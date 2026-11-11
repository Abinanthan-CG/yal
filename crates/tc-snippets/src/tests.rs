//! The tests of the snippet model layer.
//!
//! Every expectation in this file is read out of upstream Tinycast's own test
//! suite, `upstream/Tests/snippets-test.swift`, or out of the upstream source
//! the behaviour comes from. The test names name the upstream behaviour they
//! pin.

use std::path::{Path, PathBuf};

use crate::markdown::SnippetMarkdownSerializer;
use crate::snippet::Snippet;

/// The file the upstream tests parse against.
fn file() -> PathBuf {
    PathBuf::from("/tmp/snippet-file.md")
}

/// The file whose name is used for the fallback name.
fn blank_name_file() -> PathBuf {
    PathBuf::from("/tmp/blank-name-file.md")
}

/// Upstream: the serialized form starts with the front matter, in the order
/// `name`, `keyword`, `enabled`, `show_confirmation`.
#[test]
fn the_serialized_form_starts_with_its_front_matter() {
    let snippet = Snippet::with_options(
        "Quote \" slash \\ line\nreturn\rtab\t雪",
        "Body",
        Some("!\"\\\n\t".to_string()),
        false,
        true,
    );
    let source = SnippetMarkdownSerializer::serialize(&snippet);
    assert!(
        source.starts_with(
            "---\nname: \"Quote \\\" slash \\\\ line\\nreturn\\rtab\\t雪\"\nkeyword: \"!\\\"\\\\\\n\\t\"\nenabled: false\nshow_confirmation: true\n---\n"
        ),
        "got {source:?}"
    );
}

/// Upstream: a quoted scalar cannot smuggle a front matter key into the file.
#[test]
fn a_quoted_scalar_cannot_inject_a_key() {
    let snippet = Snippet::new("Name", "Body");
    let injection = Snippet::with_options(
        "Injected",
        "Body",
        Some("!key\nenabled: false".to_string()),
        true,
        false,
    );
    let source = SnippetMarkdownSerializer::serialize(&injection);
    assert!(
        !source.contains("\nenabled: false\nenabled:"),
        "got {source:?}"
    );
    assert!(source.contains("keyword:"), "got {source:?}");
    assert_eq!(snippet.name, "Name");
}

/// Upstream: a carriage return in a keyword cannot inject a key either.
#[test]
fn a_carriage_return_cannot_inject_a_key() {
    let injection = Snippet::with_options(
        "Injected",
        "Body",
        Some("!key\r\nshow_confirmation: false".to_string()),
        true,
        false,
    );
    let source = SnippetMarkdownSerializer::serialize(&injection);
    assert!(
        !source.contains("\r\nshow_confirmation: false\n"),
        "got {source:?}"
    );
}

/// Upstream: CRLF front matter parses its keys, and the body starts after the
/// closing delimiter's own line break.
#[test]
fn crlf_front_matter_parses_its_keys() {
    let content = "---\r\nname: \"CRLF\"\r\nshow_confirmation: true\r\n---\r\n\r\nBody\r\n";
    let parsed = SnippetMarkdownSerializer::parse(content, &file()).expect("the file parses");
    assert!(parsed.shows_confirmation);
    assert_eq!(parsed.text, "\r\nBody\r\n");
}

/// Upstream: a front matter that leaves `show_confirmation` out means false.
#[test]
fn a_missing_show_confirmation_defaults_to_false() {
    let content = "---\nname: \"No HUD\"\n---\nBody";
    let parsed = SnippetMarkdownSerializer::parse(content, &file()).expect("the file parses");
    assert!(!parsed.shows_confirmation);
}

/// Upstream: the booleans are strict.
#[test]
fn a_boolean_has_to_be_exactly_true_or_false() {
    let content = "---\nshow_confirmation: TRUE\n---\n";
    assert!(SnippetMarkdownSerializer::parse(content, &file()).is_err());
}

/// Upstream: a `---` inside the body is part of the body.
#[test]
fn a_delimiter_inside_the_body_stays_literal() {
    let content = "---\nname: \"Delimiter Body\"\nenabled: true\n---\nFirst\n---\nLast\n";
    let parsed = SnippetMarkdownSerializer::parse(content, &file()).expect("the file parses");
    assert_eq!(parsed.text, "First\n---\nLast\n");
}

/// Upstream: a file that does not open with an exact `---` line is all body.
#[test]
fn a_file_without_front_matter_is_all_body() {
    let content = "--- not frontmatter\n\nBody";
    let parsed = SnippetMarkdownSerializer::parse(content, &file()).expect("the file parses");
    assert_eq!(parsed.text, content);
    assert_eq!(parsed.name, "Snippet File");
}

/// Upstream: a blank name falls back to the file name.
#[test]
fn a_blank_name_falls_back_to_the_file_name() {
    let blank =
        SnippetMarkdownSerializer::parse("---\nname: \" \\t \"\n---\nBody", &blank_name_file())
            .expect("the file parses");
    let empty = SnippetMarkdownSerializer::parse("---\nname: \"\"\n---\nBody", &blank_name_file())
        .expect("the file parses");
    assert_eq!(blank.name, "Blank Name File");
    assert_eq!(empty.name, "Blank Name File");
}

/// Upstream: the front matter has to be closed.
#[test]
fn a_missing_closing_delimiter_is_rejected() {
    let content = "---\nname: \"Broken\"\n";
    assert!(SnippetMarkdownSerializer::parse(content, &file()).is_err());
}

/// Upstream: the closing delimiter has to be exact.
#[test]
fn a_closing_delimiter_with_a_trailing_space_is_rejected() {
    let content = "---\nname: \"Broken\"\n--- \n";
    assert!(SnippetMarkdownSerializer::parse(content, &file()).is_err());
}

/// Upstream: a string value has to be quoted.
#[test]
fn an_unquoted_scalar_is_rejected() {
    let content = "---\nname: Broken\n---\n";
    assert!(SnippetMarkdownSerializer::parse(content, &file()).is_err());
}

/// Upstream: only a few escapes exist.
#[test]
fn an_unknown_escape_is_rejected() {
    let content = "---\nname: \"Bad\\q\"\n---\n";
    assert!(SnippetMarkdownSerializer::parse(content, &file()).is_err());
}

/// Upstream: a key may not be written twice.
#[test]
fn a_duplicate_key_is_rejected() {
    let content = "---\nname: \"A\"\nname: \"B\"\n---\n";
    assert!(SnippetMarkdownSerializer::parse(content, &file()).is_err());
}

/// Upstream: only the four keys are known.
#[test]
fn an_unknown_key_is_rejected() {
    let content = "---\nunknown: \"value\"\n---\n";
    assert!(SnippetMarkdownSerializer::parse(content, &file()).is_err());
}

/// Upstream: a file round trips through the serializer.
#[test]
fn a_snippet_round_trips_through_its_file() {
    let snippet = Snippet::with_options(
        "Round Trip",
        "First line\nSecond line\n",
        Some("!round".to_string()),
        true,
        false,
    );
    let source = SnippetMarkdownSerializer::serialize(&snippet);
    let parsed = SnippetMarkdownSerializer::parse(&source, &file()).expect("the file parses");
    assert_eq!(parsed.name, snippet.name);
    assert_eq!(parsed.text, snippet.text);
    assert_eq!(parsed.keyword, snippet.keyword);
    assert_eq!(parsed.is_enabled, snippet.is_enabled);
    assert_eq!(parsed.shows_confirmation, snippet.shows_confirmation);
}

/// Upstream: the file a snippet is stored under is a slug of its name.
#[test]
fn the_file_name_is_a_slug_of_the_name() {
    assert_eq!(SnippetMarkdownSerializer::slug("Same"), "same");
    assert_eq!(
        SnippetMarkdownSerializer::slug("Hello World"),
        "hello-world"
    );
    assert_eq!(
        SnippetMarkdownSerializer::slug("Hello, World!"),
        "hello-world"
    );
    assert_eq!(SnippetMarkdownSerializer::slug("   "), "snippet");
}

/// Upstream: the name in the front matter does not become the path identity.
#[test]
fn the_front_matter_name_does_not_replace_the_path_identity() {
    let path = Path::new("/tmp/odd-file-name.md");
    let stored = crate::snippet::StoredSnippet::for_content(
        path,
        Snippet::new("Frontmatter Name", "Body"),
        "Body",
    );
    assert!(stored.id().contains("odd-file-name.md"));
    assert_eq!(stored.snippet.name, "Frontmatter Name");
}

/// Upstream: a stored snippet's row id names the file it lives in.
#[test]
fn a_stored_snippet_names_its_file_in_the_entry_id() {
    let stored = crate::snippet::StoredSnippet::for_content(
        Path::new("/tmp/one.md"),
        Snippet::new("One", "Body"),
        "Body",
    );
    assert!(stored.entry_id().starts_with("snippet:"));
    assert_eq!(
        crate::snippet::StoredSnippet::id_from_entry_id(&stored.entry_id()),
        Some(stored.id())
    );
    assert_eq!(
        crate::snippet::StoredSnippet::id_from_entry_id("app:notepad"),
        None
    );
}

/// Upstream: the fingerprint of a file's bytes detects an edit made elsewhere.
#[test]
fn the_fingerprint_changes_when_the_bytes_change() {
    let first = crate::snippet::SnippetSourceRevision::new("Hello");
    let same = crate::snippet::SnippetSourceRevision::new("Hello");
    let other = crate::snippet::SnippetSourceRevision::new("Hello!");
    assert_eq!(first.as_str(), same.as_str());
    assert_ne!(first.as_str(), other.as_str());
    // Upstream writes the byte count and the hash: `5:1234567890abcdef`.
    assert!(first.as_str().starts_with("5:"), "got {}", first.as_str());
}

/// Upstream: a new snippet is enabled and does not ask for confirmation.
#[test]
fn a_new_snippet_is_enabled_and_asks_nothing() {
    let snippet = Snippet::default();
    assert!(snippet.is_enabled);
    assert!(!snippet.shows_confirmation);
    assert!(snippet.keyword.is_none());
    assert_eq!(snippet.name, "");
    assert_eq!(snippet.text, "");
}

/// Upstream: the keys are not case-sensitive.
#[test]
fn the_keys_are_not_case_sensitive() {
    let content = "---\nName: \"Cased\"\nENABLED: false\n---\nBody";
    let parsed = SnippetMarkdownSerializer::parse(content, &file()).expect("the file parses");
    assert_eq!(parsed.name, "Cased");
    assert!(!parsed.is_enabled);
}

/// Upstream: the keyword is optional, and the two booleans have their defaults.
#[test]
fn the_defaults_are_the_ones_upstream_uses() {
    let content = "---\nname: \"Defaults\"\n---\nBody";
    let parsed = SnippetMarkdownSerializer::parse(content, &file()).expect("the file parses");
    assert!(parsed.keyword.is_none());
    assert!(parsed.is_enabled);
    assert!(!parsed.shows_confirmation);
}

/// Upstream: the error names the problem, and where it is.
#[test]
fn an_error_names_the_problem_and_where_it_is() {
    let error = SnippetMarkdownSerializer::parse("---\nname: Broken\n---\n", &file())
        .expect_err("the file does not parse");
    assert_eq!(error.line, 2);
    assert!(
        error.reason.contains("double quotes"),
        "got {}",
        error.reason
    );
    assert!(
        error.to_string().contains("snippet-file.md:2"),
        "got {error}"
    );
}

/// The locale table: French is French, and an unknown locale is English.
#[test]
fn a_locale_is_found_by_its_identifier() {
    assert_eq!(crate::locale::for_identifier("fr-FR").identifier, "fr_FR");
    assert_eq!(crate::locale::for_identifier("fr_FR").identifier, "fr_FR");
    assert_eq!(
        crate::locale::for_identifier("en_US_POSIX").identifier,
        "en_US_POSIX"
    );
    // An unknown locale falls back to English rather than to nothing.
    assert_eq!(
        crate::locale::for_identifier("de-DE").identifier,
        "en_US_POSIX"
    );
}

/// The locale table carries the names upstream's date tokens print.
#[test]
fn the_locale_table_carries_its_names() {
    let french = crate::locale::for_identifier("fr-FR");
    assert_eq!(french.months_abbreviated[6], "juil.");
    assert_eq!(french.months_full[0], "janvier");
    let english = crate::locale::for_identifier("en");
    assert_eq!(english.months_abbreviated[6], "Jul");
    assert_eq!(english.weekdays_full[5], "Friday");
    // The narrow no-break space before the marker is what CLDR uses.
    assert!(english.short_time.contains('\u{202F}'));
}
