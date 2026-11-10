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
