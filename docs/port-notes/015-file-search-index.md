# 015 - The file index, its scopes, its rules and its search

Date: 2026-11-12
Crate: `crates/tc-files` (`src/scope.rs`, `src/index.rs`, `src/store.rs`,
`src/search.rs`)
Upstream: `docs/features/file-search.md`, `Tinycast/Features/FileSearch/`
Donor: `C:\Users\dabin\development\yal\crates\yal-index\src\walk.rs` and
`crates\yal-store\src\file_index.rs` (GPL-3.0-only,
`github.com/Abinanthan-CG/yal`)

## What this slice is

The private index behind the Search Files command: the folders the user picks,
the rules that keep things out of them, the pass that keeps the index
current, the store it lives in, and the search over it.

**The screen is not part of it.** Upstream's file search is a screen of its
own - the results list, the preview panel, the type filter and the actions on
a row - and this port has no screen for it yet.

## The scopes

| Upstream's behaviour | Our test |
| --- | --- |
| an empty scope list indexes nothing | `an_empty_scope_list_indexes_nothing` |
| a scope keeps its folder | `a_scope_keeps_its_folder` |
| the home folder is stored with a tilde | `the_home_folder_is_stored_with_a_tilde` |
| a stored scope is read back against the home folder | `a_stored_scope_is_read_back_against_the_home_folder` |
| a scope outside the home folder is stored as it is | `a_scope_outside_the_home_folder_is_stored_as_it_is` |

Upstream's rules, carried over: only the listed folders are indexed; an
empty list indexes nothing and there is no fall back to the home folder; a
folder inside another scope is already covered; other disks are never entered
unless they are a scope; the scopes are saved with the home folder written as
`~`, so a backup still works on another machine.

## The ignore rules

Upstream's three kinds, and how they are matched:

| Kind | Matched against | Our test |
| --- | --- | --- |
| a plain word | any folder or name | `a_plain_word_matches_any_name_that_contains_it` |
| one with `*`, `?` or `[` | any folder or name | `a_star_matches_any_run_of_characters`, `a_question_mark_matches_exactly_one_character`, `a_character_class_matches_the_characters_in_it` |
| one containing `/`, or starting `~/` | the whole path | `a_rule_with_a_slash_matches_the_whole_path` |

`a_star_matches_a_slash_as_well` is upstream's own note - `*` matches `/`, so
a `**/…/**` pattern works as it is written. `a_tilde_rule_does_not_match_another_machines_folder`
is the same rule seen from the other side: `~/Movies` is the home folder's
Movies, not any folder with that name.

The built-in rules, and the user's own beside them:

| Upstream's behaviour | Our test |
| --- | --- |
| the build trees are left out | `the_built_in_rules_keep_the_build_trees_out` |
| the system folders are left out | `the_built_in_rules_keep_the_system_folders_out` |
| the user's own files are not | `the_built_in_rules_do_not_skip_the_users_own_files` |
| a user's own rule sits next to the built-in ones | `a_users_own_rule_sits_next_to_the_built_in_ones` |
| a built-in rule can be removed | `a_built_in_rule_can_be_removed` |
| Restore Defaults keeps the user's own rules | `restoring_the_defaults_keeps_the_users_own_rules` |

Upstream's built-in list is macOS-shaped. This port's is the Windows
equivalent of the same kinds of folder: build trees and dependency folders,
the caches, and the busiest parts of the system folders. Nothing else is
skipped - upstream indexes hidden files, and so does this port.

## The pass that keeps the index current

| Upstream's behaviour | Our test |
| --- | --- |
| the first build lists the whole tree | `a_fresh_tree_is_listed_whole` |
| an unchanged tree is not listed again | `an_unchanged_tree_is_not_listed_twice` |
| a creation and a removal are both caught without a full walk | `a_creation_and_a_removal_are_both_caught_without_a_full_walk` |
| a folder that is gone forgets itself and its contents | `a_removed_directory_forgets_itself_and_its_contents` |
| an ignored folder is never entered or recorded | `an_ignored_folder_is_never_entered_or_recorded` |
| an ignored file is not recorded but its folder is | `an_ignored_file_is_not_recorded_but_its_folder_is` |
| a folder renamed into an ignored name is evicted | `a_folder_renamed_into_an_ignored_name_is_evicted` |
| a file carries its size and the time it changed | `a_file_carries_its_size_and_the_time_it_changed` |
| a folder is recorded as a folder | `a_folder_is_recorded_as_a_folder` |
| no roots means nothing is listed | `no_roots_means_nothing_is_listed` |
| a root that is not a directory is not listed | `a_root_that_is_not_a_directory_is_not_listed` |

### The rules that are easy to get wrong

- **Nanosecond times, never milliseconds.** NTFS ticks at 100 nanoseconds; a
  millisecond floor buries a write that lands in the same millisecond the
  pass recorded the directory.
- **The time is read *after* the listing**, so a change that lands between
  the stat and the read is caught by the next pass instead of being buried.
- **Junctions and reparse points are treated as absent**, so a walk cannot
  loop.
- **Deletion is exact, per directory.** No prefix sweep: `LIKE` would get an
  underscore in a path wrong.

## The store

| Upstream's behaviour | Our test |
| --- | --- |
| a fresh store is empty | `a_fresh_store_is_empty` |
| a directory keeps its listing as the whole truth | `a_directory_keeps_its_listing_as_the_whole_truth` |
| a forgotten directory takes its contents along | `a_forgotten_directory_takes_its_contents_along` |
| the recorded directories come back with their times | `the_recorded_directories_come_back_with_their_times` |
| clearing the store empties every table | `clearing_the_store_empties_every_table` |
| the build time is remembered | `the_build_time_is_remembered` |
| a walk over a real folder fills the store | `a_walk_over_a_real_folder_fills_the_store` |
| a rebuild swaps the new index in | `a_rebuild_swaps_the_new_index_in` |
| a rebuild over a missing folder is not a crash | `a_rebuild_keeps_the_old_index_when_it_fails` |

**The index holds nothing about a file's contents** - only the name, where
the thing is, when it changed, how big it is, and whether it is a folder.

**The rebuild is upstream's rule.** "Changing a scope or a pattern rebuilds
the index. Search keeps using the old index until the new one is ready." The
new index is built in a file beside the old one and swapped in when it is
finished, so a search that is already running keeps the old index until the
new one is complete. Windows will not rename a file that is open, so the
store holding the old index has to be closed before the swap; the search runs
over an in-memory snapshot, so nothing is lost by letting go of the file
between rebuilds.

## The search

| Upstream's behaviour | Our test |
| --- | --- |
| every word must appear in the name | `every_word_must_appear_in_the_name` |
| an empty query matches nothing | `an_empty_query_matches_nothing` |
| the best matches come first | `the_best_matches_come_first` |
| a name that starts with the query beats one that only contains it | `a_name_that_starts_with_the_query_comes_before_one_that_only_contains_it` |
| a name that is exactly the query beats a longer one | `a_name_that_is_exactly_the_query_beats_a_longer_one` |
| the user's own files come before the rest | `the_users_own_files_come_before_the_rest` |
| a hidden file ranks below the user's own | `a_hidden_file_ranks_below_the_users_own` |
| a file in a program folder is not the user's own | `a_file_in_a_program_folder_is_never_in_the_users_own_files` |
| the limit is honoured | `the_limit_is_honoured` |
| a folder is found as well as a file | `a_folder_is_found_as_well_as_a_file` |
| the matching ignores the accents | `the_matching_ignores_the_accents` |
| the recent list is the home folder's newest files | `the_recent_list_is_the_home_folders_newest_files` |
| the recent list leaves out hidden and program files | `the_recent_list_leaves_out_hidden_and_program_files` |
| the recent list honours its limit | `the_recent_list_honours_its_limit` |

Upstream's rules, carried over: every word you type must appear in the file
name, in any order; the best matches come first - an exact name, then names
that start with what you typed; files in the home folder rank above hidden
files and the contents of the program folders; at most 200 results; and with
nothing typed the list is the files in the home folder that changed most
recently, with hidden files and program folders left out.

## What the donor contributed

The donor's pass is the design of `src/index.rs` - the two tables, the
nanosecond freshness, the reparse-point rejection and the exact deletion -
and its file `crates/tc-files/ATTRIBUTION.md` carries its GPL-3.0 notice.

**Its scope model was not taken.** It indexes the machine's fixed drives;
upstream indexes the folders the user picks, with editable rules, and this
port follows upstream. **Its search was not taken either**: upstream's own
rules are in `src/search.rs`, including the accents, which the donor does
not fold.

## What is not in this slice

- **The screen.** The results list, the preview panel, the type filter, the
  actions on a row (Open, Show in Explorer, Copy Path, Add to Favorites, Move
  to the Recycle Bin), and the "Files" section of the launcher's own results.
- **The Settings pane.** The switch, the scopes list, the ignore rules list,
  the Indexed Items count and the Rebuild Index button.
- **The messages.** "Indexing your disk…", "Couldn't build the file index",
  and the empty states.
- **The helper process.** Upstream builds the index in a separate process so
  its memory is returned; here the caller is that process.
- **The watcher.** Upstream watches the folders it indexed and updates only
  what changed; this port re-walks on demand and re-lists only the
  directories whose time moved, which reaches the same result more slowly
  and without a resident watcher.

## Why no row is `PORTED` yet

Every file-search row describes something the user sees - a result, a
preview, a filter, an action. This slice is the engine behind them. **The
rows stay `NOT PORTED` until the screen exists and a test on this machine
proves it.**

## Test count

`tc-files` has 55 tests: 19 for the scopes and the rules, 11 for the pass,
10 for the store and 15 for the search.
