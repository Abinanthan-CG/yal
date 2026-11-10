# Decisions

Dated entries. One paragraph each: the question, the options, what was chosen, and why.
Larger decisions get their own file in `docs/adr/`.

## 2026-11-10 - Ledger authoring is split into fragments

**Question.** The brief makes `parity/parity.toml` the single source of truth for scope and
status, but the ledger is far too large to write in one pass and it needs to be authored in
parallel.

**Options.** (a) Write `parity/parity.toml` directly. (b) Author per-group fragments under
`parity/parts/` and merge them into `parity/parity.toml`. (c) Generate the ledger from the
documentation pages with a script.

**Decision.** (b). `parity/parts/*.toml` are the authoring units; `cargo xtask parity` merges
them into the committed `parity/parity.toml`, validates it, and renders `PARITY.md` from it.
`cargo xtask parity --check` fails when either committed file is out of date, so CI catches a
stale ledger.

**Why.** (a) makes parallel authorship impossible without write conflicts on one file. (c)
cannot work: the atomic behaviors are only discoverable by reading the pages and the Swift
source, not by parsing them mechanically. The merged `parity/parity.toml` stays the single
source of truth that CI validates against, so the brief's requirement holds.

## 2026-11-10 - Count checks are a separate canonical file with two kinds

**Question.** The brief requires "every count check is satisfied", but upstream's count checks
are of two different shapes: some are literally a list of N items (32 system actions, 46
built-in commands), and some are a property a test must assert (a 250 ms tap window, a
32,000-character limit).

**Options.** (a) Model both as a row count. (b) Model both as free-form evidence on a row.
(c) Model them as two kinds in one canonical file.

**Decision.** (c). `parity/count_checks.toml` holds every check with `kind = "rows"` (the number
of tagged rows must equal `expected_rows`) or `kind = "asserted"` (a test must assert the value
later). Every check must be represented by at least one tagged row, so a check cannot be
declared and then quietly dropped.

**Why.** Treating an asserted limit as a row count would either force fake rows or force the
validator to accept an unchecked number. Two kinds let the validator be strict about the first
shape now and honest about the second shape later, without inventing a status the brief
forbids.

## 2026-11-10 - Upstream's `[[count_check]]` definitions live outside the ledger

**Question.** The brief's ledger schema has `count_check` entries inside `parity.toml`. Where
should they live so parallel authors cannot conflict?

**Options.** (a) Inside `parity/parity.toml`. (b) In `parity/count_checks.toml`, referenced by
id from rows.

**Decision.** (b). Rows carry `count_check = "<id>"`; the definitions live in one canonical
file. A fragment that defines a check which is not in the canonical file is a validation error.

**Why.** The count checks are a property of upstream's documentation, not of any one page
group, so they change as one unit. Splitting them across fragments would make the "46 built-in
commands" check editable from two places at once.

## 2026-11-10 - The Windows translation per page is data, not prose

**Question.** Each docs page needs a Windows translation, and the ledger's `windows` column must
be a per-behavior specialization of it rather than a generic restatement.

**Options.** (a) Keep the translation in prose in the prompt and re-derive it per row. (b) Store
the page-level translation in `parity/pages.toml` next to the page's tier and title.

**Decision.** (b). `parity/pages.toml` holds `page`, `title`, `tier`, `source` and `windows`.
The ledger rows specialize it.

**Why.** The page inventory, the tier and the translation change together and must be validated
as one unit: a row on an unknown page, or a row whose tier disagrees with its page, is an
error the validator can catch. Storing them together is what makes that check possible.

## 2026-11-10 - Deterministic TOML output for the ledger

**Question.** `cargo xtask parity --check` has to detect a stale committed ledger. What
guarantees the regenerated file matches byte for byte?

**Options.** (a) Serialize with `toml_edit`, whose formatting of table arrays is not
guaranteed stable. (b) Write the TOML by hand with an explicit string escaper.

**Decision.** (b). A small `tstr` function escapes TOML basic strings and the writer emits
sections in a fixed order: headings, rows, count checks, additions. `toml_edit`'s array API
does not accept tables in arrays in the form needed, and hand-writing removes the formatting
ambiguity entirely.

**Why.** The check has to be exact, and a ledger that reformats itself on every run produces
noise that hides real staleness.

## 2026-11-10 - Windows-dependent rows are provable here, not only in CI

**Question.** The brief says Windows-dependent rows become `PORTED` only once evidence has run
on real Windows. Is real Windows available?

**Options.** (a) Assume a non-Windows host and defer every Windows-dependent row to CI.
(b) Detect the environment and, when it is real Windows, prove those rows locally.

**Decision.** (b). The environment check in `PROGRESS.md` shows Windows 11 with the MSVC
toolchain, the Windows SDK, WebView2 and NSIS all present, so Windows-dependent rows can be
proven on this machine and CI is a second witness rather than the only one.

**Why.** The brief's `environment_adaptation` section says to find out where you are before
planning. Deferring everything to CI would have been correct but much slower, and would have
meant reporting far fewer rows as `PORTED` than the work actually justified.
