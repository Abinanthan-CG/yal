# YAL

**YAL** is a keyboard launcher for Windows 10 and 11. It is an unofficial,
community-made port of [Tinycast](https://github.com/abue-ammar/tinycast), the
macOS launcher by Abue Ammar.

Press one key, type what you want, press <kbd>Enter</kbd>. YAL finds the app,
the file, the calculation, the clipboard entry, the window or the setting you
meant, and puts it in front of you.

```text
Alt + Space      open the palette
Ctrl + Space     open the palette (alternative)
```

---

## What YAL is, and what it is not

YAL is a **port**, not a clone. It reimplements Tinycast's behaviour on
Windows, feature by feature, and it keeps a ledger of exactly how far that has
got. The ledger is [`PARITY.md`](PARITY.md), and every row in it is either
`PORTED` — with the test that proves it — or `NOT PORTED` — with the reason.

YAL is **not** affiliated with, endorsed by, or supported by the Tinycast
authors. Upstream is a macOS application; YAL is an independent Windows
project that follows its documentation.

### The original project

Tinycast is licensed under the AGPL-3.0. YAL is a derivative work and is
distributed under the same licence. The original author is credited in
[`LICENSE`](LICENSE) and [`UPSTREAM.md`](UPSTREAM.md). Third-party notices,
including the MIT licence for the Rooms window-layout work by Sara Gordić, are
in [`NOTICE.md`](NOTICE.md).

### Support

Support for Tinycast — the original project — is at
**<https://tinycast.dev/support>**. Please use it for questions about the
macOS app.

---

## Status

| | |
|---|---|
| Tier 1 rows in the parity ledger | 1,815 |
| Tier 2 rows in the parity ledger | 494 |
| Total rows | 2,309 |
| Architecture | `x86_64-pc-windows-msvc`, `aarch64-pc-windows-msvc` |
| Minimum Windows | Windows 10 22H2 |
| Licence | AGPL-3.0-only |

`PARITY.md` is generated from `parity/parity.toml` and is checked in CI, so it
cannot drift away from the code without the build failing.

---

## Installing

### The installer

Download `yal-setup-<version>-x64.exe` (or `-arm64.exe`) from the
[releases page](https://github.com/OWNER/REPO/releases) and run it.

The install is **per user**. It never asks for administrator rights, it never
writes outside your own profile, and it can be undone completely from
*Settings → Apps → Installed apps*.

The installer offers a Start menu shortcut, an optional desktop shortcut, and
an optional "start when I sign in" entry. It registers the `tinycast://` link
protocol for your account only.

### The portable build

Download `yal-<version>-x64-portable.zip`, extract it anywhere, and run
`yal.exe`. Portable builds keep their data in a `portable` folder next to the
executable instead of in your user profile, so you can run YAL from a USB
stick with nothing left behind.

Every release publishes a `SHA256SUMS.txt`. Check it before you run anything:

```powershell
Get-FileHash .\yal-setup-0.1.0-x64.exe -Algorithm SHA256
```

---

## Building from source

```powershell
git clone https://github.com/OWNER/REPO.git
cd REPO
.\scripts\bootstrap.ps1     # installs the pinned toolchain and NSIS
.\scripts\build.ps1         # formats, lints, tests, then builds the installer
```

The result is in `dist/`.

Full instructions are in [`BUILD.md`](BUILD.md). If you are working with an
AI coding assistant, read [`BUILD_WITH_AI.md`](BUILD_WITH_AI.md) instead — it
explains the repository's conventions, the parity ledger and the rules the
build enforces.

---

## Repository layout

```text
crates/          the domain, one crate per feature area
  tc-core/       commands, launcher rows, deeplinks, identifiers
  tc-search/     matching and ranking
  tc-calc/       the calculator engine
  tc-platform/   the traits every platform must satisfy, and their fakes
  tc-platform-win/  the Win32 implementation of those traits
apps/
  yal/           the Tauri shell: palette, tray, hotkey, single instance
  yal-indexer/   the file indexer helper process
  yal-ocr/       the text recognition helper process
  yal-dictation/ the dictation helper process
frontend/        the palette's user interface
installer/       the NSIS installer
parity/          the parity ledger and its parts
xtask/           the build tool: parity, ledger, i18n, dist
```

The crate map and the reasoning behind it are in
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

---

## How macOS ideas are read on Windows

The port is a translation, not a copy. Every macOS concept has a defined
Windows answer, and the table lives in
[`docs/WINDOWS-MAPPING.md`](docs/WINDOWS-MAPPING.md).

Some examples:

| Tinycast on macOS | YAL on Windows |
|---|---|
| ⌘ Command | <kbd>Ctrl</kbd> |
| ⌥ Option | <kbd>Alt</kbd> |
| ⇧ Shift | <kbd>Shift</kbd> |
| ⌃ Control | <kbd>Ctrl</kbd> + <kbd>Alt</kbd> |
| The menu bar | The notification area (system tray) |
| Spaces | Virtual desktops |
| Apple Shortcuts | Automations (Power Automate Desktop, Task Scheduler) |
| The Trash | The Recycle Bin |
| Secure Input | Administrator windows and the secure desktop |
| `/Applications` | The Start menu, `shell:AppsFolder` and App Paths |

The full shortcut list is in [`docs/SHORTCUTS.md`](docs/SHORTCUTS.md).

---

## Privacy

YAL has no accounts, no telemetry and no analytics. It does not phone home.

It talks to the network only where the original does: to check for updates, to
download a language pack or a dictation model if you ask for one, and to reach
an AI provider that you have configured yourself.

Your secrets — API keys for AI providers — are stored in **Windows Credential
Manager**, never in a file on disk.

---

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for the workflow, the commit
conventions and how to add a parity row. The short version: write the test
first, name the row, and never claim a feature is ported without evidence.

Please read the [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md) before taking part.

---

## Licence

AGPL-3.0-only. See [`LICENSE`](LICENSE) and [`NOTICE.md`](NOTICE.md).
