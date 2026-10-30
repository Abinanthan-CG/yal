# Feature parity audit

An evidence-based recount of the offline feature set of the observed launcher against what
this build carries, written when the remaining waves needed to be picked
from facts instead of a wish list. The observed side comes from
[observations.md](observations.md) §5 — the clean-room ledger, derived from
the shipped files and not from their code. The build side is the README's
feature table checked against the crates. Status words mean:

- **Built, verified** — carried and confirmed on the build machine, live or
  by timed probe.
- **Built** — carried, covered by local tests, not traced to a live run for
  this audit.
- **Partial** — the area exists, with a stated slice missing.
- **Absent** — the observed launcher carries it; this build does not.

Excluded by design throughout: AI (chat, completions, Quick AI), cloud
sync/account, the extension runtime (deferred, slot reserved), and any
network fetch, per the requester and clean-room rules. Nothing below
re-argues those.

Posture note (October 2026): the project is now GPL-3.0-only, so gaps below
may be filled by porting from [Vicinae](https://github.com/vicinaehq/vicinae)
(§6.3 of the ledger) or by reading its published docs — the method is chosen
per wave and recorded with the wave.

## The recount

| Ledger area (§5) | Status | Evidence and missing slice |
| --- | --- | --- |
| Global hotkey + palette | Built, verified | Round trip live: park-and-return hides and restores above every window; `WM_HOTKEY` + state poll both carry the press. |
| Application launcher | Built | `yal-index` over Start Menu shortcuts, Store packages, PATH; `yal apps` lists. |
| File search | Partial | Documents, desktop and downloads with tiered search; image preview and EXIF wave-verified live drive. A whole-drive index — the target's `app_index.db` scale — is not carried. |
| Clipboard history | Partial | Background watcher keeps text history searchable; image entries are detected but not stored (yal-system/clipboard reads text only). |
| Calculator | Built | Arithmetic, units, static currencies with recorded vintage, dates; verified against test vectors. |
| Snippets / text expansion | Built, verified | Live once end to end: expansion typed into Notepad after a focus check, clipboard fallback on refusal. |
| Emoji picker | **Built** | Oct-2026 wave. The full Unicode 16.0 fully-qualified catalogue — 1,906 entries — generated at build time from Unicode's `emoji-test.txt` plus the CLDR English annotations, with Vicinae's keyword-building algorithm ported (`gen/emoji-gen.mjs`, GPL-cited; §6.3). Live: the doctor reports `emoji 1906`. |
| Quicklinks | Built | `{query}` templates plus bundled search domains. |
| Screenshots | Partial | Whole-primary-screen framebuffer read (`yal screenshot`,
  screenshot.rs); the target's area-select and per-window capture are absent. |
| Window management | Built, verified | Focus, close, minimise, maximise, restore and the placement presets — halves, quarters, thirds, centre, full — from palette or CLI. Quarter/third/centre geometry verified live on the build machine by reading the moved window's rectangle back (`top-left-quarter` 853×462 at the corner, `middle-third` 569 wide, `center` from a corner lands at (1707−853)/2 = 427 with size unchanged). An arbitrary rectangle stays CLI-only, which matches the presets a launcher menu is for. |
| Colour picker / converter | Partial | Conversion both ways (named, hex, RGB, HSL) with tests; the screen eyedropper — pick a colour off the screen — is absent. |
| Currency / unit conversion | Built | Units full; currencies static by design (offline rule), vintage recorded. |
| Date & time | Partial | Time zones by IANA name with live offsets, date arithmetic in the calculator; The target's calendar command (reading one's events) is a design question this build has not taken. |
| Bookmarks / notes | Partial | Bookmarks built and searchable together with quicklinks; note-taking is absent — a local store question, designable like reminders. |
| QR generation | Built | `yal-data::qr`, rendered in the palette and by `yal qr`. |
| Password generation | Built | Length and character-class rules. |
| Regular expressions | Built | Capture groups shown. |
| Menus, preferences, search UI | Partial | Preferences pane exists (gear, hotkey recording, theme, scripts folder — each verified live; the pane's drawing was never clicked start to finish). The target's secondary action menu — `action-menu` in the ledger — is absent here: every result carries exactly one action and Enter performs it. |
| Timers | Built, verified | Toast via identity registration seen on screen; palette banner fallback; the one-second UI-loop cadence failure that made reminders late was measured and moved to a watcher thread. |
| Reminders | Built | Oct-2026 wave. One parser shared by palette and CLI; stored locally (no Apple Reminders on Windows); due watcher verified with a timed probe, cleared on the dot at t+10s; toast delivery itself not yet seen for a reminder. |

## Gaps the ledger does not list, checked in code

- **Script full output in the palette:** ~~`compact`/`silent` toast as designed;
  `full` copies the output with a toast naming it; the target shows the output
  inside its window.~~ Done as an Oct-2026 wave: a `full` script's whole output
  is now drawn in an overlay above the search (clipboard keeps its copy duty,
  `C` copies again, `Esc`/`Enter` return to the list), with the palette brought
  back if it had parked while the script ran.
- **Fonts (Vicinae-inspired, not ported):** built from GDI's
  `EnumFontFamiliesExW` enumeration — 233 families on the build machine, live-verified
  over the CLI with filters. Lives under the `font ` prefix with Enter copying
  the family name; the palette does not yet render a family's name *in* that
  family, which the target's font pane does.
- **Secondary action menu + app-wide fallback:** ~~no fallback queries~~ and
  ~~one action per result~~ — both done as one Oct-2026 wave. Every result
  with a second act now offers a panel opened with `Ctrl+K`
  (applications, files, quicklinks, snippets, scripts and clipboard entries
  carry secondaries; on clipboard entries the act removes the entry from
  the history via a new per-entry store delete). The web-search candidate
  is no longer an always-ranked row: it became the fallback act, offered
  under "Nothing matched" — together with "open it as an address" when the
  words have that shape — only when nothing the user keeps matched and the
  offers were all query-act generators (QR code, regex, calculator, text
  transforms). Recorded honestly: on this build machine's store the seeded
  quicklink templates match every query through their filled URLs, so they
  play the fallback in practice and the fallback section stays out of the
  way; the surface exists for fresh stores and for stores without seeds.
  The window list keeps one candidate per act rather than folding acts
  into a panel — a design choice that keeps window acts searchable and is
  still open to revisiting. The panel's drawing and key path, like the
  preferences pane's, have not yet been driven by a live hand.

## Order recommended by this audit

Soonest-first by scope, highest leverage:

1. ~~Wire `window-controls::place` into the palette~~ — done as the richer
   preset set (quarters, thirds, centre), geometry extracted into one pure,
   tested function and verified live by reading the moved window's rectangle
   back; see the window-management row above.
2. ~~Show script `full` output inside the palette~~ — done: overlay sheet above
   the search, delivered from the script thread through the palette channel
   with a repaint wake.
3. ~~Full Unicode emoji dataset~~ — done: 1,906 entries generated from Unicode
   upstream with Vicinae's keyword approach; see the emoji row above.
4. ~~Secondary action menu + app-wide fallback~~ — done as one wave: the
   `Ctrl+K` panel beside the selected row and the "Nothing matched"
   fallback section; the interplay with the seeded search templates and
   the unfolded window acts are recorded in the gaps list above.
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
