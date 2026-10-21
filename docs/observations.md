# Clean-room observation ledger

Target: **launcher for Windows**, installed as an MSIX package.

```
C:\Program Files\WindowsApps\launcher.launcher_2.8.2.0_x64__qypenmj9wpt2a\launcher\
```

| Field | Value |
| --- | --- |
| Package | `launcher.launcher_2.8.2.0_x64__qypenmj9wpt2a` |
| Version | `2.8.2.0` |
| Architecture | `x86_64` |
| Install kind | MSIX / Microsoft Store |
| Process | `launcher.exe` (PID 14184 at time of observation) |

This ledger records what was **observed** about the installed product, what was
**inferred** from those observations, and what remains **unknown**. It is the
provenance record for the rebuild. Implementation follows this ledger; it does
not follow any decompiled implementation.

---

## 1. Process and component topology — observed

| Component | Role | Size |
| --- | --- | --- |
| `launcher.exe` → `launcher.dll` | .NET 8 WPF application shell | 7.32 MB |
| `launcher.UIAccess.exe` | elevated helper for UI automation | 4.84 MB |
| `launcher.SystemService.exe` | background Windows service | 4.29 MB |
| `launcher.CrashHelper.exe` | crash reporter | 0.33 MB |
| `launcher.TraceRecorder.exe` | trace capture tool | 0.82 MB |
| `launcher.StartupLauncher.exe` | autostart entry | 0.42 MB |
| `native.dll` | native core (**Rust**) | 6.97 MB |
| `frontend/` | WebView2 frontend, Vite-bundled React | 52.6 MB, 1363 files |
| `backend/` | Node.js extension host | 82.95 MB `node.exe` |
| `api/` | shipped `@launcher/api` SDK + extension template | 724 KB `index.js` |

`backend/` also carries a second Node runtime (`launcher Content Indexer.exe`)
and a `build/` + `resources/` tree.

### 1.1 Native addons — observed

`backend/*.node` are Node-API addons loaded by the extension host:

| Addon | Size | Language |
| --- | --- | --- |
| `calculator-rs.win32-x64-msvc.node` | 4.31 MB | **Rust** |
| `Calculator.node` | 2.38 MB | C++ (N-API) |
| `data.win32-x64-msvc.node` | 44.01 MB | native |
| `indexer.win32-x64-msvc.node` | 17.10 MB | native |
| `fs-utils.win32-x64-msvc.node` | 1.92 MB | native |
| `process-utils.win32-x64-msvc.node` | 0.28 MB | native |

Swift and .NET support libraries are also present: `swiftCore.dll`,
`Foundation*.dll`, `SoulverCoreDynamic.dll`, `dispatch.dll`.

### 1.2 Rust confirmation — observed

`rea inspect-rust-binary` (provider `rust-auditable`) on both Rust images:

| Image | rustc commit (inferred from `/rustc/<40-hex>/` path) | `.dep-v0` |
| --- | --- | --- |
| `native.dll` | `48a229ceaefd4985c50990b14116b6d856af0985` | absent |
| `calculator-rs.win32-x64-msvc.node` | `b940084d7eb6a299eb4bfeb8e34901bc051e7ac4` | absent |

**Observation.** Both images are compiled Rust.
**Inference.** launcher ships Rust for its native core and for the calculator.
**Unknown.** Neither image retains a cargo-auditable dependency list, so the
exact crate graph is not recoverable from the binary. The rebuild therefore
selects its own dependency set and does not claim to match launcher's.

---

## 2. Extension model — observed

`api/node_modules/@launcher/api/index.js` is the published `@launcher/api` SDK
(`^2.6.3`). `api/template/package.json` is the extension scaffold.

**Command modes** (from the shipped template):

- `no-view` — run a command, render nothing
- `view` — render a React view
- `menu-bar` — render a menu bar extra

**Tool mode** — extensions may declare `tools` (AI-invocable actions) with an
optional confirmation step. *Note: the AI tool invocation path is out of scope
for this rebuild (see §6).*

**Runtime.** `@launcher/api/index.js` is a **bundle** containing a Node-API
compatibility layer: `undici` (fetch/`FormData`/`Headers`), `webidl`
converters, `node:sqlite`, and `node:crypto`. Extensions therefore execute in a
Node-like sandbox with a subset of the standard library.

**Observation.** The extension ABI is a TypeScript/JavaScript API, not a
native ABI.
**Decision.** This rebuild ships **no extension runtime** in the first
milestone. The domain model reserves an extension slot so the boundary can be
filled later without reworking the core.

---

## 3. Frontend — observed

`frontend/` is a Vite production build of a React application:

| Extension | Files | Size |
| --- | --- | --- |
| `.js` | 565 | 13.18 MB |
| `.css` | 157 | 0.98 MB |
| `.svg` | 138 | 0.93 MB |
| `.png` | 468 | 28.91 MB |
| `.html` | 8 | 0.13 MB |

Chunk names are content-hashed and descriptive, e.g.
`about-settings-view-BocsVDph.js`, `action-menu-QT81meYT.js`,
`ai-agents-settings-view-Dq54bziE.css`. The naming shows the product surface
includes settings pages, action menus, activity views, and AI agent settings.

**Decision.** This rebuild uses **egui/eframe**, not a webview. The frontend
bundle is used only as evidence of *what features exist*, never as a source of
implementation.

---

## 4. Local data model — observed

Per-user state lives in `%LOCALAPPDATA%\launcher\`:

| Database | Size | Apparent role |
| --- | --- | --- |
| `app_index.db` | 0.35 MB | installed-application index |
| `main.db` | 0.36 MB | primary store |
| `settings.db` | 0.04 MB | legacy settings (v1) |
| `settings_v2.db` | 0.09 MB | current settings |
| `frecency.db` | 0.11 MB | recency/frequency scores |
| `emoji.db` | 1.08 MB | emoji dataset |
| `clipboard.db` | 0.80 MB | clipboard history |
| `screenshots.db` | 0.16 MB | screenshot metadata |
| `user_activity.db` | 3.09 MB | usage analytics |
| `ai.db` | 0.77 MB | AI conversation state |
| `node_extensions.db` | 0.07 MB | installed Node extensions |

Each database has a `-wal` and `-shm` sidecar, so the schema is in WAL mode.

### 4.1 Storage encryption — observed

The databases are **not** plain SQLite.

Header analysis of `main.db` (372 736 bytes):

```
  0  9e cd c2 b5 1b fb f1 e2 52 d7 4b a9 a3 e3 3d 38   <- ciphertext
 16  10 00 02 02 0c 40 20 20 bd c2 cd 0b 45 1e 5d 2b   <- plaintext
 24  ...                                                <- ciphertext
```

- Bytes 0-15: ciphertext
- Bytes 16-23: **plaintext** SQLite header fields — page size `4096`, write
  version `2`, read version `2`, **reserved bytes `12`**, payload fractions
  `64/32/32`
- Bytes 24+: ciphertext
- No `SQLite format 3` magic, no `sqlite_master`, no `CREATE TABLE` anywhere
  in the file
- Byte distribution is flat (256 distinct values, max bucket 28) on every
  4 KiB page tested

**Inference.** A page-level cipher is applied with the SQLite header fields at
offsets 16-23 left in the clear; the 12 reserved bytes per page are consistent
with a per-page nonce/IV reserved out of the usable page space. The cipher
identity is not established from these bytes alone.

**Unknown.** The key material and its derivation are held by the application.
The cipher is **not** broken as part of this work, and no attempt to do so is
made.

**Consequence for the rebuild.** The rebuild defines **its own** on-disk
schema and format. It does not attempt to read or write launcher's encrypted
databases, and it does not claim compatibility with them.

---

## 5. Feature surface — observed

Feature names are derived from the `frontend/` chunk inventory and the shipped
extension template. This is the scope of the rebuild, minus cloud and AI.

| Area | Observed evidence | Milestone |
| --- | --- | --- |
| Global hotkey + palette | `launcher.UIAccess.exe`, `SystemService` | yes |
| Application launcher | `app_index.db`, `indexer.win32-x64-msvc.node` | yes |
| File search | `fs-utils.win32-x64-msvc.node`, `index/` dir | yes |
| Clipboard history | `clipboard.db`, `clipboard/` dir | yes |
| Calculator | `calculator-rs` (Rust), `Calculator.node` | yes |
| Snippets / text expansion | `snippets` implied by template | yes |
| Emoji picker | `emoji.db` | yes |
| Quicklinks | extension template, `launcher Content Indexer.exe` | yes |
| Screenshots | `screenshots.db` | yes |
| Window management | `UIAccess.exe` | yes |
| Color picker / converter | frontend chunks | yes |
| Currency / unit conversion | frontend chunks | yes |
| Date & time | frontend chunks | yes |
| Bookmarks / notes | `main.db` | yes |
| QR generation | frontend chunks | yes |
| Password generation | frontend chunks | yes |
| Regular expressions | frontend chunks | yes |
| Menus, preferences, search UI | `advanced-settings-view`, `action-menu` | yes |
| **AI chat / agents** | `ai.db`, `ai-agents-settings-view` | **excluded** |
| **Cloud sync / account** | `creds/`, account settings | **excluded** |

---

## 6. Scope decisions

Agreed with the requester:

1. **UI:** egui / eframe. Pure Rust, no webview, no JavaScript toolchain.
2. **Extensions:** deferred. The domain model reserves an extension slot; no
   runtime ships in the first milestone.
3. **Feature scope:** everything launcher offers **except** cloud and AI.

### 6.1 Excluded by the requester

- AI chat, AI agent settings, AI-driven tool invocation.
- Cloud sync, account linking, credential storage (`creds/`).

### 6.2 Excluded by clean-room discipline

- Breaking the page cipher on launcher's databases (§4.1).
- Decompiling `.NET` IL or Swift/C++ bodies to copy implementation.
- Reusing launcher's `@launcher/api` implementation. Its *published* type
  surface may inform the domain model, but no code is copied.

---

## 7. Evidence index

| Evidence | Operation | Subject |
| --- | --- | --- |
| `ev_e80bdab7aab4e40567e72097dc8541c75a5b188865e310193a3e99455e560ac8` | `inspect_rust_binary` | `native.dll` (sha256 `c03989ef…3584`) |
| `ev_…` (calculator) | `inspect_rust_binary` | `calculator-rs.win32-x64-msvc.node` (sha256 `71a83219…7e0bb`) |
| `ev_…` (sqlite) | `inspect_sqlite_database` | 5 databases — rejected as `invalid_format` / encrypted, §4.1 |

The SQLite inspection was attempted on both the live databases and the
unlocked backup copies in `%LOCALAPPDATA%\.launcher.backups\2.8.2.0\`. Both
routes produced the same rejection, consistent with §4.1.
