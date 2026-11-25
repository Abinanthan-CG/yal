# Security

YAL is a launcher. It runs with your permissions, sees what you type, and can
act on your behalf. This document says what it does with that, and how to
report a problem.

---

## What YAL does not do

- **No accounts.** There is no sign-up, no sign-in and no user identifier.
- **No telemetry.** YAL does not collect analytics, usage statistics,
  crash reports or device identifiers.
- **No advertising.** There is no ad SDK in the build.
- **No background phone-home.** YAL makes a network request only when you ask
  it to: to check for an update, to download a language pack or a dictation
  model you requested, or to reach an AI provider you configured yourself.

---

## What YAL can reach

YAL needs access to what a launcher needs access to. It is worth being plain
about it.

| | |
|---|---|
| **Keystrokes** | YAL sees the keys you type in its own palette. It needs them to search. |
| **The clipboard** | YAL reads the clipboard to build its clipboard history, and writes to it when you paste or copy. |
| **Window titles** | YAL reads other windows' titles so it can find and switch to them. It does not read their contents. |
| **Running processes** | YAL reads the process list to find and manage applications. |
| **Your files** | YAL reads file names, folders and dates to build its file index. It does not read file contents, except when you ask for text inside a document. |
| **The microphone** | Only if you use dictation, and only while you are recording. |
| **Your AI keys** | Only if you configure an AI provider. The key is read from Windows Credential Manager when it is needed. |

---

## How your data is stored

| What | Where |
|---|---|
| Settings, quicklinks, history, notes, snippets | `%APPDATA%\yal\` |
| The cache | `%LOCALAPPDATA%\yal\` |
| The settings file | `%USERPROFILE%\.config\tinycast\settings.json` |
| **Secrets** (AI API keys) | **Windows Credential Manager**, under your account |

**Secrets are never written to a file.** Not to `settings.json`, not to the
data folder, not to a log, and not into a backup. YAL reads a key from
Credential Manager when it needs it and forgets it afterwards.

A portable build keeps everything next to the executable, in its `portable`
folder. Its secrets still go to Credential Manager, because a credential that
lives in a file on a USB stick is not a secret.

---

## Backups

A backup contains your settings, quicklinks, snippets, notes and history. It
does **not** contain secrets, and it does not contain your clipboard history
unless you have explicitly turned that on.

---

## The installer

- The installer is **per user**. It runs as you, never as administrator, and
  writes only inside your own profile and your own registry hive.
- It registers the `tinycast://` protocol under `HKCU\Software\Classes`, so it
  affects your account and nobody else's.
- The uninstaller removes the program files and leaves your data alone unless
  you ask for it to be removed.

---

## Reporting a security problem

Please **do not open a public issue** for a security problem.

Report it privately through GitHub's security advisory form for this
repository:

> <https://github.com/Abinanthan-CG/yal/security/advisories/new>

Include:

- what the problem is, in one sentence,
- how to reproduce it,
- what an attacker would gain,
- the version of YAL and of Windows you saw it on.

You will get an acknowledgement. If the report is valid, a fix and an
advisory will follow, and you will be credited in the advisory unless you
would rather not be.

If you cannot use the advisory form, open a regular issue that says only
"I have a security report, please contact me privately", and nothing more.

---

## Hardening the launcher itself

Windows gives a running application an integrity level. YAL runs at **medium
integrity**, like any ordinary application, and it does not ask to be
elevated.

That has one consequence worth knowing: **YAL cannot interact with
administrator windows.** Windows puts them on a different desktop, and a
medium-integrity process cannot see them. This is the same behaviour as
macOS's Secure Input, and it is a security boundary that YAL does not try to
work around.

If you must manage an elevated window, run YAL elevated. Do not do that
casually: an elevated launcher can act on every application you run.

---

## Third-party components

YAL is built on open source components. Their licences and advisories are
the responsibility of their maintainers; `cargo deny` checks that every
dependency's licence is one the project accepts, and CI runs it on every
build.

The components are listed in [`NOTICE.md`](NOTICE.md).

---

## See also

- [`README.md`](README.md) — what YAL is
- [`UPSTREAM.md`](UPSTREAM.md) — the original project
- [`NOTICE.md`](NOTICE.md) — third-party notices
