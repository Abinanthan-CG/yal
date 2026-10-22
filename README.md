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

| | |
| --- | --- |
| **Launcher** | Searches Start Menu shortcuts, Store packages, installed programs and everything on `PATH`. |
| **Search** | Tiered fuzzy matching that understands word boundaries, camelCase acronyms and scattered subsequences. |
| **Calculator** | Arithmetic, units, currencies and dates, in the same query box as everything else. |
| **Clipboard** | History, kept by a background watcher. |
| **Snippets** | Saved text, expanded straight from the palette. |
| **Quicklinks, bookmarks** | Saved links, searchable. |
| **Files** | Search your documents, desktop and downloads. |
| **Windows** | Every open window, by title. |
| **QR codes** | Generated from any text and shown in the palette. || **One-time passwords** | RFC 6238 codes with a live countdown. |
| **Regular expressions** | Matched with capture groups. |
| **Emoji** | 363 entries, searchable by name and by meaning. |
| **Colours** | Named, hexadecimal, RGB and HSL, both ways. |
| **Passwords** | Generated to length and character-class rules. |
| **Time zones** | Every IANA zone with its current offset. |
| **Screenshots** | Capture the screen. |
| **Web search** | Hands the query to the search engine. |
| **System tray** | The menu bar extra, in its Windows form. |

The hotkey is **Ctrl+Space** by default and can be changed:

```sh
rc hotkey                      # show the current hotkey
rc hotkey ctrl+alt+k           # change it
rc hotkeys                     # see which combinations are free
```

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
cargo run --bin rc -- totp JBSWY3DPEHPK3PXP
cargo run --bin rc -- regex "(\w+)@(\w+)" "user@example"
cargo run --bin rc -- qr "hello" --output code.png
cargo run --bin rc -- windows
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
- **Snippet expansion replaces text in place.** A snippet whose keyword
  matches the query is offered as a result and copies to the clipboard; it
  does not expand inside another application the way launcher's does.
- **Window management is read-only.** Open windows are listed and searchable.
  Moving, resizing and closing them is not implemented.
- **Currency rates are static.** The bundled rates are indicative reference
  values with a recorded vintage, not live quotes. The launcher is local
  only and does not fetch anything.
- **Reminders, notes, calendar and book links** — the parts of launcher that
  are really small apps rather than launcher features — are not built.
- **The image preview and EXIF reader** are not built.

### One thing worth knowing

The hotkey registers successfully and the palette takes the keyboard focus
when it appears — both were verified by launching the app and asking Windows
which window was in front. **The keypress itself could not be verified end to
end**, because synthetic keyboard injection is blocked in the session this was
built in, so no automated check could produce a real key press. The press is
detected by reading the keyboard state directly rather than by waiting for
the window manager's hotkey notification, which is the more robust of the two
approaches. Press Ctrl+Space and it will either work or it will not — that is
a one-second check, and `rc hotkey-wait` will tell you either way.

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
