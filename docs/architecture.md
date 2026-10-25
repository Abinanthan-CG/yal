# Architecture

## Why these crates

The layers exist because the observed product has three very different kinds
of code in it, and mixing them makes every change harder than it needs to be.

| Layer | Crates | Rule |
| --- | --- | --- |
| Domain | `yal-core` | Knows nothing about the operating system, the database or the screen. |
| Adapters | `yal-index`, `yal-store`, `yal-system`, `yal-calc`, `yal-data` | Know the host. Know nothing about each other. |
| Application | `yal-engine` | Knows the adapters and the domain. Knows nothing about presentation. |
| Presentation | `yal-ui`, `yal` | Know the engine. Know nothing about the host. |

`yal-engine` is the only crate that knows both a store and an index, and
it is the only place where the decision "which commands answer this query"
is made. The palette and the command line ask the engine the same question
and get the same answer, which is why the terminal is a usable test harness
for the window.

### The rule that matters

Nothing below the application layer may import something above it. If
`yal-search` ever needs to read a file, that is a sign the boundary is in
the wrong place — the reading belongs in `yal-index`, and the domain
should receive the values already read.

---

## The domain model

`yal-core` owns four ideas.

**Command** — a searchable source of items, categorised by `CommandKind`. The
kind decides which provider answers the query, how results are grouped, and
what the section header says.

**Item** — one candidate: a title, an optional subtitle, extra search
keywords, accessories, an icon, an optional action, and an opaque payload.
The payload exists so an item can carry structured data to a provider without
the domain knowing that provider's schema.

**Action** — a closed set of intents: open an application, open a URL, open a
path, copy text, paste text, reveal a path, run a program, open a path with a
specific application. Keeping the set closed means the shell decides *how* to
carry an action out without the item needing to know.

**Match** — the result of matching a query against a candidate: a tier, a
score, the positions that matched, and the contiguous ranges for
highlighting.

### The error algebra

Errors are tagged by cause, not by message. `CoreError` distinguishes an
invalid item, an unknown command, an invalid input, no match, an unsupported
feature and an I/O failure, and each carries what a caller needs to decide
whether to retry or to report. A feature that exists in the model but is not
built yet is `Unsupported`, which is deliberately distinct from
`InvalidInput` — so "not built yet" is never reported as "you typed it
wrong".

---

## The search engine

### Tiers

Matching is tiered, and the tier dominates the score. The weights are:

| Tier | Weight |
| --- | --- |
| Exact | 1000 |
| Prefix | 700 |
| Word boundary | 550 |
| Substring | 400 |
| Acronym | 250 |
| Subsequence | 150 |

The gaps are wide enough that a within-tier quality difference can never
overturn a tier difference. This is the property that makes the ranking
predictable: a scattered match over a long string can never outrank a
prefix match over a short one.

### Within-tier quality

Bonuses and penalties are gathered in one `Weights` struct so the behaviour
can be reasoned about as a whole:

| Term | Effect |
| --- | --- |
| Word boundary | `+15` per matched character that begins a word |
| camelCase transition | `+8` per matched camelCase transition |
| Consecutive run | `+12` per adjacent matched pair |
| Gap | `-3` per unmatched character inside the match span |
| Length | `-1.5` per candidate character beyond the query |
| Position | `-1` per character of offset before the first match |

A camelCase transition earns both the boundary and the camel bonus, because
it is a stronger signal than a plain separator.

### Multi-term queries

Every whitespace-separated term must match. The combined kind is the weakest
individual kind — one weak term makes the whole result feel weak — and the
combined score is the sum of the per-term scores.

### Frecency

`FrecencyModel` applies a half-life decay to the time since last use and a
logarithmic weight to the number of uses, so a hundred uses is not a hundred
times the pull of one. The multiplier is capped at 3× and is exactly 1× for
a candidate with no history, which means frecency can never penalise an
unranked candidate against another unranked one.

The application layer reads usage history for every candidate in a single
query, so the number of database round trips does not grow with the number of
results.

---

## The application layer

`yal-engine` gathers candidates from every source, attaches the usage
history, and ranks them together. Snippets, applications and a calculator
result compete on exactly the same terms.

One special case: an **empty query** matches no text, which would leave the
palette blank. The engine therefore ranks an empty query by usage instead,
so the default view is the set of things the user reaches for most. This was
found by running the palette and looking at it, not by reading the code.

---

## The store

One SQLite database holds everything the launcher remembers. The schema is
the rebuild's own; launcher's databases use a page cipher and are not read or
written here.

Migrations are numbered and recorded in `schema_version`, so an existing
database upgrades in place. Each migration runs exactly once.

Usage history is keyed by the item's stable identifier. Identifiers are
derived from the display name *and* the launch path, because two applications
may share a name while living at different locations.

---

## The index

Four sources, each best-effort. A source that cannot be read is skipped
rather than fatal, so a partially readable system still yields results.

The interesting decision is how Store apps are found. `Program Files\
WindowsApps` is not listable by an ordinary user, but the WinRT package
manager is. `PackageManager.FindPackages` enumerates packages for every user
on the machine and therefore needs elevation; `FindPackagesByUserSecurityId`
with an empty identifier means "the current user" and works from an ordinary
process. The index uses the second and falls back to the first.

The executable is read from the package's `AppxManifest.xml`. The attribute is
`Executable="path\to\app.exe"` and may appear several times — once for the
application and once per extension — so the first occurrence is taken.

---

## The palette

`yal-ui` draws one always-on-top window: a query field and a flat list
of results. There are no modes and no pages; everything is a result in the
same list.

Searches are debounced by 35 ms. Searching is fast, but re-running it on
every keystroke still costs more than the frame it would save, and the delay
is short enough to feel immediate.

Each result row has three columns: the title, the subtitle, and the command
section. The columns are given *measured* width budgets — the title is
truncated by binary search against the painter's own measurement — so a long
path can never run over the section label. The matched characters are drawn
in an accent colour using a `LayoutJob`, and the match ranges are clamped to
the truncated text so highlighting can never point past the end of the drawn
title.

---

## Testing

The terminal front end is not a convenience; it is the test harness. Every
behaviour the palette has can be exercised without a window.

The suite covers the layers that carry logic:

- the matcher, its tiers, its bonuses and its Unicode handling
- the frecency decay
- the calculator's grammar, units, currencies and date arithmetic
- the store's migrations, round trips and history trimming
- the index's identifier derivation and manifest parsing
- the datasets' uniqueness and search behaviour
- the palette's text splitting

The host-dependent parts — the clipboard, the window list, the screen
capture, the hotkey — are written so that a busy or headless host produces a
clean absence rather than a failure, and their tests assert only the path
that is actually available.
