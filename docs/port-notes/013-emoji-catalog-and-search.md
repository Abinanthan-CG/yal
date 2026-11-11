# 013 - The emoji catalog, its search and its two stores

Date: 2026-11-11
Crate: `crates/tc-emoji` (`src/data.rs`, `src/catalog.rs`, `src/index.rs`,
`src/stores.rs`, `src/geometry.rs`)
Upstream: `Tinycast/Features/Emoji/` and `Tests/emoji-test.swift`,
`Tests/emoji-search-test.swift`

## What this slice is

The model behind the emoji picker: the whole catalog, the search over it, the
pins, the frequency, the skin tones and the grid's navigation.

**The picker screen is not part of it.** Upstream's emoji picker is a screen of
its own, and this port has no screen for it yet, so the rows that describe the
screen stay `NOT PORTED` even where the model behind them is done.

## The catalog

Upstream's dataset is 2 136 entries in a pipe-delimited table:
`glyph|name|category|tone|keywords`. It is carried over in the same format, in
the same order, because the order is the tie-break that keeps equally good
matches in the catalog's order.

| Upstream's behaviour | Our test |
| --- | --- |
| the dataset holds every record | `the_dataset_has_every_record` |
| the glyphs are unique | `the_glyphs_are_unique` |
| every section is populated | `every_category_is_populated` |
| the waving hand takes a tone and carries the CLDR keywords | `the_entries_carry_their_own_facts` |
| the multi-person emoji takes no tone | `the_entries_carry_their_own_facts` |
| `€` is in Currency, `※` in CJK Symbols, `⌘` in Keys & Technical | `the_entries_carry_their_own_facts` |
| the apple logo is searchable as *apple* | `the_entries_carry_their_own_facts` |
| all, pinned and frequent precede every section | `the_filters_lead_with_all_pinned_and_frequent` |
| the symbol sections say *Symbol*, the emoji sections say *Emoji* | `the_symbol_categories_say_symbol_and_the_others_say_emoji` |

### The aliases are the names

A unit of emoji is filed under its **name**, and the keyword list is what makes
it searchable. The glyph is the id.

## The search

The search is upstream's ranking, and it runs on the fuzzy matcher that
`tc-search` already carries — the same one the launcher uses.

| Upstream's behaviour | Our test |
| --- | --- |
| the usual characters are found | `the_search_finds_the_usual_characters` |
| the multiword terms match in either order, and every term must match | `the_multiword_rules_hold` |
| the case and the whitespace do not change the ranking | `the_case_and_the_whitespace_do_not_change_the_ranking` |
| the accents and the widths are folded | `the_accents_and_the_widths_are_folded` |
| a fuzzy match cannot cross a keyword boundary | `a_fuzzy_match_cannot_cross_a_keyword_boundary` |
| a literal phrase outranks the reordered words | `a_literal_phrase_outranks_the_reordered_words` |
| the full name ranks first | `the_full_name_ranks_first` |
| the usage breaks a tie but not a stronger match | `the_usage_breaks_a_tie_but_not_a_stronger_match` |
| the usage reranks a tie | `the_usage_reranks_a_tie` |
| the limit is honoured | `the_search_honours_its_limit` |

The scores are upstream's: a keyword match is 500 below the name, a complete
leading word is 95 000, the scattered name words are 60 000 and the mixed ones
50 000. The most-used character weighs 100 and the hundredth weighs 1.

## The pins and the frequency

The pins are the user's own order; the frequency is what the user did. They
are kept in separate files so neither can rewrite the other.

| Upstream's behaviour | Our test |
| --- | --- |
| the pins are the author's order, and a reload keeps it | `the_pins_keep_their_own_order` |
| a pin that cannot be written is reported | `a_pin_that_cannot_be_written_is_reported` |
| the newest use leads the history | `the_history_keeps_the_newest_use_first` |
| a full history evicts the oldest | `a_full_history_evicts_the_oldest` |
| an import keeps only the newest tally of a repeated glyph | `an_import_keeps_only_the_newest_tally_of_a_repeated_glyph` |

## The skin tones and the grid

| Upstream's behaviour | Our test |
| --- | --- |
| the tone is appended and the variation selector stripped | `a_tone_is_appended_and_the_selector_is_stripped` |
| the tone is ignored on the entries that take none | `a_tone_is_appended_and_the_selector_is_stripped` |
| the setting offers light through dark | `the_tones_are_the_ones_upstream_lists` |
| the densities cover six through ten columns, and eight is the default | `the_densities_cover_six_through_ten` |
| the zoom stops at the ends | `the_zoom_stops_at_the_ends` |
| the grid moves down and up through the sections | `the_grid_moves_down_and_up_through_the_sections` |
| a single section never escapes its bounds | `a_single_section_grid_never_escapes_its_bounds` |
| unpinning keeps the same slot | `unpinning_keeps_the_same_slot` |
| a rewritten section takes the selection with it | `a_rewritten_section_takes_the_selection_with_it` |
| the navigation keeps its visual column at every width | `the_navigation_keeps_its_visual_column_at_every_width` |

## The one thing that is not here

Upstream ships a keyword pack per language — German, Spanish, French, Japanese,
Korean, Portuguese, Russian and Simplified Chinese — so that searching in the
user's own language finds the same emoji. **Those packs are not in upstream's
repository**: they live in the app bundle as resources, and the clone this port
was written against does not have them.

The mechanism is ported anyway: `EmojiCatalog::parse_with_localized` joins the
packs to the English keywords, and the pack is a `glyph|keyword,keyword` table.
What is missing is the data, so the English keywords are all this port searches.

## Test count

`tc-emoji` has 32 tests, every one of them written from upstream's own test
files.
