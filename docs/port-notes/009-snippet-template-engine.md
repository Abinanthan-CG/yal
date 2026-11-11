# 009 - The snippet template engine

Date: 2026-11-11
Crate: `crates/tc-snippets` (`src/template.rs`)
Upstream: `Tinycast/Features/Snippets/Model/SnippetTemplateEngine.swift` and
`Tests/snippets-test.swift`

## What this slice is

A snippet's text is a template. A template is plain text with `{…}`
placeholders in it, and the engine expands them:

| The token | What it expands to |
| --- | --- |
| `{clipboard}` | the clipboard contents; `{clipboard offset=1}` reaches back through the history |
| `{selection}` | the text the user had selected (`{selectedText}` is Raycast's name for it) |
| `{date}`, `{time}`, `{datetime}`, `{day}` | today, in the user's locale |
| `{uuid}` | a fresh UUID for each token |
| `{cursor}` | where the cursor goes afterward |
| `{argument name="…"}` | asks the user for a value |
| `{snippet:Name}` | another snippet, inserted inline |

A token that is malformed, or that names something unknown, **stays in the text
as it was written**. That is the rule that makes the rest safe: a snippet with
a typo in it pastes what the user typed, not an error.

## The oracle

Upstream's test file has a section that tests every token, parameter and
modifier against an injected clock, locale and UUID source. The port's tests
are the same cases, with the same values:

| Upstream's check | Our test |
| --- | --- |
| `expand("{datetime}").text == "Jul 24, 2026 at 1:05\u{202F}PM"` | `the_date_and_the_time_tokens_render_their_styles` |
| `expand("{day offset=-3d}").text == "Tuesday"` | `an_offset_shifts_the_date` |
| `expand("{date offset="+2y +5M"}").text == "Dec 24, 2028"` | `several_offsets_apply_in_order` |
| `expand("{date offset="+1w"}").text == "{date offset="+1w"}"` | `an_offset_that_cannot_be_read_leaves_the_token_literal` |
| `expand("{date format=MMMM d, yyyy}").text == "July 24, 2026"` | `the_format_parameter_picks_the_pattern` |
| `expand("{uuid}|{uuid}").text == "uuid-1\|uuid-2"` | `each_uuid_token_draws_a_fresh_value` |
| `expand("{clipboard offset=1}\|{clipboard offset=2}").text == "older\|oldest"` | `the_clipboard_token_reaches_back_through_the_history` |
| `expand("{clipboard \| trim \| uppercase}").text == "NEWEST"` | `the_modifiers_chain_from_left_to_right` |
| `expand("{argument name="U" \| percent-encode}")` | `percent_encode_escapes_everything_outside_the_unreserved_set` |
| `expand("{date offset="+1d" offset="+2d"}")` stays literal | `a_duplicated_parameter_leaves_the_token_literal` |
| `expand("{uuid offset=1}")` stays literal | `a_parameter_on_a_token_that_takes_none_leaves_it_literal` |
| `expand("{selectedText offset=1}")` stays literal | `the_selection_alias_rejects_parameters` |

The clock is 24 July 2026, 13:05 UTC, the locale is `en_US_POSIX`, the clipboard
history is `["  newest  ", "older", "oldest"]` and the selection is `picked` —
upstream's own setup, so the expected values are upstream's own.

`tc-snippets` has 55 tests: 29 for the template engine and 26 for the record
and the file format.

## The locale table

Windows has no ICU, so the date tokens format with a small table per locale
instead of a framework: the month names, the weekday names and the pattern each
style uses. `en_US_POSIX` and `fr_FR` are the locales upstream's tests pin, and
an unknown locale falls back to English.

The English table uses the **narrow no-break space** (U+202F) before the AM/PM
marker, which is what CLDR 42 does and what upstream's expected strings carry:
`1:05\u{202F}PM`.

## What the engine does not know

- **The keyword expansion** — the `!keyword` that pastes a snippet without
  opening it — is a separate feature and is not ported.
- **The repository** — the engine expands a template, but nothing yet reads the
  snippet files or writes them back.
- **The palette rows** — a snippet is not in the launcher yet.

## Where this came from

Like the rest of the crate, this was written by the background worker that was
aborted: it arrived with seven compile errors and no tests. What is here now is
that code with the errors fixed — a moved value, a `&char` where a `char` was
wanted, a name that was not in scope, a `Days::new` that cannot hold a negative
offset, a trait that clashed with the module's own `Offset` — and 29 tests
written from upstream's own expectations.
