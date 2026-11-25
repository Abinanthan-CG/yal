# Building YAL

---

## Requirements

| Tool | Version |
|---|---|
| Windows | 10 22H2 or 11 (`x64` or `arm64`) |
| Rust | `1.99.1` (pinned by `rust-toolchain.toml`, MSRV `1.89`) |
| Visual Studio | 2022 / 2026 Build Tools (*Desktop development with C++*) |
| Windows SDK | 10.0.26100.0 or later |
| NSIS | 3.x (only required for building the installer) |

You can install missing Windows build prerequisites with:

```powershell
.\scripts\bootstrap.ps1
```

---

## Quick build & self-test

```powershell
cargo build --release --bin yal
.\target\release\yal.exe --self-test
```

`--self-test` verifies all 18 local subsystems (commands, system actions, deep links,
window enumeration, keyboard hook, single-instance lock, app index, search, displays,
and window mover) and exits non-zero if any check fails.

---

## Full release build (installer + portable zip)

```powershell
.\scripts\build.ps1 -Arch x64
```

This runs, in order:
1. `cargo fmt --all -- --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo test --workspace`
4. `cargo run --quiet -p xtask -- dist --arch x64`

Artifacts are written to `dist/`:

| Artifact | Description |
|---|---|
| `dist/yal-setup-<version>-x64.exe` | Per-user NSIS installer (`RequestExecutionLevel user`) |
| `dist/yal-<version>-x64-portable.zip` | Standalone portable build |
| `dist/SHA256SUMS.txt` | SHA-256 checksums for verification |

Useful switches:

```powershell
.\scripts\build.ps1 -SkipTests      # skip running cargo test
.\scripts\build.ps1 -Arch x64       # x64 only
.\scripts\build.ps1 -Arch arm64     # ARM64 cross-compile (requires MSVC ARM64 tools)
```

---

## Installer smoke test

```powershell
cargo run --quiet -p xtask -- dist --arch x64
.\scripts\smoke-test.ps1
```

Silently installs YAL, validates `tinycast://` registration, runs `--self-test`,
launches the palette, checks single-instance handover, quits cleanly, silently
uninstalls, and confirms `%APPDATA%\yal` survives untouched.

---

## Pure-logic checks on Linux / macOS

```bash
./scripts/build.sh
```

Formats, lints, and tests the platform-independent logic crates (`tc-core`,
`tc-platform`, `tc-apps`, `tc-calc`, `tc-search`, `tc-snippets`).

---

## Troubleshooting

- **`cargo` cannot find `link.exe`:** Install the *Desktop development with C++* workload in Visual Studio Build Tools.
- **`makensis` is not found:** Install NSIS (`winget install --id NSIS.NSIS -e`).
- **Installer cannot replace `yal.exe`:** Close any running instance with `yal.exe --quit-running`.
