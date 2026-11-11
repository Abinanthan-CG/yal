# Attribution

## The YAL project by Abinanthan-CG

`crates/tc-files/src/index.rs` - the incremental walk, the freshness rule and
the exact deletion - is adapted from the YAL project at
<https://github.com/Abinanthan-CG/yal>, in
`crates/yal-index/src/walk.rs` and `crates/yal-store/src/file_index.rs`.

YAL is licensed under the GNU General Public License version 3. This
repository is licensed under the GNU Affero General Public License version 3.
AGPL-3.0 is GPL-3.0 plus section 13, so the two may be combined, and the
combined work is distributed under AGPL-3.0.

What was taken from the donor:

- the design of the pass - two tables, one for the listed directories with
  the time they were listed at, one for the files and folders keyed by their
  path;
- the freshness rule - nanosecond directory times, never milliseconds, because
  NTFS ticks at 100 nanoseconds and a millisecond floor buries a write that
  lands in the same millisecond the pass recorded the directory;
- the rejection of junctions and reparse points, so a walk cannot loop;
- the exact per-directory deletion, rather than a prefix sweep that an
  underscore in a path would get wrong;
- the rule that a directory records the time read *after* its listing, so a
  change landing between the stat and the read is caught by the next pass.

What was **not** taken: the donor's scope model. It indexes the machine's
fixed drives; upstream Tinycast indexes the folders the user picks, with
editable ignore rules, and this port follows upstream. The donor's skip list
of names was replaced by upstream's three kinds of rule, and the donor's
search was not taken at all - upstream's own rules are in `src/search.rs`.

The donor's licence notice follows.

```
GNU GENERAL PUBLIC LICENSE Version 3, 29 June 2007

Copyright (C) 2026 Abinanthan-CG

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program.  If not, see <https://www.gnu.org/licenses/>.
```

## Upstream Tinycast

The behaviour this crate implements is upstream Tinycast's
(<https://github.com/abue-ammar/tinycast>), AGPL-3.0-only, read from
`docs/features/file-search.md` and from
`Tinycast/Features/FileSearch/`. The same licence covers this repository;
see `NOTICE.md` at the root for the notices that travel with the port.
