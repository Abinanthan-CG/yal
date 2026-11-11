# Progress

The state file for this port. Read it at the start of every session, before anything else.

## What this is

A Windows port of Tinycast (a macOS keyboard launcher, AGPL-3.0, upstream
`github.com/abue-ammar/tinycast`). The app is named **YAL**; the executable is
`yal.exe`. Rust + Tauri 2 on the system WebView2 runtime.

## Environment (verified 2026-11-10)

| Thing | Found |
| --- | --- |
| OS | Windows 11 Home Single Language, 10.0.26300, x64 |
| Rust | 1.99.1 (58cf8f9c3 2026-10-13), rustup 1.29.1 |
| Rust targets | `x86_64-pc-windows-msvc`, `aarch64-pc-windows-msvc` |
| C toolchain | VS 2022 BuildTools at `C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools` |
| Windows SDK | 10.0.26100.0 |
| Node | v24.16.0, npm 11.13.0 |
| Python | 3.12.10 |
| WebView2 | 157.0.4341.42 (installed system-wide) |
| NSIS | 3.13 (installed via winget, at `C:\Program Files (x86)\NSIS`) |
| ARM64 cross-compiler | `Microsoft.VisualStudio.Component.VC.Tools.ARM64`, installed on 2026-11-10 |
| Network | reachable: tinycast.dev and github.com both answer |

Everything runs natively on real Windows here, so Windows-dependent rows can be
proven locally rather than waiting for CI.

## Upstream reference

- Cloned at `upstream/` (never committed). Commit SHA: `58168bf7e0b721f7c5fbfc7f4e8a068a48cff446`.
- All 42 documentation pages downloaded to `upstream-docs/` (never committed).
- Upstream layout confirmed: 869 Swift files under `Tinycast/`, 127 test files under `Tests/`,
  42 Markdown files under `upstream/docs/`.

## Phase 0 - Recon

Status: **done**

- Environment detected and recorded above.
- Repository created, git initialized, `.gitignore` excludes `upstream/`, `upstream-docs/`, `target/`, `dist/`.
- Upstream cloned, SHA recorded in `upstream-sha.txt` and `upstream-commit.txt`.
- All 42 upstream docs pages downloaded, and re-downloaded after the encoding problem.
- Cargo workspace created with the crate map from the brief (20 members).
- `xtask` crate with the `parity` validator and generator, `ledger`, `tags`, `i18n`, `dist` and
  `selftest` commands.
- `parity/pages.toml`: canonical inventory of all 42 pages, each with its tier and its Windows
  translation.
- `parity/count_checks.toml`: every count check upstream states, with `kind` and `expected_rows`.
- `parity/tags.toml`: the binding from each count check to the rows it covers.
- The ledger is **generated**: 391 headings, 2 309 rows across 42 pages
  (Tier 1: 1 815, Tier 2: 494).
- `docs/WINDOWS-MAPPING.md` and `docs/SHORTCUTS.md` written.

## Phase 1 - Foundation

Status: **done**

- `tc-core`: 46 built-in commands, 32 system actions, the launcher-row identifier scheme, the
  `tinycast://` grammar, strict UUIDs, URL encoding. **102 tests.**
- `tc-platform`: the traits (`Paths`, `WindowSystem`, `Clipboard`, `Hotkey`, `SingleInstance`,
  `Clock`, `FileSystem`, `Settings`) and their in-memory fakes.
- `tc-platform-win`: the Win32 implementation — paths, clipboard, foreground window, window list,
  integrity level, keyboard hook, single-instance mutex, and the quit event. **6 tests, on real
  Windows.**
- `apps/yal`: the Tauri 2 shell — palette window, tray with six items, global shortcut,
  single-instance plugin, deep-link plugin, the quit watcher.
- `apps/yal-indexer`, `apps/yal-ocr`, `apps/yal-dictation`: the three helper processes, each with
  a real command-line interface and its own tests.
- `yal.exe --self-test`: twelve subsystem checks, all passing on this machine.
- `installer/yal.nsi`: the per-user NSIS installer.
- `scripts/build.ps1`, `scripts/bootstrap.ps1`, `scripts/build.sh`, `scripts/smoke-test.ps1`,
  `scripts/make-icon.py`.
- `.github/workflows/ci.yml` and `.github/workflows/release.yml`.
- The documentation set: README, BUILD, BUILD_WITH_AI, CHANGELOG, LICENSE (AGPL-3.0), NOTICE,
  SECURITY, CONTRIBUTING, CODE_OF_CONDUCT, UPSTREAM, docs/ARCHITECTURE, docs/WINDOWS-MAPPING,
  docs/SHORTCUTS, docs/user/*, docs/port-notes/*.
- `dist/` holds the x64 and arm64 installers, the portable zips and `SHA256SUMS.txt`.

## Phase 2 - Tier 1 vertical slices

Status: **in progress** — the search slice is done

### The search slice (crates/tc-search)

Ported from upstream's Tinycast/Features/Launcher/Model/:

| Module | Upstream | What it does |
| --- | --- | --- |
| uzzy | SearchRelevance.swift (the FuzzyMatch half) | the fold, and the tiered match |
| matcher | LauncherMatch.swift | the scorer, where a word start outscores any other letter |
| 
elevance | SearchRelevance.swift (the SearchRelevance half) | which alias matched, and how well |
| 
anking | LauncherRankingStore.swift | what the user opens, and by which query |
| 
omanization | ScriptRomanization.swift | how a name in another script is read |
| entry_naming | EntryNaming.swift | which names an entry is found by at all |
| order | LauncherOrder.swift | which entry comes first |

Upstream's own test files are the acceptance oracle: upstream/Tests/fuzz-test.swift pins the
matcher's 19 scores, the sensitivity levels, the transliterations, the 25 comparator rules and
the 12 category rules, and upstream/Tests/ranking-test.swift pins the frecency and the learned
terms. All of them are ported as tests here, with upstream's own numbers.

	c-search has 150 tests. The workspace has 431.

### The launcher core

- **`crates/tc-apps`** — the program model and the rules: the publisher is a keyword and never
  ranks, and entries whose names fold the same are one entry.
- **`crates/tc-platform-win/src/programs.rs`** — the Windows enumeration: the Start Menu, the
  registry's uninstall key, and the registry's App Paths. 400 programs on this machine.
- **`crates/tc-platform-win/src/launch.rs`** — opening a thing the way the shell does, so a
  Start Menu shortcut is opened as a shortcut rather than resolved first.
- **`apps/yal/src/launcher.rs`** — the index, the ranking, and the learned ranking in
  `%APPDATA%\yal\launcher-ranking.json`.
- **Four Tauri commands**: `search`, `suggestions`, `open_result`, `hide_palette`.

### The palette

The interface is Solid and TypeScript in `frontend/src/`, built by Vite into `frontend/dist/`.
The built page is **not** committed: it is a build output, and CI builds it before the Rust, on
Linux for the types and on Windows for the installer.

The palette asks the Rust side for the rows that match what was typed, shows them, and opens the
chosen one.

### The calculator's arithmetic core (crates/tc-calc)

The tokenizer, the expression parser, the evaluation engine, the math functions and the number
formatting, ported from upstream's `Features/Calculator/Model/` with upstream's own
`Tests/calc-test.swift` as the oracle. **66 tests.**

The engine asks a catalog about the units and the currencies it meets and answers
`Unsupported` for a query it cannot finish, so the unit catalog can be added later without
touching the engine. The units, the currencies, the dates, the time zones, the percentages and
the history are still to come.

See [port note 006](docs/port-notes/006-calculator-core.md).

### The system actions (crates/tc-platform-win, apps/yal)

Twenty of the thirty-two system actions are ported and really run on Windows: the session
actions, the media and volume keys, Show Desktop, the light and dark theme, hiding and showing
the windows, quitting them, and the Recycle Bin. The other twelve have no Windows answer — an
absolute volume level, Stage Manager, the notification centre, ejecting a disk, the hidden-files
toggle, the Bluetooth radio and the microphone — and the port says so instead of doing something
else. The actions are rows in the palette under their own section, and the five that upstream
asks about still ask first.

See [port note 007](docs/port-notes/007-system-actions.md).

### The snippet file format and the template engine (crates/tc-snippets)

The snippet record, the Markdown file it is stored in — front matter, the body kept exactly as
written, the rejected files — and the template engine: every placeholder (`{clipboard}`,
`{selection}`, `{date}`, `{uuid}`, `{cursor}`, `{argument …}`, `{snippet:Name}`), every modifier,
the offsets and the formats. **55 tests.** The repository that reads and writes the files, the
keyword expansion, the editor and the browser are not written yet.

See [port note 008](docs/port-notes/008-snippet-file-format.md) and
[port note 009](docs/port-notes/009-snippet-template-engine.md).

### The parity ledger

The ledger's scope is generated (cargo xtask ledger); the port's verdict on each row lives in
parity/status.toml, one entry per row, with the evidence that shows it. The validator refuses a
PORTED row whose evidence names a crate that does not exist or a function that is not there.

19 rows were PORTED first, all on docs/launcher/: the matching rules, the names an entry is
found by, the Latin spellings, the learned ranking, and the quicklink destinations.

With the calculator's arithmetic core, 42 rows were PORTED: 19 on the launcher, 21 on
docs/features/calculator/ and 2 on the favorites slots.

With the system actions, 68 rows were PORTED: the twenty actions Windows can do, the five
that ask for confirmation first, the favorites slots, the calculator's arithmetic core and the
launcher's matching rules.

With the snippet file format and the template engine, 95 rows are PORTED.

The rows that are still open take their reason from `parity/reasons.toml`, one line per page, so
each open row names the piece that is missing rather than repeating `not yet implemented`.

### Where the Tier 1 work stands

The order to work in, with what is done so far:

1. ~~The palette's real interface — search field, result list, keyboard navigation~~ — **done**
2. ~~Launcher search and ranking, and `launcher-ranking.json`~~ — **done**
3. Favorites
4. Aliases
5. Hotkeys — recording, modifier taps, conflicts
6. Commands
7. Fallbacks
8. Quicklinks — the destinations are done; the store, the editor and the section are not
9. System settings
10. System actions — all 32, with their confirmations
11. Calculator — **the arithmetic core is done**: the tokenizer, the parser, the engine and the
    number formatting are ported to `crates/tc-calc` against upstream's own calculator tests.
    The units, the currencies, the dates and the time zones are stage 2.
12. Clipboard history
13. Emoji
14. Snippets — the record, the Markdown file format and the template engine are ported to
    `crates/tc-snippets`; the repository, the keyword policy and the Raycast import are not
15. File search
16. Window management
17. Window layouts
18. Rooms
19. Navigation
20. Notes
21. Calendar
22. Camera
23. Dictionary
24. Dictation
25. Automations
26. The settings screens
27. The shortcuts audit — every row of `docs/SHORTCUTS.md`
28. Deeplinks
29. Backup
30. Import from Raycast
31. Uninstall
32. Updates

## The other port, reviewed

Another AI shipped a separate YAL port (`yal-source-0.1.0.zip`). It was unpacked outside this
repository and checked on this machine. It does not compile, its Windows layer is empty (the
credential store reports success while discarding the secret), its "1,209 passing tests" are a
JavaScript re-implementation that never runs the Rust, and its parity ledger has 1,208 rows
against this port's 2,309, so the two verdicts cannot be compared row for row.

Nothing was salvaged from it. The review, with the commands and their output, is in
[docs/audits/2026-11-11-other-port-review.md](docs/audits/2026-11-11-other-port-review.md).

## Phase 3 - Tier 2

Status: **not started**

## Phase 4 - Hardening and release

Status: **not started**

## Open problems

- `xtask i18n` needs the upstream clone present. The generated string table is committed, so a
  plain build never needs it; only regenerating needs upstream.
- The repository slug is still `OWNER/REPO`. It is written down once, in `tc-core::REPOSITORY`, and
  `Cargo.toml` repeats it because Cargo cannot read a constant.
- The installer has not been signed. The release workflow signs it when the certificate secret is
  set, and signs the checksums with minisign.
- `packaging/winget/YAL.YAML` and `packaging/scoop/yal.json` are templates. `cargo xtask dist`
  fills in the version, the repository slug, the release date and the real artifact checksums, and
  writes the result to `dist/packaging/`. The files in the repository keep their placeholders.
- Building for ARM64 needs the Visual Studio ARM64 cross-compiler. It was installed on this
  machine on 2026-11-10; see [port note 004](docs/port-notes/004-arm64-build.md) for what it
  takes and what does not work.
- The palette's built page is not committed, so a plain `cargo build` on a fresh clone fails
  until the interface has been built. `scripts/build.ps1` builds it first, and CI does too.

## Notes for the next session

- The `read` tool mis-decodes UTF-8 as Latin-1 in this environment. Use `grep`, or
  `[System.IO.File]::ReadAllText($path, [System.Text.Encoding]::UTF8)`.
- NSIS resolves relative paths against the **script's** directory, not the working directory. The
  installer's paths therefore start with `..`.
- `makensis` is not on `PATH` here. `cargo xtask dist` finds it in the standard install location.
- Run `cargo run -p xtask -- parity` after any change to `parity/parts/`, and commit
  `PARITY.md` and `parity/parity.toml` with it.
- `.cargo/config.toml` is gitignored, because it holds machine-specific toolchain paths.
