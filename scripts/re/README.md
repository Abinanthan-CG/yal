# The reverse-engineering scripts

These are the scripts used to study Raycast for Windows, the app this port
replaces. They are kept so the study can be repeated, checked or corrected.

**The ground rule.** This is a behavioural study. Nothing found by these
scripts may be copied into the port: no code, no tables, no naming, no
assets. What comes out of them is a note in `docs/port-notes/`, and every
behaviour the port adopts is written again in Rust with its own tests.

## The toolchain

It lives outside the repository, in `C:\Users\dabin\tools\rea`, because none
of it is part of the port:

| Tool | Why |
| --- | --- |
| Ghidra 12.2 | the deep analyser, run headless over the native binaries |
| Rizin 0.9.1 | fast command-line triage and disassembly |
| ILSpy (`ilspycmd` 11.2) | turns the .NET host back into readable C# |
| A .NET SDK | what `ilspycmd` needs to run |

The application is installed under `Program Files\WindowsApps`, whose
permissions upset ordinary tools. `stage.py` copies what the analysis needs
to a plain folder first.

## The scripts

| Script | What it does |
| --- | --- |
| `stage.py` | copies the binaries and bundles out of the WindowsApps folder |
| `peek.py` | prints a window of text around a regex match in a huge minified file, with the byte offset |
| `css-tokens.py` | lists every CSS custom property in the built stylesheets |
| `css-colors.py` | lists the colour roles that have literal values |
| `db-schema.py` | copies the local databases and reports their schema |
| `db-restore.py` | restores the SQLite magic that was replaced, then reports the schema |
| `read-cred.ps1` | reads one credential from Windows Credential Manager, in memory only |
| `addon-api.js` | lists the classes and methods a native addon exports |
| `addon-enums.js` | lists the enumeration values an addon exports |
| `enum-raw.js` | prints the raw shape of one exported enumeration |
| `data-model.js` | opens the local databases through the application's own data layer and reports the record shapes |

## Two things worth knowing

**The bundles are minified into one line.** `backend/index.mjs` is 7.9 MB on
a single line. Never print one; use `peek.py` with a tight pattern and read
the window around the offset.

**The databases are encrypted, and the key is available.** The eleven local
databases are encrypted with a 64-character secret held in Windows Credential
Manager under `Raycast-Production/BackendDBKey`. `read-cred.ps1` reads it into
memory and never writes it to a file; `data-model.js` opens a *copy* of the
data folder with it, so the running application is never disturbed and nothing
is migrated in place.

Only structure is ever reported - table names, columns, record fields and
counts. The user's own content is not read out and not written down.
