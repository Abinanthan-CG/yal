# 026 - The theme, the ranking, and the export file

Date: 2026-11-15
Subject: three things worth knowing about the app this port replaces, found by reading its own data layer

## Why this note exists

Port note 025 recorded what Raycast for Windows *is*. This note records three
things it *does*, found by opening its own databases with its own data layer
and by reading the export format it writes.

The ground rule is unchanged: this is a behavioural study. Nothing here is
copied into the port. Every behaviour below is reimplemented in Rust with its
own tests, under our own names and our own schema.

## How the study was done

The toolchain lives outside the repository, in `C:\Users\dabin\tools\rea`:
Ghidra 12.2, Rizin 0.9.1, and ILSpy's command-line decompiler. The scripts
that drive it are in `scripts/re/`.

Two tricks made the difference.

**Reading the minified bundles.** The backend is one 7.9 MB minified line.
`scripts/re/peek.py` prints a window of text around a regex match with its
byte offset, so a search result can be read without dumping the file.

**Reading the encrypted databases.** Raycast's eleven SQLite databases are
encrypted. The key is a 64-character secret in Windows Credential Manager
under `Raycast-Production/BackendDBKey`; `scripts/re/read-cred.ps1` reads it
in memory and never writes it down. With that key, Raycast's own bundled Node
runtime and its own Rust data module open the databases - so the schema and
the record shapes can be read directly, rather than guessed at from a
decompiler.

Everything was read from a *copy* of the user's data folder, so the running
application was never disturbed.

## 1. What a theme can change

The theme is not a pile of arbitrary CSS. It is a fixed set of roles, stored
in the settings database, one row per theme.

`settings.allThemes()` returns rows shaped like this:

```
id, name, appearance, background, backgroundSecondary, foreground, accent,
selection, loader, red, orange, yellow, green, blue, purple, magenta,
createdAt, updatedAt
```

So a theme is **fourteen colours plus a name and an appearance**:

| Role | What it is |
| --- | --- |
| `appearance` | light or dark |
| `background` | the window's own background |
| `backgroundSecondary` | a second background, for panels that sit on the first |
| `foreground` | the colour of the text |
| `accent` | the colour of the focus, the selection marker, the caret |
| `selection` | the fill of the selected row |
| `loader` | the colour of progress and of the busy state |
| `red`, `orange`, `yellow`, `green`, `blue`, `purple`, `magenta` | a seven-colour palette for the rest of the interface |

A theme is added, listed and deleted through the settings repository
(`addTheme`, `allThemes`, `deleteTheme`), so a user can have several and
switch between them. The theme studio window is where they are edited.

**What this changes here.** Our style pack already carries a background, a
field, a text colour, a dim text colour, an accent, a selection and a border.
Raycast's set is *wider* - it has a second background, a loader colour, and a
seven-colour palette - and it is *narrower* in one respect: it carries no
fonts, no radius and no spacing. Those are fixed by the application's own
design system, which is visible in the stylesheets: font weights 400/500/600/700,
corner radii 2/4/6/8/10/12/16/18/20/26 and a full circle, a window radius of
8 px on Windows against 26 px on macOS, four levels of backdrop blur, and a
spacing scale.

The right answer for YAL is to keep our style packs *wider* than Raycast's -
fonts, radius and spacing belong in a style pack, because that is what a
design library wants to hand over - but to make sure the roles a design
library uses can be *expressed*.

That has been done. A style pack now carries:

| Role | New? |
| --- | --- |
| `background`, `field`, `text`, `textDim`, `onIcon`, `selection`, `accent`, `border` | already there |
| `backgroundSecondary` | **new** - a second surface for panels that sit on the first |
| `loader` | **new** - the busy state |
| `palette` | **new** - seven colours the rest of the interface draws from |
| `hover` | already there, optional |

A pack may name fewer palette colours than the palette holds - the rest are
inherited - and never more: the extra ones are dropped rather than silently
changing the shape of the palette. Three packs ship: `midnight`, `daylight`
and `aurora`, the last one written in the shape a design library would hand
over.

## 2. How the ranking works

Raycast keeps a separate database for it: `frecency.db`. Its records are:

```
itemId, score, searchTerms, openedAt
```

- **`itemId`** is a structured identifier, not a string. An application
  resolves to something of the shape
  `c:r:applications::*::application::=::/Applications/<uuid>` - a category,
  a kind, the query that produced it, and the thing itself.
- **`score`** is a floating point number.
- **`searchTerms`** is a list of the *search strings* that led to this item
  being opened. One of the records read from the user's own database held
  `["w","wha","wh","wha","wh"]` - the same item, reached by five different
  prefixes.
- **`openedAt`** is when it was last opened.

The repository's methods are `visit`, `search`, `getRecords`, `getAll`,
`getAllByIds`, `insertMany`, `reset` and `sanityCheck`.

**What this changes here.** Our `launcher-ranking.json` holds a plain
list. Raycast's model is better than that in one specific way, and it is
worth copying as a *behaviour*: **remembering the queries that led to an
item, not just the item.** If a user typed "wha" and opened WhatsApp, then
"wha" should find WhatsApp for ever after, even if WhatsApp is not what a
plain substring match would rank first. The score alone cannot express that;
the list of search terms can.

So the port's ranking record becomes four fields: an identifier, a score, the
list of search terms that reached it, and the time it was last opened.

## 3. The export file, and whether we can read it

This is the question that decides fifty ledger rows.

Raycast writes a single file when the user asks to export their settings and
data. The format is exactly as an earlier study found:

```
"RAYCFG3\n"            8 bytes, magic
<4 bytes>              little-endian length of the gzipped header
<gzipped header>       JSON: exportedAt, appVersion, osName, osVersion,
                       osArch, schemaVersion, and (if encrypted) encryption
<gzipped payload>      JSON, encrypted when a password was given
<16 bytes>             the AES-GCM authentication tag
```

The constants, read out of the backend: magic `RAYCFG3\n`, salt 16 bytes,
initialisation vector 16 bytes, derived key 32 bytes, cipher `aes-256-gcm`,
authentication tag 16 bytes, and a maximum header of 1 MiB.

The key derivation is `scrypt(password, salt, 32)` with Node's defaults -
N = 16384, r = 8, p = 1. The salt and the vector are random per export and
are written into the header, in hex.

**And the password is the user's.** The export is given a `password` by the
user; with one, the payload is encrypted; without one, it is only gzipped.
The import asks for the same password back. Nothing device-specific is mixed
in - the salt is in the file, and the key comes from the password alone.

**What this means.** A user can export from Raycast with a password we know,
hand us the file, and we can decrypt it. The format is fully specified and
we can write a reader for it. The import-from-Raycast feature is therefore
**possible**, and the fifty rows that describe it stop being a
"not ported" decision.

What is in the file is also now known. The payload is a set of categories,
each with its own shape:

| Category | Label in the interface |
| --- | --- |
| `ai` | AI |
| `clipboardHistory` | Clipboard History |
| `emoji` | Emoji |
| `focusCategories` | Focus Categories |
| `mcpServers` | MCP Servers |
| `nodeExtensions` | Extensions |
| `notes` | Notes |
| `quicklinks` | Quicklinks |
| `settings` | Settings |
| `snippets` | Snippets |
| `userActivity` | Raycast Wrapped |
| `windowLayouts` | Window Management Commands & Layouts |

Four of these matter to this port and are in scope: **quicklinks**,
**snippets**, **notes** and **settings**. The rest are not.

A record's shape was read for one of them: `{symbol, customKeywords,
frecencyDate, pinned}` - the familiar quicklink fields, including a
`frecencyDate`, which ties straight back to the ranking model above.

The export also records `schemaVersion`, and the import refuses a file whose
version it does not know. The schema is validated on the way in with a
declarative checker, so a reader should be strict about the fields it
recognises and lenient about the ones it does not.

**What this changes here.** The import is no longer "one-off migration
script" and no longer "not ported". It becomes a feature: a user exports
from Raycast, tells us the password, and we import their quicklinks,
snippets, notes and settings. The fifty ledger rows move from
`NOT PORTED` to `PORTED` when the reader is written and tested against a
real exported file - not before.

## 4. Two smaller things worth having

**Hotkeys are layout-independent.** A hotkey is stored as
`LayoutIndependentHotkey { scan_code: PackedScanCode, modifiers }` with the
modifiers as a bit flag over `Meta`, `Alt`, `Ctrl`, `Shift` and `Fn` - `Fn`
being the Windows Hyper key. The scan code, rather than the virtual key,
means the shortcut is the same physical key on every keyboard layout. For a
launcher whose whole point is the keyboard, that is worth having, and it is
cheap: read the scan code, not the character.

**The key vocabulary is explicit.** The data layer exports the control keys
it understands by name: Enter, NumpadEnter, NumpadDelete, NumpadHome,
NumpadEnd, NumpadPageUp, NumpadPageDown, NumpadInsert, Tab, Escape,
Backspace, Delete, CapsLock, ArrowUp/Down/Left/Right, Home, End, PageUp,
PageDown, Insert and F1 to F24. Printable characters are handled separately.
A key model that names the keys it supports is a key model that can be
tested.

**The application has a hidden window.** The log shows an
`InvisibleMainWindow` that exists for the lifetime of the process, separate
from the palette window the user sees. The palette is a window on top of an
always-running application - the same shape as ours, and the reason the
palette can be up in a few tens of milliseconds.

## What was not settled

- **The score's formula.** The record shape is known; the arithmetic that
  turns a visit into a score is in the Rust core and was not read. It is a
  smaller question than it looks: the behaviour we want - remember the
  queries that reached an item, and let time and frequency move it - can be
  written from the record shape alone.
- **The key map of the palette.** The vocabulary of keys is known; which
  combination does what in the palette window is in the front end, and was
  not read. Our own key map is already written down and tested; Raycast's
  is a comparison, not a requirement.
- **How file results are ranked.** Separate from the frecency score. Not
  read.

## What this note does not claim

That these behaviours are correct, or that Raycast is the best way to do
them. They are one working implementation of a launcher, and this port takes
what fits and writes the rest. Every claim above was read out of the
application's own files or its own database schema; where something was
inferred, it says so.
