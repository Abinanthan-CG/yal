# 025 - What Raycast for Windows is actually made of

Date: 2026-11-15
Subject: an architectural study of the app this port replaces, and what it changes here

## Why this note exists

The port exists to replace Raycast. Knowing how Raycast is built is not a
curiosity - it decides three things: whether the native palette is the right
call, whether a port should ever grow a second runtime, and what "as
comfortable as Raycast" has to mean.

This note records what a static study of Raycast for Windows 2.12.1.0 (x64,
MSIX) found, and what this port does about it.

**The ground rule.** This is a behavioural study. No third-party code, table,
naming or asset is taken into this repository. Every behaviour this port
adopts is written again in Rust, with its own tests. What follows is
architecture, not implementation.

## What was found

**Raycast for Windows runs at least four runtimes at once.**

| Piece | What it is | What it does |
| --- | --- | --- |
| The host | a .NET application | windows, tray, system calls, the WebView2 |
| The backend | Node.js, a bundled runtime | most of the product: AI, clipboard, indexer, extensions, snippets, quicklinks, notes, themes, settings |
| The core | a native library | owns the backend's lifetime, and speaks to it |
| The helpers | several small native programs | clipboard and UI access, a system service, a startup launcher, a performance monitor, a trace recorder |

The frontend is a React application in a WebView2, one bundle per window:
the main window, the AI chat window, the settings window, the theme studio,
the notes window and so on. Every window is its own web page.

**The bridge between them is one message channel.** The web page and the
native side talk over the WebView2 postMessage channel with a small
envelope - a request carries an id, a method and some data; a reply carries
the id and either the data or an error; a notification carries no id. Around
**1149 methods** cross that channel, routed to one of three places: the host
(things only the host can do), the backend (most of the product), or an
external bridge.

**State lives in encrypted per-domain databases.** There is one database per
subject - the main store, the clipboard, AI, settings, emoji, screenshots,
frecency, the application index, the user's activity, extensions - each with
its own file under the local application data folder. The files are encrypted
at rest; the key is a 64-character secret held in Windows Credential Manager,
mirrored through a DPAPI-protected store, with a fallback for the session.
The host refuses to silently invent a new key when a database already exists.

**Extensions are Node extensions.** An extension is a package with a manifest,
living in the user's own configuration folder, with its data and its cache
beside it. The authoring API and the templates ship inside the application.

**Searching is ripgrep.** Content search is a copy of ripgrep, alongside a
separate indexer program and a scanner service for the file system.

**Updates are the store's business.** The application downloads a signed
package and hands it to Windows to install.

## What this changes here

**One runtime, and it stays one.** Raycast needs a .NET host, a Node backend
and a native core to do what this port does in a single Rust binary. That is
not a criticism of Raycast - its backend is where the extensions, the AI and
the indexer live, and a Node runtime is a reasonable place for all of that.
But it is a decision for this port: **YAL does not grow a second runtime.**
The calculator, the clipboard, the snippets, the file index, the palette's
decisions and the settings are all Rust, in one process, and the only thing
that may ever be spawned is the thing the user asked to open.

**The native palette is right, and the study says so twice.** Raycast's own
front end is a web view, in every window, and it pays for it: 914-929 MB of
working set on this machine, of which 488 MB belongs to six WebView2
processes, plus 312 MB for the host and 102 MB for two screen-dimming
helpers. The palette is the one surface that is opened dozens of times a day,
and it is the one surface a browser engine helps least. Port note 024 has the
measurement and the reasoning; this note explains that the app we are
replacing reached the same place by a different road - and paid 400 MB more
for it.

**Storage stays plain.** Upstream Tinycast keeps its stores in ordinary
SQLite files and its secrets in the keychain; this port keeps its stores in
ordinary SQLite files and its secrets in Windows Credential Manager. Raycast
encrypts its databases at rest, which is a defensible choice for an app that
holds a clipboard history. This port does not add that in this pass: it is a
real decision with a real cost, and it will be taken on its own merits rather
than by imitation. The secrets that matter - the encryption keys, the tokens -
are in Credential Manager either way.

**The IPC surface is a warning, not a blueprint.** 1149 methods across one
message channel is how a frontend of that size stays in touch with a backend
of that size. This port has no such channel between the palette and the
engine, because the palette and the engine are the same process talking
through ordinary Rust calls. When the palette is native and the settings
window follows, the surface should stay in the dozens, not the thousands.
Each method that does exist should have a test on this side of it.

**What we are not copying.** The extension model, the AI runtime, the MCP
bridge, dictation, the store and the theme studio are all out of scope for
this port, by the scope decision of 2026-11-11. They are recorded here as
facts about Raycast, not as a backlog.

## What this note does not claim

Nothing here was measured about Raycast's behaviour, only its structure; the
numbers in port note 024 are the measured ones. The study was static: code
built at run time was invisible to it, and a method it could not see was not
evidence of absence. Two questions it could not settle are open: the
transport between the core and the backend process, and whether the WebView2
surface exposes anything beyond the message channel.
