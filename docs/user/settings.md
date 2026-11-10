# Settings

> What each part of YAL's settings does.

YAL's settings live inside the palette. Press <kbd>Ctrl</kbd>+<kbd>,</kbd>,
or choose **Settings** from the tray menu.

The sections match Tinycast's, so that a setting you know from the original
means the same thing here.

---

## General

### Show Launcher

The global shortcut that opens the palette.

YAL ships with no shortcut. You record one the first time you open the
settings. The usual choice is <kbd>Alt</kbd>+<kbd>Space</kbd> or
<kbd>Ctrl</kbd>+<kbd>Space</kbd>.

Pick one Windows does not already use:

| Chord | Used by Windows for |
|---|---|
| <kbd>Ctrl</kbd>+<kbd>Space</kbd> | Switching input languages |
| <kbd>Alt</kbd>+<kbd>Tab</kbd> | Switching windows |
| <kbd>Alt</kbd>+<kbd>F4</kbd> | Closing a window |
| <kbd>Win</kbd>+<kbd>E</kbd> | The File Explorer |
| <kbd>Win</kbd>+<kbd>D</kbd> | The desktop |
| <kbd>Win</kbd>+<kbd>L</kbd> | Locking the machine |

YAL never takes over a chord that Windows owns, so if you pick one of these
you will fight the system.

### Launch at login

Starts YAL when you sign in. It starts hidden and waits for your shortcut.

### The Hyper key

Tinycast can turn one physical key into <kbd>⌃</kbd><kbd>⌥</kbd><kbd>⌘</kbd>.
**YAL does not have this feature**, and the setting is marked
`NOT PORTED` in [`PARITY.md`](../../PARITY.md).

The reason is that it needs hardware-level key remapping, which a normal
Windows application is not allowed to do. The closest honest equivalent is to
record the modifiers you need on each shortcut itself.

---

## Permissions

Windows has no single permissions panel like macOS, so this section explains
what YAL needs and where Windows keeps it.

| | |
|---|---|
| Microphone | Needed for dictation. Windows asks for it the first time you record, and it is per application under *Settings → Privacy → Microphone*. |
| Files and folders | The ordinary Windows rules. YAL can read the places you can read. |
| Accessibility | **Not needed.** YAL uses `EnumWindows` and `SetWindowPos`, which any process may call, rather than the Accessibility API. |

### Secure Input

macOS's Secure Input hides key presses from launchers while a password field
is focused. The Windows equivalent is the **secure desktop**: the UAC prompt,
the lock screen, and the Ctrl+Alt+Delete screen.

While a secure desktop is active, YAL cannot see key presses, so its hotkey
does not fire. This is the same behaviour as upstream, and YAL tells you so
rather than failing silently.

An application running as administrator has the same effect. YAL normally
runs at medium integrity, like any other application.

---

## Launcher

Which things appear in the palette.

| | |
|---|---|
| Applications | The apps installed on this machine |
| System Settings | The pages of the Windows Settings app |
| System Actions | The 32 actions: lock the screen, empty the Recycle Bin, change the volume, and so on |
| Commands | The 46 built-in commands |
| Quicklinks | Your own links to files, folders and web pages |
| Snippets | Your saved text |
| Custom commands | Your own commands |
| Notes | Your notes |
| The clipboard history | What you have copied |
| Files | The indexed files on this machine |

Each section has a switch that turns off **all** of its shortcuts, not just
its search results. That matches upstream.

---

## Features

One switch per feature. Turning a feature off also turns off its shortcuts.

The features are the same as upstream's, with the Windows names in
[`docs/WINDOWS-MAPPING.md`](../WINDOWS-MAPPING.md): **Automations** instead
of Apple Shortcuts, **System Settings** instead of System Preferences, and so
on.

---

## Advanced

| | |
|---|---|
| Check for updates | YAL checks GitHub for a newer release. It never sends anything about your machine. |
| Back up | Writes a backup of your settings, quicklinks, snippets, notes and history. |
| Restore | Reads a backup. |
| Import from Raycast | Imports your Raycast snippets, quicklinks and applications. |
| Uninstall | Removes other applications from this machine. |

### Backups do not contain your secrets

An AI provider key is kept in Windows Credential Manager and is **never**
written into a backup, a log or a file.

---

## Where the settings live

| | |
|---|---|
| The settings file | `%USERPROFILE%\.config\tinycast\settings.json` |
| Everything else | `%APPDATA%\yal\` |
| Secrets | Windows Credential Manager |

The settings file is plain JSON, so you can read it, edit it, or put it in
version control.

---

## See also

- [Getting started](getting-started.md)
- [Troubleshooting](troubleshooting.md)
- [`docs/WINDOWS-MAPPING.md`](../WINDOWS-MAPPING.md)
