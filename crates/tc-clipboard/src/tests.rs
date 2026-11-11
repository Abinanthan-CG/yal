//! Tests for the clipboard model layer.
//!
//! Every case here is derived from upstream's own test files, and each test
//! names the upstream behaviour it pins. The expected values are read from
//! upstream's source or its tests, never computed from this port's
//! implementation.
//!
//! Upstream oracles:
//! - `Tests/clipboard-test.swift`
//! - `Tests/clipboard-search-test.swift`
//! - `Tests/paste-sequence-test.swift`
//! - `Tests/raycast-test.swift`

use crate::color::{ColorFormat, ColorValue};
use crate::file_kind::ClipboardFileKind;
use crate::filter::ClipboardFilter;
use crate::item::{classify, ClipboardItem, ItemKind, TextForm};
use crate::paste_sequence::PasteSequence;
use crate::policy::CapturePolicy;
use crate::raycast;
use crate::store::{ClipboardStore, MEMORY_WINDOW};
use tempfile::TempDir;

/// A scratch store, torn down afterwards.
fn store() -> (TempDir, ClipboardStore) {
    let directory = TempDir::new().expect("a scratch directory");
    let store = ClipboardStore::open(directory.path()).expect("the store opens");
    (directory, store)
}

/// The texts a query answers with, under a filter.
fn texts(store: &ClipboardStore, query: &str, filter: ClipboardFilter) -> Vec<String> {
    store
        .search(query, filter)
        .into_iter()
        .filter_map(|item| item.text)
        .collect()
}

/// The entry whose text is the given one.
fn item(store: &ClipboardStore, text: &str) -> ClipboardItem {
    store
        .items()
        .iter()
        .find(|item| item.text.as_deref() == Some(text))
        .cloned()
        .expect("the entry is listed")
}

// MARK: - The entry record and its kinds

#[test]
fn an_entry_holds_the_fields_upstream_records() {
    // Upstream `ClipboardItem`: the id, kind, text, image path, creation
    // time, source bundle ID and pin stamp.
    let entry = ClipboardItem::text("hello", Some("com.example.app".to_string()));
    assert_eq!(entry.kind, ItemKind::Text);
    assert_eq!(entry.text.as_deref(), Some("hello"));
    assert_eq!(entry.image_path, None);
    assert_eq!(entry.source_bundle_id.as_deref(), Some("com.example.app"));
    assert_eq!(entry.pinned_at, None);
    assert!(!entry.is_pinned());
    assert!(!entry.id.is_empty());
}

#[test]
fn a_text_entry_is_its_own_plain_text() {
    // Upstream `plainTextSkipsTheFile`: text is itself.
    assert_eq!(ClipboardItem::text("hi", None).plain_text(), Some("hi"));
}

#[test]
fn a_file_entry_is_its_own_plain_text() {
    // Upstream `plainTextSkipsTheFile`: a file is its path.
    assert_eq!(
        ClipboardItem::file("/a/b.pdf", None).plain_text(),
        Some("/a/b.pdf")
    );
}

#[test]
fn an_image_entry_has_no_plain_text() {
    // Upstream `plainTextSkipsTheFile`: an image has no plain text.
    assert_eq!(ClipboardItem::image("/a/b.png", None).plain_text(), None);
}

#[test]
fn only_an_image_offers_text_extraction() {
    // Upstream `offersTextExtraction`: a captured blob, or an image file,
    // never text and never a PDF.
    assert!(ClipboardItem::image("/tmp/shot.png", None).offers_text_extraction());
    assert!(!ClipboardItem::text("hello", None).offers_text_extraction());
    assert!(ClipboardItem::file("/Users/me/shot.png", None).offers_text_extraction());
    assert!(!ClipboardItem::file("/Users/me/notes.txt", None).offers_text_extraction());
    assert!(!ClipboardItem::file("/Users/me/report.pdf", None).offers_text_extraction());
}

#[test]
fn an_image_without_its_blob_does_not_offer_text_extraction() {
    // Upstream `offersTextExtraction`: an image entry without its blob does
    // not.
    let mut entry = ClipboardItem::image("/tmp/shot.png", None);
    entry.image_path = None;
    assert!(!entry.offers_text_extraction());
}

#[test]
fn a_file_entry_is_never_a_text_form() {
    // Upstream `filePathsAreNeverATextForm`: a path is not prose.
    let entry = ClipboardItem::file("/Users/me/apple.com/#FF5733.txt", None);
    assert_eq!(entry.text_form(), None);
    assert_eq!(entry.color_value(), None);
    assert!(!ClipboardFilter::Link.matches(&entry));
    assert!(!ClipboardFilter::Text.matches(&entry));
    assert!(ClipboardFilter::File.matches(&entry));
}

#[test]
fn an_image_entry_is_never_a_text_form() {
    // Upstream `textFormClassification`: an image has no text form.
    assert_eq!(ClipboardItem::image("/tmp/x.png", None).text_form(), None);
}

// MARK: - The file kinds

#[test]
fn a_file_kind_is_resolved_from_its_extension() {
    // Upstream `fileKindClassification`.
    assert_eq!(
        ClipboardFileKind::of("/a/b.mov", false),
        ClipboardFileKind::Movie
    );
    assert_eq!(
        ClipboardFileKind::of("/a/b.png", false),
        ClipboardFileKind::Image
    );
    assert_eq!(
        ClipboardFileKind::of("/a/b.pdf", false),
        ClipboardFileKind::Pdf
    );
    assert_eq!(
        ClipboardFileKind::of("/a/b.m4a", false),
        ClipboardFileKind::Audio
    );
}

#[test]
fn a_bare_name_is_not_a_folder() {
    // Upstream `fileKindClassification`: a bare name is not a folder.
    assert_eq!(
        ClipboardFileKind::of("/a/README", false),
        ClipboardFileKind::Other
    );
}

#[test]
fn a_directory_is_a_folder() {
    // Upstream `fileKindClassification`: a folder is one.
    assert_eq!(
        ClipboardFileKind::of("/a/b", true),
        ClipboardFileKind::Folder
    );
}

#[test]
fn only_a_movie_and_an_audio_are_playable() {
    // Upstream `ClipboardFileKind.isPlayable`.
    assert!(ClipboardFileKind::Movie.is_playable());
    assert!(ClipboardFileKind::Audio.is_playable());
    assert!(!ClipboardFileKind::Image.is_playable());
    assert!(!ClipboardFileKind::Pdf.is_playable());
    assert!(!ClipboardFileKind::Folder.is_playable());
    assert!(!ClipboardFileKind::Other.is_playable());
}

// MARK: - The text classification

#[test]
fn a_link_is_classified_as_a_link() {
    // Upstream `textFormClassification`: the links.
    for text in [
        "https://apple.com",
        "http://apple.com/path?q=1",
        "apple.com",
        "apple.com/store",
        "www.Apple.com",
        "vscode://file/tmp/x",
        "docs.google.com",
        "bit.ly/abc",
    ] {
        assert_eq!(classify(text), TextForm::Link, "{text} is a link");
    }
}

#[test]
fn an_address_is_classified_as_an_address() {
    // Upstream `textFormClassification`: the addresses.
    for text in [
        "hi@apple.com",
        "mailto:hi@apple.com",
        "first.last@mail.example.co.uk",
    ] {
        assert_eq!(classify(text), TextForm::Email, "{text} is an address");
    }
}

#[test]
fn a_filename_is_never_a_link() {
    // Upstream `textFormClassification`: extensions that collide with a real
    // TLD are the whole reason for the TLD set.
    for text in [
        "report.pdf",
        "index.html",
        "App.swift",
        "data.json",
        "Safari.app",
        "image.png",
        "hello world",
        "visit apple.com today",
        "3.14",
        "",
        "   ",
        "no-dot-at-all",
        "two@at@signs.com",
        "@apple.com",
        "hi@localhost",
        "line one\nline two",
    ] {
        assert_eq!(classify(text), TextForm::Plain, "{text} is plain text");
    }
}

#[test]
fn a_colour_is_its_own_form() {
    // Upstream `textFormClassification`: a colour is its own form, and beats
    // the prose branch even when written with spaces.
    for text in ["#FF5733", "#0f0", "rgb(255, 87, 51)", "hsl(11, 100%, 60%)"] {
        assert_eq!(classify(text), TextForm::Color, "{text} is a colour");
    }
}

#[test]
fn an_over_long_token_is_plain_by_definition() {
    // Upstream `textFormClassification`: past the scan cap, so a multi-MB
    // copy is never walked looking for a scheme.
    let text = format!("https://apple.com/{}", "a".repeat(4096));
    assert_eq!(classify(&text), TextForm::Plain);
}

// MARK: - The filter

#[test]
fn each_filter_returns_only_its_own_kind() {
    // Upstream `typeFilterSplitsTheHistory`.
    let (_directory, mut store) = store();
    store
        .add_text("just some prose", None)
        .expect("the text is recorded");
    store
        .add_text("https://apple.com", None)
        .expect("the text is recorded");
    store
        .add_text("hi@apple.com", None)
        .expect("the text is recorded");
    store
        .add_text("second.link.dev", None)
        .expect("the text is recorded");
    store
        .add_text("#FF5733", None)
        .expect("the text is recorded");
    store
        .add_text("rgb(0 255 0 / 0.5)", None)
        .expect("the text is recorded");

    assert_eq!(texts(&store, "", ClipboardFilter::All).len(), 6);
    assert_eq!(
        texts(&store, "", ClipboardFilter::Text),
        vec!["just some prose"]
    );
    assert_eq!(
        texts(&store, "", ClipboardFilter::Color),
        vec!["rgb(0 255 0 / 0.5)", "#FF5733"]
    );
    assert_eq!(
        texts(&store, "", ClipboardFilter::Link),
        vec!["second.link.dev", "https://apple.com"]
    );
    assert_eq!(
        texts(&store, "", ClipboardFilter::Email),
        vec!["hi@apple.com"]
    );
    assert!(texts(&store, "", ClipboardFilter::Image).is_empty());
}

#[test]
fn a_pinned_link_leads_its_filtered_block() {
    // Upstream `typeFilterSplitsTheHistory`: a pinned link leads its filtered
    // block.
    let (_directory, mut store) = store();
    store
        .add_text("https://apple.com", None)
        .expect("the text is recorded");
    store
        .add_text("second.link.dev", None)
        .expect("the text is recorded");
    store
        .toggle_pinned(&item(&store, "https://apple.com"))
        .expect("the pin is set");

    assert_eq!(
        texts(&store, "", ClipboardFilter::Link),
        vec!["https://apple.com", "second.link.dev"]
    );
}

#[test]
fn the_filter_joins_the_search_memo() {
    // Upstream `typeFilterJoinsTheSearchMemo`: the same query under another
    // filter is not served from the wider memo.
    let (_directory, mut store) = store();
    store
        .add_text("shared token prose", None)
        .expect("the text is recorded");
    store
        .add_text("shared-token.com", None)
        .expect("the text is recorded");

    assert_eq!(store.search("shared", ClipboardFilter::All).len(), 2);
    assert_eq!(
        texts(&store, "shared", ClipboardFilter::Link),
        vec!["shared-token.com"]
    );
    assert_eq!(
        texts(&store, "shared", ClipboardFilter::Text),
        vec!["shared token prose"]
    );
    assert_eq!(store.search("shared", ClipboardFilter::All).len(), 2);
}

// MARK: - The store's rules

#[test]
fn a_repeated_copy_is_not_recorded_twice() {
    // Upstream `ClipboardStore.addText`: a copy of the text that is already
    // the newest entry is a repeat of the same ⌘C.
    let (_directory, mut store) = store();
    store.add_text("hello", None).expect("the text is recorded");
    store
        .add_text("hello", None)
        .expect("the repeat is dropped");
    assert_eq!(store.items().len(), 1);

    store.add_text("world", None).expect("the text is recorded");
    store.add_text("hello", None).expect("the text is recorded");
    assert_eq!(store.items().len(), 3);
}

#[test]
fn a_repeated_single_file_is_not_recorded_twice() {
    // Upstream `ClipboardStore.addFiles`: only a single file can be a ⌘C
    // repeat, which is the case `addText` also guards.
    let (_directory, mut store) = store();
    store
        .add_files(&["/tmp/c.pdf".to_string()], None)
        .expect("the file is recorded");
    store
        .add_files(&["/tmp/c.pdf".to_string()], None)
        .expect("the repeat is dropped");
    assert_eq!(store.items().len(), 1);
}

#[test]
fn a_multi_file_copy_makes_one_row_each() {
    // Upstream `multiFileCopyMakesOneRowEach`: three files make three rows,
    // and the reader inserts them newest-last.
    let (_directory, mut store) = store();
    store
        .add_files(
            &[
                "/tmp/a.png".to_string(),
                "/tmp/b.mov".to_string(),
                "/tmp/c.pdf".to_string(),
            ],
            None,
        )
        .expect("the files are recorded");
    assert_eq!(store.items().len(), 3);
    let paths: Vec<&str> = store
        .items()
        .iter()
        .filter_map(|entry| entry.file_path())
        .collect();
    assert_eq!(paths, vec!["/tmp/c.pdf", "/tmp/b.mov", "/tmp/a.png"]);
}

#[test]
fn the_history_is_ordered_newest_first() {
    // Upstream's store keeps `items` newest-first.
    let (_directory, mut store) = store();
    store.add_text("first", None).expect("the text is recorded");
    store
        .add_text("second", None)
        .expect("the text is recorded");
    store.add_text("third", None).expect("the text is recorded");
    assert_eq!(
        texts(&store, "", ClipboardFilter::All),
        vec!["third", "second", "first"]
    );
}

#[test]
fn pins_stack_in_pin_order() {
    // Upstream `pinOrder`: pins hold pin order, not the recency order they
    // had in the history.
    let (_directory, mut store) = store();
    store
        .add_text("oldest", None)
        .expect("the text is recorded");
    store
        .add_text("middle", None)
        .expect("the text is recorded");
    store
        .add_text("newest", None)
        .expect("the text is recorded");

    store
        .toggle_pinned(&item(&store, "oldest"))
        .expect("the pin is set");
    assert_eq!(
        texts(&store, "", ClipboardFilter::All),
        vec!["oldest", "newest", "middle"]
    );

    store
        .toggle_pinned(&item(&store, "middle"))
        .expect("the pin is set");
    assert_eq!(
        texts(&store, "", ClipboardFilter::All),
        vec!["oldest", "middle", "newest"]
    );

    store
        .toggle_pinned(&item(&store, "newest"))
        .expect("the pin is set");
    assert_eq!(
        texts(&store, "", ClipboardFilter::All),
        vec!["oldest", "middle", "newest"]
    );
}

#[test]
fn unpinning_rejoins_as_the_newest_entry() {
    // Upstream `unpinRejoinsAsNewest`: an unpinned row leads the history and
    // the row is re-recencied.
    let (_directory, mut store) = store();
    store.add_text("a", None).expect("the text is recorded");
    store.add_text("b", None).expect("the text is recorded");
    store.add_text("c", None).expect("the text is recorded");
    let before = item(&store, "a").created_at;

    store
        .toggle_pinned(&item(&store, "a"))
        .expect("the pin is set");
    store
        .toggle_pinned(&item(&store, "a"))
        .expect("the pin is cleared");

    assert_eq!(texts(&store, "", ClipboardFilter::All), vec!["a", "c", "b"]);
    assert!(!item(&store, "a").is_pinned());
    assert!(item(&store, "a").created_at > before);
}

#[test]
fn promoting_leaves_a_pinned_row_in_place() {
    // Upstream `pasteLeavesPinsAlone`: a pinned row holds its place, and an
    // unpinned row still promotes to the head of the history.
    let (_directory, mut store) = store();
    store.add_text("one", None).expect("the text is recorded");
    store.add_text("two", None).expect("the text is recorded");
    store
        .toggle_pinned(&item(&store, "one"))
        .expect("the pin is set");
    store
        .toggle_pinned(&item(&store, "two"))
        .expect("the pin is set");
    let stamp = item(&store, "one").created_at;

    store
        .promote(&item(&store, "one"))
        .expect("the promotion is recorded");

    assert_eq!(texts(&store, "", ClipboardFilter::All), vec!["one", "two"]);
    assert_eq!(item(&store, "one").created_at, stamp);

    store.add_text("three", None).expect("the text is recorded");
    store.add_text("four", None).expect("the text is recorded");
    store
        .promote(&item(&store, "three"))
        .expect("the promotion is recorded");
    assert_eq!(
        texts(&store, "", ClipboardFilter::All),
        vec!["one", "two", "three", "four"]
    );
}

#[test]
fn an_edit_keeps_the_entry_where_it_was() {
    // Upstream `editRewritesInPlace`: an edit keeps the entry's id, its pin
    // and its place; the old text is no longer found.
    let (_directory, mut store) = store();
    store
        .add_text("first draft", None)
        .expect("the text is recorded");
    store
        .add_text("pinned draft", None)
        .expect("the text is recorded");
    store
        .add_text("newest", None)
        .expect("the text is recorded");
    store
        .toggle_pinned(&item(&store, "pinned draft"))
        .expect("the pin is set");
    let original = item(&store, "first draft");

    let edited = store
        .update_text("final copy", &original)
        .expect("the edit is saved")
        .expect("the entry is edited");
    assert_eq!(edited.id, original.id);
    assert_eq!(
        texts(&store, "", ClipboardFilter::All),
        vec!["pinned draft", "newest", "final copy"]
    );
    assert_eq!(
        texts(&store, "draft", ClipboardFilter::All),
        vec!["pinned draft"]
    );
    assert_eq!(
        store
            .search("final", ClipboardFilter::All)
            .iter()
            .map(|entry| entry.id.clone())
            .collect::<Vec<_>>(),
        vec![original.id.clone()]
    );
}

#[test]
fn an_edit_keeps_the_pin() {
    // Upstream `editRewritesInPlace`: an edit keeps the pin.
    let (_directory, mut store) = store();
    store
        .add_text("pinned draft", None)
        .expect("the text is recorded");
    store
        .toggle_pinned(&item(&store, "pinned draft"))
        .expect("the pin is set");
    store
        .update_text("pinned final", &item(&store, "pinned draft"))
        .expect("the edit is saved");
    assert!(item(&store, "pinned final").is_pinned());
}

#[test]
fn a_deleted_entry_cannot_be_edited() {
    // Upstream `editRewritesInPlace`: a deleted entry cannot be edited and
    // does not come back.
    let (_directory, mut store) = store();
    store
        .add_text("newest", None)
        .expect("the text is recorded");
    let deleted = item(&store, "newest");
    store.remove(&deleted).expect("the entry is removed");
    assert_eq!(
        store
            .update_text("ghost", &deleted)
            .expect("the edit is answered"),
        None
    );
    assert!(!texts(&store, "", ClipboardFilter::All).contains(&"ghost".to_string()));
}

#[test]
fn only_a_text_entry_is_editable() {
    // Upstream `editRewritesInPlace`: only a text entry is editable.
    let (_directory, mut store) = store();
    let image = ClipboardItem::image("/tmp/shot.png", None);
    assert_eq!(
        store
            .update_text("caption", &image)
            .expect("the edit is answered"),
        None
    );
}

#[test]
fn an_edit_is_held_to_the_capture_limit() {
    // Upstream `editHonoursTheCaptureLimit`: an edit at the limit saves, and
    // one past it is refused with its length.
    let (_directory, mut store) = store();
    store.add_text("short", None).expect("the text is recorded");
    let longest = "a".repeat(32_000);
    assert!(store.update_text(&longest, &item(&store, "short")).is_ok());
    assert_eq!(
        store.update_text(&format!("{longest}a"), &item(&store, &longest)),
        Err(crate::UpdateError::TooLong)
    );
    assert_eq!(texts(&store, "", ClipboardFilter::All), vec![longest]);
}

#[test]
fn pins_outlive_a_cleared_history() {
    // Upstream `persistence`: Clear History spares the pin.
    let (_directory, mut store) = store();
    store.add_text("first", None).expect("the text is recorded");
    store
        .add_text("second", None)
        .expect("the text is recorded");
    store.add_text("third", None).expect("the text is recorded");
    store
        .toggle_pinned(&item(&store, "third"))
        .expect("the pin is set");
    store
        .toggle_pinned(&item(&store, "first"))
        .expect("the pin is set");

    store.clear_all().expect("the history is cleared");
    assert_eq!(
        texts(&store, "", ClipboardFilter::All),
        vec!["first", "third"]
    );
}

#[test]
fn the_pin_order_is_restored_from_disk() {
    // Upstream `persistence`: pin order is restored from disk, not recomputed
    // from recency.
    let (directory, mut store) = store();
    store.add_text("first", None).expect("the text is recorded");
    store
        .add_text("second", None)
        .expect("the text is recorded");
    store.add_text("third", None).expect("the text is recorded");
    store
        .toggle_pinned(&item(&store, "third"))
        .expect("the pin is set");
    store
        .toggle_pinned(&item(&store, "first"))
        .expect("the pin is set");

    let mut reopened = ClipboardStore::open(directory.path()).expect("the store reopens");
    reopened.load().expect("the history is read");
    assert_eq!(
        texts(&reopened, "", ClipboardFilter::All),
        vec!["third", "first", "second"]
    );

    reopened
        .toggle_pinned(&item(&reopened, "third"))
        .expect("the pin is cleared");
    assert_eq!(
        texts(&reopened, "", ClipboardFilter::All),
        vec!["first", "third", "second"]
    );
}

#[test]
fn a_referenced_file_outlives_its_row() {
    // Upstream `referencedFilesOutliveTheirRows`: removing a row leaves the
    // referenced file on disk, and so does a retention cut.
    let (directory, mut store) = store();
    let outside = directory.path().join("original.txt");
    std::fs::write(&outside, b"keep me").expect("the file is written");

    store
        .add_files(&[outside.to_string_lossy().to_string()], None)
        .expect("the file is recorded");
    store
        .remove(&store.items()[0].clone())
        .expect("the row is removed");
    assert!(outside.exists());

    store
        .add_files(&[outside.to_string_lossy().to_string()], None)
        .expect("the file is recorded");
    store.set_max_age(-1);
    store.enforce_limits().expect("the retention cut runs");
    assert!(store.items().is_empty());
    assert!(outside.exists());
}

#[test]
fn the_resident_window_is_capped() {
    // Upstream `fileBatchesRespectTheMemoryWindow`: the resident window holds
    // back some of the history, and trimming removes the oldest unpinned rows
    // while the trimmed rows remain searchable on disk.
    let (_directory, mut store) = store();
    for index in 0..1000 {
        store
            .add_text(&format!("entry {index}"), None)
            .expect("the text is recorded");
    }
    assert_eq!(store.items().len(), MEMORY_WINDOW);

    let pinned = item(&store, "entry 0");
    store.toggle_pinned(&pinned).expect("the pin is set");

    let paths: Vec<String> = (0..32)
        .map(|index| format!("/Users/me/batch-{index}.txt"))
        .collect();
    store
        .add_files(&paths, None)
        .expect("the files are recorded");
    assert_eq!(store.items().len(), MEMORY_WINDOW);
    assert!(store
        .items()
        .iter()
        .any(|entry| entry.id == pinned.id && entry.is_pinned()));
    assert_eq!(
        store
            .items()
            .iter()
            .rev()
            .find(|entry| !entry.is_pinned())
            .and_then(|entry| entry.text.clone()),
        Some("entry 97".to_string())
    );
    assert!(store
        .search("entry 1", ClipboardFilter::Text)
        .iter()
        .any(|entry| entry.text.as_deref() == Some("entry 1")));
}

#[test]
fn an_export_sees_past_the_memory_window() {
    // Upstream `exportSeesPastTheMemoryWindow`: the export streams every row,
    // not just the window.
    let (_directory, mut store) = store();
    let total = 1_200;
    let entries: Vec<ClipboardItem> = (0..total)
        .map(|index| {
            let mut entry = ClipboardItem::text(format!("entry {index}"), None);
            entry.created_at = index as i64;
            entry
        })
        .collect();
    store
        .import_entries(&entries)
        .expect("the entries are imported");
    assert!(store.items().len() < total);

    let mut streamed = 0usize;
    let mut seen = std::collections::HashSet::new();
    store
        .for_each_stored_item(|entry| {
            streamed += 1;
            if let Some(text) = &entry.text {
                seen.insert(text.clone());
            }
        })
        .expect("the export runs");
    assert_eq!(streamed, total);
    assert_eq!(seen.len(), total);
}

#[test]
fn an_imported_file_arrives_once_per_path() {
    // Upstream `importedFilesArriveOncePerPath`: two distinct paths import as
    // two rows, and re-importing one adds nothing.
    let (_directory, mut store) = store();
    let mut one = ClipboardItem::file("/tmp/one.pdf", None);
    one.created_at = 1;
    let mut two = ClipboardItem::file("/tmp/two.pdf", None);
    two.created_at = 2;

    assert_eq!(
        store
            .import_entries(&[one.clone(), two.clone()])
            .expect("the import runs"),
        2
    );
    assert_eq!(store.import_entries(&[one]).expect("the import runs"), 0);
    assert!(store.items().iter().all(|entry| entry.image_path.is_none()));
}

#[test]
fn a_file_entry_is_found_by_name_and_by_folder() {
    // Upstream `fileEntriesAreFoundByNameAndFolder`: the path lives in the
    // text, so the entry is found by name, by folder and by the sub-trigram
    // fallback.
    let (_directory, mut store) = store();
    store
        .add_files(
            &["/Users/me/Downloads/Quarterly Report.pdf".to_string()],
            None,
        )
        .expect("the file is recorded");
    assert_eq!(store.search("report", ClipboardFilter::All).len(), 1);
    assert_eq!(store.search("downloads", ClipboardFilter::All).len(), 1);
    assert_eq!(store.search("re", ClipboardFilter::All).len(), 1);
}

#[test]
fn a_pinned_match_leads_a_filtered_search() {
    // Upstream `pinsLeadFilteredSearches`: the pinned match leads the results
    // even when the index's limit cannot reach it, and it is not duplicated.
    let (_directory, mut store) = store();
    let mut seed: Vec<ClipboardItem> = Vec::new();
    // The pinned hit is the oldest of 260 matches; the index stops at 200.
    let mut pinned = ClipboardItem::text("needle in the haystack", None);
    pinned.created_at = 0;
    seed.push(pinned);
    for index in 1..=259 {
        let mut filler = ClipboardItem::text(format!("haystack filler {index}"), None);
        filler.created_at = index;
        seed.push(filler);
    }
    store
        .import_entries(&seed)
        .expect("the entries are imported");
    store
        .toggle_pinned(&item(&store, "needle in the haystack"))
        .expect("the pin is set");

    let results = store.search("haystack", ClipboardFilter::All);
    assert_eq!(
        results.first().and_then(|entry| entry.text.clone()),
        Some("needle in the haystack".to_string())
    );
    assert_eq!(results.iter().filter(|entry| entry.is_pinned()).count(), 1);
}

#[test]
fn a_reset_passes_the_pins_for_the_newest_clip() {
    // Upstream `landingSkipsThePins`.
    let (_directory, mut store) = store();
    store
        .add_text("alpha one", None)
        .expect("the text is recorded");
    store
        .add_text("beta two", None)
        .expect("the text is recorded");
    store
        .add_text("https://example.com", None)
        .expect("the text is recorded");
    store
        .add_text("beta three", None)
        .expect("the text is recorded");
    assert_eq!(store.landing_index("", ClipboardFilter::All), 0);

    store
        .toggle_pinned(&item(&store, "alpha one"))
        .expect("the pin is set");
    store
        .toggle_pinned(&item(&store, "beta two"))
        .expect("the pin is set");
    let landing = store.landing_index("", ClipboardFilter::All);
    assert_eq!(landing, 2);
    assert_eq!(
        store.search("", ClipboardFilter::All)[landing].text.clone(),
        Some("beta three".to_string())
    );
    assert_eq!(store.landing_index("  ", ClipboardFilter::All), 2);
    assert_eq!(store.landing_index("beta", ClipboardFilter::All), 0);
}

#[test]
fn a_pinned_slot_maps_to_the_visible_pinned_row() {
    // Upstream `pinnedSlotResolutionUsesVisiblePins`: slot picks are taken
    // from the visible pinned block after the query and filter.
    let (_directory, mut store) = store();
    store
        .add_text("alpha one", None)
        .expect("the text is recorded");
    store
        .add_text("beta two", None)
        .expect("the text is recorded");
    store
        .add_text("beta three", None)
        .expect("the text is recorded");
    store
        .add_text("plain text", None)
        .expect("the text is recorded");

    store
        .toggle_pinned(&item(&store, "alpha one"))
        .expect("the pin is set");
    store
        .toggle_pinned(&item(&store, "beta two"))
        .expect("the pin is set");
    store
        .toggle_pinned(&item(&store, "beta three"))
        .expect("the pin is set");

    assert_eq!(
        store
            .pinned_item(0, "", ClipboardFilter::All)
            .and_then(|entry| entry.text),
        Some("alpha one".to_string())
    );
    assert_eq!(
        store
            .pinned_item(1, "", ClipboardFilter::All)
            .and_then(|entry| entry.text),
        Some("beta two".to_string())
    );
    assert!(store.pinned_item(3, "", ClipboardFilter::All).is_none());
    assert_eq!(
        store
            .pinned_item(0, "beta", ClipboardFilter::All)
            .and_then(|entry| entry.text),
        Some("beta two".to_string())
    );
    assert_eq!(
        store
            .pinned_item(1, "beta", ClipboardFilter::All)
            .and_then(|entry| entry.text),
        Some("beta three".to_string())
    );
    assert!(store.pinned_item(2, "beta", ClipboardFilter::All).is_none());
    assert!(store.pinned_item(0, "", ClipboardFilter::Link).is_none());
}

// MARK: - The search

#[test]
fn a_search_finds_the_rows_past_the_window() {
    // Upstream `editReachesPastTheMemoryWindow`: the oldest entry is found
    // past the window.
    let (_directory, mut store) = store();
    let total = 1_200;
    let entries: Vec<ClipboardItem> = (0..total)
        .map(|index| {
            let mut entry = ClipboardItem::text(format!("entry {index}"), None);
            entry.created_at = index as i64;
            entry
        })
        .collect();
    store
        .import_entries(&entries)
        .expect("the entries are imported");

    let oldest = store
        .search("entry 0", ClipboardFilter::All)
        .first()
        .cloned()
        .expect("the oldest entry is found past the window");
    assert!(!store.items().iter().any(|entry| entry.id == oldest.id));
}

#[test]
fn a_search_matches_case_insensitively() {
    // Upstream's search compares against `localizedCaseInsensitiveContains`.
    let (_directory, mut store) = store();
    store
        .add_text("Common alphabet", None)
        .expect("the text is recorded");
    assert_eq!(store.search("COMMON", ClipboardFilter::All).len(), 1);
    assert_eq!(store.search("common", ClipboardFilter::All).len(), 1);
    assert_eq!(store.search("absent-value", ClipboardFilter::All).len(), 0);
}

#[test]
fn an_empty_query_answers_with_the_whole_window() {
    // Upstream's search: an empty query is the ordered history.
    let (_directory, mut store) = store();
    store.add_text("alpha", None).expect("the text is recorded");
    store.add_text("beta", None).expect("the text is recorded");
    assert_eq!(
        texts(&store, "", ClipboardFilter::All),
        vec!["beta", "alpha"]
    );
    assert_eq!(
        texts(&store, " ", ClipboardFilter::All),
        vec!["beta", "alpha"]
    );
}

// MARK: - The colours

/// Whether two colours agree to a channel.
fn near(left: &ColorValue, right: &ColorValue) -> bool {
    let channels = [
        (left.red, right.red),
        (left.green, right.green),
        (left.blue, right.blue),
        (left.alpha, right.alpha),
    ];
    channels.iter().all(|(a, b)| (a - b).abs() < 1.0 / 255.0)
}

#[test]
fn every_colour_notation_parses_to_its_components() {
    // Upstream `colorParsing`: every notation the parser accepts.
    let cases: [(String, ColorValue); 10] = [
        (
            "#FF5733".to_string(),
            ColorValue::new(1.0, 87.0 / 255.0, 51.0 / 255.0, 1.0),
        ),
        // Shorthand doubles each digit rather than padding it with a zero.
        ("#0f0".to_string(), ColorValue::new(0.0, 1.0, 0.0, 1.0)),
        (
            "#0f08".to_string(),
            ColorValue::new(0.0, 1.0, 0.0, 136.0 / 255.0),
        ),
        (
            "rgb(255, 87, 51)".to_string(),
            ColorValue::new(1.0, 87.0 / 255.0, 51.0 / 255.0, 1.0),
        ),
        (
            "rgb(0 255 0 / 0.5)".to_string(),
            ColorValue::new(0.0, 1.0, 0.0, 0.5),
        ),
        (
            "rgba(255,87,51,0.5)".to_string(),
            ColorValue::new(1.0, 87.0 / 255.0, 51.0 / 255.0, 0.5),
        ),
        (
            "hsl(120, 100%, 50%)".to_string(),
            ColorValue::new(0.0, 1.0, 0.0, 1.0),
        ),
        (
            "hsl(10.6deg 100% 60%)".to_string(),
            ColorValue::new(1.0, 87.0 / 255.0, 51.0 / 255.0, 1.0),
        ),
        // The notation a colour picker hands an extension, and its
        // percentage chroma.
        (
            "oklch(62.7955% 0.257683 29.2338)".to_string(),
            ColorValue::new(1.0, 0.0, 0.0, 1.0),
        ),
        (
            "oklch(0.627955 64.42% 29.2338deg / 0.5)".to_string(),
            ColorValue::new(1.0, 0.0, 0.0, 0.5),
        ),
    ];
    for (text, expected) in cases {
        let parsed = ColorValue::parse(&text).unwrap_or_else(|| panic!("{text} parses"));
        assert!(
            near(&parsed, &expected),
            "{text} resolves to its components"
        );
    }
}

#[test]
fn a_colour_that_is_not_one_is_rejected() {
    // Upstream `colorParsing`: the near-misses it must reject.
    for text in [
        "#FF5733 and more",
        "rgb(255, 87, 51) plus",
        "(255, 87, 51)",
        "#",
        "rgb",
    ] {
        assert!(ColorValue::parse(text).is_none(), "{text} is not a colour");
    }
}

#[test]
fn a_malformed_colour_is_rejected() {
    // Upstream `colorParsing`: a hex-shaped word and a malformed digit count
    // must never read as a colour.
    for text in [
        "#GGGGGG",
        "#12345",
        "report.pdf",
        "rgb(1, 2)",
        "rgb(1, 2, 3",
        "hsl(1, 2, 3, 4, 5)",
        "cmyk(0, 1, 1, 0)",
        "255, 87, 51",
        // Non-finite literals parse as doubles, and every notation ends in an
        // integer conversion.
        "rgb(nan, 0, 0)",
        "hsl(inf, 100%, 50%)",
        "rgb(1e400, 0, 0)",
        "rgba(0, 0, 0, nan)",
        // CSS writes an HSL channel as a percentage; `100` would otherwise
        // clamp to white.
        "hsl(120, 100, 50)",
        "hsl(120 100 50)",
        "hsla(120, 100, 50, 1)",
        // An argument list is counted, so a hole in it is malformed rather
        // than dropped.
        "rgb(255,,87,51)",
        "rgb(255,87,51,)",
        "rgb(255, 87)",
        "rgb()",
        "rgb(1,2,3,4,5)",
        // The spellings never mix, so a comma body may not also carry a slash.
        "rgb(1/2, 3, 4)",
        "rgb(255, 87, 51 / 0.5)",
        "rgb(0 255 0 / 0.5 / 1)",
        "rgb(/0.5)",
        // Each side of the slash is counted: flattening reads an alpha as a
        // blue channel.
        "rgb(0 255 / 0.5)",
        "hsl(120 100% / 50%)",
        "rgb(0 / 255 0)",
        "rgb(0 255 0 0)",
        // A double reads Swift literals CSS never writes, and a hex digit
        // reads the fullwidth forms.
        "rgb(0x10, 0, 0)",
        "rgb(1e2, 0, 0)",
        "rgb(+255, 0, 0)",
        "\u{ff46}\u{ff46}\u{ff46}",
    ] {
        assert!(ColorValue::parse(text).is_none(), "{text} is not a colour");
    }
}

#[test]
fn an_alpha_is_optional_in_both_spellings() {
    // Upstream `colorParsing`: `rgba()` is an alias of `rgb()` under CSS
    // Color 4, so alpha is optional in both.
    for text in [
        "rgba(255, 87, 51)",
        "rgb(255, 87, 51, 0.5)",
        "hsla(120, 100%, 50%)",
        "hsl(120, 100%, 50%, 0.5)",
        "hsl(120, 100%, 50%, 50%)",
        "rgb(0 255 0)",
        // CSS treats every whitespace alike, and a copied declaration spans
        // lines.
        "rgb(255,\n87,\n51)",
        "rgb(0\n255\n0)",
    ] {
        assert!(ColorValue::parse(text).is_some(), "{text} is a colour");
    }
}

#[test]
fn an_almost_white_colour_states_a_value_rather_than_trapping() {
    // Upstream `colorParsing`: rounding leaves a chroma of ~1e-16 at a
    // lightness of 1, whose saturation divisor is 0.
    let almost_white = ColorValue::parse("rgb(100%, 100%, 99.99999999999999%)")
        .expect("an almost-white colour parses");
    assert!(ColorFormat::offered(&almost_white)
        .iter()
        .all(|format| !format.string(&almost_white).is_empty()));
    assert!(almost_white.hsl().saturation.is_finite());
}

#[test]
fn a_four_digit_shorthand_doubles_its_alpha_digit() {
    // Upstream `colorParsing`: the four-digit shorthand doubles its alpha
    // digit like the three-digit form doubles the rest.
    let parsed = ColorValue::parse("#0f0f").expect("the colour parses");
    assert_eq!(ColorFormat::HexWithAlpha.string(&parsed), "#00FF00FF");
}

#[test]
fn an_alpha_that_rounds_back_to_opaque_is_opaque() {
    // Upstream `colorParsing`: an alpha that rounds back to opaque is opaque,
    // so no alpha row is offered.
    let parsed = ColorValue::parse("#000000FF").expect("the colour parses");
    assert!(!parsed.has_alpha());
}

#[test]
fn the_hsl_conversion_round_trips() {
    // Upstream `colorParsing`: the two directions of the HSL conversion must
    // agree, or a copied value drifts.
    let original = ColorValue::new(0.2, 0.6, 0.9, 1.0);
    let hsl = original.hsl();
    let back = ColorValue::from_hsl(hsl.hue, hsl.saturation, hsl.lightness, 1.0);
    assert!(near(&back, &original));
}

#[test]
fn a_colour_is_written_in_each_notation() {
    // Upstream `colorFormatting`: the exact strings a colour card shows.
    let color = ColorValue::parse("#FF5733").expect("the fixture parses");
    assert_eq!(ColorFormat::Hex.string(&color), "#FF5733");
    assert_eq!(ColorFormat::Rgba.string(&color), "rgba(255, 87, 51, 1)");
    assert_eq!(ColorFormat::Hsl.string(&color), "hsl(10.6deg, 100%, 60%)");
    assert_eq!(ColorFormat::Oklch.string(&color), "oklch(68% 0.21 33.7deg)");
    assert!(!ColorFormat::offered(&color).contains(&ColorFormat::HslWithAlpha));
}

#[test]
fn a_translucent_colour_is_offered_every_notation() {
    // Upstream `colorFormatting`: a colour with alpha is offered every
    // notation, and alpha is the fourth hex channel.
    let translucent = ColorValue::parse("rgba(255, 87, 51, 0.5)").expect("the fixture parses");
    assert_eq!(
        ColorFormat::offered(&translucent).len(),
        ColorFormat::ALL.len()
    );
    assert_eq!(ColorFormat::HexWithAlpha.string(&translucent), "#FF573380");
    assert_eq!(
        ColorFormat::primary(&translucent),
        ColorFormat::HexWithAlpha
    );
    assert_eq!(ColorFormat::primary(&color_opaque()), ColorFormat::Hex);
}

/// The opaque colour upstream's formatting test uses.
fn color_opaque() -> ColorValue {
    ColorValue::parse("#FF5733").expect("the fixture parses")
}

#[test]
fn a_neutral_colour_has_no_hue() {
    // Upstream `colorSpaceConversions`: a neutral has no hue, and `atan2` over
    // two rounding errors would still name one.
    let grey = ColorValue::parse("#DDDDDD").expect("the fixture parses");
    assert_eq!(ColorFormat::Oklch.string(&grey), "oklch(89.8% 0 0deg)");
}

#[test]
fn a_slash_carries_alpha_and_an_opaque_colour_states_none() {
    // Upstream `colorSpaceConversions`: the CSS4 spellings carry alpha behind
    // a slash, and say nothing when there is none.
    let translucent = ColorValue::parse("rgba(0, 255, 0, 0.5)").expect("the fixture parses");
    assert_eq!(
        ColorFormat::Oklch.string(&translucent),
        "oklch(86.6% 0.295 142.5deg / 0.5)"
    );
    let orange = ColorValue::parse("#FF5733").expect("the fixture parses");
    assert_eq!(
        ColorFormat::Oklch.string(&orange),
        "oklch(68% 0.21 33.7deg)"
    );
    assert_eq!(
        ColorFormat::offered(&orange).len(),
        ColorFormat::ALL.len() - 2
    );
}

#[test]
fn every_offered_notation_re_parses_to_its_own_colour() {
    // Upstream `colorFormatsRoundTrip`: one sweep for loss and lost alpha.
    let mut checked = 0usize;
    for red in (0..=255).step_by(17) {
        for green in (0..=255).step_by(29) {
            for blue in (0..=255).step_by(43) {
                for alpha in [255, 128] {
                    let color = ColorValue::new(
                        f64::from(red) / 255.0,
                        f64::from(green) / 255.0,
                        f64::from(blue) / 255.0,
                        f64::from(alpha) / 255.0,
                    );
                    for format in ColorFormat::offered(&color) {
                        // `oklch()` re-parses, but states too few digits to land
                        // on a channel.
                        if format == ColorFormat::Oklch {
                            continue;
                        }
                        let text = format.string(&color);
                        let back =
                            ColorValue::parse(&text).unwrap_or_else(|| panic!("{text} re-parses"));
                        assert!(near(&back, &color), "{text} states the colour it came from");
                        checked += 1;
                    }
                }
            }
        }
    }
    assert!(checked > 500, "the sweep actually covered the notations");
}

// MARK: - The paste sequence

/// The history upstream's paste-sequence test walks.
fn walk_history() -> Vec<ClipboardItem> {
    vec![
        ClipboardItem::text("newest", None),
        ClipboardItem::image("/scratch/shot.png", None),
        ClipboardItem::text("middle", None),
        ClipboardItem::file("/scratch/report.pdf", None),
        ClipboardItem::text("oldest", None),
    ]
}

/// The start of upstream's paste-sequence test clock.
const START: i64 = 0;

/// Every entry the walk pastes from a live history.
fn pasted(sequence: &mut PasteSequence, live: &[ClipboardItem]) -> Vec<String> {
    let mut out = Vec::new();
    while let Some(entry) = sequence.next(live) {
        out.push(entry.id);
    }
    out
}

/// A fresh walk over the history.
fn fresh_sequence() -> PasteSequence {
    PasteSequence::new(&walk_history(), 1, START)
}

#[test]
fn the_walk_pastes_every_kind_newest_first() {
    // Upstream `walksEveryKindNewestFirst`: the walk pastes text, images and
    // files alike, newest first.
    let history = walk_history();
    let mut sequence = fresh_sequence();
    let expected: Vec<String> = history.iter().map(|entry| entry.id.clone()).collect();
    assert_eq!(pasted(&mut sequence, &history), expected);
}

#[test]
fn the_walk_stops_at_the_end_rather_than_wrapping() {
    // Upstream `stopsAtTheEndRatherThanWrapping`: a press past the oldest
    // entry pastes nothing.
    let history = walk_history();
    let mut sequence = fresh_sequence();
    let _ = pasted(&mut sequence, &history);
    assert!(sequence.next(&history).is_none());
}

#[test]
fn an_empty_history_has_nothing_to_paste() {
    // Upstream `emptyHistoryHasNothingToPaste`.
    let mut sequence = PasteSequence::new(&[], 1, START);
    assert!(sequence.next(&[]).is_none());
}

#[test]
fn a_promoted_entry_neither_repeats_nor_jumps_the_walk() {
    // Upstream `promotionMidWalkNeitherRepeatsNorSkips`.
    let history = walk_history();
    let mut sequence = fresh_sequence();
    let _ = sequence.next(&history);
    let mut promoted = history.clone();
    promoted.swap(0, 2);
    let expected: Vec<String> = ["image", "middle", "file", "oldest"]
        .iter()
        .map(|label| {
            history
                .iter()
                .find(|entry| {
                    entry.text.as_deref() == Some(*label)
                        || entry.file_path() == Some(*label)
                        || entry.image_path.as_deref() == Some(*label)
                })
                .expect("the entry is listed")
                .id
                .clone()
        })
        .collect();
    assert_eq!(pasted(&mut sequence, &promoted), expected);
}

#[test]
fn an_entry_captured_mid_walk_is_not_pasted_by_it() {
    // Upstream `captureMidWalkWaitsForTheNextWalk`.
    let history = walk_history();
    let mut sequence = fresh_sequence();
    let mut captured = vec![ClipboardItem::text("captured", None)];
    captured.extend(history.iter().cloned());
    let expected: Vec<String> = history.iter().map(|entry| entry.id.clone()).collect();
    assert_eq!(pasted(&mut sequence, &captured), expected);
}

#[test]
fn an_entry_deleted_mid_walk_is_skipped_rather_than_pasted() {
    // Upstream `entryDeletedMidWalkIsSkipped`.
    let history = walk_history();
    let mut sequence = fresh_sequence();
    let middle = history[2].id.clone();
    let remaining: Vec<ClipboardItem> = history
        .iter()
        .filter(|entry| entry.id != middle)
        .cloned()
        .collect();
    let expected: Vec<String> = remaining.iter().map(|entry| entry.id.clone()).collect();
    assert_eq!(pasted(&mut sequence, &remaining), expected);
}

#[test]
fn clearing_the_history_ends_the_walk() {
    // Upstream `clearedHistoryEndsTheWalk`.
    let history = walk_history();
    let mut sequence = fresh_sequence();
    let _ = sequence.next(&history);
    assert!(sequence.next(&[]).is_none());
}

#[test]
fn our_own_paste_keeps_the_walk_going() {
    // Upstream `ownPasteKeepsTheWalkGoing`: the count our own paste left keeps
    // the walk going.
    let mut sequence = fresh_sequence();
    sequence.record_paste(4, START);
    assert!(sequence.continues(4, START));
}

#[test]
fn a_copy_since_the_last_paste_ends_the_walk() {
    // Upstream `newCopyEndsTheWalk`.
    let mut sequence = fresh_sequence();
    sequence.record_paste(4, START);
    assert!(!sequence.continues(5, START));
}

#[test]
fn a_pause_under_the_timeout_keeps_the_walk_going() {
    // Upstream `pauseUnderTheTimeoutKeepsTheWalkGoing`.
    let later = START + 60_000 - 1;
    assert!(fresh_sequence().continues(1, later));
}

#[test]
fn a_pause_of_the_full_timeout_ends_the_walk() {
    // Upstream `pauseOfTheFullTimeoutEndsTheWalk`.
    let later = START + 60_000;
    assert!(!fresh_sequence().continues(1, later));
}

#[test]
fn each_paste_restarts_the_timeout() {
    // Upstream `eachPasteRestartsTheTimeout`: the timeout counts from the
    // last paste, not the first press.
    let mut sequence = fresh_sequence();
    let paste = START + 60_000 - 1;
    sequence.record_paste(2, paste);
    assert!(sequence.continues(2, paste + 60_000 - 1));
}

#[test]
fn a_press_inside_the_settle_interval_is_held_back() {
    // Upstream `pressInsideTheSettleIntervalIsHeldBack`: a press before the
    // last ⌘V can land is held back.
    let mut sequence = fresh_sequence();
    sequence.record_paste(2, START);
    assert!(sequence.is_settling(START + 125));
}

#[test]
fn a_press_at_the_settle_interval_goes_through() {
    // Upstream `pressAtTheSettleIntervalGoesThrough`: a press once the last
    // paste has landed goes through.
    let mut sequence = fresh_sequence();
    sequence.record_paste(2, START);
    assert!(!sequence.is_settling(START + 250));
}

// MARK: - The Raycast import

/// A Raycast clipboard entry with one text representation.
fn raycast_entry(text: &str, created_at: &str, pinned: Option<bool>) -> serde_json::Value {
    let mut entry = serde_json::json!({
        "createdAt": created_at,
        "items": [{"representations": [{"mimeType": "text/plain", "content": text}]}]
    });
    if let Some(pinned) = pinned {
        entry["pinned"] = serde_json::Value::Bool(pinned);
    }
    entry
}

/// The Raycast export upstream's test builds.
fn raycast_export(entries: Vec<serde_json::Value>) -> serde_json::Value {
    serde_json::json!({ "clipboardEntries": entries })
}

/// The clock upstream's import test injects.
const NOW: i64 = 1_700_000_000_000;

#[test]
fn a_raycast_text_entry_imports_with_its_pin() {
    // Upstream `clipboardMapping`: text/plain still wins over rich text and
    // images, and a Boolean pin uses the original creation date.
    let export = raycast_export(vec![raycast_entry(
        "plain text",
        "2001-01-01T00:00:00.123Z",
        Some(true),
    )]);
    let result = raycast::parse(&export, || NOW, |_| true);

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.missing, 0);
    let entry = &result.items[0];
    assert_eq!(entry.kind, ItemKind::Text);
    assert_eq!(entry.text.as_deref(), Some("plain text"));
    assert_eq!(entry.image_path, None);
    assert_eq!(entry.source_bundle_id, None);
    assert_eq!(entry.created_at, 978_307_200_123);
    assert!(entry.is_pinned());
    assert_eq!(entry.pinned_at, Some(978_307_200_123));
}

#[test]
fn a_raycast_pin_that_is_false_stays_unpinned() {
    // Upstream `clipboardMapping`: an explicit false stays unpinned.
    let export = raycast_export(vec![raycast_entry(
        "ordinary",
        "2001-01-01T00:00:00.123Z",
        Some(false),
    )]);
    let result = raycast::parse(&export, || NOW, |_| true);
    assert_eq!(result.items.len(), 1);
    assert!(!result.items[0].is_pinned());
}

#[test]
fn a_malformed_raycast_pin_stays_unpinned_and_keeps_the_text() {
    // Upstream `clipboardPinMetadata`: a malformed or missing pin does not
    // discard the text, and stays unpinned.
    for value in [
        serde_json::Value::Null,
        serde_json::json!("true"),
        serde_json::json!(0),
        serde_json::json!(1),
        serde_json::json!(42),
        serde_json::json!(1.0),
        serde_json::json!([]),
        serde_json::json!([{"pinned": true}]),
    ] {
        let mut entry = raycast_entry("malformed", "2001-01-01T00:00:00.123Z", None);
        entry["pinned"] = value;
        let export = raycast_export(vec![entry]);
        let result = raycast::parse(&export, || NOW, |_| true);
        assert_eq!(result.items.len(), 1, "the text is not discarded");
        assert!(!result.items[0].is_pinned(), "the pin stays unpinned");
    }
}

#[test]
fn a_raycast_entry_with_a_bad_date_uses_the_clock() {
    // Upstream `clipboardPinMetadata`: bad dates still use the clock.
    for date in ["invalid", ""] {
        let export = raycast_export(vec![raycast_entry("fallback", date, Some(true))]);
        let result = raycast::parse(&export, || NOW, |_| true);
        assert_eq!(result.items[0].created_at, NOW);
    }
}

#[test]
fn a_raycast_image_imports_when_its_file_is_there() {
    // Upstream `clipboardMapping`: the original image path is preserved and
    // the image kind and text stay unchanged.
    let entry = serde_json::json!({
        "createdAt": "2001-01-02T00:00:00Z",
        "pinned": true,
        "items": [{"representations": [
            {"mimeType": "image/png", "contentType": "url", "content": "/synthetic/pin.png"}
        ]}]
    });
    let export = raycast_export(vec![entry]);
    let result = raycast::parse(&export, || NOW, |path| path == "/synthetic/pin.png");

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.missing, 0);
    let image = &result.items[0];
    assert_eq!(image.kind, ItemKind::Image);
    assert_eq!(image.text, None);
    assert_eq!(image.image_path.as_deref(), Some("/synthetic/pin.png"));
    assert_eq!(image.source_bundle_id, None);
    assert_eq!(image.created_at, 978_393_600_000);
    assert!(image.is_pinned());
}

#[test]
fn a_raycast_image_whose_file_is_gone_counts_as_missing() {
    // Upstream `clipboardMapping`: missing images count equally with and
    // without a pin.
    let entry = serde_json::json!({
        "createdAt": "2001-01-02T00:00:00Z",
        "pinned": true,
        "items": [{"representations": [
            {"mimeType": "image/png", "contentType": "url", "content": "/synthetic/missing.png"}
        ]}]
    });
    let export = raycast_export(vec![entry]);
    let result = raycast::parse(&export, || NOW, |_| false);

    assert_eq!(result.items.len(), 0);
    assert_eq!(result.missing, 1);
}

#[test]
fn an_empty_raycast_text_falls_through_to_the_image() {
    // Upstream `clipboardMapping`: empty text still falls through to an
    // image.
    let entry = serde_json::json!({
        "createdAt": "2001-01-02T00:00:00Z",
        "items": [{"representations": [
            {"mimeType": "text/plain", "content": ""},
            {"mimeType": "image/png", "contentType": "url", "content": "/synthetic/pin.png"}
        ]}]
    });
    let export = raycast_export(vec![entry]);
    let result = raycast::parse(&export, || NOW, |_| true);
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].kind, ItemKind::Image);
}

#[test]
fn a_raycast_export_without_entries_imports_nothing() {
    // Upstream `RaycastClipboardImport.parse`: a value that is not a
    // clipboard export imports nothing.
    let export = serde_json::json!({ "somethingElse": [] });
    let result = raycast::parse(&export, || NOW, |_| true);
    assert_eq!(result.items.len(), 0);
    assert_eq!(result.missing, 0);
}

// MARK: - The capture policy

#[test]
fn a_password_manager_is_excluded_by_default() {
    // Upstream `AppSettings.clipboardDisabledApps`: password managers ship
    // excluded, until the user first edits the list.
    let policy = CapturePolicy::default();
    assert!(policy.allows_source(None));
    assert!(policy.allows_source(Some("com.example.editor")));
    assert!(!policy.allows_source(Some("com.apple.keychainaccess")));
    assert!(!policy.allows_source(Some("com.apple.Passwords")));
}

#[test]
fn an_excluded_app_is_never_recorded() {
    // Upstream `ClipboardManager.poll`: a copy from an excluded app is
    // dropped.
    let policy = CapturePolicy::with_disabled_apps(&["com.example.secret".to_string()]);
    assert!(!policy.allows_source(Some("com.example.secret")));
    assert!(policy.allows_source(Some("com.example.editor")));
}

#[test]
fn a_long_copy_is_not_recorded() {
    // Upstream `ClipboardManager.poll`: a copy longer than the capture limit
    // is skipped, since truncation would lose the tail.
    let policy = CapturePolicy::default();
    assert!(policy.allows_text_length(32_000));
    assert!(!policy.allows_text_length(32_001));
}

#[test]
fn a_huge_file_selection_is_not_recorded() {
    // Upstream `ClipboardManager.maxCapturedFiles`: a finder's select-all must
    // not insert ten thousand rows on one poll tick.
    let policy = CapturePolicy::default();
    assert!(policy.allows_file_count(32));
    assert!(!policy.allows_file_count(33));
}
