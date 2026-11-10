# Getting started with YAL

> Five minutes from installing to using it.

---

## Install it

Run `yal-setup-<version>-x64.exe` from the releases page. The installer is per
user: it never asks for administrator rights, and it can be removed completely
from *Settings → Apps → Installed apps*.

If you prefer to try it without installing, download the portable zip, extract
it anywhere, and run `yal.exe`. A portable build keeps its data in a folder
next to the executable, so nothing is left in your profile.

---

## Open it

YAL has no global shortcut until you give it one. The first time you start
it, the palette opens and asks what you want to press.

The usual choice is:

```text
Alt + Space
```

or

```text
Ctrl + Space
```

Both are free on a clean Windows install. <kbd>Ctrl</kbd>+<kbd>Space</kbd> is
the one Windows itself uses to switch input languages, so if you switch
languages often, pick <kbd>Alt</kbd>+<kbd>Space</kbd> instead.

To change it later: **Settings → General → Show Launcher**.

---

## Use it

Press your shortcut. Type what you want. Press <kbd>Enter</kbd>.

```text
notepad        open Notepad
12*8           calculate it
chrome         open Chrome
:file.txt      search your files
```

The palette searches apps, files, settings, snippets, quicklinks, the
clipboard history, your notes, and everything else you have added to it. It
searches as you type, and the best match is always highlighted.

### The two actions

| | |
|---|---|
| <kbd>Enter</kbd> | The main action |
| <kbd>Ctrl</kbd>+<kbd>Enter</kbd> | The second action — usually "show in File Explorer" |

### When you do not know what to type

Press <kbd>Ctrl</kbd>+<kbd>K</kbd> for the actions menu. Every row has one,
and they are listed there with their shortcuts.

### Getting out

| | |
|---|---|
| <kbd>Esc</kbd> | Clear the search, then close |
| <kbd>Ctrl</kbd>+<kbd>Esc</kbd> | Back to the root search from any screen |
| Clicking away | The palette hides itself |

The palette hides when it loses focus, so you can never leave it stranded on
top of your work.

---

## The tray icon

YAL lives in the notification area, next to the clock.

| | |
|---|---|
| Click the icon | Show or hide the palette |
| Right-click | Open, Settings, Check for Updates, About, Support, Quit |

If the icon is not there, it may be hidden behind the chevron. Drag it out of
the overflow area and pin it.

---

## Start it when you sign in

Turn on **Settings → General → Launch at login**. YAL starts hidden and
waits for your shortcut.

---

## Where your data is

```text
%APPDATA%\yal\       settings, quicklinks, snippets, notes, history
%LOCALAPPDATA%\yal\  the cache
```

Nothing leaves your machine unless you ask it to.

---

## Next

- [Installing](installing.md) — the installer, the portable build, the
  checksums, and how to remove YAL
- [Settings](settings.md) — what each setting does
- [Troubleshooting](troubleshooting.md) — when it does not work
- [Shortcuts](../SHORTCUTS.md) — every shortcut in the palette
