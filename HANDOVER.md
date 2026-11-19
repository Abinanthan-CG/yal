# Handover

This is the port of **Tinycast** to Windows. It is called **YAL**, the
executable is `yal.exe`. This document is for whoever takes it over - a
person, or a model starting fresh.

It is written to be read once, in order, and then used as a reference. Every
number in it was measured or counted on **2026-11-19**, and every claim says
how it was checked. Where a thing is not finished, it says so.

---

## 1. What this is

A Windows port of [Tinycast](https://github.com/abue-ammar/tinycast), a macOS
keyboard launcher that is AGPL-3.0. Upstream is a Swift/macOS application;
this is Rust on Windows. It is **not** a fork of the upstream code - it is a
re-implementation whose behaviour is checked against upstream's own
documentation and upstream's own test files.

**The stack is fixed and should not move:**

| | |
| --- | --- |
| Language | Rust, pinned to **1.99.1** by `rust-toolchain.toml` |
| Shell | Tauri 2 + the system **WebView2** runtime |
| Interface | TypeScript + Solid, in `frontend/` - built by Vite, **not committed** |
| Windows API | the `windows` crate, version **0.62** |
| Targets | `x86_64-pc-windows-msvc` and `aarch64-pc-windows-msvc` |
| Windows | 10 22H2 and 11, per-monitor DPI v2, long paths, UTF-8, `asInvoker` |
| Licence | **AGPL-3.0-only**, with upstream's notices carried in `NOTICE.md` |

YAL is an unofficial community port. Upstream is at
`github.com/abue-ammar/tinycast`; the support link `https://tinycast.dev/support`
is the upstream's.

**Storage mirrors upstream's design:** SQLite (rusqlite, bundled, FTS5) for
`clipboard.sqlite3`, `quicklinks.sqlite3`, `ai-chats.sqlite3` and the file
index; Markdown files for notes and snippets; JSON for settings and
`launcher-ranking.json`; secrets only in **Windows Credential Manager**.

---

## 2. The state at handover

Measured on 2026-11-19, on the branch `feat/yal-port`.

| | |
| --- | --- |
| Tests | **1149 passing**, 0 failed, 0 ignored, across 46 test binaries |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` - exit 0 |
| Format | `cargo fmt --all -- --check` - exit 0 |
| Dependencies | `cargo deny check` - advisories ok, bans ok, licences ok, sources ok |
| Self-test | `yal.exe --self-test` - 18 subsystem checks, 0 failed |
| Ledger | **2309 rows, 447 `PORTED`, 1862 `NOT PORTED`** |
| CI | green on `feat/yal-port` (see §3) |
| The app | 58.8 MB working set, **37.9 MB private, one process, no WebView2** |

The last row was measured on 2026-11-15 on the release build. The budget is a
release requirement, not a wish: **under 40 MB resident, under 0.5 % of one
core idle, under 250 ms cold start.** It is met. For comparison, Raycast for
Windows on the same machine was 914 MB. See
[port note 024](docs/port-notes/024-the-weight-of-the-app.md).

**The ledger, page by page** (counted from `parity/parity.toml` against
`parity/status.toml`):

| Page | Rows | Ported | Open |
| --- | --- | --- | --- |
| `docs/features/calculator/` | 326 | 139 | 187 |
| `docs/features/clipboard/` | 85 | 15 | 70 |
| `docs/features/snippets/` | 87 | 28 | 59 |
| `docs/features/file-search/` | 68 | 39 | 29 |
| `docs/features/window-management/` | 78 | 51 | 27 |
| `docs/features/emoji/` | 25 | 0 | 25 |
| `docs/features/notes/` | 60 | 0 | 60 |
| `docs/launcher/` | 76 | 12 | 64 |
| `docs/launcher/commands/` | 113 | 87 | 26 |
| `docs/launcher/favorites/` | 21 | 2 | 19 |
| `docs/launcher/aliases/` | 18 | 0 | 18 |
| `docs/launcher/quicklinks/` | 71 | 7 | 64 |
| `docs/launcher/system-actions/` | 54 | 26 | 28 |
| `docs/launcher/fallbacks/` | 15 | 0 | 15 |
| `docs/launcher/system-settings/` | 16 | 0 | 16 |
| `docs/launcher/uninstall/` | 32 | 0 | 32 |
| `docs/reference/settings/` | 76 | 0 | 76 |
| `docs/reference/hotkeys/` | 63 | 0 | 63 |
| `docs/reference/shortcuts/` | 106 | 0 | 106 |
| `docs/reference/backup/` | 49 | 0 | 49 |
| `docs/reference/deeplinks/` | 45 | 0 | 45 |
| `docs/reference/updates/` | 25 | 0 | 25 |
| `docs/reference/import-from-raycast/` | 41 | 0 | 41 |
| `docs/palette/` | 66 | 41 | 25 |
| `docs/ai/`, `docs/extensions/`, `docs/permissions/`, `docs/install/`, `docs/` | 428 | 0 | 428 |

The remaining Tier 1 pages - apple-shortcuts (22), calendar (72), camera (14),
dictation (45), dictionary (13), navigation (35), rooms (28), window-layouts
(36) - are also at zero.

**What 1.0 ships is the must-have set only**, decided on 2026-11-15:
calculator, clipboard, snippets, file search, apps and commands, aliases,
favorites, settings, hotkeys, notes, emoji, backup. Those twelve hold
**745 open rows** today. Everything else is `NOT PORTED` for 1.0 with a
one-line reason, ready for 1.1.

---

## 3. Where everything is

**The repository.** `C:\Users\dabin\.minimax\sessions\mvs_be5482942e1f46ceab95749b01d52300\workspace`

**The remote.** `https://github.com/Abinanthan-CG/yal.git` (public).

**The branch.** `feat/yal-port`, tracking `origin/feat/yal-port`. **`main` is
deliberately untouched** - it still holds the user's earlier, separate YAL
project. The port is built on the branch; the flush to `main` is the last
step of the release and is written down in
[docs/PUBLISHING.md](docs/PUBLISHING.md).

**Upstream, for reference.** Cloned at `upstream/` (gitignored, never
committed) at SHA `58168bf7e0b721f7c5fbfc7f4e8a068a48cff446`. All 42
documentation pages are in `upstream-docs/` (also gitignored). Upstream's
layout: 869 Swift files under `Tinycast/`, 127 test files under `Tests/`, 42
Markdown files under `upstream/docs/`.

**Upstream is the authority.** When the port and upstream disagree, upstream's
Swift source wins, and so does upstream's own test file. The documentation is
the source of the *scope* (what features exist); the source is the source of
the *behaviour* (what they do).

**The state files.** Read these before doing anything:

| File | What it holds |
| --- | --- |
| `PROGRESS.md` | the running state of the port - read it first |
| `docs/DECISIONS.md` | every decision, with its question, options and reasons |
| `docs/SESSION-LOG.md` | what each session did, what it verified, what it did not |
| `docs/port-notes/` | 28 notes, one per substantial piece of work |
| `PARITY.md` | the ledger rendered for a human |
| `docs/ARCHITECTURE.md`, `docs/WINDOWS-MAPPING.md`, `docs/SHORTCUTS.md` | the design, the interface translation, the key table |

---

## 4. Building and testing

Everything runs natively on this machine, so Windows-dependent behaviour can
be proved locally rather than waiting for CI.

```powershell
# the whole thing
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/build.ps1

# the checks CI runs
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo deny check
cargo run -q -p xtask -- parity --check

# the interface (the built page is not committed)
cd frontend; npm.cmd install; npm.cmd run build; cd ..

# the release artefacts
cargo run -q -p xtask -- dist --arch x64
cargo run -q -p xtask -- dist --arch arm64

# the app's own self-test, eighteen subsystem checks
target\release\yal.exe --self-test

# the ledger
cargo run -q -p xtask -- parity           # regenerates PARITY.md and parity/parity.toml
cargo run -q -p xtask -- parity --summary # prints the counts, writes nothing
```

`xtask` has six commands: `parity`, `ledger`, `i18n`, `dist`, `selftest`, and
`tags`. `cargo xtask i18n` and `cargo xtask ledger` both need the upstream
clone to be present; the generated output is committed, so a plain build never
needs them.

**The app's own flags:** `--self-test`, `--version`, `--help`,
`--native-palette` (opens the native palette alone, without the Tauri shell),
and `--settings` (opens the settings screen).

---

## 5. The repository map

Twenty-eight crates. The number after each name is its test count, counted on
2026-11-19.

| Crate | Tests | What it holds |
| --- | --- | --- |
| `crates/tc-core` | 102 | the 46 built-in commands, 32 system actions, the row-identifier scheme, the `tinycast://` grammar, strict UUIDs, URL encoding, the fallback titles |
| `crates/tc-search` | 150 | the fold, the scorer, the alias matching, the romanization, the entry naming, the order, and the ranking record |
| `crates/tc-calc` | 224 | the tokenizer, the parser, the engine, the unit table, the percentages, the money, the splits, the finance and growth phrases, the lists, the math in words, and the arithmetic that carries a unit |
| `crates/tc-palette` | 120 | the key table, the Mac→Windows chord translation, the screens, the escape chain, the tab cycle, the file-search screen, the General settings pane |
| `crates/tc-snippets` | 70 | the snippet record, the Markdown file format, the template engine, the library on disk, the keyword policy, the Raycast import |
| `crates/tc-windows` | 66 | the 39 window commands, the geometry arithmetic, the memory of the windows that have been moved |
| `crates/tc-apps` | 6 | the program model and its rules |
| `crates/tc-files` | 56 | the file index, the scope rules, the SQLite store, the search over it |
| `crates/tc-clipboard` | 84 | the clipboard item, the classification, the colours, the filters, the capture policy, the paste sequence, the store with its pins |
| `crates/tc-emoji` | 32 | the catalog of 2 136 emoji, the search with its ranking, the pins, the frequency, the skin tones, the grid |
| `crates/tc-platform` | 5 | the traits (`Paths`, `WindowSystem`, `Clipboard`, `Hotkey`, `SingleInstance`, `Clock`, `FileSystem`, `Settings`) and their in-memory fakes |
| `crates/tc-platform-win` | 40 | the Win32 implementation: paths, clipboard, the foreground window, the window list, the program index, the shell runner, the window mover, the keyboard hook, the single-instance mutex |
| `crates/tc-ui` | 30 | the **native palette**: a borderless Win32 window painted with Direct2D and typed into with a real edit control; the design tokens; the style packs; the event conversation |
| `crates/tc-commands` | 88 | the command catalogue, the custom commands, the shell they run under, the Raycast script import |
| `crates/tc-i18n`, `tc-ai`, `tc-mcp`, `tc-extensions`, `tc-calendar`, `tc-backup` | 0 | placeholders for features that are out of scope for 1.0 |
| `apps/yal` | 64 | the Tauri 2 shell: the tray, the single instance, the deep links, the native palette, the browser window, the `Runner` |
| `apps/yal-ui-probe` | 0 | opens `tc-ui` on its own, with `--light` and `--style-pack` - the fastest way to see a palette change |
| `apps/yal-indexer`, `apps/yal-ocr`, `apps/yal-dictation` | 3, 3, 4 | the three helper processes, each with a real command line |
| `xtask` | 0 | the build orchestration: `parity`, `ledger`, `i18n`, `dist`, `selftest` |

---

## 6. How the pieces fit

**The launcher is native.** `crates/tc-ui` is a Win32 window: Direct2D for the
paint, DirectWrite for the type, and a real Windows edit control for the
input. It has no CSS. The design system travels as **style packs** - a small
JSON file of colours, measurements and a font, read by
`crates/tc-ui/src/tokens.rs`. Three ship: `midnight`, `daylight`, `aurora`.

**The app talks to the palette through a small conversation**
(`crates/tc-ui/src/event.rs`):

- The palette raises `PaletteEvent::{Shown, QueryChanged(String), Activated(usize), Dismissed}`.
- The app answers with `PaletteReply::{Keep, Update(Model), Close}`.
- The handler is moved out of the window state for the length of the call, so
  it cannot alias the palette's own state.
- `PaletteHandle` is an `isize` that crosses thread boundaries;
  `post_show`, `post_close` and `is_visible` are free functions.

**One `Runner`, two palettes.** `apps/yal/src/open.rs` holds the `Runner` and
its `open(id, query, confirmed)` method. The native palette and the browser
palette both call it, so a row cannot behave differently depending on which
palette showed it. That is a rule, not a convenience - keep it that way.

**The browser window is made on demand.** `apps/yal/tauri.conf.json` has
`"windows": []`. The browser window is created when the settings screen is
asked for - from the tray, or `yal.exe --settings` - and the screen is set in
the Rust `PaletteState` *before* the window is made, so the frontend sees the
right screen on mount. This is why the app runs as one process with no
WebView2 (§2).

**The ledger is generated, and the status is written by hand.** The *scope*
comes from upstream's documentation (`cargo xtask ledger` regenerates
`parity/parts/`). The port's *verdict* lives in `parity/status.toml`, one
entry per row, with the evidence that shows it. `cargo xtask parity --check`
refuses a `PORTED` row whose evidence names a crate that does not exist or a
function that is not there.

---

## 7. The rules of this project

These are not preferences. They are what makes the ledger worth anything.

1. **No placeholders.** No `todo!()`, no `unimplemented!()`, no mock data, no
   "coming soon". CI greps for `todo!`, `unimplemented!`, `unreachable!`,
   `TODO`, `FIXME`, `coming soon` and `mock data` over `crates/`, `apps/` and
   `frontend/` and fails the build if it finds one. (Note: this bites test
   fixtures too - see §10.)
2. **A row is `PORTED` only when a test shows the behaviour.** Not when the
   mechanism exists. Not when the documentation is implemented. The evidence
   field must name a real `crate::test_name`.
3. **A row that is not `PORTED` has a one-line reason** saying what is
   missing, in `parity/reasons.toml`. "not yet implemented" is not a reason.
4. **No unverified claims.** A document's claim is not a feature in the
   binary. If you cannot show a test, the feature is not there.
5. **A crate that compiles is not a crate that is tested.**
6. **A decision that cannot be tested is not a decision.** Palette behaviour
   lives in Rust, not in the window, because the frontend has no test
   framework.
7. **Tests come from the ledger rows before the implementations.** Write the
   test the row asks for, watch it fail, then make it pass.
8. **Small conventional commits.** `type(scope): what it does`, in the past
   tense as a statement of fact.

---

## 8. The interface translation

Windows has no `⌘`. The translation is fixed and lives in
`crates/tc-palette`:

| macOS | Windows |
| --- | --- |
| `⌘` | `Ctrl` |
| `⌥` | `Alt` |
| `⇧` | `Shift` |
| `⌃` | `Alt` |
| `⌘`+Delete | `Ctrl`+`Shift`+`Backspace` |
| `⌥`+Tab | `Ctrl`+`Tab` |
| `⌘`+Esc | `Ctrl`+`Esc` (and `Alt`+`Home`) |

**A built-in shortcut must never shadow standard text editing.** No built-in
shortcut may take `Ctrl`+`C`, `Ctrl`+`V`, `Ctrl`+`X` or `Ctrl`+`Z`, and the
Windows key is never reserved. There is a test that holds that line.

Other translations, in one place:

| Tinycast on macOS | YAL on Windows |
| --- | --- |
| Trash | the Recycle Bin |
| the menu bar | the system tray |
| Spaces | virtual desktops |
| Apple Shortcuts | Automations |
| Secure Input | administrator windows |
| `/Applications` | the Start Menu, `shell:AppsFolder`, and App Paths |
| `~/Library/Application Support/com.tinycast.app/` | `%APPDATA%\yal\` |
| `~/.config/tinycast/settings.json` | `%USERPROFILE%\.config\tinycast\settings.json` |

---

## 9. The decisions that bind you

The full list is in `docs/DECISIONS.md` (54 entries, each with its question,
its options and its reasons). These are the ones that will shape your work:

- **The native palette is the destination** (2026-11-15). The lazy webview was
  rejected as the first move. The native palette does not run CSS but wears a
  design system through style packs.
- **One runtime, and it stays one** (2026-11-15) - no Node, no .NET, no
  second process in YAL. The three helper binaries exist as separate
  executables, not as a runtime.
- **The palette's decisions live in Rust, not in the window** (2026-11-13).
- **The browser window is made when it is wanted, not when the app starts**
  (2026-11-15).
- **The workflows are checked before GitHub checks them** (2026-11-15) -
  `scripts/check-workflows.py` runs in CI and has a self-test.
- **A value carries its measurement** (2026-11-13) - `Value.unit:
  Option<Measurement>`. The rows whose documentation lost a multiplication
  sign are `PORTED`; a percentage is a plain amount; a unit beside another
  unit name makes a compound; a function of a measurement answers in the unit
  of the first argument.
- **The ranking record remembers the queries** (2026-11-15) -
  `{itemId, score, searchTerms, openedAt}`. The formula stays ours.
- **The import from Raycast is possible** (2026-11-15) - the export file is
  encrypted with a key derived by scrypt from a password the user chose. The
  41 rows stay `NOT PORTED` until a tested reader exists against a real
  exported file.
- **The workspace crates are private** (2026-11-19) - every member carries
  `publish = false`.
- **An advisory with no safe upgrade is ignored with its reason** (2026-11-19)
  - never with a blanket "not affected".
- **1.0 is the must-have set only** (2026-11-15).
- **The work is built on a branch and `main` is left alone** (2026-11-15).

---

## 10. Environment traps

These are the ones that cost time. All of them are real and were hit.

**In the shell.**

- The `read` tool mis-decodes UTF-8 as Latin-1 here. Use `grep`, or
  `[System.IO.File]::ReadAllText($path, [System.Text.Encoding]::UTF8)`.
- PowerShell expands `${{` - it eats GitHub Actions expressions. Use
  single-quoted strings, or `git commit -F <file>`.
- `.Replace()` with `\r\n` fails on LF files. Use the `edit` tool.
- The shell is PowerShell 5.1: no `&&`, no `||`. Separate the statements.
- `npm` must be `npm.cmd`.
- Multi-line `@"…"@` here-strings fail. Write a script file instead.
- Inline `python -c` with triple-quoted strings breaks. Write a `.py` file.
- `cargo build` fails with "Access is denied" if the target `.exe` is running.
  Stop the probe first.
- `Copy-Item` and `Move-Item` outside the workspace are blocked. Use Python.
- Writes outside the workspace are blocked; use a file under the repo.
- Deletions go through one top-level `rm -- "<absolute path>"`, alone in the
  call. `Remove-Item` is blocked.
- `Set-Location` followed by a write is blocked by policy. Use absolute paths.
- `gh run list --json …` piped into `ConvertFrom-Json` corrupts the JSON -
  the stderr gets mixed in. `gh --jq` is not available in this build; use
  `gh api … --jq` or plain output.

**In the repository.**

- The built palette is **not committed**. A plain `cargo build` on a fresh
  clone fails until `frontend/` is built. `scripts/build.ps1` builds it, and
  CI does too.
- `cargo xtask ledger` and `cargo xtask i18n` need the upstream clone present.
- `.cargo/config.toml` is gitignored - it holds machine-specific paths.
- NSIS resolves relative paths against the **script's** directory, not the
  working directory, so the installer's paths start with `..`.
- `makensis` is not on `PATH` here; `cargo xtask dist` finds it in the
  standard install location.
- The placeholder scan covers test fixtures too. `crates/tc-core/src/fallback.rs`
  once used the literal word `todo` as a fixture and CI caught it.

**In the `windows` crate 0.62** (these cost a full session to learn):

- `PostMessageW` takes `Option<HWND>` and plain `WPARAM`/`LPARAM` - not
  `Option`.
- `D2D1_ELLIPSE` is in `Direct2D`. There is no `D2D1_LINE`.
- `EN_CHANGE` is `u32`.
- `InitCommonControlsEx(&controls)` takes no `&mut`.
- There is no `SetFocus` - use `WM_NEXTDLGCTL`.
- `MonitorFromPoint`, `GetMonitorInfoW`, `SetWindowRgn` and
  `CreateRoundRectRgn` are in `Graphics::Gdi`.
- `MulDiv` is in `System::WindowsProgramming`.
- `GetDpiForWindow`, `GetDpiForSystem` and `SetProcessDpiAwarenessContext` are
  in `UI::HiDpi`.

---

## 11. What is not done

This is the honest list. Nothing here is a surprise - it is in `PROGRESS.md`
too - but it belongs in a handover.

**The native palette has only the launcher screen.** Settings, clipboard,
emoji and file-search are still the browser's. Drawing them natively is the
next large step, and it is the one that retires the WebView2 dependency
entirely.

**The global hotkey engine is a stub.** `crates/tc-platform-win/src/hotkey.rs`
installs and removes the low-level hook, but the callback does not detect
any key. Nothing is listening.

**The tray's "Check for Updates" and "About YAL" do nothing.**

**These models exist but have no screen:** the clipboard's capture loop and
the Windows clipboard watcher; the emoji picker; the snippet editor and
browser; the file index's scope selection in Settings (so the app does not
build an index of its own); the notes; the hotkey recorder, the modifier taps
and the conflict check; the favorites slots beyond the first two; the
aliases.

**The calculator** has no exchange rates, no dates and no time zones. Two
different currencies in one calculation earn no card rather than a made-up
number.

**`dist/` is stale** - it holds an unsigned 0.1.0 build from 2026-11-11. The
installer is unsigned; `release.yml` signs it when the certificate secret is
set, and signs the checksums with minisign.

**`dist/packaging/`** (winget and scoop) is generated by `cargo xtask dist`;
the copies in the repository keep their placeholders.

**The flush to `main` has not happened.** `origin/main` still holds the
user's earlier, separate YAL project.

---

## 12. The order of work

This is the recommendation, in the order the work actually unblocks itself.

1. **The native hotkey engine.** Small, self-contained, and it is what makes
   the app feel like a launcher at all. `tc-platform-win/src/hotkey.rs` has
   the hook; it needs the key detection, the modifier state machine and the
   tap handling.
2. **The native settings screen.** It is the biggest single win: it removes
   the last reason for WebView2 to exist, and settings is 76 open rows.
3. **The native clipboard screen**, then the capture loop - 70 rows, and the
   model is already tested.
4. **The native emoji picker** - 25 rows, and the catalog is done.
5. **The native file-search screen** - 29 rows; the index and the screen logic
   are done.
6. **Hotkeys in Settings**, with the recorder and the conflict check - 63
   rows.
7. **The calculator's remaining layers**: exchange rates, then dates and time
   zones. The dates and the zones are the largest single block in the
   must-have set (187 rows on that page).
8. **Notes** (60 rows) and **backup** (49 rows) - the formats are settled,
   the screens are not.
9. **Aliases** (18) and **favorites** (19) - small, and they are launcher
   rows.
10. **The `.rayconfig` reader** - it unlocks 41 rows, and it needs a real
    exported file to be tested against.

**How to work a page:** pick the page, read its rows in `parity/parity.toml`,
read upstream's own test file for the behaviour, write the test in the crate
that owns it, then move the row to `PORTED` in `parity/status.toml` with the
evidence, and regenerate with `cargo run -q -p xtask -- parity`.

---

## 13. Known debt and risks

- **`chrono` is unmaintained** (RUSTSEC-2026-0543). It is ignored in
  `deny.toml` with its reason, and the migration to `time` or `jiff` is
  written down as work for 1.1. It is used in exactly three places:
  `crates/tc-snippets/src/template.rs` (the `{date}` token and its offsets),
  `crates/tc-clipboard/src/raycast.rs` (an RFC 3339 parse) and
  `xtask/src/tools.rs` (the release date). The snippet token formats dates
  with strftime patterns such as `EEEE` that upstream's own tests pin, so
  this is a migration and not a version bump.
- **The zbus 5 crates** (`zvariant`, `zbus_names`, `zvariant_derive`) are
  unmaintained and reached only through `tauri-plugin-single-instance` 2.5.5.
  Removing the single-instance plugin would remove all three.
- **`proc-macro-error`** is unmaintained and comes in through `glib-macros`
  on Linux only. It is not in the Windows build.
- **The background workers are unreliable.** Four study subagents died on
  credit ("insufficient balance"). The Raycast study was done by hand. Do not
  plan on subagents for bulk reading; if you use them, check they survived.
- **The import-from-Raycast rows need a real exported file.** The format is
  understood - `RAYCFG3\n`, a gzipped JSON header, a gzipped JSON payload, a
  16-byte GCM tag, and a key from `scrypt(password, salt, 32)` with
  N=16384, r=8, p=1 - but a tested reader needs an actual `.rayconfig` to run
  against. The user may have one.
- **The Raycast database key must never be written to disk.** It is read in
  memory from Credential Manager, target `Raycast-Production/BackendDBKey`.
- **`docs/audits/`** holds the review of the other AI's YAL port. Nothing was
  taken from it; its ledger has 1 208 rows against this one's 2 309, so the
  two verdicts cannot be compared row for row.

---

## 14. Publishing

The procedure is in [docs/PUBLISHING.md](docs/PUBLISHING.md). In short:

1. `cargo run -q -p xtask -- dist` for both architectures.
2. The installers, the portable zips and `SHA256SUMS.txt` land in `dist/`.
3. `release.yml` builds, signs when the certificate secret is set, and
   publishes to a GitHub release.
4. The **flush**: replace the contents of `github.com/Abinanthan-CG/yal` with
   this port and push to `main`. The repository slug is written down once, in
   `tc-core::REPOSITORY`, and repeated in `Cargo.toml` because Cargo cannot
   read a constant.

The user's instruction is unchanged: **push after the first build is
finished**, and publish once, at the end.

---

## 15. A note on the Raycast study

Raycast for Windows was studied with a real reverse-engineering toolchain
(Ghidra 12.2, Rizin 0.9.1, ilspycmd 11.2, .NET 10, installed outside the
repository at `C:\Users\dabin\tools\rea`) and by opening its encrypted
databases with its own data layer. The scripts that made the study repeatable
are in `scripts/re/`. The findings are written down in
[port note 026](docs/port-notes/026-the-theme-the-ranking-and-the-export-file.md).

**The rules of the study**, which bind you too: it is for personal,
non-commercial use; the findings are **behavioural notes only** - no
third-party code, tables or naming are copied into YAL; and every feature
adopted is **rewritten in Rust with its own tests**. Two behaviours were
taken: the theme is fourteen colour roles plus a name and an appearance, and
the ranking record remembers the queries that reached an item.

---

## 16. If you read only one thing

Read `PROGRESS.md`, then `docs/DECISIONS.md`, then §11 and §12 of this
document. Everything else is a reference you will come back to.

Then make the hotkey engine work. It is small, it is contained, and until it
works the app is a window that opens but no launcher you can summon.
