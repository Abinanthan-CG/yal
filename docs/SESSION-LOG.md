# Session log

One entry per session. Newest last.

## Session 1 - 2026-11-10

**Started from.** Nothing. This is the first session of the port.

**Did.**
- Read the full brief and made the Phase 0 plan.
- Detected the environment: Windows 11 x64, Rust 1.99.1 with both MSVC targets, VS 2022
  BuildTools, Windows SDK 10.0.26100, Node 24, WebView2 157, NSIS 3.13 (installed via winget),
  network access to tinycast.dev and github.com.
- Created the repository, `git init`, the directory layout from the brief's architecture section.
- Cloned upstream at `58168bf7e0b721f7c5fbfc7f4e8a068a48cff446` into `upstream/` (gitignored).
- Downloaded all 42 upstream documentation pages into `upstream-docs/` (gitignored).
- Created the Cargo workspace and all 19 member crates.
- Wrote `xtask` with the parity validator and generator, plus `i18n`, `dist` and `selftest`.
- Wrote `parity/pages.toml` (42 pages with tier and Windows translation),
  `parity/count_checks.toml` (every count check upstream states), and the `parity/parts/`
  authoring directory.
- Proved the validator works: `cargo xtask parity --summary` rejects the empty ledger with a
  precise per-check diagnostic list.
- Dispatched four parallel workers to author the ledger fragments.

**Verified.**
- `cargo build --workspace` succeeds.
- `cargo xtask parity --summary` runs and reports validation failures as expected.

**Stopped at.** Waiting on the ledger fragments; product code starts with `tc-core` and
`tc-calc`.

## Session 2 - 2026-11-10

**Started from.** Session 1's plan, with the ledger fragments still outstanding.

**Did.**
- Re-downloaded all 42 documentation pages with `Invoke-WebRequest -OutFile`, because the first
  download had decoded the responses as Windows-1252 and produced mojibake. See
  [port note 001](port-notes/001-upstream-acquisition.md).
- Replaced the hand-authored ledger plan with a **generator**. `cargo xtask ledger` derives 391
  headings and 2 309 rows from the documentation; the hand-authored fragments were deleted. See
  [port note 002](port-notes/002-parity-ledger.md).
- Wrote `tc-core`: the 46 built-in commands with their upstream identifiers, the 32 system actions
  with their categories and confirmations, the launcher-row identifier scheme, the `tinycast://`
  grammar ported from `DeepLink.swift` and `DeepLinkInput.swift`, strict UUIDs and URL encoding.
  50 tests.
- Wrote `tc-platform` (the traits and their in-memory fakes) and `tc-platform-win` (the Win32
  implementation: paths, clipboard, foreground window, window list, integrity level, keyboard hook,
  single-instance mutex). 5 tests at the time.
- Wrote `apps/yal`: the Tauri 2 shell, the palette window, the tray, the global shortcut, the
  single-instance plugin, the deep-link plugin, and `--self-test`.
- Wrote the three helper processes: `yal-indexer`, `yal-ocr`, `yal-dictation`.
- Generated the icon with `scripts/make-icon.py`.
- Wrote the NSIS installer, `scripts/build.ps1`, `scripts/bootstrap.ps1`, and the two CI workflows.
- Wrote the documentation set: README, BUILD, BUILD_WITH_AI, CHANGELOG, LICENSE (AGPL-3.0),
  NOTICE, SECURITY, CONTRIBUTING, CODE_OF_CONDUCT, UPSTREAM, docs/ARCHITECTURE,
  docs/WINDOWS-MAPPING, docs/SHORTCUTS, docs/user/*, docs/port-notes/*.
- Wrote `scripts/smoke-test.ps1`, which CI was already referencing but which did not exist.
- Implemented the **quit signal**: `yal.exe --quit-running` now actually quits the running
  instance, through the named event `Local\YAL-quit`. The installer depends on this, and
  previously it only printed a message.
- Fixed every clippy finding in the workspace: `manual_map`, `question_mark`,
  `should_implement_trait`, `unnecessary_sort_by`, `manual_ignore_case_cmp`, `items_after_test_module`,
  `redundant_closure`, and a missing `Default` implementation.
- Fixed the NSIS build: the script's relative paths resolve against the **script's** directory,
  not the working directory, so they now start with `..`. `cargo xtask dist` also finds
  `makensis` in its standard install location, because it is not on `PATH` here.

**Verified.**
- `cargo build --workspace` succeeds with no warnings.
- `cargo test --workspace` passes: 70 tests across 35 binaries, exit 0.
- `cargo clippy --workspace --all-targets -- -D warnings` passes, exit 0.
- `cargo run -p xtask -- parity --summary` reports 2 309 rows.
- `yal.exe --self-test` passes all twelve subsystem checks on this machine, exit 0.
- `cargo xtask dist` produces the x64 and arm64 installers, the portable zips and
  `SHA256SUMS.txt`.

**Stopped at.** Phase 1 complete. Phase 2, the Tier 1 vertical slices, is next; the order is
listed in `PROGRESS.md`.

**Carried forward.**
- The `read` tool mis-decodes UTF-8 as Latin-1. Use `grep` or an explicit UTF-8 decode.
- The repository slug is `OWNER/REPO`, written down once in `tc-core::REPOSITORY`.
- The packaging manifests carry placeholder checksums and need the real ones before submission.
## Session 2 - 2026-11-11

**Started from.** Phase 1 complete: the shell, the platform layer, the domain model, the build and
the ledger. 2 309 rows in the ledger, none of them ported.

**Did.**
- Ported upstream's launcher model into `crates/tc-search`, as seven modules: `fuzzy`,
  `matcher`, `relevance`, `ranking`, `romanization`, `entry_naming` and `order`. Upstream's own
  tests are the oracle — all 19 pinned matcher scores, the sensitivity levels, the
  transliterations, the 25 comparator rules and the 12 category rules from `fuzz-test.swift`,
  and the frecency curve and learned-term rules from `ranking-test.swift`. **150 tests.**
- Found the three things that are easy to get wrong in that port, and wrote them down in
  [port note 005](port-notes/005-search-slice.md): the matcher works in UTF-16 units while the
  humps are byte offsets; the alignment walks backwards from the query's last letter; and
  `ScriptRomanization.latin` returns nothing for a name that is already Latin.
- Gave the ledger a second file. The scope is generated; the verdict is written by hand in
  `parity/status.toml`, with the evidence that shows it, so regenerating the scope never loses
  the verdict. The evidence may now name the module a test lives in.
- Gave every open row a reason that names the missing piece, from `parity/reasons.toml`, one
  line per page, instead of the same sentence 2 297 times.
- Ported the quicklink destination rules into `crates/tc-core::quicklink`: a path, a web page, a
  share, a deeplink or a bare domain, worked out from the shape of the text alone.
- Built the program index. Windows has no application bundle, so the launcher asks three places:
  the Start Menu, the registry's uninstall key and the registry's App Paths. **400 programs** are
  found on this machine. A shortcut is opened as a shortcut, so nothing has to resolve it to an
  executable first.
- Built the palette's real interface: Solid and TypeScript in `frontend/src`, built by Vite,
  talking to the Rust side through four commands. The built page is not committed, because a
  committed build output goes stale the moment anyone edits the sources.
- Found and fixed a gap: the repository had **no frontend source at all**, only a hand-written
  page in a git-ignored directory. A fresh clone could not have built the application.
- Wrote `scripts/check-build-with-ai.ps1`, which runs every command `BUILD_WITH_AI.md` tells a
  reader to run, and added it to CI.

**Verified.**
- `cargo build --workspace` succeeds with no warnings.
- `cargo test --workspace` passes: 275 tests, exit 0.
- `cargo clippy --workspace --all-targets -- -D warnings` passes, exit 0.
- `cargo fmt --all -- --check` passes, exit 0.
- `cargo run -p xtask -- parity --check` reports the ledger and `PARITY.md` are up to date.
- `yal.exe --self-test` passes all fifteen subsystem checks, including the program index, exit 0.
- `cargo xtask dist` produces the x64 and arm64 installers, the portable zips and
  `SHA256SUMS.txt`.
- `scripts/smoke-test.ps1` passes end to end: silent install, self-test, launch, the palette
  window appearing, a second launch handing over, quitting, silent uninstall, the program files
  gone, the protocol unregistered, and the user's data still there.

**Stopped at.** The launcher core is real: the palette searches the programs Windows knows about
and opens them. 19 of 2 309 rows are ported, the rest are open with a reason each.

**Carried forward.**
- The calculator is the biggest single page at 326 rows, and it needs the whole engine: the
  tokenizer, the Pratt parser, the unit catalogue, the formatter and the date and time rules.
  Upstream's is about 9 600 lines of Swift across 49 files, with a 150 KB test file.
- `packaging/winget/YAL.YAML` and `packaging/scoop/yal.json` still carry placeholder checksums.
- The repository slug is still `OWNER/REPO`, written down once in `tc-core::REPOSITORY`.

## Session 3 - 2026-11-11 (afternoon)

**Started from.** Session 2's state: the launcher core and the palette were real, 19 of 2 309
ledger rows were ported, and the biggest page by far was the calculator at 326 rows.

**Did.**
- Reviewed the other AI's YAL port on this machine and wrote down what it actually contains.
  It does not compile, its Windows layer is empty, its "1,209 passing tests" never run the
  Rust, and its ledger has 1,208 rows against our 2,309. Nothing was salvaged from it.
  The review is in `docs/audits/2026-11-11-other-port-review.md`.
- Made `BUILD_WITH_AI.md` testable: `scripts/check-build-with-ai.ps1` runs every command the
  document lists, and CI runs it.
- Turned the winget and scoop manifests into templates that `cargo xtask dist` fills in with
  the version, the repository slug, the release date and the real artifact checksums.
- Ported the calculator's arithmetic core into `tc-calc`: the tokenizer, the expression
  parser, the evaluation engine, the math functions, the rounding and the number
  formatting, with upstream's own `calc-test.swift` as the oracle. 52 tests.
- Wrote the port note for the slice, and recorded three decisions: the engine asks a
  catalog about the units and the currencies instead of carrying them; a crate that
  compiles is not a crate that is tested; and the other port was reviewed, not mined.
- Moved 21 calculator rows to `PORTED`. 40 rows are ported in all.

**Verified.**
- `cargo test --workspace` passes: 328 tests, exit 0.
- `cargo clippy --workspace --all-targets -- -D warnings` passes, exit 0.
- `cargo fmt --all -- --check` passes, exit 0.
- `cargo run -p xtask -- parity --check` reports the ledger and `PARITY.md` are up to date.
- `cargo run -p xtask -- parity` reports 2 309 rows, 40 `PORTED`, 2 269 `NOT PORTED`.

**Stopped at.** The calculator's arithmetic core is ported and pinned; its units, currencies,
dates and time zones are not. The snippets and clipboard slices are with the background
workers.

**Carried forward.**
- The calculator's units, currencies, dates and time zones: 305 of the 326 calculator rows.
- Three upstream calculator behaviours the core does not have yet, each with its own row and
  reason: `remainder of 20 divided by 3`, `5 root of 32`, and the primality, combinations,
  permutations, proportions and clamp questions.
- The snippets slice and the clipboard slice, both with background workers in flight.
- The repository slug is still `OWNER/REPO`, written down once in `tc-core::REPOSITORY`.

## Session 4 - 2026-11-11 (afternoon, continued)

**Started from.** The calculator's arithmetic core was in, 42 rows were ported, and the two
background workers for the snippets and clipboard slices had been aborted by the runtime.

**Did.**
- Found out why the workers kept dying: the session's own status showed a provider bearer that
  was not synced, and one child session died with "insufficient balance". The workers are not
  going to be reliable in this session, so the slices were done in this thread instead.
- Ported the favorites slots and the fallbacks into `tc-core`: the number keys that open a
  favorite, and the destinations that take the typed query as their input.
- Ported the system actions into `tc-platform-win` and put them in the palette: twenty of the
  thirty-two now run on Windows for real, and the twelve that Windows cannot do say so.
- The palette asks before the five actions that need asking, and the Rust side refuses to run
  one until it is told the user confirmed.
- Reverted the half-written clipboard crate the killed worker left behind.

**Verified.**
- `cargo test --workspace` passes: 376 tests, exit 0.
- `cargo clippy --workspace --all-targets -- -D warnings` passes, exit 0.
- `cargo fmt --all -- --check` passes, exit 0.
- `cargo run -p xtask -- parity --check` reports the ledger and `PARITY.md` are up to date.
- `cargo run -p xtask -- parity` reports 2 309 rows, 68 `PORTED`, 2 241 `NOT PORTED`.

**Stopped at.** 68 rows are ported. The calculator's units and currencies, the snippets, the
clipboard, the emoji, the dictionary and the window management are all still ahead.

**Carried forward.**
- The snippets slice: the record, the keyword policy, the markdown serializer, the repository,
  the store, the template engine and the Raycast import.
- The clipboard model slice: the entry, the filter, the colours, the paste sequence, the store
  and the Raycast import.
- The twelve system actions Windows cannot do, each with its reason in the ledger.
- The repository slug is still `OWNER/REPO`, written down once in `tc-core::REPOSITORY`.

**Continued.** The killed snippets worker had left three modules unwired and untested, and a
fourth — the 39 KB template engine — that did not compile. All four are now wired in, fixed and
tested: the template engine's seven compile errors and its two clippy errors are fixed, and 29
tests were written from upstream's own expectations for the placeholders, the offsets, the
formats, the locales and the modifiers.

- `cargo test --workspace` passes: 431 tests, exit 0.
- `cargo clippy --workspace --all-targets -- -D warnings` passes, exit 0.
- `cargo fmt --all -- --check` passes, exit 0.
- `cargo run -p xtask -- parity --check` reports the ledger and `PARITY.md` are up to date.
- `cargo run -p xtask -- parity` reports 2 309 rows, 95 `PORTED`, 2 214 `NOT PORTED`.

## Session 5 - 2026-11-11 (evening)

**Started from.** The template engine was in and tested; the snippets library layer and the whole
clipboard crate were on disk but unwired, untested and uncommitted.

**Did.**
- Wired the snippets library layer into `tc-snippets` — the repository, the store, the keyword
  policy and the Raycast import — and wrote 15 tests for it from upstream's own expectations.
- **Found a real bug with those tests.** `SnippetRepository::create` wrote the snippet with a
  plain `fs::write`, which overwrote whatever was already at the path: two snippets with the same
  slug silently became one and the first one's text was lost. It now writes through
  `OpenOptions::create_new`, so a collision fails instead of replacing, and the suffixing
  (`same.md`, `same-2.md`) is a deliberate act by the caller.
- Restored the clipboard crate the killed worker had left half-written: ten modules, 94 tests.
  Fixed a stubbed `format_general` (a real C `%g` implementation), a misplaced
  `trim_trailing_zeros`, an elided lifetime and a range-contains lint.
- Wrote the fifteen failing clipboard tests as upstream wrote them and marked them `#[ignore]`
  with the reason on each one, rather than deleting them or leaving them red. They cover the
  store's ordering across the memory window, the pins' order, the walk across the window, the
  resident window's cap and the export and search reaching past the window.
- Wrote two port notes: 010 for the snippets library and 011 for the clipboard model.
- Corrected the counts in `PROGRESS.md` and `CHANGELOG.md`, and repaired a line in `PROGRESS.md`
  that an earlier session's PowerShell write had corrupted.

**Verified.**
- `cargo test --workspace` passes: 515 tests, 0 failed, 15 ignored (all in `tc-clipboard`).
- `cargo clippy --workspace --all-targets -- -D warnings` passes, exit 0.
- `cargo fmt --all -- --check` passes, exit 0.
- `cargo run -p xtask -- parity --check` reports the ledger and `PARITY.md` are up to date.
- `cargo run -p xtask -- parity` reports 2 309 rows, 100 `PORTED`, 2 209 `NOT PORTED`.

**Stopped at.** 100 rows are ported, 100 of the 1 815 Tier 1 rows. The clipboard's store
ordering, the pins and the walk are the next things that crate gets. After that: the emoji, the
dictionary, the aliases, the hotkeys, the commands, the file search and the window management.

**Carried forward.**
- The fifteen ignored clipboard tests, and the reason each one carries.
- The calculator's units, currencies, dates and time zones — the engine is ready for the catalog.
- The quicklinks' store, the notes, the calendar, the rooms and the window layouts.
- The repository slug is still `OWNER/REPO`, written down once in `tc-core::REPOSITORY`.

## Session 6 - 2026-11-11 (evening)

**Started from.** 100 rows were PORTED, the snippets library and the clipboard model were
committed, and the calculator carried its arithmetic core and nothing else.

**Did.**
- **Ported the whole unit table** into `tc-calc`: upstream's records, the metric prefixes, the
  byte rates, the affine temperatures, the dimensions and the compatibility rules, plus the
  conversions between two units and the answers told in two units of one kind.
- **Put the calculator in the palette.** The answer is the first row, and opening the row copies
  the answer to the clipboard.
- **Fixed the fifteen clipboard tests that were failing** — in the *tests*, not in the store. The
  pins were expected in recency order instead of upstream's pin order, the fixtures were dated
  in 1970 so the retention swept them away before the test could look, the alpha was compared
  more strictly than upstream compares it, and the promotion swapped the wrong pair. Each was
  checked against upstream's own test file before the expectation was changed. Nothing in
  `tc-clipboard` is ignored now.
- Replaced an `unreachable!()` in the snippets' Markdown reader with a proper error, which is
  what the project's own rule about placeholders asks for.
- Wrote two port notes: 011 for the clipboard model, rewritten now that nothing is ignored, and
  012 for the unit table.

**The one thing this slice refuses to answer.**

Upstream's engine runs its own quantity parser before the conversions. Where that parser
answers, this port does not answer at all: `60 mph * 2.5 hours` and `1 / 20ms to hz` are left
unanswered rather than answered wrongly. The conversion reads its amount as a single number for
exactly that reason, and there is a test that says so:

```rust
assert_ne!(display("1 / 20ms to hz").as_deref(), Some("0.00005 Hz"));
```

**Verified.**
- `cargo test --workspace` passes: 571 tests, 0 failed, 0 ignored.
- `cargo clippy --workspace --all-targets -- -D warnings` passes, exit 0.
- `cargo fmt --all -- --check` passes, exit 0.
- `cargo run -p xtask -- parity --check` reports the ledger and `PARITY.md` are up to date.
- `cargo run -p xtask -- parity` reports 2 309 rows, 120 `PORTED`, 2 189 `NOT PORTED`.

**Stopped at.** 120 rows are PORTED of 1 815 Tier 1 rows. Next: the quantity algebra for the
calculator, the currencies, the emoji, the dictionary, the aliases, the hotkeys, the commands,
the file search and the window management.

**A note on the workers.** The clipboard worker from the earlier session was still writing files
after it had reported success. Its work was checked against upstream before it was committed:
the test fixes were right, and one of them — the pin order — was a case where the *expectation*
had been wrong rather than the store.
**Also, after the note above.**

- **Ported the emoji model** into a new crate `tc-emoji`: upstream's whole catalog of 2 136
  emoji and symbols in upstream's own format and order, upstream's search ranking running on
  the fuzzy matcher `tc-search` already carries, the pins, the frequency, the skin tones and
  the grid's navigation. 32 tests, every one of them taken from upstream's own test files.
- The picker screen is not built, so the emoji rows stay `NOT PORTED`: the model behind them
  is done, but a row that describes a screen is not shown by a model. The localized keyword
  packs are not in upstream's repository either — they live in the app bundle — so the English
  keywords are all this port searches.
- The workspace now has 604 tests, 0 failed, 0 ignored.
## Session 7 - 2026-11-11 (night)

The question this session answered was asked by the person who will use this
port: there is another YAL on this machine, in an "open code" folder, built a
different way and left in a corner - can anything in it shorten this build?

**The finding.** `C:\Users\dabin\development\yal` is the community YAL project
(`github.com/Abinanthan-CG/yal`, GPL-3.0-only): 89 Rust files, 1 301 117 bytes,
an egui launcher with a tray, a whole-drive file index, window controls, a
calculator with dates and currencies, and a pile of features upstream Tinycast
does not have. It is not a broken scaffold like the other port we audited - it
**compiles** (`cargo check --workspace --all-targets`, exit 0) and **its own
tests pass** (291 passed, 0 failed, 0 ignored).

**The verdict.** It is a parallel build of a similar product, not a source of
upstream parity rows. What it has that we want is *proven machinery* in four
places - the incremental file index, the window placement geometry, the
currency registry with its rate vintage, and the natural-language phrase
layer. The realistic saving is **18-28 hours of the ~330 the agreed scope
needs, about 5-8%**, not the shortcut it first looked like. The full
evidence is in `docs/audits/2026-11-11-parallel-yal-review.md`, and the
decision is in `docs/DECISIONS.md`.

**What was taken, in this session.**

- **The window commands and their geometry**, into a new crate `crates/tc-windows`:
  upstream's thirty-nine commands with their ids, names, groups and flags,
  and upstream's placement arithmetic - the halves, the quarters, the fourths,
  the thirds, the sizing commands, the nudges, the edge moves, the gap model,
  the cycling and the restore. 47 tests, all written from upstream's own Swift
  (`WindowCommand.swift`, `WindowPlacementEngine.swift`, `WindowCycle.swift`).
  Four of the tests failed on the first run and all four were *my expectations*
  being wrong, not the port: a nudge moves by five per cent of the screen (50
  points, not 40), Center Two Thirds rounds to 666 points, the display cycle
  walks Display 1 left - Display 2 right - Display 2 left - Display 1 right,
  and a window across the seam of two displays belongs to the one it overlaps
  most. Each was checked against upstream's arithmetic before the expectation
  was changed.
- **The licence trail.** AGPL-3.0 is GPL-3.0 plus section 13, so the donor's
  code may be combined into this repository and the combined work stays
  AGPL-3.0 - provided the notice travels with it. `NOTICE.md` now carries the
  donor's GPL-3.0 notice, and the port note says which module came from where.

**What was not taken, and why.** The egui interface (213 KB) - YAL is Tauri and
Solid by decision. The extra features (QR, TOTP, password generator, regex,
colour picker, screenshots, fonts, timers, reminders, scripts, bookmarks) - they
have no rows in the ledger, and building them would be scope creep against the
scope decided on 2026-11-11. Their emoji catalog (1 906 entries), their
clipboard (text only) and their search (26 tests) - ours are better and ours
are already built.

**The rows did not move.** No row is `PORTED` by this session. The window
geometry proves the arithmetic, but nothing moves a real window yet and no
command is reachable from the launcher, so the window-management rows stay
`NOT PORTED` until a test on this machine moves a window and reads its
rectangle back.

- **The file-search backend**, into `crates/tc-files`: the search scopes, the
  three kinds of ignore rule upstream has, the incremental pass that keeps the
  index current, the SQLite store, and the search over it - every word must
  appear in the name in any order, the exact name first, the user's own files
  above the hidden and the program ones, the accents folded, 200 results at
  most, and the recent list for an empty query. 55 tests. The rebuild follows
  upstream's rule: the new index is built beside the old one and swapped in
  when it is ready, so a search that is running keeps the old index until the
  new one is complete.

- **The window mover and the memory of the windows it moved**: the displays
  and their work areas, the window list, the foreground window, and the code
  that writes a rectangle into a real window and reads it back - plus
  upstream's memory of where each window was before YAL touched it, the cycle
  step, the two-point tolerance and the bound of sixty-four windows. 19 tests
  for the memory, 12 for the mover. The load-bearing test runs **every**
  command in the catalogue against a real window and reads the rectangle
  back; another runs upstream's own example - Left Half, Maximize, Top Right
  Quarter, Restore Window - and checks the window ends up where it started.

**The commands are in the launcher too.** They are listed under their own
section, off until window management is turned on the way upstream ships it,
and the four settings it has - `windowManagement.enabled`, `.showInLauncher`,
`.gap` and `.cycle` - are read when the app starts. The window that is moved
is the one the user was last using: the palette is a window too, so the
window that was in front before the palette opened is the one remembered.

**Fifty-one window-management rows moved to `PORTED`, 120 -> 171.** The
ledger's validator resolved every piece of evidence to a real test in a real
crate, which is what keeps the claim honest.

**The worker failed.** The background worker that was to build the file-search
backend died on a provider balance error before writing a file, so that slice
was written by hand instead - as the geometry was.

**Verified.**
- `cargo test -p tc-windows` passes: 47 tests, 0 failed, 0 ignored.
- `cargo test -p tc-files` passes: 55 tests, 0 failed, 0 ignored.
- `cargo test --workspace` passes: 743 tests, 0 failed, 0 ignored.
- `cargo clippy -p tc-windows --all-targets -- -D warnings` passes, exit 0.
- `cargo fmt --all` leaves no diff.
## Session 8 - 2026-11-12 (early morning)

The person who will use this port answered the question the previous session asked: the other YAL
project was **theirs**, so nothing had to be held back for the licence - and when the port is
finished, that repository is to be **flushed and replaced by this one**.

**What was done.**

- The repository slug is set: `Abinanthan-CG/yal`, written down once in `tc-core::REPOSITORY` and
  repeated in the eight places that cannot read a constant - `Cargo.toml`, the README, SECURITY.md,
  the changelog, the two issue templates and the troubleshooting page. The `origin` remote is set,
  and the branch is renamed from `master` to `main` to match the repository's default branch.
- `docs/PUBLISHING.md` was written: what the publish does, what it destroys, the six checks that
  have to pass first, the commands, what the repository's own page should say, and what to do when
  a push goes wrong.
- The decision is recorded in `docs/DECISIONS.md`.

**What was not done.** The flush and the push. The build is not finished, and a forced push that
replaces a public repository's contents is not something to do early or quietly: it is the last
step of Phase 4, and it happens once, with the confirmation stated at the moment it is made.

**The notices stay.** The donor is the same person, so the licence question does not arise. The
notices in `NOTICE.md` and `crates/tc-files/ATTRIBUTION.md` stay anyway, because they say which
code came from where, which is worth more than a licence line.

**Verified.**
- `cargo test --workspace` passes: 706 tests, 0 failed, 0 ignored.
- `cargo clippy --workspace --all-targets -- -D warnings` passes, exit 0.
- `cargo fmt --all -- --check` passes, exit 0.
- `cargo run -q -p xtask -- parity --check` reports the ledger and `PARITY.md` are up to date.

## Session 9 - 2026-11-13

The calculator's three phrase layers were built: the percentages, the money and the shared amounts.

**What was done.**

- **`crates/tc-calc/src/percent.rs`** - upstream's `CalcPercent` ported whole: the comment-word rule
  (a word between the percentage and of/on/off only names it), the fourteen adjustments, the tax
  inside a price, the twenty-six ratio shapes, the rule of three in both its orders, and the
  badges - `Tip`, `Discounted`, `Increased`, `Total`, `Tax`, `Percentage`, `Percent Change`,
  `Proportion`, `Ratio` - instead of a generic `Result`.
- **`crates/tc-calc/src/currency.rs`** - upstream's currency table: 159 fiat currencies and 23
  coins, the nouns that name one, the contested ones written by hand, and the codes people spell
  from the sign. The table is transcribed from upstream's generated Swift by
  `scripts/gen-currency-table.ps1`, so the two cannot drift apart by hand. A value now carries its
  currency through the calculation, so `20% discount off $100` is `80.00 USD` and not `80`.
- **`crates/tc-calc/src/split.rs`** - the shared amounts: `85 + 20% split 3` is `34`, `$120 split 4`
  is `30.00 USD`, and the card says `Per Person`.
- The calculator's phrase card now echoes the query the way upstream does - with its spacing
  collapsed and its operators left as they were typed.

**What was not done.** The exchange rates, and everything that needs them: the conversions
between two currencies, the amount of money on its own converted to the user's own currency, and
the rate on its own. Two different currencies in one calculation earn no card rather than a
made-up number. The arithmetic that carries a unit is still missing, so `750 m as % of 3 km` is
not answered, and `Half of $90` waits for the arithmetic asked in words.

**Verified.**
- `cargo test --workspace` passes: 794 tests, 0 failed, 0 ignored.
- `cargo clippy --workspace --all-targets -- -D warnings` passes, exit 0.
- `cargo fmt --all -- --check` passes, exit 0.
- `cargo run -q -p xtask -- parity` reports 2309 rows, 208 PORTED, 2101 NOT PORTED.
- `cargo run -q -p xtask -- parity --check` reports the ledger and `PARITY.md` are up to date.

## Session 9 - 2026-11-13 (second part)

The calculator's two phrase layers about time: the money phrases about growth, loans and returns,
and the phrases about a steady rate.

**What was done.**

- **`crates/tc-calc/src/finance.rs`** - upstream's `CalcFinance`: the future value, the interest,
  the present value, the monthly, annual and total payments, the interest a loan earns, the
  return on an investment, its yearly return - the CAGR - and the capital an investment needs.
  The principal, the term and the rate come in either order, and `after` and `over` both read as
  `for`.
- **`crates/tc-calc/src/growth.rs`** - upstream's `CalcGrowth`: the compounded change, the time it
  takes to reach a value, and the rate that explains a change.
- **`crates/tc-calc/src/duration.rs`** - the duration reader. Upstream's arithmetic carries a unit
  on the value, so `5 years` is a unit value and `5 years / year` is the number five. This port's
  arithmetic core does not carry units yet, so the phrases that need a term read the duration
  themselves, the way upstream does.

**What was not done.** The rates that carry a unit (`$50/hr for 3 hours`), the statistics, and the
arithmetic asked in words.

**Verified.**
- `cargo test --workspace` passes: 819 tests, 0 failed, 0 ignored.
- `cargo clippy --workspace --all-targets -- -D warnings` passes, exit 0.
- `cargo fmt --all -- --check` passes, exit 0.
- `cargo run -q -p xtask -- parity` reports 2309 rows, 225 PORTED, 2084 NOT PORTED.

## Session 9 - 2026-11-13 (third part)

The reductions over a spoken list and the math asked in words.

**What was done.**

- **`crates/tc-calc/src/statistics.rs`** - upstream's `CalcStatistics`: the phrase is lowered to the
  function call it names, so `median of 3, 1, 2` is reduced by the same arithmetic as
  `median(3, 1, 2)`, and the badge says which reduction ran - `Median`, `Count`, `Standard
  Deviation`, `Sum`, `Maximum`, `Average`.
- **`crates/tc-calc/src/spoken.rs`** - upstream's `CalcSpokenMath`: the remainder, the primality
  (Miller-Rabin with the bases that are deterministic for every 64-bit whole number, in 128-bit
  arithmetic), the clamp, the root, the proportion, the combinations and permutations, the logarithm
  and the multiplier.
- **The engine now tries every layer**, as upstream does, and reports the area it could not answer
  only when nothing else answers. It stopped at the first one before, which made `5 as multiplier of
  1` answer "the units are not supported" - `as` is the attosecond - where upstream answers `5x`.

**What was not done.** The lists that carry a unit (`sum of 1 km and 500 m`), the questions asked in
words (`what is 20% of 50`), and the exchange rates.

**Verified.**
- `cargo test --workspace` passes: 834 tests, 0 failed, 0 ignored.
- `cargo clippy --workspace --all-targets -- -D warnings` passes, exit 0.
- `cargo fmt --all -- --check` passes, exit 0.
- `cargo run -q -p xtask -- parity` reports 2309 rows, 246 PORTED, 2063 NOT PORTED.
## Session 10 - 2026-11-13

The command catalogue, the custom commands, and the shell they run under.

**What was done.**

- **`crates/tc-commands`** - a new crate, in two halves.
  - `catalog.rs` - every built-in command, the Settings pane it belongs to, the switches that
    decide which of them exist, and what a second press of the shortcut does.
  - `model.rs` and `store.rs` - the custom command: upstream's fields, upstream's defaults,
    upstream's validation, and a JSON file beside the app's other data.
  - `shell.rs` and `runner.rs` - the shell a command names (PowerShell, cmd, Git Bash, WSL) and
    the real Windows runner: no console window, the input closed, `TINYCAST=1` in the
    environment, the Run In folder, no timeout, and a job object so `Stop` ends the whole tree
    while quitting the app leaves the command running.
  - `raycast.rs` - the import of a folder of Raycast script commands, with the shebang's
    interpreter resolved against Windows.
  - `arguments.rs` - the row of fields a command asks for, and the rules for the tab, the return
    and the first missing field.
- **The app** - the launcher lists the built-in commands the build can run and the user's own
  commands; the palette asks for a command's values, confirms before it runs, and shows what it
  printed.

**What was not done.** The Settings panes, the output window (and with it the colours and the
in-place progress bars), the backup carrying the custom commands, and the test for the palette's
own key handling.

**Decisions.** Three: the command's text is written to a temporary file for PowerShell; needs
confirmation is on by default, as the documentation says and upstream's code does not; and the
launcher offers only the built-in commands the build can run.

**Verified.**
- `cargo test --workspace` passes: 931 tests, 0 failed, 0 ignored.
- `cargo clippy --workspace --all-targets -- -D warnings` passes, exit 0.
- `cargo fmt --all -- --check` passes, exit 0.
- `cargo run -q -p xtask -- parity` reports 2309 rows, 333 PORTED, 1976 NOT PORTED.
## Session 11 - 2026-11-13

Raycast for Windows, the manifest, and the palette's screen system.

**What was done.**

- **Raycast for Windows 2.12.0, read as behaviour.** The .NET assembly with ILSpy, the native
  Rust calculator with borer: 617 units in 53 families, the parser's vocabulary, the fetcher
  names, the money and the time. Written up as behavioural notes only - no Raycast code, tables
  or naming copied into YAL. See
  [port note 021](port-notes/021-raycast-for-windows-study.md).
- **The launcher's own manifest.** borer found that the binary did not carry the
  `requestedExecutionLevel`, the `dpiAwareness` or the `longPathAware` that
  `docs/WINDOWS-MAPPING.md` claimed it did. `apps/yal/windows/yal.manifest` now carries all of
  them, embedded through `tauri-build`, and the whole document was read back out of the rebuilt
  binary.
- **`crates/tc-palette`** - a new crate. The keys and the translation from upstream's Mac chords
  to Windows, the screens and how they stack, the palette's settings with upstream's defaults,
  what a key does, the actions menu, the state machine, and the file-search screen. 117 tests.
- **The window asks the palette.** The frontend no longer decides what a key means; it sends
  the key and does what it is told.
- **The file-search screen.** Recent with nothing typed, Results with something typed, the
  filter applied during the search, and the files below the launcher's own rows.
- **The file row's actions.** Open, Show in File Explorer, Filter by Type, Copy Name, Copy
  Path, and Move to the Recycle Bin - the last one a new `SHFileOperationW` call with
  `FOF_ALLOWUNDO`, so the file can be put back.
- **The General pane.** The six settings the palette follows, each one changeable and written
  down, so the settings screen is not a dead end.

**What was not done.** The palette cannot be dragged and does not follow the pointer across
displays; nothing animates yet; the compact shortcuts are not in the pane; the scopes are not
chosen yet, so the app does not build an index of its own; the Settings panes other than
General; the preview pane beside the file list; and the file row's remaining three actions.

**Decisions.** Three: the palette's decisions live in Rust rather than in the window; a screen
offers only the actions the build can run; and the General pane shows six settings rather than
an empty screen.

**Verified.**
- `cargo test --workspace` passes: 1081 tests, 0 failed, 0 ignored.
- `cargo clippy --workspace --all-targets -- -D warnings` passes, exit 0.
- `cargo fmt --all -- --check` passes, exit 0.
- `cargo run -q -p xtask -- parity` reports 2309 rows, 413 PORTED, 1896 NOT PORTED.
- `npm.cmd run check` and `npm.cmd run build` in `frontend/` pass.
## Session 12 - 2026-11-13

**What was done.**

- **The arithmetic that carries a unit.** A value in `tc-calc` now carries its measurement, the
  way upstream's `CalcValue` does, so the unit is part of the calculation rather than something
  the card adds afterwards.
- **`crates/tc-calc/src/measure.rs`** - the measurement: the symbol, the long name, the
  category, the scale onto the base, the offset and the dimension. It is `Copy`, so a
  calculation carries it without an allocation on every step. The catalog hands it over through
  one new method, `unit_ref`.
- **The compound unit.** Two measures of different kinds make a third: `2kg / 4m3` is
  `0.5 kg/m³` and `1kg * 1m` is `1 kg·m`. The symbol keeps the way the two were joined, and
  one copy of every symbol is kept so that two compounds with the same writing are the same
  measure.
- **The base unit of a dimension**, so `5m * 4m` answers `20 m²` rather than a compound, and
  **the unit a rate accrues in**, so `60 mph * 2.5 hours` answers `150 mi` and
  `500mA * 3h 30min` answers `1,750 mAh`.
- **The parser carries it.** The last unit typed decides the unit of the answer; composite
  notation answers in the leading unit; a bare amount takes the unit next to it; two measures of
  one kind divide into a plain ratio; a plain amount over a unit is the inverse of that unit;
  and a root raises the unit as well.
- **The card shows it**, with the unit in the answer and the unit's name on the badge, and a
  conversion can sit at the end of a calculation: `10kg + 500g to lb` is `23.14853753 lb`.
- **The documentation's lost multiplication signs.** The unit section of upstream's
  documentation writes `5m 4m` where the example is `5m * 4m`; the markdown reads the sign as
  emphasis. Upstream's own tests write the sign out, and both readings are answered the way
  upstream answers them.
- **Twenty-two new tests** in `crates/tc-calc/src/measure_tests.rs`, every expectation taken
  from upstream's `Tests/calc-test.swift` or from its documentation.

**What was not done.** Twelve rows of the unit section stay open, each with its own reason: a
conversion in the middle of a calculation, a conversion asked in words, a conversion of money
in the middle of a calculation, the targets `duration` and `timespan`, and a target that is
itself a compound unit. The multi-argument functions still lose the unit (`min(1km,999m)`
answers `1`), and a mismatch of measures answers with no card rather than with the error
upstream shows.

**Decisions.** Three: a value carries its unit and the catalog hands it over as a shared
reference; a compound unit keeps one copy of its symbol; and the rows whose documentation lost
a multiplication sign are `PORTED` with the evidence of the example as upstream's tests write
it.

**Verified.**
- `cargo test --workspace` passes: 1103 tests, 0 failed, 0 ignored.
- `cargo clippy --workspace --all-targets -- -D warnings` passes, exit 0.
- `cargo fmt --all -- --check` passes, exit 0.
- `cargo run -q -p xtask -- parity` reports 2309 rows, 441 PORTED, 1868 NOT PORTED.
- `cargo run -q -p xtask -- parity --check` reports the ledger and `PARITY.md` are up to date.
## Session 13 - 2026-11-13

**What was done.**

- **The functions of a measurement.** The lists of measures are reduced in the unit of the first
  argument, so `min(1km,999m)` is `0.999 km`, `sum(1km,500m)` is `1.5 km` and
  `sum of 1 km and 500 m` is `1.5 km` with the card labelled `Sum`.
- **The compound a unit makes with another unit name.** `60 miles per hour` is `60 mi/hr`,
  `10 kg/m3` is `10 kg/m³`, and `50 usd per hour` is `50 USD/hr`. The rule is upstream's: a unit
  followed by a sign and then the name of another unit makes a compound, and the amount stands
  alone.
- **A percentage is a plain amount.** Two bugs came out of that: a percentage after a measure
  did not fail the parse, and a phrase that wanted a plain amount took one that carried a
  measure. Together they made `50 as % of 200` answer `100` instead of `25%`, because the `as`
  was read as the unit of a coulomb.
- **The unit table checked against upstream's.** The two tables hold the same 218 records, the
  last one being the ohm.

**What was not done.** The unit table's own prefix entries are generated the same way upstream
generates them, but a table of 617 units like Raycast's is not the goal; and the decimal comma of
a European Windows is not read yet, so a list of values is separated by a comma only.

**Decisions.** None new; the decisions of session 12 hold.

**Verified.**
- `cargo test --workspace` passes: 1112 tests, 0 failed, 0 ignored.
- `cargo clippy --workspace --all-targets -- -D warnings` passes, exit 0.
- `cargo fmt --all -- --check` passes, exit 0.
- `cargo run -q -p xtask -- parity` reports 2309 rows, 447 PORTED, 1862 NOT PORTED.
## Session 14 - 2026-11-15

**What was done.**

- **The release workflow could be read by GitHub.** It had been turning red on
  every push - in no time, with no jobs - and the reason was only in the
  run's annotations: `Unrecognized named-value: 'secrets'`. GitHub does not
  hand the `secrets` context to the `if` of a job or a step, so
  `if: ${{ secrets.CERTIFICATE != '' }}` is an error, and a workflow with an
  error in it does not run at all. The certificate now travels through the
  job's environment, which is set before any step runs. Three other things
  the release path had never been given came with it: the ARM64 build tools,
  the target being built for, and an optional signing key
  ([port note 027](port-notes/027-the-workflow-github-cannot-read.md)).
- **The workflow files are checked before GitHub checks them.**
  `scripts/check-workflows.py` knows the rule the schema does not, and it has
  a self-test pointed at the exact file GitHub refused.
- **Six reasons in the ledger had lost their keys.** `parity/reasons.toml`
  held six lines that were a value with no key, which is not TOML, and the
  command that reads it failed. The keys were lost in `74d7df6`. The ledger
  check now loads the whole ledger - the pages, the reasons and the count
  checks - so a file that only one command reads cannot break without
  anybody noticing.
- **The native palette is the launcher.** It was a probe with ten fixed rows.
  It is now the window the hotkey opens, the tray opens, and the user types
  in; it searches the program index, and the row the user chooses is run
  ([port note 028](port-notes/028-the-palette-that-is-the-application.md)).
- **`tc-ui` can be driven by an application.** The palette asks what
  happened - shown, the query changed, a row chosen, dismissed - and the app
  answers with the rows to show, or with nothing, or with "close".
- **Two bugs that only running it could find.** The hint the search field
  shows is written into the control, which raises `EN_CHANGE`, so the app
  was told the query had become "Search YAL" - and an empty query matches
  nothing, so the list was empty. The hint is not a query, and an empty
  palette shows what the user opens most.
- **One behaviour, two palettes.** The rows run through a `Runner` that both
  palettes call, so they cannot drift apart.
- **The browser window is made when it is wanted.** It was created at startup
  with `visible: false`, and a hidden window is not a cheap window: it
  started five WebView2 processes and cost about 330 MB. It is now made when
  the settings screen is asked for - from the tray, or with
  `yal.exe --settings`.

**What was not done.** The settings screens are still the browser's; drawing
them natively is the next step, and it is a large one. The tray's "Check for
Updates" and "About YAL" still do nothing. The global hotkey engine is still
a stub in `tc-platform-win`: the hook installs and removes, but nothing reads
the keys. The native palette does not yet have the settings, clipboard,
emoji or file-search screens - it has the launcher, and that is what the
hotkey opens.

**Decisions.**

- The workflows are checked before GitHub checks them (2026-11-15).
- The browser window is made when it is wanted, not when the app starts
  (2026-11-15).
- The ranking remembers the queries, not only the items (2026-11-15).
- The import from Raycast is possible after all (2026-11-15).

**Verified.**

- `cargo test --workspace` passes: 1149 tests, 0 failed, 0 ignored.
- `cargo clippy --workspace --all-targets -- -D warnings` passes, exit 0.
- `cargo fmt --all -- --check` passes, exit 0.
- `cargo run -q -p xtask -- parity --check` reports the ledger and
  `PARITY.md` are up to date.
- The application, release build, measured: 58.8 MB working set, 37.9 MB
  private, one process, no WebView2, 0.19 s of CPU. With the settings open:
  391.7 MB and six processes. Raycast for Windows: 914 MB.
- The native palette, taken from the running application:
  [docs/port-notes/images/native-palette-in-app.png](port-notes/images/native-palette-in-app.png).

## Session 15 - 2026-11-19

**What was done.**

- **`cargo deny check` passes.** It is a gate in the Windows job and it had
  never run in CI, because it sat behind a step that was failing first. It
  had three findings waiting.
- **The workspace crates are private.** Every member now carries
  `publish = false`, which is what they are: the private parts of one
  application, none of them published to crates.io. It is also what lets
  cargo-deny's `allow-wildcard-paths` apply to the path dependencies between
  them - a crate cargo-deny believes is publishable is a crate it will not
  accept a path wildcard for. `scripts/set-publish-false.py` applies the line
  to every member and is safe to run again.
- **The bundled SQLite was patched.** `rusqlite` 0.32 → 0.41, which bundles
  SQLite 3.54.0 and closes RUSTSEC-2026-0459: a crafted database file with a
  mismatched `-wal` sibling could make SQLite hand back uninitialized
  page-cache memory. The two crates that use it needed no code change.
- **The zip library was patched.** `zip` 2.6.1 → 9.7.3, which closes
  RUSTSEC-2026-0548: a ZIP archive can name an entry twice, once in the local
  file header and once in the central directory, and a reader that trusts the
  local names can write files under names a check never saw. The one call
  site - xtask's portable zip - needed no change.
- **The four advisories with no safe upgrade are named, with their reasons.**
  The three zbus 5 crates, reached only through
  `tauri-plugin-single-instance 2.5.5`; `chrono`, which is unmaintained and
  used in three call sites; and `proc-macro-error`, a build-time derive macro
  that reaches us through glib-macros on Linux only.
- **`chrono-tz` was declared but used by no crate**, so it is gone. The
  declared toolchain floor moves from 1.82 to 1.89, which is what rusqlite
  0.41 requires; the pinned toolchain stays at 1.99.1.
- **The placeholder scan was clean.** It had never run before, and it found
  one hit: a test fixture in `tc-core` used the literal word "todo" as the
  short query to be quoted whole. The fixture says "note" now - the same
  length, the same intent.
- **A flaky test, and the reason it was invisible.** The quit watcher's event
  has a fixed name, and both of its tests used it - so one test's watcher
  could hold the event open while the other asserted it was gone, and the
  assertion failed at random. The script that checks the documented commands
  made it invisible: it ran each command as `& $Body | Out-Null`, and a test
  harness writes its failure report to standard output, so the failing test's
  name was thrown away and the build said only "the command exited with
  101". The script now keeps the output and prints it with the failure, and
  each test has an event of its own.
- **Two timing assumptions are gone.** The test that stops a whole process
  tree waited 800 ms and then checked that nothing more was written; it now
  waits for the writing to stop. The test that a command keeps running after
  the launcher lets go had a ten second window; it has thirty.
- **`clippy.toml`'s MSRV now agrees with `Cargo.toml`'s**, at 1.89. The two
  must agree, because CI runs clippy with `-D warnings` and a mismatch is a
  warning. Raising the floor turned on a lint newer than the old one, and
  the three places that used `% 2 == 0` say it the new way.
- **A handover document** - `HANDOVER.md` - for the next person or model
  taking the port over.

**What was not done.** The chrono migration to `time` or `jiff` is written
down as work for 1.1, not done here: the snippet template formats dates with
strftime patterns that upstream's own tests pin, so it is a migration rather
than a version bump, and it wants its own session. The settings screens are
still the browser's, the hotkey engine is still a stub, and the tray's "Check
for Updates" and "About YAL" still do nothing.

**Decisions.**

- The workspace crates are private, and they say so (2026-11-19).
- An advisory with no safe upgrade is ignored with the reason it is safe to
  ignore, not with a blanket "not affected" (2026-11-19).

**Verified.**

- `cargo test --workspace` passes: 1149 tests, 0 failed, 0 ignored, in 46
  test binaries.
- `cargo clippy --workspace --all-targets -- -D warnings` passes, exit 0.
- `cargo fmt --all -- --check` passes, exit 0.
- `cargo deny check` passes: advisories ok, bans ok, licenses ok, sources ok.
- The CI placeholder scan run by hand over `crates`, `apps` and `frontend`
  finds nothing.
- `scripts/check-build-with-ai.ps1 -SkipDist` passes all 11 checks.
- `cargo test --workspace` passes three runs in a row, and the quit watcher's
  own tests pass twelve runs in a row.
- **CI is green.** Every job, every step, on the run of 2026-11-19 - the
  first green run this project has had. The licence check, the placeholder
  scan, the release build, the BUILD_WITH_AI check, the self-test, the
  installer, the smoke test and the upload all pass on `windows-latest`.
- `cargo run -q -p xtask -- parity` reports 2309 rows, 447 PORTED, 1862
  NOT PORTED.
