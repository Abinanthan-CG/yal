//! The grid's navigation over sectioned rows.
//!
//! Mirrors upstream `Tinycast/Features/Emoji/Model/EmojiGridGeometry.swift`.
//!
//! The rows are one flat index over the sections; a vertical move keeps the
//! column across the sections.

/// The grid's shape: how many entries each section holds, and how wide it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmojiGridGeometry {
    /// How many entries each section holds.
    pub counts: Vec<usize>,
    /// How many columns the grid is wide.
    pub columns: usize,
    /// Where each section starts in the flat index.
    starts: Vec<usize>,
}

impl EmojiGridGeometry {
    /// The geometry of a grid with the given sections.
    pub fn new(counts: Vec<usize>, columns: usize) -> EmojiGridGeometry {
        let mut starts: Vec<usize> = Vec::with_capacity(counts.len());
        let mut running = 0usize;
        for count in &counts {
            starts.push(running);
            running += count;
        }
        EmojiGridGeometry {
            counts,
            columns,
            starts,
        }
    }

    /// The section an index is in, if it is in one.
    fn section(&self, selected: usize) -> Option<usize> {
        for (index, start) in self.starts.iter().enumerate().rev() {
            if selected >= *start {
                let count = self.counts[index];
                return (count > 0 && selected < start + count).then_some(index);
            }
        }
        None
    }

    /// The cell one row down from the selection.
    pub fn down(&self, selected: usize) -> usize {
        let Some(section) = self.section(selected) else {
            return selected;
        };
        let local = selected - self.starts[section];
        let candidate = local + self.columns;
        let count = self.counts[section];
        if candidate < count {
            return self.starts[section] + candidate;
        }
        // A lower partial row in this section clamps to its last cell before
        // spilling over.
        if local / self.columns < (count - 1) / self.columns {
            return self.starts[section] + count - 1;
        }
        if section + 1 >= self.counts.len() {
            return selected;
        }
        let next = self.counts[section + 1];
        self.starts[section + 1] + (local % self.columns).min(next - 1)
    }

    /// The cell one row up from the selection.
    pub fn up(&self, selected: usize) -> usize {
        let Some(section) = self.section(selected) else {
            return selected;
        };
        let local = selected - self.starts[section];
        if local >= self.columns {
            return self.starts[section] + local - self.columns;
        }
        if section == 0 {
            return selected;
        }
        let previous = self.counts[section - 1];
        let last_row_start = ((previous - 1) / self.columns) * self.columns;
        self.starts[section - 1] + (last_row_start + local % self.columns).min(previous - 1)
    }

    /// The selection that keeps the same slot after a pin disappears.
    pub fn selection_after_removing_pin(index: usize, remaining_count: usize) -> usize {
        index.min(remaining_count.saturating_sub(1))
    }

    /// The selection that follows a glyph through a rewritten section.
    pub fn selection<S: AsRef<str>>(
        selected: usize,
        after_section_at: usize,
        old: &[S],
        new: &[S],
    ) -> usize {
        if selected < after_section_at {
            return selected;
        }
        let offset = selected - after_section_at;
        if offset >= old.len() {
            return selected + new.len() - old.len();
        }
        match new
            .iter()
            .position(|glyph| glyph.as_ref() == old[offset].as_ref())
        {
            Some(moved) => after_section_at + moved,
            None => after_section_at + offset.min(new.len().saturating_sub(1)),
        }
    }
}
