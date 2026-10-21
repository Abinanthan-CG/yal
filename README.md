# legacy-store

A keyboard launcher for Windows, written in Rust, rebuilt from observed
behaviour rather than from any source code.

This is a clean-room reimplementation. It was written by observing a running
copy of launcher for Windows — its process topology, its data model, its
extension surface — and rebuilding the product behaviour from those
observations. No launcher source was read, decompiled or copied. The
provenance record is in [`docs/observations.md`](docs/observations.md).

---

## What it does

Press the palette, type a few characters, and get the thing you meant.

- **Application launcher** — searches Start Menu shortcuts, Store packages,
  installed programs and everything on `PATH`.
- **Fuzzy search** — tiered matching that understands word boundaries,
  camelCase acronyms and scattered subsequences.
- **Calculator** — arithmetic, units, currencies and dates, in the same query
  box as everything else.
- **Snippets, quicklinks, bookmarks** — saved text and saved links.
- **Clipboard history, emoji, colours, passwords, time zones** — the small
  utilities that make a launcher worth keeping open.

---

## Build and run

```sh
cargo build --release

# The palette window.
cargo run --bin launcher

# The headless front end, which is also how the behaviour is tested.
cargo run --bin rc -- search "launcher"
cargo run --bin rc -- calc "100 km in miles"
cargo run --bin rc -- apps --filter visual
cargo run --bin rc -- doctor
```

```sh
cargo test --workspace
```

---

## Layout

The crates are layered so that the domain stays free of the operating system
and of the user interface.

```
yal-core      domain model: commands, items, actions, ranking
yal-search    fuzzy matching, ranking, frecency
yal-calc      expression engine: units, currency, dates
yal-data      static datasets: emoji, colours, time zones, passwords
yal-index     host discovery: applications and files
yal-store     local persistence: SQLite
yal-system    host integration: hotkey, clipboard, windows, capture
yal-engine    application layer: assembles commands into ranked results
yal-ui        the egui palette
yal-cli       the `rc` headless front end
```

Dependencies point inward. `yal-core` depends on nothing but `serde` and
`thiserror`; `yal-ui` depends on everything.

See [`docs/architecture.md`](docs/architecture.md) for the reasoning.

---

## How the search works

A query is matched against a candidate in six tiers, and the tier dominates
the score:

| Tier | Example | Meaning |
| --- | --- | --- |
| Exact | `code` → `code` | the whole candidate |
| Prefix | `cod` → `code editor` | the start of the candidate |
| Word boundary | `edit` → `code editor` | the start of a word |
| Substring | `od` → `code editor` | anywhere inside a word |
| Acronym | `vsc` → `visual studio code` | the initials of the words |
| Subsequence | `vde` → `visual studio code` | the characters, in order |

The tier weights are separated by wide gaps, so a stronger tier always
outranks a weaker one no matter how good the weaker match looks. Within a
tier, the score rewards word boundaries, camelCase transitions and
consecutive characters, and penalises gaps, a long candidate and a match that
starts late.

Multi-term queries use AND semantics: every whitespace-separated term must
match, and the combined score is the sum of the per-term scores.

Frecency then scales the score. It is deliberately a modifier rather than a
score of its own: text quality decides which candidates are plausible, and
frecency decides which plausible candidate was meant. A never-used candidate
is neither penalised nor boosted.

---

## How the application index works

| Source | How it is read | Needs elevation |
| --- | --- | --- |
| Start Menu shortcuts | `IShellLink` COM interface | no |
| Store packages | WinRT `PackageManager`, current user | no |
| Installed programs | uninstall registry keys, 32- and 64-bit views | no |
| `PATH` executables | directory listing | no |

Store apps are enumerated through the package manager rather than by listing
`Program Files\WindowsApps`, because that folder is not readable by an
ordinary user but the package manager is. On the machine this was built on,
that one change took the index from 1010 applications to 1087 and made launcher
itself discoverable.

---

## What is not here yet

This is an honest list, not a disclaimer.

- **Extension runtime.** Deliberately deferred. launcher extensions are
  TypeScript running against `@launcher/api`; a Rust rebuild needs a different
  extension model, and the domain reserves a slot for it but nothing ships.
- **AI and cloud.** Excluded at the requester's instruction.
- **The hotkey does not open the palette.** The hotkey registers with the
  window manager and the palette window exists, but nothing yet connects the
  two. Until that is wired, the palette is a window you focus by hand.
- **Clipboard watching.** The store keeps clipboard history and the palette
  reads it, but no watcher is running to record new entries.
- **Snippet expansion.** Snippets appear as results and their expansion is
  copied to the clipboard, but typing a keyword does not expand it in place.
- **Currency rates are static.** The bundled rates are indicative reference
  values with a recorded vintage, not live quotes. The launcher is local
  only and does not fetch anything.
- **File search is implemented but not wired into the palette.** The walker
  exists and is tested; no command exposes it yet.

---

## Provenance

The observations this rebuild is based on are recorded in
[`docs/observations.md`](docs/observations.md), separating what was observed
from what was inferred from it and what remains unknown.

Two boundaries were held deliberately:

1. launcher's local databases are protected by a page cipher. The cipher was
   **not** broken; the rebuild defines its own schema instead.
2. No launcher implementation was decompiled or copied. The rebuild follows
   the observed *contract* — the feature set, the extension surface, the
   data model — and writes its own implementation of it.

REA was used to produce the observations. See the evidence index in the
ledger.
