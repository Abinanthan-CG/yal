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
