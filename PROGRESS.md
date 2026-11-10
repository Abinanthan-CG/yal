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
| NSIS | 3.13 (installed via winget) |
| Network | reachable: tinycast.dev and github.com both answer |

Everything runs natively on real Windows here, so Windows-dependent rows can be
proven locally rather than waiting for CI.

## Upstream reference

- Cloned at `upstream/` (never committed). Commit SHA: `58168bf7e0b721f7c5fbfc7f4e8a068a48cff446`.
- All 42 documentation pages downloaded to `upstream-docs/` (never committed).
- Upstream layout confirmed: 869 Swift files under `Tinycast/`, 127 test files under `Tests/`,
  42 Markdown files under `upstream/docs/`.

## Phase 0 - Recon

Status: **in progress**

Done:
- Environment detected and recorded above.
- Repository created, git initialized, `.gitignore` excludes `upstream/`, `upstream-docs/`, `target/`, `dist/`.
- Upstream cloned, SHA recorded in `upstream-sha.txt` and `upstream-commit.txt`.
- All 42 upstream docs pages downloaded.
- Cargo workspace created with the crate map from the brief.
- `xtask` crate with the `parity` validator and generator, `i18n`, `dist` and `selftest` commands.
- `parity/pages.toml`: canonical inventory of all 42 pages, each with its Tier and its Windows translation.
- `parity/count_checks.toml`: every count check upstream states, with `kind` and `expected_rows`.
- Validator proven to work: it rejects the empty ledger with precise diagnostics.

In progress:
- Parity ledger rows (`parity/parts/*.toml`) being authored in parallel, one worker per page group.

Next:
- Finish the ledger fragments, then `cargo xtask parity` must pass and generate `PARITY.md`.
- `docs/WINDOWS-MAPPING.md` and `docs/SHORTCUTS.md`.

## Phase 1 - Foundation

Status: **not started**

## Phase 2 - Tier 1 vertical slices

Status: **not started**

## Phase 3 - Tier 2

Status: **not started**

## Phase 4 - Hardening and release

Status: **not started**

## Open problems

- `cargo xtask parity` requires every count check to be represented by at least one row, so the
  ledger cannot be generated until the fragments land. This is intentional: it stops a half
  ledger from being published as if it were complete.
- `xtask i18n` needs the upstream clone present. The generated string table is committed, so a
  plain build never needs it; only regenerating needs upstream.
