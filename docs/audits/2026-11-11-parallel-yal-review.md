# The other YAL: what the parallel build is, and what of it we may take

**Date:** 2026-11-11
**Subject:** `C:\Users\dabin\development\yal` — the community YAL project
(`https://github.com/Abinanthan-CG/yal`), GPL-3.0-only, the same product built a
different way and left in a corner.
**Question asked:** does anything in it shorten this build?

**Verdict: yes, in four places, and the saving is real but it is not a shortcut around
the work.** The project is a *finished, tested launcher of its own*, not a source of
upstream-Tinycast parity rows. What it gives us is proven machinery for the two or
three hardest parts of the YAL scope. Everything else in it is either a different
product's extra features (which have no row in our ledger) or an egui UI we cannot
use. Realistic saving: **18–28 hours of the ~330 hours the agreed scope needs — about
5–8%**, concentrated in File Search, Window Management and the calculator's currency
and date layers.

This is a different verdict from the earlier audit of `C:\Users\dabin\yal-audit\yal-0.1.0`
(see `2026-11-11-other-port-review.md`), which was a broken scaffold. This project
compiles, passes its own tests, and has a measured file index. It is worth reading.

## What was actually verified

Run on this machine, 2026-11-11, against the working tree as it stands:

| Check | Command | Result |
| --- | --- | --- |
| Compiles | `cargo check --workspace --all-targets` | exit 0, "Finished `dev` profile … in 53.57s" |
| Tests pass | `cargo test --workspace` | exit 0 — **291 passed, 0 failed, 0 ignored** across 9 crates |
| Size | file census | 89 Rust files, 1 301 117 bytes, 316 `#[test]`/`#[tokio::test]` attributes |
| History | `git log --oneline` | 15+ feature commits, newest "docs: record the launch-pipeline wave in the parity ledger and README" |
| Licence | `Cargo.toml` `license = "GPL-3.0-only"` | GPL-3.0-only, `repository = "https://github.com/Abinanthan-CG/yal"` |

Per-crate test counts from that run: `yal_calc` 55, `yal_data` 68, `yal_engine` 38,
`yal_index` 20, `yal_search` 26, `yal_store` 15, `yal_system` 57, `yal_ui` 10,
`yal_core` 2.

**Its own words about its limits** (`docs/parity.md`) are worth quoting, because they
are the same lesson our build has already learned: the preferences pane and the
secondary-action panel "have not yet been driven by a live hand", the drag overlay
"awaits a live hand like the other overlays". A green test suite there means the model
layer is real; it does not mean the screens are.

## What it is, architecturally

A pure-Rust launcher: `egui`/`eframe` for the window, `windows` crate for the platform,
`rusqlite` for storage, `chrono`/`chrono-tz` for dates, a `yal-engine` that ranks
candidates from many `yal-source`s behind one `Item` model.

**Its UI layer cannot be used by us.** YAL is Tauri 2 + WebView2 with a TypeScript/Solid
frontend, by decision and by brief. `crates/yal-ui/src/lib.rs` (149 549 bytes) and
`crates/yal/src/main.rs` (63 069 bytes) are egui drawing code — about 213 KB of it, the
single largest thing in the project. Porting a palette is work; porting an egui palette
into Solid is work plus a rewrite, so none of it moves.

**Its product is not ours.** It was built as a clean-room rebuild of the *observed*
launcher with extras drawn in from Vicinae. That means it carries features upstream
Tinycast does not have, and they have no rows in our ledger:

| Their feature | Our ledger |
| --- | --- |
| QR generation | no row |
| Password generator, TOTP | no row |
| Regular expressions, colour picker/converter | no row |
| Screenshots (region/window), eyedropper | no row |
| Font browser | no row |
| Timers, reminders, bookmarks | no row |
| Script runner | no row |

Copying these would be scope creep, and our scope was decided page by page on
2026-11-11. We take none of them.

## The four things worth taking

### 1. The incremental file index — the biggest win (File Search, 68 open rows)

`crates/yal-index/src/walk.rs` is a whole-tree walker whose design we want and whose
bugs have already been paid for:

- **Freshness by nanosecond directory mtime.** NTFS ticks at 100 ns; the donor's own
  history records that a millisecond floor "buried same-millisecond writes in the first
  test build". A pass stats every recorded directory and re-lists only the ones whose
  mtime moved.
- **Reparse points and junctions are treated as absent**, so the walk cannot loop.
- **Deletion is exact, per directory** — no `LIKE`-prefix sweeps, which get an
  underscore in a path wrong.
- **The mtime is recorded *after* the listing**, so a change landing between the stat
  and the read is caught on the next pass instead of being buried.
- **Measured at scale**: a cold pass over 16 674 directories and 159 524 files in 52 s,
  a warm pass in 0.78–0.88 s, with a creation and a removal both proven live.

Their **scope model is different from upstream's** and upstream wins: they index the
machine's fixed drives, upstream indexes the folders the user picks (default: the home
folder) with editable ignore patterns, and an empty scope list indexes nothing at all.
We keep the machinery and replace the policy.

*Saving: roughly 8–12 hours.* It is the part of File Search that is genuinely hard to
get right, and it is the part that would have taken us the longest to debug on a real
disk.

### 2. The window placement geometry (Window Management, 78 open rows)

`crates/yal-system/src/window_controls.rs` splits the geometry into one pure, tested
function (`placed_rect`) and keeps the Win32 calls beside it. The tests are the good
part: `halves_and_thirds_tile_the_screen_without_gaps`, `quarters_split_both_axes`,
`centring_keeps_the_window_whole_and_on_screen`, `odd_widths_leave_the_remainder_on_the_far_edge`.

**But upstream's window management is much larger than this.** Upstream ships 39
commands: halves, quarters, *fourths* (First Three Fourths, Last Three Fourths), thirds
(first/centre/last, and First Two Thirds, Last Two Thirds), seven sizing commands
(Maximize, Almost Maximize, Reasonable Size at 60 % capped at 1025×900, Maximize Height,
Maximize Width, Center, Center Half, Center Two Thirds, Make Larger/Make Smaller at 5 %
of the *screen*, Restore Window), eight nudges, four edge moves, display moves, fullscreen,
and two Space switches. It also has a gap model (edges next to the screen border get the
full gap, edges between two windows get half), two cycling modes, a restore point that
remembers up to 64 windows, and custom sizes on a 3×3 grid.

The donor has halves, quarters, thirds, centre, full, focus/close/minimise/maximise.
That is a genuine head start on roughly 18–22 of the 78 rows and a clean way to write
the geometry — the gap rules, the cycling, the restore point and the custom-size grid
remain ours to build.

*Saving: roughly 3–5 hours.*

### 3. The currency registry with a vintage (Calculator, 296 open rows, ~98 of them about money, dates and statistics)

`crates/yal-calc/src/currency.rs` has the shape upstream requires: a bundled snapshot
that answers offline, a fetched table that overrides and extends the bundle, a vintage
stamped on every row and shown with the answer, and a base currency. The upstream spec
is bigger (159 currencies, crypto tickers, ambiguous currency words, a refresh button,
"the last saved rates keep working" offline), but the *mechanism* — registry, vintage,
bundled-vs-fetched, base currency — is exactly the part that is easy to get subtly
wrong and expensive to rediscover.

The rates fetch is legitimate for us: upstream fetches rates and crypto prices too, and
"network only where upstream makes it" allows this.

*Saving: roughly 4–6 hours.*

### 4. The natural-language phrase layer (Calculator)

`crates/yal-calc/src/nl.rs` is a pre-parser that turns a handful of English shapes
("10 days from now", "in 2 weeks", "90 minutes ago", "tomorrow", "next friday",
"20 percent of 250") into ordinary expression trees the one evaluator already
understands, falling through to the plain grammar when a shape does not match. The
*architecture* is upstream's shape — upstream's calculator answers "3 days from next
monday at 7:30", "end of month", "days until 29.10", "weekday on 1 january 2030",
"workdays between july 27 and august 7" — so a pre-parser in front of one evaluator is
the right skeleton, and the donor has a working one with calendar month/year arithmetic
that is day-clamped on the local zone and a `next`/`last` weekday that never answers for
today.

Their phrase grammar is adapted from Vicinae's Numen backend, which is BSD-3-Clause;
that notice travels with the file (see below).

*Saving: roughly 3–5 hours*, and it removes the guesswork about where the phrase layer
belongs.

## Two lessons worth keeping even where no code moves

1. **The empty-`IShellLink` bug.** Their launcher "resolved" every application to its
   own `.lnk` file, so no `exe` ever reached the icon miner or the launcher, because an
   unloaded `IShellLink` answers `GetPath` with the shortcut's own path. We launch
   shortcuts as Windows launches them (`ShellExecuteW` on the `.lnk`), so we are not
   affected — checked, not assumed: `crates/tc-platform-win/src/programs.rs` never
   resolves a link. The lesson stands for any future "resolve the target for the icon"
   work.
2. **A hidden window stops painting, and with it the event loop.** Their palette parks
   off-screen rather than hiding, because a hidden Windows window stops painting and
   the loop where the global hotkey is polled goes quiet with it. They also had to claim
   the tool-window style twice (at birth and every frame) to keep the taskbar button
   away. Our Tauri shell should be checked against the same two facts before the
   tray/park behaviour is called done.

## What we are not taking, and why

| From the donor | Verdict |
| --- | --- |
| The egui UI (213 KB) | Different toolkit; we are Tauri + Solid. Nothing moves. |
| QR, TOTP, password, regex, colour, screenshot, fonts, timers, reminders, scripts, bookmarks | Not upstream features; no ledger rows; out of scope by the 2026-11-11 decision. |
| Their emoji catalogue (1 906 entries, generated from Unicode) | Upstream's own catalogue is 2 136 entries and ours is already built from upstream. Ours stays. |
| Their clipboard | Text-only; ours is upstream-1:1 with 94 tests. Ours stays. |
| Their search and ranking (26 tests) | Weaker than our `tc-search` (150 tests). Ours stays. |
| Their store schema | Their own product's tables; we mirror upstream's `clipboard.sqlite3`, `quicklinks.sqlite3`, file index. We take the index *design*, not the schema. |
| Their window-management menu set | Covers about a quarter of upstream's 39 commands. Geometry only. |
| Their file index scope policy (fixed drives) | Upstream's scopes-and-ignore-patterns policy wins. |

## The licence question, answered

The donor is **GPL-3.0-only**. This repository is **AGPL-3.0-only** and must stay so.
AGPL-3.0 is GPL-3.0 plus section 13, so GPL-3.0-only code may be combined into an
AGPL-3.0 work; the combined work is then AGPL-3.0, which is what this repository
already is. The conditions on the donor's code are that its notices travel with it —
so:

- every adapted module states its origin in its module doc-comment;
- `crates/tc-files/ATTRIBUTION.md` carries the GPL-3.0 notice and names the donor;
- the BSD-3-Clause notice for Vicinae's Numen travels with any adapted phrase
  grammar, and this repository's own upstream notice for Tinycast
  (`github.com/abue-ammar/tinycast`, AGPL-3.0-only) is unaffected and still stands.

The donor's own upstream notices are its business; we take the code as published under
GPL-3.0 and carry that licence, not its other claims.

## How this changes the plan

Nothing in the scope changes. The order stays: M1 the daily driver, M2 the memory
features, M3 the long tail. The File Search backend moves earlier than planned and is
built on the donor's walker rather than from scratch; the window geometry and the
currency registry are taken when their pages come up, not before — a row is `PORTED`
only when a test that ran proves it, and a copied algorithm still needs its own tests
against upstream's behaviour.
