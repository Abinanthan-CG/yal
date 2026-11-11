# 010 - The snippet library: files on disk, keywords, import

Date: 2026-11-11
Crate: `crates/tc-snippets` (`src/repository.rs`, `src/store.rs`, `src/keyword.rs`,
`src/raycast.rs`)
Upstream: `Tinycast/Features/Snippets/Model/SnippetStore.swift`,
`SnippetRepository.swift`, `SnippetKeywordPolicy.swift`, and
`Tests/snippets-test.swift`

## What this slice is

Notes 008 and 009 ported a single snippet and the engine that expands it. This
slice is the library around them: the files the snippets live in, the store that
holds them, the keyword matcher that pastes a snippet without opening it, and
the importer that reads a Raycast export.

A snippet is a Markdown file. The file's front matter holds the name, the
keyword and the tags; the body is the template. The file's name is the
snippet's identity - the front matter's `name` is a label, not an identity, and
two snippets may carry the same label without colliding.

## The repository

`SnippetRepository` reads a directory of `.md` files and writes back to it.

| Upstream's behaviour | Our test |
| --- | --- |
| a snippet is read from its Markdown file | `the_front_matter_name_does_not_replace_the_path_identity` |
| a file that cannot be parsed is reported, the rest still load | `a_file_that_cannot_be_read_is_reported_and_the_others_are_read` |
| creating a snippet writes a new file | `a_created_snippet_never_overwrites_another` |
| a batch import writes every file | `a_batch_import_round_trips_through_the_files` |
| a save that would overwrite a file changed on disk is refused | `a_save_refuses_to_overwrite_a_file_that_changed` |
| a delete removes the file | `deleting_a_snippet_removes_its_file` |

### The overwrite bug this slice found

The worker wrote the repository first and the tests after. The tests caught a
real defect: `create` wrote the snippet with a plain `fs::write`, which
**overwrote** whatever was already at the path. Two snippets with the same slug
silently became one, and the first one's text was lost.

The fix is `write_new_file`, which opens with `OpenOptions::create_new(true)` -
the write fails if the path exists rather than replacing it. The collision
suffixes (`same.md`, `same-2.md`, …) are then applied deliberately, by the
caller that meant to make a second snippet, and the test
`a_created_snippet_never_overwrites_another` holds the line.

This is the second time in this port that a slice which "compiled fine" was
wrong in a way only a test could see. The first was the calculator's 47 tests
that were never registered. Both are the same lesson, now written down in
`docs/DECISIONS.md`: **a crate that compiles is not a crate that is tested**.

## The keyword matcher

Upstream's snippets can be pasted by typing `!keyword` followed by a space.
The matcher buffers what the user types and fires when the buffer matches a
snippet's keyword.

| Upstream's behaviour | Our test |
| --- | --- |
| the keyword is trimmed and lowercased | `a_keyword_is_trimmed_and_lowercased` |
| the buffer matches only when the typed text ends with it | `the_buffer_matches_the_keyword_that_was_typed` |
| text that is not the keyword does not match | `a_keyword_that_is_not_typed_is_not_matched` |
| when two keywords match, the longer one wins | `the_longest_match_wins` |
| an empty keyword is not a keyword | `an_empty_keyword_is_rejected` |
| the buffer can be cleared | `a_reset_clears_the_buffer` |

The longest-match rule matters: with keywords `a` and `ab`, typing `!ab` must
paste the `ab` snippet, not the `a` one.

## The Raycast import

Upstream can import a Raycast snippet export - a JSON file with a list of
entries. `raycast::parse` reads that shape and produces snippets; a container
it does not recognise is ignored rather than an error, so an export from a
newer Raycast still imports the parts we understand.

| Upstream's behaviour | Our test |
| --- | --- |
| a Raycast export is read | `a_raycast_export_is_read` |
| an unrecognised container is ignored | `an_unrecognized_raycast_container_is_ignored` |

## The store

`SnippetsStore` is the state the palette reads: the snippets, the issues found
while loading, and the error from the last operation. It delegates the file
work to the repository and keeps the last error visible, so a failed load shows
why instead of showing an empty list.

## What this slice does not do

- **The palette rows** - a snippet is still not in the launcher, and the
  `!keyword` expansion is not wired to the keyboard hook.
- **The settings UI** - there is no screen for creating a snippet yet; the
  store's `create` is reachable only from Rust.
- **Snippets in the launcher search** - `store.snippets()` is the whole
  collection, not a searched one.

## Test count

`tc-snippets` now has 70 tests: 26 for the record and the file format, 29 for
the template engine, and 15 for the library layer above.
