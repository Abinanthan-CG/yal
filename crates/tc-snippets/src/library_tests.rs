//! The tests of the snippet library on disk, the store, the keyword policy and
//! the Raycast import.
//!
//! Every expectation in this file is read out of upstream Tinycast's own test
//! suite, `upstream/Tests/snippets-test.swift`.

use std::time::Duration;

use serde_json::json;

use crate::keyword::{Input, Keyword, SnippetKeywordPolicy};
use crate::raycast::RaycastSnippetImport;
use crate::repository::SnippetRepository;
use crate::snippet::Snippet;
use crate::store::SnippetsStore;

/// A directory of its own, so the tests never touch the user's real folder.
fn temporary_folder(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("yal-snippets-{name}"));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("the temporary folder can be made");
    root
}

/// Upstream: a Raycast export is a list of objects with a title, a text and an
/// optional keyword.
#[test]
fn a_raycast_export_is_read() {
    let imported = RaycastSnippetImport::parse(&json!([
        {"title": "Email", "text": "person@example.com", "keyword": "  !email  "},
        {"title": "Multiline 雪", "text": "First\nSecond"},
        {"title": "Blank Keyword", "text": "Body", "keyword": "   "},
    ]));
    // Upstream: "Raycast import keeps valid entries and source order".
    assert_eq!(
        imported.iter().map(|s| s.name.clone()).collect::<Vec<_>>(),
        vec!["Email", "Multiline 雪", "Blank Keyword"]
    );
    // Upstream: "Raycast import preserves text and Unicode".
    assert_eq!(imported[1].text, "First\nSecond");
    // Upstream: "Raycast import trims keywords and normalizes blanks".
    assert_eq!(imported[0].keyword.as_deref(), Some("!email"));
    assert_eq!(imported[2].keyword, None);
    // Upstream: "Raycast import uses safe Tinycast defaults".
    assert!(imported
        .iter()
        .all(|s| s.is_enabled && !s.shows_confirmation));
}

/// Upstream: "Raycast import ignores an unrecognized container".
#[test]
fn an_unrecognized_raycast_container_is_ignored() {
    assert!(RaycastSnippetImport::parse(&json!({"snippets": []})).is_empty());
}

/// Upstream: "create never overwrites an existing slug".
#[test]
fn a_created_snippet_never_overwrites_another() {
    let root = temporary_folder("create");
    let repository = SnippetRepository::new(&root);
    let first = repository
        .create(&Snippet::new("Same", "One"))
        .expect("the snippet is created");
    let second = repository
        .create(&Snippet::new("Same", "Two"))
        .expect("the snippet is created");
    assert_eq!(first.file_url.file_name().unwrap(), "same.md");
    assert_eq!(second.file_url.file_name().unwrap(), "same-2.md");
}

/// Upstream: "batch import creates every snippet without overwriting duplicate
/// names", and "batch import round-trips through Markdown storage".
#[test]
fn a_batch_import_round_trips_through_the_files() {
    let root = temporary_folder("batch");
    let repository = SnippetRepository::new(&root);
    let imported = repository
        .create_many(&[
            Snippet::new("Imported", "One"),
            Snippet::new("Imported", "Two"),
        ])
        .expect("the snippets are created");
    let names: Vec<String> = imported
        .iter()
        .map(|record| {
            record
                .file_url
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(names, vec!["imported.md", "imported-2.md"]);

    let reloaded = repository.load().expect("the folder is read");
    assert_eq!(
        reloaded
            .records
            .iter()
            .filter(|record| record.snippet.name == "Imported")
            .count(),
        2
    );
}

/// Upstream: the front matter name does not become the path identity.
#[test]
fn the_front_matter_name_does_not_replace_the_path_identity() {
    let root = temporary_folder("identity");
    let repository = SnippetRepository::new(&root);
    let created = repository
        .create(&Snippet::new("Frontmatter Name", "Body"))
        .expect("the snippet is created");
    assert!(created.id().contains("frontmatter-name.md"));
    assert_eq!(created.snippet.name, "Frontmatter Name");
}

/// Upstream: a file that cannot be read is reported, and the others are still
/// read.
#[test]
fn a_file_that_cannot_be_read_is_reported_and_the_others_are_read() {
    let root = temporary_folder("issues");
    let repository = SnippetRepository::new(&root);
    std::fs::write(root.join("good.md"), "---\nname: \"Good\"\n---\nBody")
        .expect("the file is written");
    std::fs::write(root.join("bad.md"), "---\nname: Broken\n---\n").expect("the file is written");
    let snapshot = repository.load().expect("the folder is read");
    assert_eq!(snapshot.records.len(), 1);
    assert_eq!(snapshot.records[0].snippet.name, "Good");
    assert_eq!(snapshot.issues.len(), 1);
    assert!(
        snapshot.issues[0].message.contains("bad.md"),
        "got {}",
        snapshot.issues[0].message
    );
}

/// Upstream: a save refuses to overwrite a file that changed underneath it.
#[test]
fn a_save_refuses_to_overwrite_a_file_that_changed() {
    let root = temporary_folder("stale");
    let repository = SnippetRepository::new(&root);
    let created = repository
        .create(&Snippet::new("Stale", "Body"))
        .expect("the snippet is created");
    // Somebody else edits the file.
    std::fs::write(created.file_url.clone(), "---\nname: \"Stale\"\n---\nOther")
        .expect("the file is written");
    let edited = created.clone();
    let result = repository.save(
        &Snippet::new("Stale", "New body"),
        &edited.file_url,
        &edited.source_revision,
    );
    assert!(result.is_err(), "a stale save has to be refused");
}

/// Upstream: deleting a snippet removes its file.
#[test]
fn deleting_a_snippet_removes_its_file() {
    let root = temporary_folder("delete");
    let repository = SnippetRepository::new(&root);
    let created = repository
        .create(&Snippet::new("Doomed", "Body"))
        .expect("the snippet is created");
    assert!(created.file_url.exists());
    repository
        .delete(&created.file_url, &created.source_revision)
        .expect("the snippet is deleted");
    assert!(!created.file_url.exists());
}

/// Upstream: the store is the collection the app shows.
#[test]
fn the_store_holds_the_snippets() {
    let root = temporary_folder("store");
    let mut store = SnippetsStore::new(SnippetRepository::new(&root));
    store.start().expect("the store starts");
    store
        .create(&Snippet::new("One", "Body"))
        .expect("the snippet is created");
    assert_eq!(store.snippets().len(), 1);
    assert_eq!(store.snippets()[0].snippet.name, "One");
    let id = store.snippets()[0].id();
    store.delete(&id).expect("the snippet is deleted");
    assert!(store.snippets().is_empty());
}

/// Upstream: a keyword is trimmed and lowercased, and the deletion count is
/// the length of the trimmed value.
#[test]
fn a_keyword_is_trimmed_and_lowercased() {
    let keyword = Keyword::new("snippet-1", "  !Email  ");
    assert_eq!(keyword.value, "!email");
    assert_eq!(keyword.deletion_count, 6);
}

/// Upstream: the keyword buffer matches the keyword that was typed and reports
/// how many characters have to be deleted.
#[test]
fn the_buffer_matches_the_keyword_that_was_typed() {
    let mut policy = SnippetKeywordPolicy::new(vec![Keyword::new("snippet-1", "!email")]);
    let now = Duration::from_millis(0);
    assert!(policy
        .process(&Input::Text("hi ".to_string()), now)
        .is_none());
    let matched = policy
        .process(&Input::Text("!email".to_string()), now)
        .expect("the keyword matches");
    assert_eq!(matched.snippet_id, "snippet-1");
    assert_eq!(matched.keyword, "!email");
    assert_eq!(matched.deletion_count, 6);
}

/// Upstream: a keyword that is not typed is not matched.
#[test]
fn a_keyword_that_is_not_typed_is_not_matched() {
    let mut policy = SnippetKeywordPolicy::new(vec![Keyword::new("snippet-1", "!email")]);
    let now = Duration::from_millis(0);
    assert!(policy
        .process(&Input::Text("hello".to_string()), now)
        .is_none());
    assert_eq!(policy.buffer(), "hello");
}

/// Upstream: the longest match wins, so a more specific keyword takes
/// precedence.
#[test]
fn the_longest_match_wins() {
    let mut policy = SnippetKeywordPolicy::new(vec![
        Keyword::new("snippet-1", "!e"),
        Keyword::new("snippet-2", "!email"),
    ]);
    let now = Duration::from_millis(0);
    let matched = policy
        .process(&Input::Text("!email".to_string()), now)
        .expect("the keyword matches");
    assert_eq!(matched.snippet_id, "snippet-2");
    assert_eq!(matched.keyword, "!email");
}

/// Upstream: an empty keyword is rejected.
#[test]
fn an_empty_keyword_is_rejected() {
    let policy = SnippetKeywordPolicy::new(vec![Keyword::new("snippet-1", "   ")]);
    assert!(policy.keywords().is_empty());
}

/// Upstream: a reset clears the buffer.
#[test]
fn a_reset_clears_the_buffer() {
    let mut policy = SnippetKeywordPolicy::new(vec![Keyword::new("snippet-1", "!email")]);
    let now = Duration::from_millis(0);
    policy.process(&Input::Text("hello".to_string()), now);
    policy.process(&Input::Reset, now);
    assert_eq!(policy.buffer(), "");
}
