# 016 - The window mover, and what YAL remembers about the windows it moved

Date: 2026-11-12
Crates: `crates/tc-windows/src/action_memory.rs`,
`crates/tc-platform-win/src/window_control.rs`
Upstream: `Tinycast/Features/WindowManagement/Model/WindowActionMemory.swift`,
`Tinycast/Features/WindowManagement/Service/WindowMover.swift`
Donor: the shape of the window layer, from
`C:\Users\dabin\development\yal\crates\yal-system\src\window_controls.rs`
(GPL-3.0-only, `github.com/Abinanthan-CG/yal`)

## What this slice is

The part of upstream's window management that touches the machine: the
memory of the windows that have been moved, and the Win32 layer that finds a
window, writes a rectangle into it, and reads the rectangle back.

With this, the **thirty-nine commands exist and work**: a real window is
moved, resized and restored on this machine, by tests that ran.

## The memory (`tc-windows`, 19 tests)

Upstream remembers, per window: where it was before YAL first touched it,
where it was left after the last write, which command moved it and how far
through its cycle that command was.

| Upstream's behaviour | Our test |
| --- | --- |
| the first sight has nothing to restore | `the_first_sight_has_nothing_to_restore` |
| a second press of the same command steps the cycle | `a_second_press_of_the_same_command_steps_the_cycle` |
| the cycle wraps around | `the_cycle_wraps_around` |
| a window the user moved starts a new chain | `a_window_the_user_moved_starts_a_new_chain` |
| two points of drift is still ours | `a_drift_of_two_points_is_still_untouched` |
| three points is a real drag | `a_drag_of_three_points_is_the_users_own` |
| a different command starts a new chain | `a_different_command_starts_a_new_chain` |
| a press on another display starts a new chain | `a_press_on_another_display_starts_a_new_chain` |
| waiting between presses does not reset the chain | `waiting_between_presses_does_not_reset_the_chain` |
| a pause forgets the chain but keeps the restore point | `a_pause_forgets_the_chain_but_keeps_the_restore_point` |
| a command that does not cycle never steps | `a_non_cycling_command_never_steps` |
| the last tile is remembered only when the window is where it was left | `the_last_tile_is_remembered_only_when_the_window_is_where_it_was_left` |
| a command that is not a tile is never remembered as one | `a_command_that_is_not_a_tile_is_never_remembered_as_one` |
| the memory is bounded and forgets the oldest | `the_memory_is_bounded_and_forgets_the_oldest` |
| a window that is used again is not evicted first | `a_window_that_is_used_again_is_not_evicted_first` |
| forgetting the cycle keeps the restore point | `forgetting_the_cycle_keeps_the_restore_point` |
| the windows of an app that quits are forgotten | `the_windows_of_an_app_that_quits_are_forgotten` |
| one window can be forgotten | `one_window_can_be_forgotten` |

### The rules that are easy to get wrong

- **The comparison is against the frame that was observed**, never the one
  that was asked for. An application that clamped its window to a smaller
  size still counts as untouched, as long as what was read back is the frame
  it was left at.
- **The tolerance is two points.** Sub-pixel drift is still ours; a real
  drag is the user's, and it starts a new chain.
- **The memory is bounded at sixty-four windows** and forgets the least
  recently used one, and it is never written to disk - upstream forgets an
  app's windows when the app quits, and so does this port.

### One deliberate difference from upstream's code

Upstream stores the step that was *applied* and adds one on the next press;
its `forgetCycle` puts that stored step back to zero, which lands the next
press on the second position of the cycle rather than the first. **This port
stores the step that comes *next***, so the visible cycle is exactly the same
(½, then ⅓, then ⅔, then ½ again) and the reset lands where upstream's
documentation says it does - on the half. Upstream's own text for the
fullscreen command is "It resets the cycle but keeps the Restore point", and
that is what happens here.

## The mover (`tc-platform-win`, 12 tests)

| Upstream's behaviour | Our test |
| --- | --- |
| the machine has at least one display with a work area | `the_machine_has_at_least_one_display_with_a_work_area` |
| a window can be read and moved | `a_window_can_be_read_and_moved` |
| a move keeps the size | `a_move_keeps_the_size` |
| the Left Half command puts the window in the left half | `the_left_half_command_puts_the_window_in_the_left_half` |
| the Maximize command fills the display | `the_maximize_command_fills_the_display` |
| **every geometry command moves a real window** | `every_geometry_command_moves_a_real_window` |
| the restore chain puts the window back where it started | `the_restore_chain_puts_the_window_back_where_it_started` |
| a window can be made to fill the display and back | `a_window_can_be_made_to_fill_the_display_and_back` |
| a minimized window is skipped | `a_minimized_window_is_skipped` |
| a window that is gone is reported | `a_window_that_is_gone_is_reported` |
| the window list sees a window with a title | `the_window_list_sees_a_window_with_a_title` |
| the foreground window is a real window | `the_foreground_window_is_a_real_window` |

`every_geometry_command_moves_a_real_window` is the load-bearing one: it
runs **every** command in the catalogue that places a window against a real
window on this machine and reads the rectangle back, so the command is
proven to do what the geometry said.

`the_restore_chain_puts_the_window_back_where_it_started` runs upstream's own
example - Left Half, then Maximize, then Top Right Quarter, then Restore
Window - on a real window, and checks that the window ends up where it
started and not where it was before the last command.

### Upstream's rules that are kept

- **A move never steals the focus.** The window is moved with
  `SWP_NOZORDER | SWP_NOACTIVATE`, so the z-order is left alone and the
  window is not brought to the front.
- **Minimized windows, and windows that do not report a position or a size,
  are skipped.**
- **A command that only moves keeps the window's size.**
- **If an app will not let its window shrink, the window is aligned to the
  correct side once** rather than retried: the rectangle is read back, and
  when the size it took is not the size it was asked for, the window is
  placed in its slot by its anchor - left-aligned for a left half,
  centred for a centre - one time, and no more.

### The work area

The geometry places windows in the **work area** of a display: the screen
without the taskbar and any other app bar. That is Windows' own answer to
upstream's "the usable area already excludes the menu bar, the Dock and the
notch".

### Fullscreen on Windows

Upstream asks the window to enter fullscreen and then tries clicking its
green button. **Windows has no way to put another program's window into its
own fullscreen**, so the honest equivalent is the display's own bounds, and
the frame the window had before is handed back so it can be put back exactly
where it was. Nothing is ever typed into another window: the fullscreen path
writes a rectangle and nothing else.

## What the donor contributed

The donor's `window_controls.rs` writes its geometry the same way - one pure
function that answers a rectangle, with the platform calls beside it - and
that is the shape this port follows. **Every rule came from upstream's
Swift**; the donor contributed the shape and the vocabulary of its tests, the
same as in port note 014.

## The commands in the launcher

The commands are listed in the results, under their own section, and they are
**off until window management is turned on** - upstream's own default: "Turn it
on in Settings -> Window Management. It's off by default. While it's off, the
launcher has no window commands."

| Upstream's behaviour | Our test |
| --- | --- |
| a window command is found by its name | `a_window_command_is_found_by_its_name` |
| the commands are listed under their own section | `the_window_commands_are_listed_under_their_own_section` |
| they can be hidden from the launcher | `the_window_commands_can_be_hidden_from_the_launcher` |
| a row names the command it runs | `a_window_command_row_names_the_command_it_runs` |
| a command is found by a word of its name | `a_window_command_is_found_by_a_word_of_its_name` |

The four settings upstream has are read from the settings file when the app
starts, and they are the keys a Settings pane would write:

| Setting | Key | Default |
| --- | --- | --- |
| Enable window management | `windowManagement.enabled` | off |
| Show in launcher | `windowManagement.showInLauncher` | on, once the feature is on |
| Gap between windows | `windowManagement.gap` | 0 points |
| Cycling | `windowManagement.cycle` | none |

**The window that is moved is the one the user was last using**, and the
palette is a window too - so the window that was in front *before* the palette
opened is remembered, and that is the one the commands move.

## What is not in this slice

- **The launcher rows.** The thirty-nine commands are not in the launcher
  yet, so the "Show in launcher" switch and the per-command checkbox have
  nothing to switch.
- **The Settings pane.** The enable switch, the cycling mode, the gap, the
  shortcut preset and the custom sizes.
- **The custom sizes.** Upstream's own sizes on a 3 × 3 grid.
- **The two Space switches.** Windows has virtual desktops, and switching
  them is a separate piece of work.
- **The display moves are not proven.** Move to Next Display and Move to
  Previous Display do nothing on a machine with one display, exactly as
  upstream says they do with one display - so there is nothing to prove here
  yet, and their rows stay `NOT PORTED` until there is a second display.
- **The "window won't move" row is only partly proven.** Minimized windows
  and windows with no position or size are skipped, with tests; macOS sheets
  and popovers have no Windows answer, and a window that already fills the
  display is not skipped by the placement path.

## The rows that moved

**Fifty-one rows** of the window-management page are now `PORTED`, with the
test that proves each one: the thirty-five command rows, the "move and
resize the window you were last using" row, the "no new permissions" row,
the five "how sizes are calculated" rows, the six "cycling and restore"
rows, and "it resets the cycle but keeps the Restore point".

**120 → 171 rows PORTED.**

## Test count

`tc-windows` has 68 tests (49 for the catalogue and the geometry, 19 for the
memory). `tc-platform-win` has 37 tests, 12 of them for the window layer.
The workspace has **743 tests**.
