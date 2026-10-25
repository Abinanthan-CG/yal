# Feature parity audit

An evidence-based recount of the offline launcher feature set against what
this build carries, written when the remaining waves needed to be picked
from facts instead of a wish list. The launcher side comes from
[observations.md](observations.md) §5 — the clean-room ledger, derived from
the shipped files and not from their code. The build side is the README's
feature table checked against the crates. Status words mean:

- **Built, verified** — carried and confirmed on the build machine, live or
  by timed probe.
- **Built** — carried, covered by local tests, not traced to a live run for
  this audit.
- **Partial** — the area exists, with a stated slice missing.
- **Absent** — launcher carries it; this build does not.

Excluded by design throughout: AI (chat, completions, Quick AI), cloud
sync/account, the extension runtime (deferred, slot reserved), and any
network fetch, per the requester and clean-room rules. Nothing below
re-argues those.

## The recount

| Ledger area (§5) | Status | Evidence and missing slice |
| --- | --- | --- |
| Global hotkey + palette | Built, verified | Round trip live: park-and-return hides and restores above every window; `WM_HOTKEY` + state poll both carry the press. |
| Application launcher | Built | `yal-index` over Start Menu shortcuts, Store packages, PATH; `rc apps` lists. |
| File search | Partial | Documents, desktop and downloads with tiered search; image preview and EXIF wave-verified live drive. A whole-drive index — launcher's `app_index.db` scale — is not carried. |
| Clipboard history | Partial | Background watcher keeps text history searchable; image entries are detected but not stored (yal-system/clipboard reads text only). |
| Calculator | Built | Arithmetic, units, static currencies with recorded vintage, dates; verified against test vectors. |
| Snippets / text expansion | Built, verified | Live once end to end: expansion typed into Notepad after a focus check, clipboard fallback on refusal. |
| Emoji picker | Partial | 363 curated entries with meaning keywords; launcher ships the full Unicode set. Mechanical gap, no design needed. |
| Quicklinks | Built | `{query}` templates plus bundled search domains. |
| Screenshots | Partial | Whole-primary-screen framebuffer read (`rc screenshot`,
  screenshot.rs); launcher's area-select and per-window capture are absent. |
| Window management | Partial | Focus, close, minimise, maximise, restore, half-snap from the palette, verified through `window-controls`; an arbitrary rectangle is CLI-only — `window-controls::place` is not wired to the palette. |
| Colour picker / converter | Partial | Conversion both ways (named, hex, RGB, HSL) with tests; the screen eyedropper — pick a colour off the screen — is absent. |
| Currency / unit conversion | Built | Units full; currencies static by design (offline rule), vintage recorded. |
| Date & time | Partial | Time zones by IANA name with live offsets, date arithmetic in the calculator; launcher's calendar command (reading one's events) is a design question this build has not taken. |
| Bookmarks / notes | Partial | Bookmarks built and searchable together with quicklinks; note-taking is absent — a local store question, designable like reminders. |
| QR generation | Built | `yal-data::qr`, rendered in the palette and by `rc qr`. |
| Password generation | Built | Length and character-class rules. |
| Regular expressions | Built | Capture groups shown. |
| Menus, preferences, search UI | Partial | Preferences pane exists (gear, hotkey recording, theme, scripts folder — each verified live; the pane's drawing was never clicked start to finish). launcher's secondary action menu — `action-menu` in the ledger — is absent here: every result carries exactly one action and Enter performs it. |
| Timers | Built, verified | Toast via identity registration seen on screen; palette banner fallback; the one-second UI-loop cadence failure that made reminders late was measured and moved to a watcher thread. |
| Reminders | Built | Oct-2026 wave. One parser shared by palette and CLI; stored locally (no Apple Reminders on Windows); due watcher verified with a timed probe, cleared on the dot at t+10s; toast delivery itself not yet seen for a reminder. |

## Gaps the ledger does not list, checked in code

- **Script full output in the palette:** `compact`/`silent` toast as designed;
  `full` copies the output with a toast naming it. launcher shows the output
  inside its window. Self-contained UI slice.
- **No fallback queries:** launcher offers an app-wide fallback (a choice act
  when nothing matches); the build offers none. Related to the absent
  secondary-action surface above.
- **One action per result:** confirmed by reading the UI this audit — there
  is no Cmd+K-style second list anywhere in the palette.

## Order recommended by this audit

Soonest-first by scope, highest leverage:

1. Wire `window-controls::place` into the palette — closes the window-management
   partial with code that is already verified elsewhere.
2. Show script `full` output inside the palette — small, before any new surfaces.
3. Full Unicode emoji dataset — mechanical, offline, removes the curious
   smallness of the picker.
4. Secondary action menu — a design wave: one surface improves parity for
   quicklinks, files, processes, windows at once, and enables the app-wide
   fallback act.
5. Notes in the local store — reminders' pattern again: local, no Apple
   counterpart on Windows, parser-less so simpler.
6. Area/window screenshot — screen sampling exists in the shape of the
   framebuffer read; needs the region-selection interaction.
7. Colour eyedropper — same screen-read foundation.
8. Whole-drive file index — largest index wave; needs a design for
   freshness and cost before anything is written.
9. Preferences pane click-along — needs the user's hands, no code.

Deferred pending a design decision: calendar event reading (what is the
local source on Windows?), the app-wide fallback beyond item 4's decision.
