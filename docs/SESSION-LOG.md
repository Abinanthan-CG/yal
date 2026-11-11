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
