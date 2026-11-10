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
  `tinycast://` grammar, strict UUIDs, URL encoding. **50 tests.**
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

Status: **not started**

The order to work in:

1. The palette's real interface — search field, result list, keyboard navigation
2. Launcher search and ranking, and `launcher-ranking.json`
3. Favorites
4. Aliases
5. Hotkeys — recording, modifier taps, conflicts
6. Commands
7. Fallbacks
8. Quicklinks
9. System settings
10. System actions — all 32, with their confirmations
11. Calculator
12. Clipboard history
13. Emoji
14. Snippets
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
- `packaging/winget/YAL.YAML` and `packaging/scoop/yal.json` carry placeholder checksums. They
  need the real ones from `dist/SHA256SUMS.txt` before they can be submitted.
- Building for ARM64 needs the Visual Studio ARM64 cross-compiler. It was installed on this
  machine on 2026-11-10; see [port note 004](docs/port-notes/004-arm64-build.md) for what it
  takes and what does not work.

## Notes for the next session

- The `read` tool mis-decodes UTF-8 as Latin-1 in this environment. Use `grep`, or
  `[System.IO.File]::ReadAllText($path, [System.Text.Encoding]::UTF8)`.
- NSIS resolves relative paths against the **script's** directory, not the working directory. The
  installer's paths therefore start with `..`.
- `makensis` is not on `PATH` here. `cargo xtask dist` finds it in the standard install location.
- Run `cargo run -p xtask -- parity` after any change to `parity/parts/`, and commit
  `PARITY.md` and `parity/parity.toml` with it.
- `.cargo/config.toml` is gitignored, because it holds machine-specific toolchain paths.
