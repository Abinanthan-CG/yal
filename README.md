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
| **Snippets** | Saved text: summon the palette, type the keyword, Enter — the expansion is typed into the application you came from, with the clipboard as the fallback. |
| **Quicklinks, bookmarks** | Saved links, searchable, with `{query}` templates. Shut with the bundled search templates Google, YouTube, GitHub and friends. |
| **Files** | Search your documents, desktop and downloads; pictures get a preview and their photo metadata (EXIF, JPEG). |
| **Windows** | Every open window, by title - then close, minimise, maximise or restore it, or place it: halves, quarters, thirds, centre or the full screen. |
| **Script commands** | The little programs you keep in `~\\.legacy-store\\scripts`, with their own `@title`, `@mode` and `@keywords` header: run detached, or in the background with the outcome toasted. |
| **Browser tabs** | The tabs each running browser holds, searchable. |
| **Kill process** | The running processes, heaviest first; end one from the list. |
| **System commands** | Lock, sleep, shutdown, restart, log out, empty the recycle bin. |
| **Volume and mute** | Master volume read and set through the audio endpoint. |
| **Display** | Brightness where the monitor can report it, light or dark theme everywhere. |
| **Timer** | Countdowns that ring with a toast, with a palette banner as the guaranteed fallback. |
| **Reminders** | `remind me to check the oven in 40m` in the palette, `rc reminder` and `rc reminders` from the command line; both share one parser, and both raise the same toast. |
| **Text transforms** | Case, camel/snake/kebab, base64, percent, hexadecimal, JSON, Morse; digests SHA-256, SHA-1, MD5; UUIDs; placeholder text. |
| **QR codes** | Generated from any text and shown in the palette. |
| **One-time passwords** | RFC 6238 codes with a live countdown. |
| **Regular expressions** | Matched with capture groups. |
| **Emoji** | 363 entries, searchable by name and by meaning. |
| **Colours** | Named, hexadecimal, RGB and HSL, both ways. |
| **Passwords** | Generated to length and character-class rules. |
| **Time zones** | Every IANA zone with its current offset. |
| **Screenshots** | Capture the screen. |
| **Web search** | Hands the query to the search engine. |
| **System tray** | The menu bar extra, in its Windows form. |

The hotkey is **Ctrl+Space** by default and can be changed from the palette
itself — the gear button beside the query opens the preferences pane, where
the hotkey can be recorded, the theme picked (system, dark or light, kept
between runs), and the script commands' folder pointed somewhere else.
The CLI reaches the hotkey as well:

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
cargo run --bin rc -- window <handle> left-half
cargo run --bin rc -- alias set pwsh "Windows PowerShell"
cargo run --bin rc -- identify
cargo run --bin rc -- tabs grok
cargo run --bin rc -- processes
cargo run --bin rc -- audio get
cargo run --bin rc -- display dark
cargo run --bin rc -- transform upper "hello"
cargo run --bin rc -- transform snake "Clean Room Rebuild"
cargo run --bin rc -- hash abc
cargo run --bin rc -- uuid
cargo run --bin rc -- timer 10m pomodoro
cargo run --bin rc -- reminder "call the dentist" "in 20m"
cargo run --bin rc -- reminders
cargo run --bin rc -- reminder-done 1792851433728
cargo run --bin rc -- scripts
cargo run --bin rc -- script-run hello-clean
cargo run --bin rc -- script-run "Clean-room batch probe" --detached
cargo run --bin rc -- exif "C:\path\to\photo.jpg"
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

[docs/parity.md](docs/parity.md) is the full-feature recount behind this
section: every area of the observed launcher feature surface, what the build
carries, and the missing slice, with the next waves ordered by that
evidence.

- **Extension runtime.** Deliberately deferred. launcher extensions are
  TypeScript running against `@launcher/api`; a Rust rebuild needs a different
  extension model, and the domain reserves a slot for it but nothing ships.
- **AI and cloud.** Excluded at the requester's instruction.
- **An arbitrary window rectangle is CLI-only.** The palette's window menu
  carries the placement presets — halves, quarters, thirds, centre, full —
  and giving a window exact coordinates stays with
  `rc window <handle>` on the command line, where the numbers make sense.
- **Currency rates are static.** The bundled rates are indicative reference
  values with a recorded vintage, not live quotes. The launcher is local
  only and does not fetch anything.
- **Timer toasts deliver a banner after identity registration.** `rc identify`
  writes a Start Menu shortcut carrying the launcher's user model identifier,
  which is what makes Windows raise a banner for an unpackaged program.
  Verified on the machine this was built on: the platform log records the
  delivery and the banner was seen on screen. A timer that finishes always
  raises the palette itself with a banner, which cannot fail; the toast is
  an addition, not the guarantee.
- **Notes, calendar and book links** — the parts of launcher that are really
  small apps rather than launcher features — are not built.
- **Reminders live in the local store, not in a cloud account.** launcher's
  reminders are backed by Apple Reminders, which does not exist on Windows;
  this launchers' reminders stand alone: stored locally, raised by a toast
  with the palette banner as the guaranteed fallback, shown soonest first
  with `rc reminders`. The wording (`in 40m`, `tomorrow at 9:30`, `12am`,
  am/pm) is parsed by one documented function the palette and the CLI both
  use, and spans are capped at ninety days.
- **The image preview covers pictures and their photo metadata (`rc exif`)**,
  read from JPEG files. Other formats rarely carry EXIF on Windows, and the
  files on the machine this was built on carry only the date and pixel
  size, so camera fields — ISO, exposure, aperture — read through the same
  code path but were not seen on a real camera file here.
- **Script command output in "full" mode** lands on the clipboard with a
  toast naming the outcome; launcher's launcher shows the full output inside
  its window. The script itself, its header language and the two other
  modes work the same way.
- **The preferences pane is click-verified only through its parts**: gear,
  recording, theme and folder saving each exercise stores and listeners
  that were verified live, but the pane's own drawing was not clicked.
- **Snippet expansion in place is verified in one live manual test.** A
  snippet result hides the palette, returns the focus to the window the
  user came from, and types the expansion through Unicode key injection —
  confirmed end to end once on the build machine (the text appeared in
  Notepad). The injection is only sent after that window is confirmed to
  hold the focus; if the focus cannot be verified or the system refuses
  the events, the text lands on the clipboard with a toast instead.
  Delivery inside every kind of application — elevated windows, terminals
  with custom input handling, remote sessions — is plausible but not
  exhaustively verified.

### Things worth knowing

The hotkey registers successfully and the palette takes the keyboard focus
when it appears — both verified on the build machine, and the press itself
has been verified live since this paragraph last claimed otherwise. The
press is detected two ways at once: the window manager's `WM_HOTKEY`
notification is the primary source, and a direct read of the keyboard state
backs it up — a state poll alone loses quick press-and-release cycles, which
was observed with synthetic taps. `rc hotkey-wait --control` reports what
your combination produces. **Verified live**: Ctrl+Space hides the palette
and brings it back above every other window. The hide parks the window
off-screen instead of hiding it, because a hidden window stops painting and
then nothing — not even the polling that waits for the hotkey — ever wakes
it again; that was observed live and is the reason the palette never
vanishes from the window manager's view.

**Verified live on the build machine**: volume read, set and mute through the
audio endpoint; the theme round-trip through the registry and its broadcast;
tab titles taken from running browser windows; ending a spawned process; the
text transformations and the digests against published test vectors. **Never
run**: shutdown, restart, log out, sleep, lock and emptying the recycle bin —
they are implemented exactly as the system documents them, and each acts only
when someone explicitly asks.

Timers and reminders are read by a watcher thread with its own database
connection, not from the palette's frame loop — an idle palette only runs an
update when something wakes its window, and an overdue reminder that sat
unnoticed for half a minute past its time was observed live because of that.
The watcher was verified with a timed probe: a reminder due ten seconds out
left the store on the dot while the palette sat idle. Its toast delivery goes
the same identity-registered route as a timer's, and the banner fallback
applies to it identically; the toast itself was not seen for a reminder yet.

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
