# Installing YAL

---

## The installer

Download `yal-setup-<version>-x64.exe` from the releases page and run it.

| | |
|---|---|
| Needs administrator rights | **No** |
| Installs to | `%LOCALAPPDATA%\Programs\yal` |
| Writes to the registry | `HKCU` only — your account, not the machine |
| Registers `tinycast://` | Yes, per user |

The installer offers four components:

| Component | Default | What it does |
|---|---|---|
| YAL | On | The launcher and its helper processes |
| Start menu shortcut | On | Adds YAL to the Start menu |
| Desktop shortcut | Off | Adds a shortcut to your desktop |
| Launch at login | Off | Starts YAL when you sign in |

### What the installer does to your registry

Everything is under `HKCU`, so it affects your account and nobody else's, and
removing it leaves the machine as it was.

```text
HKCU\Software\Classes\tinycast                     the link protocol
HKCU\Software\Classes\tinycast\shell\open\command  "…\yal.exe" "%1"
HKCU\Software\Microsoft\Windows\CurrentVersion\Run  the login item
HKCU\Software\YAL                                    the install location
HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\YAL-stable
```

---

## The portable build

Download `yal-<version>-x64-portable.zip`, extract it, and run `yal.exe`.

A portable build keeps **everything** next to the executable, in a `portable`
folder:

```text
yal-0.1.0-x64-portable\
  yal.exe
  yal-indexer.exe
  yal-ocr.exe
  yal-dictation.exe
  portable
  portable\
    settings.json
    quicklinks.sqlite3
    ...
```

Nothing is written to `%APPDATA%` and nothing is written to the registry, so
you can run it from a USB stick with nothing left behind.

**One exception:** your AI provider keys are still kept in Windows Credential
Manager, not in the portable folder. A key in a file on a USB stick is not a
secret.

---

## The checksums

Every release publishes a `SHA256SUMS.txt`. Check it before you run
anything:

```powershell
Get-FileHash .\yal-setup-0.1.0-x64.exe -Algorithm SHA256
```

Compare the value with the one in `SHA256SUMS.txt`. They must be the same.

On macOS or Linux:

```sh
shasum -a 256 -c SHA256SUMS.txt
```

---

## Which build do you want?

| | x64 | arm64 |
|---|---|---|
| Intel and AMD processors | **yes** | no |
| Qualcomm Snapdragon (Windows on ARM) | no | **yes** |

Windows on ARM can run the x64 build through emulation, but the arm64 build is
faster and is the one to use there.

---

## Removing YAL

### From Windows

*Settings → Apps → Installed apps → YAL → Uninstall*

The uninstaller removes the program files and leaves your data alone, unless
you say otherwise. It asks:

> Remove your YAL data as well?

Answering **No** keeps your settings, quicklinks, snippets, notes and history
in `%APPDATA%\yal`, ready for the next install. Answering **Yes** removes it.

**No is the default**, both for a silent uninstall and for a user who presses
Enter without reading. Losing a configuration is worse than leaving a small
folder behind.

### By hand

```powershell
& "$env:LOCALAPPDATA\Programs\yal\yal.exe" --quit-running
& "$env:LOCALAPPDATA\Programs\yal\uninstall.exe" /S
```

The `/S` flag is the silent uninstall, which is what the scripts use.

Then, if you want the data gone too:

```powershell
Remove-Item -Recurse -Force "$env:APPDATA\yal"
Remove-Item -Recurse -Force "$env:LOCALAPPDATA\yal"
```

And the registry, if the uninstaller did not already remove it:

```powershell
Remove-Item -Recurse -Force "HKCU:\Software\Classes\tinycast"
Remove-Item -Recurse -Force "HKCU:\Software\YAL"
```

---

## See also

- [Getting started](getting-started.md)
- [Troubleshooting](troubleshooting.md)
