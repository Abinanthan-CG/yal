# Changelog

All notable changes to YAL are recorded here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and YAL uses [semantic versioning](https://semver.org/spec/v2.0.0.html).

YAL is a port of [Tinycast](https://github.com/abue-ammar/tinycast). Feature
parity with Tinycast is tracked in [`PARITY.md`](PARITY.md), not here: this
file records changes to YAL itself, and `PARITY.md` records how far the port
has got.

---

## [Unreleased]

### Added

- **The search slice** in `tc-search`, ported from upstream's launcher model:
  the fold and the tiered match, the scorer where a word start outscores any
  other letter, the relevance cells that weigh a name against the thing that
  provides it, the learned ranking that remembers what the user opens, the
  Latin spellings for names in other scripts, the rules that decide which
  names an entry is found by, and the ordering that decides which entry comes
  first.
- **Upstream's own tests as the oracle.** All 19 pinned matcher scores, the
  sensitivity levels, the transliterations, the 25 comparator rules and the
  12 category rules, plus the frecency curve and the learned-term rules, are
  tests in this repository with upstream's own numbers.
- **A second ledger file.** `parity/status.toml` holds the port's verdict on
  each generated row, with the evidence that shows it, so regenerating the
  ledger's scope from upstream's documentation never loses it.
- **The quicklink destination rules** in `tc-core`: what a saved link points
  at is worked out from its shape alone — a path, a web page, a share, a link
  into another application, or a bare domain written down from memory. The
  `url` crate's own conversion is per-platform, so a `file:` URL is read here
  instead: a host is a UNC share, and a leading `/C:` is a drive.

### Changed

- The parity ledger's evidence may now name the module a test lives in, as
  `crate::module::test_name`.
- Every open parity row now carries a reason that names the piece that is
  missing, from `parity/reasons.toml`, one line per page.
- `BUILD_WITH_AI.md` describes the ledger's two files and the new frontend
  build step, and `scripts/check-build-with-ai.ps1` runs every command it
  lists. CI runs it too.

### Fixed

- The repository had no frontend source: the palette was a hand-written page
  in a directory that is not committed, so a fresh clone could not build the
  application. The interface is now Solid and TypeScript in `frontend/src/`,
  built by Vite.

## [0.1.0] — 2026-11-10

The first release. It is the foundation of the port: the shell, the platform
layer, the domain model, the build and the ledger. Most features are not
ported yet, and `PARITY.md` says so for each one.

### Added

- **The palette shell.** A borderless, always-on-top WebView2 window that
  appears on the global hotkey and hides again when it loses focus.
- **The system tray.** Six items: Open YAL, Settings, Check for Updates,
  About YAL, Support Tinycast, and Quit YAL.
- **The single-instance lock.** A named mutex, `Local\YAL-single-instance`.
  A second launch hands its arguments to the running instance instead of
  starting another one.
- **The quit signal.** A named event, `Local\YAL-quit`, so the installer can
  close a running instance before it replaces the program files.
- **The `tinycast://` protocol**, registered per user under
  `HKCU\Software\Classes`.
- **The domain model** in `tc-core`: all 46 built-in commands with their exact
  upstream identifiers, all 32 system actions with their categories and
  confirmations, the launcher-row identifier scheme, the deeplink grammar, and
  the strict UUID rules. 50 tests.
- **The platform layer**: `tc-platform` holds the traits and their in-memory
  fakes, `tc-platform-win` holds the Win32 implementation — paths, the
  clipboard, the foreground window, the window list, the integrity level, the
  keyboard hook and the single-instance lock. 6 tests, all on real Windows.
- **Three helper processes**: the file indexer, the text recognition helper
  and the dictation helper. Each runs in its own process so that a large
  scan, a large document or a large model cannot block the palette.
- **The self-test.** `yal.exe --self-test` checks twelve subsystems on the
  machine it runs on and prints the result as JSON. It is how the CI job
  proves the build works on real Windows.
- **The parity ledger.** 2 309 rows across all 42 upstream documentation
  pages, generated rather than hand-written, validated on every build, and
  rendered into [`PARITY.md`](PARITY.md).
- **The build tool** `xtask`: `parity`, `ledger`, `tags`, `i18n`, `dist` and
  `selftest`.
- **The installer.** A per-user NSIS installer with optional Start menu,
  desktop and login-item sections. It never asks for administrator rights,
  and it leaves your data alone when it is uninstalled unless you ask it not
  to.
- **The portable build**, with its data in a folder next to the executable.
- **The checksums.** `SHA256SUMS.txt` for every artifact.
- **The CI workflows.** A fast pure-logic job on Ubuntu, a full Windows job
  on `windows-latest`, and a release workflow that builds both architectures
  from a tag.
- **The documentation**: this file, [`README.md`](README.md),
  [`BUILD.md`](BUILD.md), [`BUILD_WITH_AI.md`](BUILD_WITH_AI.md),
  [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md),
  [`docs/WINDOWS-MAPPING.md`](docs/WINDOWS-MAPPING.md),
  [`docs/SHORTCUTS.md`](docs/SHORTCUTS.md), and the decision and port-note
  logs.

### Not included yet

Most of the product. `PARITY.md` lists every feature and sub-feature with its
status and, where it is not ported, the reason. At this release every row is
`NOT PORTED` except the ones the domain-model tests prove.

### Licence

AGPL-3.0-only, as a derivative work of Tinycast. See
[`LICENSE`](LICENSE), [`NOTICE.md`](NOTICE.md) and
[`UPSTREAM.md`](UPSTREAM.md).

[Unreleased]: https://github.com/OWNER/REPO/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/OWNER/REPO/releases/tag/v0.1.0
