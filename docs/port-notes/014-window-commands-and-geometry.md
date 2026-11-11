# 014 - The window commands and their geometry

Date: 2026-11-11
Crate: `crates/tc-windows` (`src/command.rs`, `src/geometry.rs`)
Upstream: `Tinycast/Features/WindowManagement/Model/WindowCommand.swift`,
`Tinycast/Features/WindowManagement/Model/WindowPlacementEngine.swift`,
`Tinycast/Features/WindowManagement/Model/WindowCycle.swift`
Donor: `C:\Users\dabin\development\yal\crates\yal-system\src\window_controls.rs`
(GPL-3.0-only, `github.com/Abinanthan-CG/yal`)

## What this slice is

Upstream's window management in two pure parts:

- **the catalogue** — the thirty-nine commands, their ids, their names, the
  eight groups they are listed under, and the two flags that decide how the
  mover treats them;
- **the geometry** — the arithmetic that answers the rectangle a command
  puts a window in.

Neither part touches Windows. The mover — the part that finds a window and
writes a new rectangle for it — is not in this slice.

## The catalogue

| Upstream's behaviour | Our test |
| --- | --- |
| there are thirty-nine commands | `there_are_exactly_thirty_nine_commands` |
| the groups count up to thirty-nine | `the_groups_count_up_to_thirty_nine` |
| the stored ids are unique and round-trip | `the_stored_ids_are_unique_and_round_trip` |
| the names are unique | `the_names_are_unique` |
| the four halves and Center Half are the five that cycle | `the_five_cycling_commands_are_the_halves_and_the_center_half` |
| the eight nudges and edge moves do not resize | `the_eight_moving_commands_do_not_resize` |
| Restore, Toggle Fullscreen and the two Space switches each have their own kind | `the_three_special_commands_have_their_own_kinds` |
| a command's entry id is `window-command:<id>` | `an_entry_id_carries_the_command_id` |
| that entry id resolves to the action that runs it | `the_entry_id_resolves_to_the_action_that_runs_it` |
| the Settings pane lists the groups in order | `the_grouped_listing_keeps_the_settings_order` |

The ids are upstream's own strings, so a stored shortcut, a deeplink and a
backup entry name a window command the same way they will in the finished
port. `tc-core` already had the `window-command:` prefix and the
`HotKeyAction::WindowCommand` variant; this crate supplies the ids those
actions name.

## The geometry

Upstream's rules, ported from `WindowPlacementEngine.swift`.

| Upstream's behaviour | Our test |
| --- | --- |
| the left and right halves split the width | `the_left_half_takes_the_left_half_of_the_display`, `the_right_half_takes_the_right_half` |
| the top and bottom halves split the height | `the_top_and_bottom_halves_split_the_height` |
| the four quarters split both axes | `the_four_quarters_split_both_axes` |
| the thirds tile the width without gaps | `the_thirds_tile_the_width_without_gaps` |
| the two thirds take the rest of the width | `the_two_thirds_take_the_rest_of_the_width` |
| the three fourths overlap in the middle by a quarter | `the_three_fourths_overlap_in_the_middle_by_a_quarter` |
| Center Half is half the width at full height | `the_center_half_is_half_the_width_at_full_height` |
| Center Two Thirds is two thirds at full height | `the_center_two_thirds_is_two_thirds_at_full_height` |
| Maximize fills the display | `maximize_fills_the_display` |
| Almost Maximize leaves a tenth around the window | `almost_maximize_leaves_a_tenth_around_the_window` |
| Reasonable Size is sixty per cent of the display | `reasonable_size_is_sixty_per_cent_of_the_display` |
| Reasonable Size never grows past 1025 × 900 | `reasonable_size_never_grows_past_the_ceiling` |
| Center keeps the window's size and centres it | `center_keeps_the_window_size_and_centres_it` |
| Maximize Height keeps the width and clamps it on screen | `maximize_height_keeps_the_width_and_clamps_it_on_screen` |
| Make Larger and Make Smaller undo each other exactly | `make_larger_and_make_smaller_are_exactly_invertible` |
| Make Smaller stops at the floor | `make_smaller_stops_at_the_floor` |
| a nudge moves the window without resizing it | `a_nudge_moves_the_window_without_resizing_it` |
| a nudge stops at the edge of the display | `a_nudge_stops_at_the_edge_of_the_display` |
| an edge move puts the window against the edge | `an_edge_move_puts_the_window_against_the_edge` |
| a gap insets the screen edge by one gap and splits the middle by half | `a_gap_insets_the_screen_edge_by_one_gap_and_splits_the_middle_by_half` |
| a gap is capped at a tenth of the display | `a_gap_is_capped_at_a_tenth_of_the_display` |
| the gap setting is held to 0–64 in steps of two | `the_gap_setting_is_held_to_its_range_and_step` |
| two tiles never overlap and never leave a line between them | `two_tiles_never_overlap_and_never_leave_a_line_between_them` |
| the cycling half steps through ½, ⅓ and ⅔ | `the_cycling_half_steps_through_half_a_third_and_two_thirds` |
| without cycling a half keeps its frame | `without_cycling_a_half_keeps_its_frame` |
| the cycle length is one for a command that does not cycle | `the_cycle_length_is_one_for_a_command_that_does_not_cycle` |
| the display cycle walks the documented strip | `the_display_cycle_walks_the_strip_of_displays` |
| a display move walks the ordered strip | `moving_to_the_next_display_walks_the_ordered_strip` |
| one display makes a display move a no-op | `one_display_makes_a_display_move_a_no_op` |
| Restore puts the window back where it started | `restore_puts_the_window_back_where_it_started` |
| a restore point stranded off every display comes back centred | `a_restore_point_stranded_off_every_display_comes_back_centred` |
| a window without a position is not placed | `a_window_without_a_position_is_not_placed` |
| a command with no geometry answers nothing | `a_command_with_no_geometry_answers_nothing` |
| a machine with no display answers nothing | `a_machine_with_no_display_answers_nothing` |
| the tile commands are the ones a display move can re-derive | `the_tile_commands_are_the_ones_a_display_move_can_re_derive` |
| the display a window sits on is the one it overlaps most | `the_display_the_window_sits_on_is_the_one_it_overlaps_most` |

### The rules that are easy to get wrong

- **The gap.** An edge next to the screen border gets the full gap; an edge
  between two windows gets half. So two windows side by side have exactly one
  gap between them, and every screen edge is inset by one gap.
- **The rounding.** The four edges are rounded, not the width, so two tiles
  sharing a boundary round it identically and never overlap or leave a
  one-point line between them.
- **Make Larger and Make Smaller** step by five per cent of the *screen*,
  not of the window, so each one exactly undoes the other. The step is even,
  so every edge moves a whole point.
- **The floor.** The smallest a window may shrink to is 200 × 150 points or
  fifteen per cent of the screen, whichever is larger.
- **The screen cycle** walks the half-slots of every display in a strip:
  Display 1 left → Display 2 right → Display 2 left → Display 1 right. Left
  and Top walk backwards, so one shortcut sweeps the whole desktop in one
  direction.

### Coordinates

The engine works with the origin at the top left and +Y pointing down, which
is what Windows already uses. The caller supplies the work area in the unit
it wants the answer in; on Windows that is physical pixels, and the gap —
which upstream counts in points — is scaled by the display before it is
handed over.

## What the donor contributed, and what it did not

The donor project in `C:\Users\dabin\development\yal` writes its window
geometry the same way — one pure function that answers a rectangle, with the
platform calls kept out of it — and that shape is what this crate follows.

The donor's rules are its own: it tiles halves, quarters, thirds and the
centre, and it has no gap model, no cycling, no restore point, no custom
sizes and no display moves. **Upstream's rules win, and every one of them
here came from upstream's Swift.** The donor's contribution is the shape,
the split between a pure geometry and a platform layer, and the test
vocabulary for tiling.

## What is not in this slice

- **The mover.** Finding the window the user was last using, reading its
  rectangle, writing the new one, and the rules for a window that refuses to
  move or to shrink. That is Win32 work, and it lives in `tc-platform-win`.
- **The palette rows.** The window commands are not in the launcher yet.
- **The Settings pane.** The enable switch, the Show in launcher switch, the
  cycling mode, the gap, the shortcut preset and the custom sizes.
- **The custom sizes.** Upstream's own sizes on a 3 × 3 grid, with a
  percentage or an absolute measure.
- **The restore memory.** Upstream remembers up to 64 windows' starting
  positions, forgets an app's windows when the app quits, and never writes
  this to disk.
- **Fullscreen and the Space switches.** On Windows, fullscreen is the
  window's own answer and a Space is a virtual desktop; both are commands
  with no geometry.

## Why no row is `PORTED` yet

Every window-management row describes a command a person can press and a
window that visibly moves. This slice proves the arithmetic, and it proves
it well, but nothing moves a real window yet and no command is reachable
from the launcher. **The rows stay `NOT PORTED` until a test on this machine
moves a real window and reads its rectangle back.** A model behind a screen
is not the screen, and the ledger says so.

## Test count

`tc-windows` has 47 tests: 10 for the catalogue and 37 for the geometry.

## The donor's licence

The donor is GPL-3.0-only; this repository is AGPL-3.0-only. AGPL-3.0 is
GPL-3.0 plus section 13, so the combination is lawful and the combined work
is AGPL-3.0. The notice travels with the code — see `NOTICE.md`.
