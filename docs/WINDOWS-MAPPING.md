# Windows mapping

> How every macOS concept in Tinycast is read on Windows.

This is the port's translation table. When upstream uses a macOS idea, this
document says what YAL does instead, and why.

Every entry here is also machine-readable: `parity/pages.toml` carries the
same Windows translation as data, so the parity validator can check that a
row's Windows answer matches the page it belongs to.

---

## Modifiers

| macOS | Windows | Note |
|---|---|---|
| <kbd>⌘</kbd> Command | <kbd>Ctrl</kbd> | The primary modifier on both. <kbd>Ctrl</kbd>+<kbd>C</kbd> is copy in YAL too, and YAL never takes it over. |
| <kbd>⌥</kbd> Option | <kbd>Alt</kbd> | |
| <kbd>⇧</kbd> Shift | <kbd>Shift</kbd> | |
| <kbd>⌃</kbd> Control | <kbd>Alt</kbd> | Windows has no separate Control modifier. |
| <kbd>⌘</kbd>+<kbd>⌃</kbd> | <kbd>Ctrl</kbd>+<kbd>Alt</kbd>+<kbd>Shift</kbd> | Keeps <kbd>⌥</kbd><kbd>⌘</kbd> and <kbd>⌃</kbd><kbd>⌘</kbd> chords distinct. |
| Globe <kbd>🌐︎</kbd> | The Windows key <kbd>Win</kbd> | An ordinary modifier on Windows. YAL does not reserve it. |

The full shortcut table is in [SHORTCUTS.md](SHORTCUTS.md).

---

## System surfaces

| macOS | Windows | Why |
|---|---|---|
| The menu bar at the top of the screen | The **notification area** (system tray) | Windows has no menu bar. The tray holds the YAL icon and its menu: Open, Settings, Check for Updates, About, Support, Quit. |
| Mission Control | <kbd>Win</kbd>+<kbd>Tab</kbd> | Windows already has it. YAL does not reimplement it. |
| Spaces | **Virtual desktops** | <kbd>Win</kbd>+<kbd>Ctrl</kbd>+<kbd>←</kbd>/<kbd>→</kbd> or <kbd>Win</kbd>+<kbd>Ctrl</kbd>+<kbd>↑</kbd>/<kbd>↓</kbd>. |
| The Dock | The **taskbar**, or the Start menu | The launcher searches the Start menu instead of the Dock. |
| Spotlight | The YAL palette | The whole point of the port. |
| Notification Centre | **Windows Action Centre** | YAL posts no notifications of its own except through the tray. |
| Quick Look | Windows' own <kbd>Win</kbd>+<kbd>Space</kbd> | YAL does not reimplement it. |
| Automator | **Power Automate Desktop**, or a Task Scheduler task | See *Automations* below. |
| Apple Shortcuts | **Automations** | See *Automations* below. |
| System Settings | **Settings** | YAL's own settings live inside the palette; Windows' own Settings is reached through the *System Settings* feature. |
| Activity Monitor | **Task Manager** | |
| Console | **Windows Terminal** | |
| Preview | **Photos**, or the default viewer | |
| Finder | **File Explorer** | |
| System Information | **System → About**, or `msinfo32` | |
| Accessibility | See *Permissions* below | |
| Secure Input | Administrator windows and the **secure desktop** | See *Permissions* below. |
| The Trash | The **Recycle Bin** | Never a permanent delete. See *Deleting* below. |
| The Login Items | The **Run** key, `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` | Set by the installer's optional "Launch at login" section. |
| `~/Library/Application Support/com.tinycast.app/` | `%APPDATA%\yal\` | YAL's data folder. |
| `~/Library/Caches/` | `%LOCALAPPDATA%\yal\cache` | |
| `~/.config/tinycast/settings.json` | `%USERPROFILE%\.config\tinycast\settings.json` | Kept at the same relative place so the two are easy to compare. |
| `/Applications` | The **Start menu**, `shell:AppsFolder`, and **App Paths** | See *Finding applications* below. |

---

## Applications

### Finding applications

macOS resolves an app by bundle identifier. Windows has three ways to name
one, and YAL uses all three, in this order:

1. **App Paths** in the registry,
   `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\<exe>`.
   This is the most reliable, because it names the executable, not the icon.
2. **`shell:AppsFolder`**, the virtual folder behind the Start menu. It holds
   every app that has a shortcut, including the ones the Store installs.
3. **The Start menu itself**, by reading its `.lnk` files.

Each app gets a stable id of the form `app:<name>`, and the id never changes
when the app moves on disk, because it is derived from the name rather than
the path.

### Launching and switching

macOS uses AppleScript and the Accessibility API. Windows uses:

- `ShellExecuteW` to launch, which handles files, folders and URIs.
- `SetForegroundWindow` to bring a window forward, with the
  `AllowSetForegroundWindow` and `AttachThreadInput` workarounds that Windows
  requires for a process that was not started by the foreground process.
- `ShowWindow` with `SW_RESTORE` to un-minimise.

### Quitting an application

macOS asks the application to quit politely. Windows has no polite request
that every application honours, so YAL sends `WM_CLOSE` to the application's
top-level windows and waits briefly, then offers to force the process to end.
The force is always the user's explicit choice.

---

## Deleting

**YAL never permanently deletes anything.**

| macOS | Windows |
|---|---|
| Move to the Trash | Move to the **Recycle Bin**, through `SHFileOperationW` with the `FOF_ALLOWUNDO` flag |
| Empty the Trash | Empty the **Recycle Bin** |
| Delete immediately (<kbd>⌃</kbd><kbd>⌘</kbd><kbd>delete</kbd>) | YAL does not offer this |

Emptying the Recycle Bin is one of the five actions that asks for
confirmation first, exactly as upstream does.

---

## Window management

macOS's Accessibility API is the only way to see and move other
applications' windows. Windows has three equivalent mechanisms:

| Capability | Windows |
|---|---|
| List an application's windows | `EnumWindows`, filtered by process id |
| Read a window's position and size | `GetWindowRect`, and `DwmGetWindowAttribute` for the extended frame bounds |
| Move and resize a window | `SetWindowPos` |
| Read a window's title and process | `GetWindowTextW` and `GetWindowThreadProcessId` |

### The metrics problem

macOS reports window positions in **points**, which are scaled by the
display's backing scale factor. Windows reports them in **physical pixels**.

YAL works in physical pixels internally and converts at the edge, so a window
placed at 50 % of the screen is 50 % of the screen on a 150 % display and on a
100 % one. Custom sizes therefore use the ranges upstream uses — 1 to 16 000
points on a side, 1 to 100 % — and are applied against the monitor's own
working area, not against the virtual screen.

### Per-monitor DPI

Windows 10 22H2 and Windows 11 both support per-monitor DPI awareness
version 2. YAL's manifest declares it, so a window moved to a display with a
different scale factor is rescaled by Windows rather than by the application.

### Custom window sizes

Upstream's `CustomWindowSize` uses nine anchors — `top-left`, `top`,
`top-right`, `left`, `center`, `right`, `bottom-left`, `bottom`,
`bottom-right` — and an offset in the range −4000 to 4000 points. YAL keeps
the same anchors and the same ranges, and applies them to the work area of
the monitor the window is on.

---

## Rooms

Upstream's Rooms feature is based on
[Rooms](https://github.com/saragordic/rooms) by Sara Gordić, used with the
author's permission under the MIT licence.

A room on macOS arranges windows into a layout. On Windows, YAL arranges
windows with `SetWindowPos` into the same nine-anchor grid, and the "gliding"
preview becomes an animated preview of the target rectangles.

The notice for the original Rooms work is in
[NOTICE.md](../NOTICE.md).

---

## Automations

macOS's Shortcuts app is an automation tool built into the system. Windows has
no equivalent, so YAL's **Automations** feature covers what can be covered:

| macOS | Windows |
|---|---|
| A Shortcut | A **Power Automate Desktop** flow, or a **Task Scheduler** task |
| A Shortcut's input | The flow's input, or the task's arguments |
| Triggering it from the palette | Running the flow or task from the palette |
| Shortcuts that take text | Flows that accept text |

Rows that depend on Shortcuts-specific behaviour are marked `NOT PORTED` in
[PARITY.md](../PARITY.md), with the reason given.

---

## Permissions

macOS asks for permissions in one place, *System Settings → Privacy &
Security*. Windows has no single equivalent, so YAL asks for what it needs,
in the place it needs it:

| macOS | Windows |
|---|---|
| Accessibility | Not needed: YAL uses `EnumWindows` and `SetWindowPos`, which any process may call on its own windows |
| Screen Recording | Not needed: YAL never captures the screen |
| Input Monitoring | Not needed: the global hotkey uses `RegisterHotKey` and the low-level hook |
| Automation | Not needed: YAL does not drive other applications' user interfaces |
| Microphone | The **Microphone** privacy setting, per application |
| Files and folders | The normal Windows file system rules |

### Secure Input

macOS's Secure Input hides key presses from launchers while a password field
is focused. The Windows equivalent is the **secure desktop** — the UAC
prompt, the lock screen, the Ctrl+Alt+Delete screen — and an application
running as administrator.

While a secure desktop is active, YAL cannot see key presses, so its hotkeys
do not fire. The behaviour matches upstream, and YAL says so in its
diagnostics rather than failing silently.

---

## Files and paths

| macOS | Windows |
|---|---|
| `~` | `%USERPROFILE%` |
| `/` | `\` |
| A path separator in a shortcut | The same |
| `~/Library/Application Support/com.tinycast.app/` | `%APPDATA%\yal\` |
| `~/Library/Caches/` | `%LOCALAPPDATA%\yal\` |
| A case-insensitive file system | Windows is case-insensitive by default; YAL preserves the case it found |
| A path separator in a deeplink | `/` in the link, resolved with `PathBuf` |

YAL keeps every text file UTF-8, and Windows long paths are enabled through
the manifest and the `longPathAware` setting.

---

## Storage

The storage layout mirrors upstream, so the two are easy to compare:

| Upstream | YAL | Format |
|---|---|---|
| `clipboard.sqlite3` | `%APPDATA%\yal\clipboard.sqlite3` | SQLite, with FTS5 |
| `quicklinks.sqlite3` | `%APPDATA%\yal\quicklinks.sqlite3` | SQLite |
| `ai-chats.sqlite3` | `%APPDATA%\yal\ai-chats.sqlite3` | SQLite |
| The file index | `%APPDATA%\yal\files.sqlite3` | SQLite |
| Notes | `%APPDATA%\yal\notes\*.md` | Markdown |
| Snippets | `%APPDATA%\yal\snippets\*.md` | Markdown |
| `settings.json` | `%APPDATA%\yal\settings.json` | JSON |
| `launcher-ranking.json` | `%APPDATA%\yal\launcher-ranking.json` | JSON |
| Keychain | **Windows Credential Manager** | Never a file |

---

## Secrets

macOS's Keychain is one store for every secret. Windows has several, and the
right one here is the **Credential Manager**, under the user's own account.

API keys for AI providers are stored there and nowhere else. They are never
written to `settings.json`, never written to a log, and never included in a
backup.

---

## Links

| macOS | Windows |
|---|---|
| `tinycast://` | `tinycast://`, registered under `HKCU\Software\Classes\tinycast` |
| A universal link | A `tinycast://` link |
| `NSWorkspace.open` | `ShellExecuteW` |
| The app being the handler for a scheme | The per-user registry key above |

The deeplink grammar is ported unchanged from upstream's `DeepLink.swift` and
`DeepLinkInput.swift`, including the 65 536-byte parameter limit and the
RFC 3986 unreserved encoding. `tc-core` has 50 tests over it.

---

## The window server

| macOS | Windows |
|---|---|
| The app is a menu bar app | The app is a **tray** app; the palette is a separate always-on-top window |
| The palette is a borderless panel | A borderless, always-on-top WebView2 window |
| `NSWindow` on a space | A window that follows the user between virtual desktops |
| The palette hides when it loses focus | The palette hides when it loses focus |
| `NSStatusItem` | A `Shell_NotifyIcon` tray icon |

---

## What is deliberately not ported

These have no Windows answer, and the parity ledger says so rather than
pretending:

- **Apple Shortcuts** as such. Automations cover Power Automate Desktop and
  Task Scheduler; a Shortcut with an Apple-only action cannot be imported.
- **Secure Input** as a macOS API. The behaviour it causes is reproduced; the
  API is not.
- **The Globe key** and the **Hyper key**, which need hardware-level key
  remapping that a normal Windows application cannot do.
- **The menu bar**, which does not exist on Windows.

Each one is a row in [PARITY.md](../PARITY.md) with `NOT PORTED` and a reason.

---

## See also

- [Shortcuts](SHORTCUTS.md)
- [Architecture](ARCHITECTURE.md)
- [PARITY.md](../PARITY.md)
