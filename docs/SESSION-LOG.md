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