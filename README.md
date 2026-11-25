<p align="center">
  <img src="assets/yal.svg" width="96" height="96" alt="YAL — Yet Another Launcher" />
</p>

<h1 align="center">YAL</h1>

<p align="center">
  <strong>A fast, pure-Rust Win32 + Direct2D keyboard launcher for Windows 10 and 11.</strong><br/>
  No Electron. No WebView2. No Node runtime. Single native binary.
</p>

---

**YAL** (*Yet Another Launcher*) is an unofficial, community-made Windows port of
[Tinycast](https://github.com/abue-ammar/tinycast), rebuilt from the ground up in pure
Rust on native Win32, Direct2D, DirectWrite, and SQLite.

Press one key, type what you want, press <kbd>Enter</kbd>. YAL finds the app,
file, calculation, clipboard entry, snippet, emoji, window, or setting you meant
in milliseconds and puts it in front of you.

```text
Alt + Space       Open / toggle the palette
Ctrl + Space      Alternative open shortcut
↓ (Compact mode)  Unfold suggestions without leaving the keyboard
Ctrl + K          Open contextual Action Panel for any item
Ctrl + I          Toggle Master-Detail info & preview pane
Tab / Shift+Tab   Fill inline Quicklink arguments or drill into folders
```

---

## Features (v1.0.0)

- **Native Win32 + Direct2D UI:** Hardware-accelerated rendering with Windows 11 rounded corners, native shadows, per-monitor DPI scaling, automatic Dark & Light themes, and **Compact** (single search pill) or **Expanded** window mode.
- **Instant App & Command Search:** Indexes Start Menu, App Paths, registry uninstall entries, and Microsoft Store / MSIX apps with real extracted Windows icons, frecency learning, favorites (`Ctrl+1..0`), and per-command hotkeys & aliases.
- **Fast Local File Search:** Background SQLite WAL + FTS5 trigram index across your user folders with a live overlapped Win32 `FileWatcher`, type filters, folder drill-down (`Tab`), preview pane (`Ctrl+I`), Reveal in Explorer, and Recycle Bin support.
- **Swiss-Army Calculator:** Live arithmetic, units & compound measures, 159 fiat + 23 crypto currencies (live FX), DST-aware world clocks & date countdowns (`hours till 27 nov`, `days between ...`), variables & `ans`, color space transforms (`#hex` / `rgb` / `hsl`), statistics, finance, chemistry, and Calculator History.
- **Rich Clipboard History & Paste Queue:** Captures text, links, emails, color swatches, files, and images with pinning, filtering, preview, and sequential `Paste Queue` (`Paste Next`).
- **Snippets & Auto-Expansion:** Markdown-backed snippets with `{clipboard}`, `{selection}`, `{date}`, `{time}`, `{uuid}`, `{cursor}`, `{argument}`, and nested `{snippet:...}` templates, plus background `!keyword` auto-expansion and Raycast snippet import.
- **2D Color Emoji & Symbol Picker:** Full searchable DirectWrite color emoji grid with frequent recents and pinning.
- **Window Management & Switcher:** 40 native window tiling/snapping commands (halves, thirds, quarters, sixths, center, display moves, undo) plus live open-window switching.
- **Quicklinks, Quick Notes & Fallback Search:** Parameterized `{query}` Quicklinks with inline `Tab` argument pills, persistent Quick Notes, and always-on `Use "<query>" with...` fallback search commands.
- **Custom Commands & Shell Runner:** PowerShell, `cmd`, Git Bash, and WSL script commands running inside Win32 Job Objects, plus Raycast script-command import.
- **Power-User Input & Backup:** Optional `CapsLock` Hyperkey, live hotkey recorder, 64 built-in Settings, `tinycast://` deep links, and SHA-256 verified backup & restore.

---

## Installing

### Per-user installer (recommended)

Download `yal-setup-1.0.0-x64.exe` (or `-arm64.exe`) from the
[Releases page](https://github.com/Abinanthan-CG/yal/releases) and run it.

- Installs **per user** (`%LOCALAPPDATA%\Programs\yal`) — never asks for administrator rights.
- Offers optional Start Menu shortcut, Desktop shortcut, and Launch at Login.
- Registers the `tinycast://` link protocol for your user account only.
- Cleanly uninstallable from *Settings → Apps → Installed apps* while preserving your data in `%APPDATA%\yal` unless you choose to remove it.

### Portable build

Download `yal-1.0.0-x64-portable.zip`, extract it anywhere, and run `yal.exe`.
Portable builds store all data in a `portable` folder beside `yal.exe`.

Verify any release download against `SHA256SUMS.txt`:

```powershell
Get-FileHash .\yal-setup-1.0.0-x64.exe -Algorithm SHA256
```

---

## Building from source

Requires the pinned Rust toolchain (`rust-toolchain.toml`, MSRV `1.89`) and [NSIS](https://nsis.sourceforge.io/) if building the installer.

```powershell
git clone https://github.com/Abinanthan-CG/yal.git
cd YAL
cargo build --release --bin yal
.\target\release\yal.exe --self-test
```

To build the release installer, portable zip, and `SHA256SUMS.txt`:

```powershell
cargo run --quiet -p xtask -- dist --arch x64
```

Artifacts are written to `dist/`. Full build notes live in [`BUILD.md`](BUILD.md).

---

## Repository layout

```text
crates/
  tc-core/          commands, launcher rows, deeplinks, identifiers
  tc-search/        fuzzy matching, folding, scoring, frecency ranking
  tc-apps/          Start Menu, registry, App Paths & MSIX app discovery
  tc-calc/          calculator engine (units, FX, dates, timezones, colors, finance)
  tc-clipboard/     clipboard history model, classification, paste queue & SQLite store
  tc-emoji/         color emoji catalog, search & favorites
  tc-snippets/      Markdown snippet library, template engine & Raycast import
  tc-files/         SQLite WAL + FTS5 trigram file indexer & scoped search
  tc-windows/       40 window-tiling commands & multi-monitor geometry
  tc-commands/      built-in command catalog, custom commands & Job Object shell runner
  tc-backup/        SHA-256 verified export/restore & Raycast import
  tc-palette/       screen stack, key chords, settings schema & compact/expanded policy
  tc-platform/      platform traits and deterministic test fakes
  tc-platform-win/  Win32 hotkeys, Hyperkey, snippet expander, clipboard, icons & FileWatcher
  tc-ui/            pure Win32 + Direct2D + DirectWrite palette renderer & HUD
apps/
  yal/              single-binary Windows launcher (palette, tray, hotkeys, single instance)
  yal-ui-probe/     standalone Direct2D layout & render probe
assets/             official Launch Ring SVG (`yal.svg`, `yal-mark.svg`), PNG & multi-res ICO
installer/          per-user NSIS installer script (`yal.nsi`)
packaging/          WinGet and Scoop manifest templates
xtask/              release packaging (`dist`) and self-test orchestration
```

---

## Attribution & Licence

YAL is a derivative work of [Tinycast](https://github.com/abue-ammar/tinycast) by Abue Ammar,
licensed under **AGPL-3.0-only**. YAL is **not** affiliated with, endorsed by, or supported by
the Tinycast authors. Upstream credit and third-party notices (including the MIT licence for
the Rooms window-layout work by Sara Gordić) live in [`LICENSE`](LICENSE),
[`UPSTREAM.md`](UPSTREAM.md), and [`NOTICE.md`](NOTICE.md).

### Privacy

YAL has no accounts, no telemetry, and no analytics. Network calls happen only when you
explicitly use live currency exchange rates or open external URLs/Quicklinks.
