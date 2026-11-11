# Architecture

> How YAL's code is arranged, and why.

---

## The shape

```text
                        ┌──────────────────────────────────┐
                        │  apps/yal                        │
   the palette  ──────► │  the Tauri shell                 │
                        │  window, tray, hotkey, instance  │
                        └───────────────┬──────────────────┘
                                        │ commands, events
                        ┌───────────────▼──────────────────┐
                        │  crates/tc-*                     │
                        │  the domain                      │
                        │  commands, search, calculator,   │
                        │  snippets, clipboard, files, ... │
                        └───────────────┬──────────────────┘
                                        │ traits
                        ┌───────────────▼──────────────────┐
                        │  crates/tc-platform              │
                        │  the traits, and their fakes     │
                        └───────────────┬──────────────────┘
                                        │ implemented by
                        ┌───────────────▼──────────────────┐
                        │  crates/tc-platform-win          │
                        │  the Win32 implementation        │
                        └──────────────────────────────────┘
```

The rule that holds this together: **nothing but `tc-platform-win` talks to
Windows.**

---

## The crates

### `tc-core` — the domain

Pure. No Windows, no I/O beyond what a test needs.

| Module | What it holds |
|---|---|
| `command.rs` | The 46 built-in commands, their upstream identifiers (`command:ai-chat` … `command:quit`), their names, and whether they are query-driven or shortcut-exempt |
| `entry.rs` | The launcher-row identifier scheme and the 32 system actions with their categories and confirmations |
| `deeplink.rs` | The `tinycast://` grammar, ported from upstream's `DeepLink.swift` and `DeepLinkInput.swift` |
| `ids.rs` | Strict UUID parsing and formatting |
| `url.rs` | Percent encoding and decoding, and query parsing |

50 tests. Everything here is decided by upstream's behaviour, and every
decision is a test.

### `tc-platform` — the traits

The seams between the domain and the machine.

| Trait | What it abstracts |
|---|---|
| `Paths` | Where the data, the cache and the settings file live |
| `WindowSystem` | Listing windows, reading their geometry, moving them |
| `Clipboard` | Reading and writing text |
| `Hotkey` | Installing and removing a global shortcut |
| `SingleInstance` | Taking and releasing the one-instance lock |
| `Clock` | The current time |
| `FileSystem` | Reading and writing files, and making directories |
| `Settings` | A keyed store of settings |

Every trait has an in-memory fake in `fakes.rs`. The fakes are not a
convenience: they are how the domain is tested without a desktop, which is
what lets CI run the pure crates on Linux in seconds.

### `tc-platform-win` — the Win32 implementation

The only crate that imports the `windows` crate.

| Module | What it wraps |
|---|---|
| `paths.rs` | `%APPDATA%\yal`, `%LOCALAPPDATA%\yal`, the settings file, the portable marker |
| `clipboard.rs` | `CF_UNICODETEXT` through `OleGetClipboard` and `OleSetClipboard` |
| `windows.rs` | The foreground window, `EnumWindows`, the token's integrity level, `DwmGetWindowAttribute` |
| `hotkey.rs` | `WH_KEYBOARD_LL` |
| `single_instance.rs` | The named mutex |
| `quit.rs` | The named quit event |

6 tests, all run on real Windows.

### The feature crates

One crate per feature area, so that a change to the calculator cannot break
the clipboard, and so that CI can test them independently:

`tc-search`, `tc-apps`, `tc-calc`, `tc-snippets`, `tc-clipboard`, `tc-files`,
`tc-windows`, `tc-calendar`, `tc-backup`, `tc-i18n`, `tc-ai`, `tc-mcp`,
`tc-extensions`.

### `apps/yal` — the shell

The Tauri application.

| File | What it does |
|---|---|
| `main.rs` | The command line: `--self-test`, `--version`, `--help`, and everything else goes to the shell |
| `shell.rs` | The palette window, the tray, the global shortcut, the single-instance plugin, the deep-link plugin, and the quit watcher |
| `selftest.rs` | Twelve subsystem checks, run on the machine the binary is on |

### The helper processes

`apps/yal-indexer`, `apps/yal-ocr` and `apps/yal-dictation` are plain
command-line programs that print JSON.

They are separate processes on purpose:

- a scan of a million files must not block the palette,
- a 32-megabyte document must not block the palette,
- a 1.7-billion-parameter model must not bloat the launcher,
- and a crash in any of them must not take the app down.

Each one is started by the launcher and its standard output is read as JSON.

### `xtask` — the build tool

| Command | What it does |
|---|---|
| `cargo xtask parity` | Merge the parts, validate them, write `parity/parity.toml` and `PARITY.md` |
| `cargo xtask parity --summary` | Print the counts |
| `cargo xtask parity --check` | Fail if the committed output is out of date |
| `cargo xtask ledger` | Generate `parity/parts/generated/` from the upstream documentation |
| `cargo xtask tags` | Print the rows a count check binds to |
| `cargo xtask i18n` | Regenerate the string catalogue from upstream's `Localizable.xcstrings` |
| `cargo xtask dist` | Build the release binaries, the installer, the portable zip and the checksums |
| `cargo xtask selftest` | Run the self-test |

### The frontend

`frontend/src/` is the palette's interface: Solid and TypeScript, built by
Vite into `frontend/dist/`. Tauri loads the built page from there, and the
built page is **not** committed — it is a build output, and CI builds it
before the Rust.

The frontend talks to the Rust side through four commands and nothing else:
`search`, `suggestions`, `open_result` and `hide_palette`.

---

## The parity ledger

This is the part of the architecture that is unusual, so it is worth
explaining.

```text
upstream documentation  ──►  cargo xtask ledger  ──►  parity/parts/generated/*.toml
                                                                       │
                                                     cargo xtask parity│
                                                                       ▼
                            parity/parity.toml  ◄──── merge & validate
                                    │
                                    ▼
                               PARITY.md
```

### Why it is generated

Hand-writing 2 309 rows is how you get 2 309 rows that are subtly wrong. The
generator reads the documentation's headings, tables, list items and
paragraphs, and produces one row per behaviour. The count is then checked
against the counts upstream states.

### Why it is validated

The validator is the thing that keeps the ledger honest. It checks that:

- a status is exactly `PORTED` or `NOT PORTED`, and nothing else;
- every row's identifier is unique;
- every row is on a page that exists in `parity/pages.toml`;
- a row's tier agrees with its page's tier;
- every heading in the generated fragment is covered by a row;
- a `PORTED` row names evidence that is a real test in a real crate;
- a `NOT PORTED` row has a reason;
- a count check of kind `rows` binds exactly the number of rows it claims;
- an `asserted` count check has a test that proves the number.

An `asserted` check with no test is reported as **pending**. It is never
reported as passed.

---

## The decisions

They are in [`DECISIONS.md`](DECISIONS.md), with the date and the reasoning.
The ones that shape the code most:

1. **The ledger is generated, not hand-authored.**
2. **Count checks have two kinds**, `rows` and `asserted`, and they mean
   different things.
3. **The count-check definitions live in `parity/count_checks.toml`**, not in
   the ledger.
4. **The Windows translation is data**, in `parity/pages.toml`, next to the
   tier, so the validator can check a row against its page.
5. **The TOML output is deterministic**, written by a hand-rolled serialiser,
   so `--check` can compare bytes.
6. **Real Windows is available**, so Windows-dependent rows can be proven
   locally rather than deferred to CI.

---

## Data flow

```text
the user presses the hotkey
        │
        ▼
the keyboard hook ──► the palette window is shown
        │
        ▼
the user types
        │
        ▼
the frontend ──► a command ──► the domain crates
                                    │
                                    ▼
                            the platform traits
                                    │
                                    ▼
                            tc-platform-win
                                    │
                                    ▼
                                Windows
```

The frontend never talks to Windows. The domain never talks to Windows. Only
the platform layer does, and it does so through traits that have fakes.

---

## Error handling

- The domain returns `Result` with errors that say what failed and what was
  being done.
- The platform layer returns `Result<_, String>`, because the message is
  what the user sees.
- The shell turns an error into a message and an exit code. There is no
  `unwrap()` in a path that a user can reach, and no `panic!()` that a user
  can cause.
- A helper process that fails reports the failure on its standard error and
  exits non-zero. The launcher treats that as "no result", not as a crash.

---

## See also

- [`docs/WINDOWS-MAPPING.md`](WINDOWS-MAPPING.md) — the translation table
- [`docs/SHORTCUTS.md`](docs/SHORTCUTS.md) — every shortcut
- [`docs/DECISIONS.md`](docs/DECISIONS.md) — the decisions
- [`docs/port-notes/`](port-notes/) — notes on the difficult parts
- [`BUILD_WITH_AI.md`](../BUILD_WITH_AI.md) — the working rules
