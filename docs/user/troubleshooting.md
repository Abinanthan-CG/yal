# Troubleshooting

---

## The shortcut does not open the palette

**Windows is using the chord.** YAL never takes over a chord Windows owns.
Try <kbd>Alt</kbd>+<kbd>Space</kbd> instead of <kbd>Ctrl</kbd>+<kbd>Space</kbd>,
and check the table of Windows' own chords in
[Settings](settings.md#general).

**The shortcut is not recorded yet.** YAL ships with no global shortcut.
Open the palette once from the tray icon and record one in *Settings →
General → Show Launcher*.

**A secure desktop is active.** The UAC prompt, the lock screen and the
Ctrl+Alt+Delete screen all block key presses. Close it and try again.

**YAL is running as administrator.** An elevated process cannot see a
medium-integrity process's hotkey. Run both at the same level.

---

## The palette does not appear, but the tray icon is there

The palette hides itself when it loses focus. Click the tray icon, or press
the shortcut again.

If it appears behind your windows, it is not on top. That happens when
another application also claims always-on-top. Close it and try again.

---

## The tray icon is missing

Windows puts new icons behind the chevron. Click the chevron next to the
clock, find YAL, and drag it out.

If it is not there at all, YAL may have crashed. Check with:

```powershell
yal.exe --self-test
```

That prints one line per subsystem and says which one failed.

---

## The installer cannot replace `yal.exe`

A YAL instance is still running. Close it from the tray menu, or:

```powershell
yal.exe --quit-running
```

The installer does this for you, but it cannot help if the running instance
is wedged.

---

## The palette is slow to open

The first open after a restart is slower, because the palette window is built
then and the WebView2 runtime warms up. Later opens are instant.

If it stays slow, the file index may be rebuilding. Check the indexer:

```powershell
yal-indexer.exe --scan %USERPROFILE%\Documents
```

---

## The clipboard history is empty

The clipboard history records **text** you copy. It does not record images or
PDFs.

If the text you copied came from another application that uses a private
clipboard format, it may not be visible to YAL.

---

## YAL cannot see an administrator window

This is deliberate. YAL runs at medium integrity, and an administrator
window lives on a different desktop. This is the same boundary as macOS's
Secure Input, and it is there to stop a launcher from being used to act on
elevated applications.

If you need to manage an elevated window, run YAL elevated too.

---

## A link does not open YAL

The protocol is registered per user. Check:

```powershell
Get-ItemProperty 'HKCU:\Software\Classes\tinycast\shell\open\command'
```

If the key is missing, the install did not register it. Re-run the installer.

---

## The checksum does not match

```powershell
Get-FileHash .\yal-setup-0.1.0-x64.exe -Algorithm SHA256
```

Compare it with the line for that file in `SHA256SUMS.txt`. If they differ,
the download is incomplete or corrupted. Download it again.

---

## YAL is not on `PATH`

The installer does not add to `PATH`, on purpose — it would change the
environment of every program you run afterwards.

If you want the command line tools, add the install directory to `PATH`
yourself:

```powershell
[Environment]::SetEnvironmentVariable(
    'Path',
    [Environment]::GetEnvironmentVariable('Path', 'User') + ';%LOCALAPPDATA%\Programs\yal',
    'User')
```

---

## Getting the logs

YAL writes no log file by default. To see what it is doing:

```powershell
yal.exe --self-test
```

That prints the state of every subsystem as JSON, which is usually enough to
tell which part is unhappy.

---

## Reporting a bug

Open an issue at <https://github.com/Abinanthan-CG/yal/issues> with:

- what you did,
- what you expected,
- what happened,
- the output of `yal.exe --self-test`,
- your Windows version.

If you are not sure whether something is a bug or an unimplemented feature,
check [`PARITY.md`](../../PARITY.md) first — every feature is listed there
with its status.

---

## See also

- [Getting started](getting-started.md)
- [Installing](installing.md)
- [Settings](settings.md)
