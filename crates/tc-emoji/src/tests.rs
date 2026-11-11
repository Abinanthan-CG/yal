//! The emoji catalog, the search, the stores and the grid, against upstream's
//! own expectations.
//!
//! Every expectation here is upstream's, taken from `Tests/emoji-test.swift` and
//! `Tests/emoji-search-test.swift`.

use std::path::PathBuf;

use crate::catalog::{EmojiCategory, EmojiCategoryFilter, EmojiGridColumns, EmojiGridZoom};
use crate::geometry::EmojiGridGeometry;
use crate::index::EmojiIndex;
use crate::stores::{FrequentEmoji, FrequentEmojiStore, PinnedEmojiStore};
use crate::{EmojiCatalog, EmojiSkinTone};

/// A directory the test may write in.
fn directory() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "yal-emoji-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|since| since.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&path).expect("the directory is made");
    path
}

// MARK: - The dataset

#[test]
fn the_dataset_has_every_record() {
    // Upstream `emoji-test`: the dataset has more than 1 900 records.
    let entries = EmojiCatalog::parse(crate::data::RAW);
    assert!(
        entries.len() > 1900,
        "the dataset holds {} records",
        entries.len()
    );
}

#[test]
fn the_glyphs_are_unique() {
    let entries = EmojiCatalog::parse(crate::data::RAW);
    let mut glyphs: Vec<&str> = entries.iter().map(|entry| entry.glyph.as_str()).collect();
    glyphs.sort_unstable();
    let count = glyphs.len();
    glyphs.dedup();
    assert_eq!(count, glyphs.len(), "the glyphs are unique");
}

#[test]
fn every_category_is_populated() {
    let entries = EmojiCatalog::parse(crate::data::RAW);
    let mut seen: Vec<EmojiCategory> = Vec::new();
    for entry in &entries {
        if !seen.contains(&entry.category) {
            seen.push(entry.category);
        }
    }
    assert_eq!(seen.len(), EmojiCategory::ALL.len());
}

#[test]
fn the_entries_carry_their_own_facts() {
    // Upstream `emoji-test`: the waving hand takes a tone and carries the CLDR
    // keywords, the multi-person emoji does not, and the symbols are filed.
    let entries = EmojiCatalog::parse(crate::data::RAW);
    let wave = entries
        .iter()
        .find(|entry| entry.name == "waving hand")
        .expect("the waving hand is in the dataset");
    assert!(wave.supports_skin_tone, "👋 is tone-capable");
    assert!(
        wave.keywords.contains("hello"),
        "👋 carries the CLDR keywords"
    );

    let holding_hands = entries
        .iter()
        .find(|entry| entry.glyph == "🧑‍🤝‍🧑")
        .expect("the holding-hands emoji is in the dataset");
    assert!(
        !holding_hands.supports_skin_tone,
        "the multi-person emoji is not tone-capable"
    );

    let euro = entries
        .iter()
        .find(|entry| entry.glyph == "€")
        .expect("the euro sign is in the dataset");
    assert_eq!(euro.category, EmojiCategory::Currency);

    let reference = entries
        .iter()
        .find(|entry| entry.glyph == "※")
        .expect("the reference mark is in the dataset");
    assert_eq!(reference.category, EmojiCategory::Cjk);

    let command = entries
        .iter()
        .find(|entry| entry.glyph == "⌘")
        .expect("the command sign is in the dataset");
    assert_eq!(command.category, EmojiCategory::KeysAndTechnical);

    let apple = entries
        .iter()
        .find(|entry| entry.glyph == "\u{F8FF}")
        .expect("the apple logo is in the dataset");
    assert!(
        apple.keywords.contains("apple"),
        "the apple logo is searchable"
    );
}

#[test]
fn the_filters_lead_with_all_pinned_and_frequent() {
    // Upstream `emoji-test`: all, pinned and frequent precede every category.
    let filters = EmojiCategoryFilter::all();
    assert_eq!(filters.len(), EmojiCategory::ALL.len() + 3);
    assert_eq!(filters[0], EmojiCategoryFilter::All);
    assert_eq!(filters[1], EmojiCategoryFilter::Pinned);
    assert_eq!(filters[2], EmojiCategoryFilter::FrequentlyUsed);
}

#[test]
fn the_symbol_categories_say_symbol_and_the_others_say_emoji() {
    // Upstream `emoji-test`.
    assert_eq!(EmojiCategory::Symbols.item_title(), "Symbol");
    assert_eq!(EmojiCategory::Flags.item_title(), "Emoji");
}

// MARK: - The grid's density

#[test]
fn the_densities_cover_six_through_ten() {
    // Upstream `emoji-test`.
    let counts: Vec<usize> = EmojiGridColumns::ALL.iter().map(|c| c.count()).collect();
    assert_eq!(counts, vec![6, 7, 8, 9, 10]);
    assert_eq!(EmojiGridColumns::default(), EmojiGridColumns::Eight);
}

#[test]
fn the_zoom_stops_at_the_ends() {
    // Upstream `emoji-test`.
    assert_eq!(
        EmojiGridColumns::Six.applying(EmojiGridZoom::ZoomIn, EmojiGridColumns::Eight),
        None,
        "zoom-in stops at six"
    );
    assert_eq!(
        EmojiGridColumns::Ten.applying(EmojiGridZoom::ZoomOut, EmojiGridColumns::Eight),
        None,
        "zoom-out stops at ten"
    );
    assert_eq!(
        EmojiGridColumns::Eight.applying(EmojiGridZoom::ZoomIn, EmojiGridColumns::Eight),
        Some(EmojiGridColumns::Seven),
        "zoom-in drops one"
    );
    assert_eq!(
        EmojiGridColumns::Eight.applying(EmojiGridZoom::ZoomOut, EmojiGridColumns::Eight),
        Some(EmojiGridColumns::Nine),
        "zoom-out adds one"
    );
    assert_eq!(
        EmojiGridColumns::Six.applying(EmojiGridZoom::ActualSize, EmojiGridColumns::Nine),
        Some(EmojiGridColumns::Nine),
        "actual size returns to the configured default"
    );
    assert_eq!(
        EmojiGridColumns::Nine.applying(EmojiGridZoom::ActualSize, EmojiGridColumns::Nine),
        None,
        "actual size at the default changes nothing"
    );
}

// MARK: - The keyword packs

#[test]
fn the_keyword_packs_append_their_terms() {
    // Upstream `emoji-test`: the packs join after English, in preference order.
    let merged = EmojiCatalog::parse_with_localized(
        "A|alpha|ob|0|red\nB|beta|ob|0|",
        &[
            "A|rouge,vif\nbad line\nB|bleu".to_string(),
            "A|rot".to_string(),
        ],
    );
    assert_eq!(
        merged
            .iter()
            .map(|entry| entry.keywords.as_str())
            .collect::<Vec<&str>>(),
        vec!["red,rouge,vif,rot", "bleu"]
    );
}

// MARK: - The skin tones

#[test]
fn a_tone_is_appended_and_the_selector_is_stripped() {
    // Upstream `emoji-test`.
    assert_eq!(
        EmojiCatalog::apply_tone(EmojiSkinTone::Dark, "👋"),
        "👋🏿",
        "the modifier is appended"
    );
    let entries = EmojiCatalog::parse(crate::data::RAW);
    let victory = entries
        .iter()
        .find(|entry| entry.name == "victory hand")
        .expect("the victory hand is in the dataset");
    assert!(
        victory.glyph.chars().any(|scalar| scalar == '\u{FE0F}'),
        "the base carries the variation selector"
    );
    let toned = victory.display(EmojiSkinTone::Light);
    assert!(
        !toned.chars().any(|scalar| scalar == '\u{FE0F}')
            && toned.chars().any(|scalar| scalar == '\u{1F3FB}'),
        "the tone strips the selector and appends the modifier"
    );
    assert_eq!(
        victory.display(EmojiSkinTone::None),
        victory.glyph,
        "no tone leaves the glyph alone"
    );
    let holding_hands = entries
        .iter()
        .find(|entry| entry.glyph == "🧑‍🤝‍🧑")
        .expect("the holding-hands emoji is in the dataset");
    assert_eq!(
        holding_hands.display(EmojiSkinTone::Dark),
        holding_hands.glyph,
        "the tone is ignored on the entries that do not take one"
    );
}

#[test]
fn the_tones_are_the_ones_upstream_lists() {
    // Upstream `emoji-test`: the setting offers light, medium light, medium,
    // medium dark and dark.
    let titles: Vec<&str> = EmojiSkinTone::ALL.iter().map(|tone| tone.title()).collect();
    assert_eq!(
        titles,
        vec![
            "Default",
            "Light",
            "Medium Light",
            "Medium",
            "Medium Dark",
            "Dark"
        ]
    );
}

// MARK: - The grid's navigation

#[test]
fn the_grid_moves_down_and_up_through_the_sections() {
    // Upstream `emoji-test`.
    let grid = EmojiGridGeometry::new(vec![16, 10, 3], 8);
    assert_eq!(
        grid.down(0),
        8,
        "down within a full section keeps the column"
    );
    assert_eq!(grid.up(8), 0, "up within a full section keeps the column");
    assert_eq!(
        grid.down(8),
        16,
        "down enters the next section at column zero"
    );
    assert_eq!(
        grid.down(15),
        16 + 7,
        "down from column seven keeps the column"
    );
    assert_eq!(
        grid.down(16 + 4),
        16 + 9,
        "down clamps to the end of a short row"
    );
    assert_eq!(
        grid.down(16),
        16 + 8,
        "down within the second section keeps the column"
    );
    assert_eq!(
        grid.down(16 + 9),
        26 + 1,
        "down from a partial row keeps the column"
    );
    assert_eq!(grid.down(28), 28, "down from the last row changes nothing");
    assert_eq!(
        grid.up(26),
        16 + 8,
        "up lands on the last row of the section above"
    );
    assert_eq!(grid.up(28), 16 + 9, "up clamps into the shorter row above");
    assert_eq!(grid.up(3), 3, "up from the first row changes nothing");
    assert_eq!(
        grid.up(16 + 2),
        10,
        "up keeps the column into the section above"
    );
    assert_eq!(
        grid.up(16 + 7),
        15,
        "up clamps to the last cell of the section above"
    );
}

#[test]
fn a_single_section_grid_never_escapes_its_bounds() {
    // Upstream `emoji-test`.
    let single = EmojiGridGeometry::new(vec![5], 8);
    assert_eq!(single.down(2), 2, "down is a no-op");
    assert_eq!(single.up(2), 2, "up is a no-op");
    let empty = EmojiGridGeometry::new(vec![], 8);
    assert_eq!(empty.down(0), 0, "an empty grid is safe");
}

#[test]
fn unpinning_keeps_the_same_slot() {
    // Upstream `emoji-test`.
    assert_eq!(
        EmojiGridGeometry::selection_after_removing_pin(1, 3),
        1,
        "the neighbour that shifts into the removed slot is selected"
    );
    assert_eq!(
        EmojiGridGeometry::selection_after_removing_pin(2, 2),
        1,
        "the last pin selects its preceding neighbour"
    );
    assert_eq!(
        EmojiGridGeometry::selection_after_removing_pin(0, 0),
        0,
        "the only pin leaves a safe empty selection"
    );
}

#[test]
fn a_rewritten_section_takes_the_selection_with_it() {
    // Upstream `emoji-test`.
    let history = ["a", "b", "x", "d"];
    assert_eq!(
        EmojiGridGeometry::selection(3, 1, &history, &["x", "a", "b", "d"]),
        1,
        "a used emoji takes the selection with it to the front"
    );
    assert_eq!(
        EmojiGridGeometry::selection(6, 1, &history, &["n", "a", "b", "x", "d"]),
        7,
        "a growing section shifts the cells after it"
    );
    assert_eq!(
        EmojiGridGeometry::selection(4, 1, &history, &["a", "b"]),
        2,
        "a shrinking section clamps a dropped glyph to its last cell"
    );
    assert_eq!(
        EmojiGridGeometry::selection(0, 1, &history, &[]),
        0,
        "the cells before the section keep their index"
    );
}

#[test]
fn the_navigation_keeps_its_visual_column_at_every_width() {
    // Upstream `emoji-test`.
    let six = EmojiGridGeometry::new(vec![12, 8], 6);
    assert_eq!(six.down(2), 8);
    let ten = EmojiGridGeometry::new(vec![20], 10);
    assert_eq!(ten.down(7), 17);
}

// MARK: - The pins

#[test]
fn the_pins_keep_their_own_order() {
    // Upstream `emoji-search-test`: the pins are the author's order, not the
    // usage order; a reload keeps it and an import sanitizes it.
    let directory = directory();
    let path = directory.join("pinned.json");
    std::fs::write(&path, br#"["A", "", "A", "B"]"#).expect("the pins are written");

    let mut pinned = PinnedEmojiStore::open(&path);
    assert_eq!(
        pinned.glyphs(),
        ["A", "B"],
        "the load drops the blanks and duplicates"
    );

    pinned.toggle("C");
    assert_eq!(pinned.glyphs(), ["A", "B", "C"], "a new pin is appended");

    pinned.swap("C", "B");
    assert_eq!(
        pinned.glyphs(),
        ["A", "C", "B"],
        "a swap exchanges the two named pins"
    );

    pinned.swap("A", "missing");
    assert_eq!(
        pinned.glyphs(),
        ["A", "C", "B"],
        "a swap with an unpinned glyph changes nothing"
    );

    pinned.toggle("C");
    assert_eq!(
        pinned.glyphs(),
        ["A", "B"],
        "toggling an existing pin removes it"
    );

    pinned.replace(vec![
        "B".to_string(),
        "B".to_string(),
        "D".to_string(),
        String::new(),
    ]);
    assert_eq!(
        pinned.glyphs(),
        ["B", "D"],
        "the backup replacement keeps the order"
    );

    let reloaded = PinnedEmojiStore::open(&path);
    assert_eq!(
        reloaded.glyphs(),
        ["B", "D"],
        "the pin order survives a reload"
    );
}

#[test]
fn a_pin_that_cannot_be_written_is_reported() {
    // Upstream `emoji-search-test`: the persistence failure is reported.
    let directory = directory();
    let mut pinned = PinnedEmojiStore::open(&directory);
    assert!(!pinned.persistence_failure());
    pinned.toggle("A");
    assert!(pinned.persistence_failure(), "the failure is reported");
}

// MARK: - The history

#[test]
fn the_history_keeps_the_newest_use_first() {
    // Upstream `emoji-search-test`.
    let directory = directory();
    let path = directory.join("frequency.json");
    let mut history = FrequentEmojiStore::open(&path);
    history.record_at("A", 100);
    history.record_at("B", 200);
    history.record_at("A", 300);
    let glyphs: Vec<&str> = history.records().iter().map(|r| r.glyph.as_str()).collect();
    assert_eq!(glyphs, ["A", "B"], "the newest use leads");
    assert_eq!(history.records()[0].count, 2, "the tally counts the uses");

    let reloaded = FrequentEmojiStore::open(&path);
    let glyphs: Vec<&str> = reloaded
        .records()
        .iter()
        .map(|r| r.glyph.as_str())
        .collect();
    assert_eq!(glyphs, ["A", "B"], "the order survives a reload");
}

#[test]
fn a_full_history_evicts_the_oldest() {
    // Upstream `emoji-search-test`.
    let mut history = FrequentEmojiStore::in_memory();
    history.replace(
        (0..300)
            .map(|index| FrequentEmoji {
                glyph: index.to_string(),
                count: 100,
                last_used: index,
            })
            .collect(),
    );
    history.record_at("new", 1_000);
    assert_eq!(history.records()[0].glyph, "new", "the new usage leads");
    assert_eq!(history.records().len(), 300, "the history is capped");
    assert!(
        !history.records().iter().any(|record| record.glyph == "0"),
        "the oldest emoji is evicted"
    );
}

#[test]
fn an_import_keeps_only_the_newest_tally_of_a_repeated_glyph() {
    // Upstream `emoji-search-test`.
    let mut history = FrequentEmojiStore::in_memory();
    history.replace(vec![
        FrequentEmoji {
            glyph: "B".to_string(),
            count: 2,
            last_used: 200,
        },
        FrequentEmoji {
            glyph: "B".to_string(),
            count: 1,
            last_used: 100,
        },
    ]);
    assert_eq!(history.records().len(), 1, "the repeated glyph is one row");
    assert_eq!(history.records()[0].count, 2, "the newest tally is kept");
}

// MARK: - The search

/// The glyphs the search answers with, in order.
fn search(index: &EmojiIndex, query: &str, frequent: &FrequentEmojiStore) -> Vec<String> {
    index
        .search(query, frequent, 320)
        .into_iter()
        .map(|entry| entry.glyph.clone())
        .collect()
}

#[test]
fn the_search_finds_the_usual_characters() {
    // Upstream `emoji-search-test`.
    let index = EmojiIndex::load();
    let frequent = FrequentEmojiStore::in_memory();
    for (query, glyph, rank) in [
        ("pray", "🙏", 5),
        (":+1:", "👍", 1),
        (":-1:", "👎", 1),
        (":rocket:", "🚀", 1),
        ("hand waving", "👋", 1),
        ("face joy", "😂", 1),
        ("tears joy", "😂", 1),
        ("birthday", "🎂", 1),
        ("party", "🎉", 1),
        ("states united", "🇺🇸", 1),
    ] {
        let results = index.search(query, &frequent, 320);
        assert!(
            results.iter().take(rank).any(|entry| entry.glyph == glyph),
            "{query} finds {glyph} in the first {rank} results"
        );
    }
}

#[test]
fn the_multiword_rules_hold() {
    // Upstream `emoji-search-test`.
    let index = EmojiIndex::load();
    let frequent = FrequentEmojiStore::in_memory();
    assert!(
        search(&index, "hand waving", &frequent).contains(&"👋".to_string()),
        "the multiword terms can match in either order"
    );
    assert!(
        !search(&index, "and waving", &frequent).contains(&"👋".to_string()),
        "the multiword terms must begin a word"
    );
    assert!(
        !search(&index, "hello missing", &frequent).contains(&"👋".to_string()),
        "every multiword term must match"
    );
    assert!(
        search(&index, "quuxxyz", &frequent).is_empty(),
        "a query that matches nothing finds nothing"
    );
}

#[test]
fn the_case_and_the_whitespace_do_not_change_the_ranking() {
    // Upstream `emoji-search-test`.
    let index = EmojiIndex::load();
    let frequent = FrequentEmojiStore::in_memory();
    assert_eq!(
        search(&index, "  RED\t HEART\n", &frequent),
        search(&index, "red heart", &frequent),
        "the case and the whitespace do not change the ranking"
    );
}

#[test]
fn the_accents_and_the_widths_are_folded() {
    // Upstream `emoji-search-test`.
    let index = EmojiIndex::load();
    let frequent = FrequentEmojiStore::in_memory();
    assert_eq!(
        search(&index, "piñata", &frequent),
        search(&index, "ＰＩＮＡＴＡ", &frequent),
        "the accent and the width folding are preserved"
    );
}

#[test]
fn a_fuzzy_match_cannot_cross_a_keyword_boundary() {
    // Upstream `emoji-search-test`.
    let index = EmojiIndex::load_raw("A|zebra|ob|0|red,blue\nB|ladybug bug|ob|0|red");
    let frequent = FrequentEmojiStore::in_memory();
    assert!(
        !search(&index, "db", &frequent).contains(&"A".to_string()),
        "a fuzzy match cannot cross a keyword boundary"
    );
    assert_eq!(
        search(&index, "red blue", &frequent)
            .first()
            .map(String::as_str),
        Some("A"),
        "the separate query words may match separate keywords"
    );
    assert!(
        !search(&index, "red,blue zebra", &frequent).contains(&"A".to_string()),
        "one term cannot cross the keyword separator"
    );
    assert_eq!(
        search(&index, "bug red", &frequent)
            .first()
            .map(String::as_str),
        Some("B"),
        "a later word-start hit survives an earlier mid-word hit"
    );
}

#[test]
fn a_literal_phrase_outranks_the_reordered_words() {
    // Upstream `emoji-search-test`.
    let index =
        EmojiIndex::load_raw("A|zebra|ob|0|red blue\nB|blue red|ob|0|\nC|zebra|ob|0|red,blue");
    let frequent = FrequentEmojiStore::in_memory();
    assert_eq!(
        search(&index, "red blue", &frequent),
        vec!["A", "B", "C"],
        "a literal phrase outranks the reordered name words, then the separate keywords"
    );
    assert_eq!(
        search(&index, "blue red", &frequent)
            .first()
            .map(String::as_str),
        Some("B"),
        "a literal full name outranks the metadata matches"
    );
}

#[test]
fn the_full_name_ranks_first() {
    // Upstream `emoji-search-test`.
    let index =
        EmojiIndex::load_raw("A|red balloon|ob|0|\nB|zebra|ob|0|red\nC|redwood|ob|0|\nD|red|ob|0|");
    let frequent = FrequentEmojiStore::in_memory();
    assert_eq!(
        search(&index, "red", &frequent),
        vec!["D", "A", "B", "C"],
        "the full name, the complete leading word, the exact keyword, then the partial one"
    );
}

#[test]
fn the_usage_breaks_a_tie_but_not_a_stronger_match() {
    // Upstream `emoji-search-test`.
    let index = EmojiIndex::load_raw("A|alpha|ob|0|\nB|beta|ob|0|alpha");
    let mut frequent = FrequentEmojiStore::in_memory();
    frequent.record_at("B", 1);
    assert_eq!(
        search(&index, "alpha", &frequent)
            .first()
            .map(String::as_str),
        Some("A"),
        "the usage cannot overtake a stronger text match"
    );
}

#[test]
fn the_usage_reranks_a_tie() {
    // Upstream `emoji-search-test`.
    let index = EmojiIndex::load_raw("A|alpha|ob|0|red\nB|beta|ob|0|red");
    let mut frequent = FrequentEmojiStore::in_memory();
    frequent.replace(vec![
        FrequentEmoji {
            glyph: "A".to_string(),
            count: 2,
            last_used: 100,
        },
        FrequentEmoji {
            glyph: "B".to_string(),
            count: 1,
            last_used: 200,
        },
    ]);
    assert_eq!(
        search(&index, "red", &frequent).first().map(String::as_str),
        Some("A"),
        "the count breaks the text tie before the recency"
    );
    frequent.replace(vec![
        FrequentEmoji {
            glyph: "A".to_string(),
            count: 2,
            last_used: 100,
        },
        FrequentEmoji {
            glyph: "B".to_string(),
            count: 2,
            last_used: 200,
        },
    ]);
    assert_eq!(
        search(&index, "red", &frequent).first().map(String::as_str),
        Some("B"),
        "the recency breaks the tie between equal counts"
    );
    frequent.replace(vec![]);
    assert_eq!(
        search(&index, "red", &frequent).first().map(String::as_str),
        Some("A"),
        "clearing the usage restores the catalog order"
    );
}

#[test]
fn the_search_honours_its_limit() {
    // Upstream `emoji-search-test`.
    let index = EmojiIndex::load();
    let frequent = FrequentEmojiStore::in_memory();
    assert_eq!(
        index.search("face", &frequent, 1).len(),
        1,
        "the limit is honoured"
    );
    assert_eq!(
        index.search("face", &frequent, 12).len(),
        12,
        "the limit is honoured"
    );
    assert!(
        index.search(" ", &frequent, 320).is_empty(),
        "an empty query finds nothing"
    );
    assert!(
        index.search("face", &frequent, 0).is_empty(),
        "a zero limit finds nothing"
    );
}

#[test]
fn the_sections_are_in_the_catalogs_order() {
    // Upstream `EmojiIndex.load`: the sections are the catalog's order.
    let index = EmojiIndex::load();
    let sections = index.category_sections();
    let categories: Vec<EmojiCategory> = sections.iter().map(|(category, _)| *category).collect();
    let mut expected: Vec<EmojiCategory> = EmojiCategory::ALL.to_vec();
    expected.retain(|category| sections.iter().any(|(name, _)| name == category));
    assert_eq!(
        categories, expected,
        "the sections keep the catalog's order"
    );
    assert!(!sections.is_empty(), "the catalog has sections");
}
