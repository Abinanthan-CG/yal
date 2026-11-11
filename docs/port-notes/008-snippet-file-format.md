# 008 - The snippet record and its Markdown file

Date: 2026-11-11
Crate: `crates/tc-snippets`
Upstream: `Tinycast/Features/Snippets/Model/Snippet.swift`,
`SnippetMarkdownSerializer.swift`, and `Tests/snippets-test.swift`

## What this slice is

A snippet in upstream is a name, the text that is pasted, an optional keyword
that triggers it, whether it may be used at all, and whether it asks before it
runs. It is stored as **one Markdown file**, with a small front matter block and
the body after it.

This slice ports that: the record, the file format, and the locale table the
date tokens will format with.

| Our module | Upstream file | What it does |
| --- | --- | --- |
| `snippet.rs` | `Snippet.swift`, `SnippetSourceRevision` | the record, the file's fingerprint, the stored snippet |
| `markdown.rs` | `SnippetMarkdownSerializer.swift` | the file format: reading it and writing it |
| `locale.rs` | — | the month names, weekday names and patterns the date tokens use |

## The file format

The front matter is optional, and a file that does not open with an exact `---`
line is all body. The keys are `name`, `keyword`, `enabled` and
`show_confirmation`, and they are not case-sensitive:

```
---
name: "Quote \" slash \\ line\nreturn\rtab\t雪"
keyword: "!\"\\\n\t"
enabled: false
show_confirmation: true
---
```

A text value has to be quoted. An unknown key, an unquoted value, a repeated
key, a boolean that is not exactly `true` or `false`, a missing closing
delimiter and a delimiter with a trailing space are all rejected, and the error
names the file, the line and the problem.

Everything after the closing `---` is the body, kept exactly as written: blank
lines, CRLF line endings, Unicode, and any later `---` lines.

## The oracle

Upstream's `Tests/snippets-test.swift` pins all of this, and the port's tests
are the same cases with upstream's values:

| Upstream's check | Our test |
| --- | --- |
| the serialized form starts with its front matter | `the_serialized_form_starts_with_its_front_matter` |
| a quoted scalar cannot inject a key | `a_quoted_scalar_cannot_inject_a_key` |
| a carriage return cannot inject a key | `a_carriage_return_cannot_inject_a_key` |
| CRLF front matter parses its keys | `crlf_front_matter_parses_its_keys` |
| a missing `show_confirmation` defaults to false | `a_missing_show_confirmation_defaults_to_false` |
| the booleans are strict | `a_boolean_has_to_be_exactly_true_or_false` |
| a `---` inside the body stays literal | `a_delimiter_inside_the_body_stays_literal` |
| a file without front matter is all body | `a_file_without_front_matter_is_all_body` |
| a blank name falls back to the file name | `a_blank_name_falls_back_to_the_file_name` |
| a missing closing delimiter is rejected | `a_missing_closing_delimiter_is_rejected` |
| a closing delimiter with a trailing space is rejected | `a_closing_delimiter_with_a_trailing_space_is_rejected` |
| an unquoted scalar is rejected | `an_unquoted_scalar_is_rejected` |
| an unknown escape is rejected | `an_unknown_escape_is_rejected` |
| a duplicate key is rejected | `a_duplicate_key_is_rejected` |
| an unknown key is rejected | `an_unknown_key_is_rejected` |

`tc-snippets` has 26 tests.

## The locale table

Windows has no ICU, so each locale carries its own table: the month names, the
weekday names, and the pattern each date and time style uses. The table covers
the locales upstream's own tests pin — `en_US_POSIX` and `fr_FR` — and any
locale it does not know falls back to English rather than to nothing.

This is the same decision the search slice made for transliteration: a small
table per script beats a framework that is not there.

## What is not here

- **The template engine** — the placeholders (`{clipboard}`, `{date}`,
  `{argument name="…"}`), the modifiers and the offsets. This is the largest
  part of the snippet feature and it is not written yet.
- **The repository** — the part that reads and writes the files, keeps a slug
  unique (`same.md`, then `same-2.md`) and refuses a stale write when the file
  changed underneath it. The fingerprint that refuses it *is* ported
  (`SnippetSourceRevision`).
- **The keyword policy**, the **store**, the **Raycast import**, the **editor**
  and the **browser**.

## Where this came from

The three modules were written by the background worker that was aborted; they
were never wired into the crate, and they had no tests. What is here now is
that code with the wiring, the two compile errors and the two clippy errors
fixed, and 26 tests written from upstream's own expectations.
