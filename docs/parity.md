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
| File search | **Built** | Documents, desktop and downloads with tiered search; image preview and EXIF wave-verified live drive. The whole-drive index — the target's `app_index.db` scale — closed as the second Oct-2026 wave: fixed drives only, walked by a thread of its own that keeps per-directory modification times (nanoseconds; a millisecond floor buried same-millisecond writes in the first test build) and re-lists only moved directories, with an exact per-row deletion — no `LIKE`-prefix sweeps. Live on this machine: a cold pass 52 s → 159,524 files over 16,674 directories; a warm pass 0.78–0.88 s; a file created in the home root (outside the three live-walk folders) was answerable only after the refresh and both `yal files` and `yal search` returned it; deleted, the next pass re-listed one directory and it was gone. The supplement never shadows the live walk, it only adds. Honest edges: skip list is by name (`node_modules`, `target`, caches, `AppData`, `ProgramData`, `Windows`, `Program Files`, dot-dirs; junctions treated absent — a recorded dir renamed into a skip name is evicted the pass after, but whole already-recorded subtrees under *newly* skip-listed ancestors reset via `--off`/`--refresh`), the snapshot is republished only after a pass that changed something so a query can see a stale directory when its change is under thirty minutes old, and the palette's walker counts on SQLite's two-second busy wait rather than a queue. |
| Clipboard history | Partial | Background watcher keeps text history searchable; image entries are detected but not stored (yal-system/clipboard reads text only). |
| Calculator | Built | Arithmetic, units, currencies and dates; the NL phrase layer closed as the Oct-2026 wave 9a — `10 days from now`, `in 2 weeks`, `90 minutes ago`, `tomorrow`, `yesterday`, `next friday`, `last monday`, `20 percent of 250` become ordinary expression trees over the one evaluator (phrase grammar ported from Vicinae's Numen backend, BSD-3-Clause notice carried in `yal-calc/src/nl.rs`). Months and years are calendar spans, not factors: a month past the 31st of January lands on February's last day, and the local wall time is kept; weekday shifts never answer for today. Answers verified live through `yal calc`. |
| Snippets / text expansion | Built, verified | Live once end to end: expansion typed into Notepad after a focus check, clipboard fallback on refusal. |
| Emoji picker | **Built** | Oct-2026 wave. The full Unicode 16.0 fully-qualified catalogue — 1,906 entries — generated at build time from Unicode's `emoji-test.txt` plus the CLDR English annotations, with Vicinae's keyword-building algorithm ported (`gen/emoji-gen.mjs`, GPL-cited; §6.3). Live: the doctor reports `emoji 1906`. |
| Quicklinks | Built | `{query}` templates plus bundled search domains. |
| Screenshots | **Built** | Whole-screen read was the start; the Oct-2026 wave added the rest. Region capture (`--region x,y,w,h`, clamped onto the screen — parse and clamp are tested), per-window capture through `PrintWindow` with `PW_RENDERFULLCONTENT` (verified live on a DirectComposition browser window), pixels-to-clipboard via arboard, stamped PNGs into `Pictures\YAL`. In the palette: "Capture the screen" (the palette leaves the screen a beat before the frame is read, so it is not in its own picture), "Capture an area" (the screen freezes under the palette, which grows over the screen while you drag a band, with the band's pixel size live), and per-window capture from a window row's secondary panel. Honest edges: the drag overlay is primary-screen only (multi-monitor is a design for another day), the pixels freeze before the drag so what you capture is what was on screen when the act began, the CLI's own capture runs DPI-virtualized (a whole-screen PNG on this machine is 1707×960 logical for a 125% display), and — like the preferences pane — the drag has not yet been driven by a live hand. Also fixed in passing: the framebuffer arrives BGRA and the original full-screen path wrote it raw, which traded red for blue; every capture now swizzles to RGBA and forces the alpha opaque. |
| Window management | Built, verified | Focus, close, minimise, maximise, restore and the placement presets — halves, quarters, thirds, centre, full — from palette or CLI. Quarter/third/centre geometry verified live on the build machine by reading the moved window's rectangle back (`top-left-quarter` 853×462 at the corner, `middle-third` 569 wide, `center` from a corner lands at (1707−853)/2 = 427 with size unchanged). An arbitrary rectangle stays CLI-only, which matches the presets a launcher menu is for. |
| Colour picker / converter | **Built** | Conversion both ways (named, hex, RGB, HSL) with tests; the eyedropper closed as the Oct-2026 wave — "Pick a colour" in the palette runs the same stop-the-world machine as the area capture: the screen freezes a beat after the palette leaves, a loupe (drawn straight out of the frozen texture, nearest-texel so magnified pixels stay crisp) follows the pointer with the to-be-taken pixel marked and the colour named, a click or Enter takes it, the hex goes to the clipboard with an `rgb`/`hsl` receipt row left in front of the results. verified live, its sample read back identical to an independent region
capture of the same screen point (rgb(23, 23, 23) both ways). Honest edges: primary screen only, the screen is frozen at the act's start so the loupe does not show live updates, and the loupe click itself awaits a live hand like the other overlays. |
| Currency / unit conversion | **Built** | Units full. Currencies closed as the Oct-2026 wave 9a: the bundled snapshot (25 codes, indicative, its own vintage) is the offline default, and an explicit act — `yal rates-update`, or the palette's `Update exchange rates` row — fetches once from Frankfurter (ECB daily fixing, 15 s / 2 MB caps), validates the reply whole (three-letter codes, finite positive rates, a vintage day; one doubtful row throws the whole reply out) and stores it in one transaction. The store's table overrides and extends the bundle; a code the bundle has no symbol for is shown as its bare code. `base=USD` replies list no USD row, so a USD/1.0 anchor is appended. The revised offline rule in force: fully functional unplugged, network allowed where it earns its place by an explicit act — and this is the build's only network act. Live-verified: the store fetched 2026-10-30 (30 currencies) and `yal calc "100 usd in eur"` answers 89.509 EUR `rates as of 2026-10-30`; a fresh machine with no fetch yet answers 92 EUR `rates as of 2026-01-15 (bundled)` — both honest, both labelled. Rates keep their vintage in every consumer: the palette row's subtitle, the CLI's answer, and the update act's subtitle ("fetch a fresh table; the bundled snapshot as of … answers until then"). |
| Date & time | Partial | Time zones by IANA name with live offsets; date arithmetic in the calculator: fixed-factor durations (seconds through weeks) and, since wave 9a, calendar month/year moves and next/last weekday — all judged on the local calendar. The target's calendar command (reading one's events) is a design question this build has not taken. |
| Bookmarks / notes | **Built** | Bookmarks built and searchable together with quicklinks; the note-taking gap closed as an Oct-2026 wave — `note <text>` from the palette or the CLI, one record interpretation shared by both, listed newest first, copied with Enter, removed from the secondary panel (and `yal note-remove`). The reminders' key pattern (`note:<id>` in settings), no parser needed. |
| QR generation | Built | `yal-data::qr`, rendered in the palette and by `yal qr`. |
| Password generation | Built | Length and character-class rules. |
| Regular expressions | Built | Capture groups shown. |
| Menus, preferences, search UI | Partial | Preferences pane exists (gear, hotkey recording, theme, scripts folder — each verified live; the pane's drawing was never clicked start to finish). The secondary action menu — `action-menu` in the ledger — is **built** as the Oct-2026 wave: `Ctrl+K` beside the selected row, with the app-wide fallback act beside it; see the "Secondary action menu + app-wide fallback" entry below for the honest edges. The pane's drawing and the panel itself still await a live hand. |
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
5. ~~Notes in the local store~~ — done: the reminders' pattern again; listed
   newest first, copied, removed; no parser because a note needs nothing
   but its text. See the bookmarks/notes row above.
6. ~~Area/window screenshot~~ — done as the Oct-2026 wave: region CLI
   (`--region x,y,w,h`), window capture (`--window`, PrintWindow +
   `PW_RENDERFULLCONTENT`), pixels-to-clipboard, the leave-then-read
   whole-screen act, and the freeze-and-drag area overlay in the palette,
   with a receipt row left in front of the results. Primary-screen only
   and the drag awaits a live hand; both recorded in the table above.
7. ~~Colour eyedropper~~ — done as the same Oct-2026 machine's third
   act: freeze-and-loupe in the palette (click or Enter takes the pixel
   under the crosshair), hex to the clipboard, an rgb/hsl receipt row,
   and `yal color --at x,y` on the command line. Primary-screen only and
   frozen at the act's start; the click awaits a live hand.
8. ~~Whole-drive file index~~ — done as the second Oct-2026 wave, design
   first (architecture.md, "The whole-drive file index"): fixed
   drives, per-directory mtime freshness (nanosecond floors after the
   first test build buried a same-millisecond write), exact per-row
   deletion, a name-matched skip list, snapshots in memory for the
   synchronous search. Measured honestly in the table above: cold 
   52 s over 16,674 directories, warm under a second, creation and
   removal both proven live through `file-index` / `files`.
9. Preferences pane click-along — needs the user's hands, no code.
11. ~~The launcher's home is the tray~~ — done on the same day, at the
   launcher user's ask: the palette parks off-screen rather than hiding
   (a hidden Windows window stops painting, and with it the event loop
   where the global hotkey is polled — verified live early in the
   build), and a window the manager still counts as visible keeps its
   taskbar button, which is a ghost for a thing not on screen. The
   answer is the Windows *tool-window* exclusion, claimed twice: at
   birth through egui's own `with_taskbar(false)` — winit answers it
   with `skip_taskbar`, so the window is styled a tool window before
   it ever shows — and again every frame by a style probe that heals
   whatever undoes the style at runtime. No taskbar button, no Alt+Tab
   entry — a launcher is not a document. Both halves earned their
   places live: the first attempt, a one-shot applied once per
   process and re-claimed at every show, read back `tool=False` on
   the built palette — its style write had landed on the wrong
   window (the tray icon's little helper, this process's biggest
   visible one before the palette exists) had consumed the one-shot
   for good, and the framework's `WS_EX_APPWINDOW` brand kept the
   ghost on the taskbar regardless of the bit under it. The tray icon (built but
   orphaned until now) is started with the palette and forwards its
   three commands — Open Launcher, Preferences…, Quit — onto the
   palette's own channel through a small forwarder thread; Quit ends
   the process. The first tray, shown to its user, sat inert: showing
   an icon is not running it — a click is a queue message for the tray
   thread, and the crate opens the menu only while that thread
   dispatches its Win32 messages, which the first loop never did (it
   slept and polled the event channels, which are filled during
   dispatch). The loop now pumps the queue before it polls. In
   passing: the tray's unimplemented
   "Clipboard History" menu item was scrubbed (the palette already
   lists that history in an empty search), and `Tray::stop` now
   releases the tray thread instead of joining it, which would have
   held the process open until the tray's next poll. Honest edges: the
   icon is drawn in code — a slate square with a lighter bar, the
   palette's own row shape, monochrome so no byte-order surprise can
   recolour it — until the brand-icons wave brings a real one; and the
   button's absence is proven by the style readback itself: the live
   palette, built with all of the above, reads back the tool-window
   style with the app-window brand gone (`EXSTYLE 0x198`), the exact
   style a manual `SetWindowLongPtrW` experiment on that same window
   had already shown is what takes the taskbar button away.
10. ~~NL calculator + fetched exchange rates~~ — done as the first
   half of the Oct-2026 wave 9, design first (architecture.md, "The
   calculator that answers in plain words" and "The one network act"):
   the phrase pre-parser over the one evaluator (Numen's shapes,
   BSD-3-Clause cited in the module), calendar month/year arithmetic
   day-clamped on the local zone, the `next`/`last` weekday functions
   that never answer for today, and the percent-of sugar. Rates: the
   store's third migration, whole-table writes only, a vintage stamped
   on every row, the Frankfurter fetch behind 15 s / 2 MB caps run only
   when asked — `yal rates-update` or the palette's update act, which
   refuses to run for an in-memory store and answers by toast. Live:
   30 currencies stored as of 2026-10-30, `100 usd in eur` → 89.509 EUR
   with the stored vintage named; with no stored table the bundled
   snapshot answers and is named as bundled. In passing, the clipboard
   tests now retry the shared-resource race a history keeper imposes —
   another launcher running on this machine watches writes — instead of
   failing on it, with the shape refusals asserted before the clipboard
   is touched at all.

Deferred pending a design decision: calendar event reading (what is the
local source on Windows?), the app-wide fallback beyond item 4's decision.
