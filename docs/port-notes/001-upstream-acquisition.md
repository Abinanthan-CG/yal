# Port note 001 — Getting the upstream sources and the documentation

**Date.** 2026-11-10
**Upstream revision.** `58168bf7e0b721f7c5fbfc7f4e8a068a48cff446` (2026-11-08)

---

## What was needed

The port needs two things from upstream:

1. **The Swift sources**, which are the authority on what a feature actually
   does.
2. **The 42 documentation pages**, which are the authority on what features
   exist at all, and which the parity ledger is generated from.

## Getting the sources

```powershell
git clone --depth 50 https://github.com/abue-ammar/tinycast upstream
```

The clone is shallow on purpose: the port follows one revision, and the
history is not needed. The exact revision is written to `upstream-sha.txt`
and `upstream-commit.txt`, and `upstream/` is in `.gitignore` — the sources
are never committed here.

At that revision the tree holds:

| | |
|---|---|
| Swift files under `Tinycast/Features/` | 869 |
| Test files under `Tests/` | 127 |
| Markdown files | 42 |

## Getting the documentation

The upstream website publishes every page in a machine-readable form:

```text
https://tinycast.dev/llms.md/<page>/index.md
```

All 42 pages were fetched into `upstream-docs/`, which is also
git-ignored.

## The encoding trap

**This is the part worth remembering.**

The first download used:

```powershell
Invoke-WebRequest -Uri $url -OutFile $dest      # wrong
```

`Invoke-WebRequest` without `-OutFile` decodes the response body into a .NET
string using the system code page — Windows-1252 on this machine — and then
writes that string back out as UTF-8. The result is double-encoded: `×`
became `Ã—`, `→` became `â†’`, and the CJK and Cyrillic in the Russian and
Chinese pages became unreadable.

The fix is to write the **raw bytes**:

```powershell
Invoke-WebRequest -Uri $url -OutFile $dest      # correct
```

With `-OutFile`, the response body is written without being decoded, so the
file is exactly what the server sent. All 42 pages were re-downloaded that
way, and the non-ASCII characters are correct.

### A second trap: the reader

Even with correct files, some tools in this environment decode UTF-8 as
Latin-1 when they read a file, which produces the same mojibake from good
files.

So there are two separate things:

| | |
|---|---|
| The file on disk | Correct UTF-8 |
| What a given reader shows you | Sometimes wrong |

**Check the file, not the reader.** If the bytes on disk are right, the
problem is the tool.

Reliable ways to read a file here:

```powershell
# Explicit UTF-8, through .NET
$t = [System.IO.File]::ReadAllText($path, [System.Text.Encoding]::UTF8)

# The grep tool renders correctly
```

The `read` tool shows mojibake for these files. Use `grep`, or an explicit
UTF-8 decode, for anything where the exact characters matter.

## The upstream Swift is the authority

Where the documentation and the source disagree, the source wins. The
documentation describes what a feature is for; the source says what it does.

That is why the ledger is generated from the documentation — it is the
exhaustive list of behaviours — and then checked against the source as each
feature is ported, with the tests as the evidence.

## See also

- [Port note 002 — The parity ledger is generated](002-parity-ledger.md)
- [Port note 003 — The Windows API shapes in windows-rs 0.62](003-windows-rs-api.md)
- [`docs/DECISIONS.md`](../DECISIONS.md)
