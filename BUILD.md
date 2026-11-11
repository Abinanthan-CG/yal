# Building YAL

This document is for people who want to build YAL themselves. If you are
working with an AI coding assistant, use
[`BUILD_WITH_AI.md`](BUILD_WITH_AI.md) instead — it covers the same ground plus
the repository's conventions.

---

## What you need

| | |
|---|---|
| Windows | 10 22H2 or 11, x64 or ARM64 |
| Rust | `1.99.1`, pinned by `rust-toolchain.toml` |
| Visual Studio | 2022 Build Tools with the *Desktop development with C++* workload |
| Windows SDK | 10.0.26100.0 or later |
| Node.js | 20 or later (needed to build the palette's interface) |
| NSIS | 3.x, for the installer |

Everything above is checked by `scripts/bootstrap.ps1`, which reports the
version of each tool and installs what is missing with `winget`.

```powershell
.\scripts\bootstrap.ps1
```

The script is idempotent: run it as often as you like. It never installs
anything twice and it never upgrades a toolchain that is already correct.

---

## The one command build

```powershell
.\scripts\build.ps1
```

That runs, in order:

1. `cargo fmt --all -- --check`
2. `npm ci && npm run build` in `frontend/` — the palette
3. `cargo clippy --workspace --all-targets -- -D warnings`
4. `cargo test --workspace`
5. `cargo run -p xtask -- parity --check`
6. `cargo run -p xtask -- dist`

and leaves the artifacts in `dist/`.

Useful switches:

```powershell
.\scripts\build.ps1 -SkipTests      # skip the test suite
.\scripts\build.ps1 -Arch x64       # one architecture only
.\scripts\build.ps1 -Channel beta   # the beta channel
```

---

## The artifacts

`cargo xtask dist` produces, for each architecture:

| File | What it is |
|---|---|
| `dist/yal-setup-<version>-x64.exe` | The NSIS installer |
| `dist/yal-<version>-x64-portable.zip` | The portable build |
| `dist/SHA256SUMS.txt` | The SHA-256 of every artifact |

The installer is per user: `RequestExecutionLevel user`, no administrator
rights, nothing written outside `%LOCALAPPDATA%`.

---

## Building without the wrapper

```powershell
# The release binaries
cargo build --release --workspace

# Just the installer for one architecture
cargo run -p xtask -- dist --arch x64

# A headless check of every subsystem on this machine
.\target\release\yal.exe --self-test
```

`--self-test` prints one line per subsystem and exits non-zero if any of them
fails. It is the fastest way to find out whether a change broke something on
this machine.

---

## The parity ledger

The ledger is not hand-written. It is generated from the upstream
documentation and validated on every build.

```powershell
cargo run -p xtask -- ledger          # regenerate parity/parts/generated/
cargo run -p xtask -- parity          # merge, validate, write PARITY.md
cargo run -p xtask -- parity --summary # a one-line count
cargo run -p xtask -- parity --check   # fail if PARITY.md is out of date
```

`--check` is what CI runs. If you change a row's status or its evidence, run
`cargo run -p xtask -- parity` and commit the result.

---

## Cross-compiling for ARM64

The `aarch64-pc-windows-msvc` target is installed by `bootstrap.ps1`. To build
it you need the ARM64 build tools; `cargo xtask dist` uses the right
toolchain automatically when both are present.

```powershell
rustup target add aarch64-pc-windows-msvc
cargo run -p xtask -- dist --arch arm64
```

If the ARM64 linker is missing, install the *MSVC v143 ARM64 build tools*
component of the Visual Studio Build Tools.

---

## The palette's interface

The palette is a web view: Solid and TypeScript, built by Vite from
`frontend/src/`. Tauri loads the built page from `frontend/dist/`, which is
**not** committed — it is a build output, and a committed one goes stale.

Build it before the Rust build:

```powershell
cd frontend
npm ci
npm run build
```

`npm run check` is the type check on its own, without a build.

CI builds the palette twice: once on Linux, where the types are checked and the
page is built, and once on Windows, where the built page is what the installer
is actually made from.

If you change the interface and only run `cargo build`, the window will show
the old page. Build the palette first.

---

## Running the installer smoke test

```powershell
cargo run -p xtask -- dist --arch x64
.\scripts\smoke-test.ps1
```

The script installs YAL silently, runs the self-test, starts the app, checks
that the palette window appears, checks that a second launch hands over to the
first, asks the running instance to quit, uninstalls silently, and checks that
the program files are gone while your data folder survives.

---

## Common problems

**`cargo` cannot find `link.exe`.**
The Visual Studio Build Tools are not installed, or the build tools are not
selected in the "Tools → Visual Studio → Install → Available" list. Run
`scripts/bootstrap.ps1`.

**`makensis` is not recognized.**
NSIS is not installed. Run `scripts/bootstrap.ps1`, or install it with
`winget install --id NSIS.NSIS -e`.

**`cargo fmt` reports differences.**
Run `cargo fmt --all` and commit the result.

**The self-test fails on `foreground window`.**
A secure desktop is active — for example the UAC prompt or the lock screen.
Run the self-test from a normal desktop session.

**The installer cannot replace `yal.exe`.**
A YAL instance is still running. `yal.exe --quit-running` asks it to close; the
installer does this for you.
