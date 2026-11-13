# 022 - The palette's screen system, and the file-search screen

Date: 2026-11-13
Crate: `crates/tc-palette`
Subject: the palette's screens, its keys, its settings, and the file-search screen

## Why this note exists

Upstream's palette is one floating panel: the launcher is its root screen and
every other screen stacks on it. Until now YAL had a search field and a result
list, and the keys were handled in the window.

That was the wrong place for them. A key's meaning is a decision, and a
decision that cannot be tested is not a decision - the frontend has no test
framework, so a key handled there could never earn evidence, and the parity
ledger could only ever say `NOT PORTED`.

So the decisions moved into Rust. `tc-palette` holds them, and the window asks
what a key means and is told what to do about it.

## The crate

| Module | What it holds |
| --- | --- |
| `keys` | the keys, the modifiers, and the translation from upstream's Mac chords to Windows |
| `screen` | the screens, and how they stack |
| `settings` | the palette's own settings, with upstream's defaults |
| `action` | what a key does, and the menu that lists it |
| `palette` | the state machine that turns a key into an action |
| `files` | the file-search screen, and the files the launcher offers below its own rows |

**117 tests.**

## The translation, and the line it holds

`docs/SHORTCUTS.md` already held the whole shortcut table before this crate
existed. The crate implements that table, and the test holds the line the
document states: **a built-in shortcut must never shadow standard text
editing**. Every chord in the upstream table is translated, the translations
are pairwise distinct, and none of them is <kbd>Ctrl</kbd>+<kbd>C</kbd>,
<kbd>Ctrl</kbd>+<kbd>V</kbd>, <kbd>Ctrl</kbd>+<kbd>X</kbd> or
<kbd>Ctrl</kbd>+<kbd>Z</kbd>.

The three chords that are not a plain swap are in the crate as they are in the
document: <kbd>⌘</kbd><kbd>delete</kbd> becomes
<kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>Backspace</kbd>,
<kbd>⌥</kbd><kbd>tab</kbd> becomes <kbd>Ctrl</kbd>+<kbd>Tab</kbd>, and
<kbd>⌘</kbd><kbd>esc</kbd> has two answers,
<kbd>Ctrl</kbd>+<kbd>Esc</kbd> and <kbd>Alt</kbd>+<kbd>Home</kbd>.

The Windows key is never reserved, and there is a test that says so.

## The state machine

The rules the palette follows, all from upstream's *palette* page:

- **The escape chain.** The first press clears the text, the second goes back
  a screen, and the third closes the window. The setting decides whether going
  back is the second step or whether the window closes right away and starts
  at the launcher next time. In the compact mode the palette collapses
  between the two, with either setting.
- **The tab cycle.** The launcher, the AI and the clipboard, skipping the ones
  whose feature is off. A screen outside the cycle returns to the launcher
  with the search text. The one exception is a row that takes arguments: the
  tab moves through its fields first.
- **The emacs chords.** <kbd>⌃</kbd><kbd>N</kbd> and <kbd>⌃</kbd><kbd>P</kbd>
  are <kbd>↓</kbd> and <kbd>↑</kbd>; <kbd>⌃</kbd><kbd>F</kbd> and
  <kbd>⌃</kbd><kbd>B</kbd> move the text cursor - except on the emoji grid,
  where all four move the selection.
- **The slim bar.** In the compact mode the launcher is a bar: the left and
  right arrows choose between the search field and the four shortcuts,
  stopping at either end; the down arrow on the search field expands the list,
  and on a shortcut it opens the item; typing from a shortcut starts a search.
- **The actions menu.** <kbd>Ctrl</kbd>+<kbd>K</kbd> opens it, the menu's
  search filters it, and the escape clears the menu's search first and closes
  the menu second.

## The Settings screen is not a dead end

<kbd>Ctrl</kbd>+<kbd>,</kbd> opens the settings screen, so the screen has to
have something in it. The General pane shows the six settings the palette
itself follows - the appearance, the animations, the interface size, the
window mode, the escape behaviour and the pop-to-root delay - and each one
moves to its next value on return. Each is written to the settings file and
read back when the app starts.

This is the *General* pane only. The other panes - the editor screen, the
output window, the command rows - are the next slice.

## The file-search screen

The engine was already built (`crates/tc-files`): the index, the scope rules,
the incremental pass and the search over it. What was missing was the screen.

- **With nothing typed** the list is **Recent**: the twenty files in the home
  folder that changed most recently, with the hidden ones and the program
  folders left out.
- **With something typed** it is **Results**: every word typed has to appear
  in the name, in any order.
- **The filter is applied during the search**, so it never hides a match that
  the result limit had already cut off. The filter is applied first, and the
  limit is taken afterwards - and a test says so with ten matching documents
  and a limit of two.
- **The seven kinds** are All Types, Folders, Documents, Images, Audio,
  Videos and Archives, and <kbd>Ctrl</kbd>+<kbd>P</kbd> moves through them.
- **The launcher's own rows sit above the files**, so return still opens the
  top result. The files appear below them only when the search has three or
  more characters, and there are at most five of them.

## The file row's actions

A screen that offers an action that does nothing is a dead end, and a dead end
is worse than a missing row. So the file screen offers only what this build
really runs:

| Action | Chord | What it does |
| --- | --- | --- |
| Open File / Open Folder | <kbd>Enter</kbd> | the shell opens it |
| Show in File Explorer | <kbd>Ctrl</kbd>+<kbd>Enter</kbd> | Explorer, with it selected |
| Filter by Type | <kbd>Ctrl</kbd>+<kbd>P</kbd> | the next kind |
| Copy Name | <kbd>Alt</kbd>+<kbd>Ctrl</kbd>+<kbd>C</kbd> | the name goes on the clipboard |
| Copy Path | <kbd>Ctrl</kbd>+<kbd>Alt</kbd>+<kbd>Shift</kbd>+<kbd>C</kbd> | the path goes on the clipboard |
| Move to the Recycle Bin | <kbd>Alt</kbd>+<kbd>X</kbd> | `SHFileOperationW`, with `FOF_ALLOWUNDO` |

Copying the file itself, pasting it into another application and adding it to
the favorites are **not** offered, because they are not built. There is a test
that says every listed action is one this build can run.

The Recycle Bin is a new Windows call: `SHFileOperationW` with
`FOF_ALLOWUNDO`, so the file can be put back. The test moves a real file and
checks that it left its place.

## The manifest, which was not there

The borer audit in [port note 021](021-raycast-for-windows-study.md) found
that the launcher did not carry the manifest `docs/WINDOWS-MAPPING.md` claimed
it did. It does now: `apps/yal/windows/yal.manifest`, embedded through
`tauri-build`'s `app_manifest`, with `asInvoker`,
`PerMonitorV2,PerMonitor`, `longPathAware` and UTF-8 as the active code
page. The whole document was read back out of the rebuilt binary.

## The ledger

**Eighty rows moved**: the palette's key table, the emacs chords, the tab
cycle, the screen stacking, the escape behaviour, the compact bar, the
appearance settings and the General pane; and the file search's screen, its
filter, its actions, its scope rules, its ignore patterns and its search
rules. 333 → 413.

The rows that stay open say why in one line: the palette cannot be dragged,
the pointer is not followed across displays, nothing animates yet, the compact
shortcuts are not in the pane, the scopes are not chosen yet, and so the app
does not build an index of its own.

## Verified

- `cargo test --workspace`: **1 081 tests**, all passing.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo fmt --all -- --check`: clean.
- `cargo run -p xtask -- parity`: 2 309 rows, 413 PORTED; `parity --check` is up
  to date.
- `npm.cmd run check` and `npm.cmd run build` in `frontend/`: clean.
- The manifest: `RT_MANIFEST` read out of `target/release/yal.exe`, whole
  document present.
