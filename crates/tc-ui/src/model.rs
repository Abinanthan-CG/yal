//! What the palette is showing, and the single-source-of-truth layout geometry
//! shared by both Direct2D painting and mouse hit-testing.

use std::sync::Arc;

use crate::theme::{Color, Metrics};

/// Height of a section divider header in DIPs.
pub const SECTION_HEADER_HEIGHT: f32 = 26.0;

/// Height of a Calculator hero card in DIPs.
pub const CALC_HERO_HEIGHT: f32 = 90.0;

/// Premultiplied top-down RGBA bitmap for a real application icon.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IconBitmap {
    pub width: u32,
    pub height: u32,
    pub premultiplied_rgba: Vec<u8>,
}

/// The badge/tile that leads a row when no bitmap icon is attached.
#[derive(Clone, Debug, PartialEq)]
pub struct Icon {
    /// The letter or symbol drawn in the badge.
    pub letter: String,
    /// The badge's tint colour.
    pub color: Color,
}

impl Icon {
    /// A badge with a symbol/letter and a tint colour.
    pub fn new(letter: impl Into<String>, color: Color) -> Self {
        Self {
            letter: letter.into(),
            color,
        }
    }
}

/// One result in the list.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Row {
    /// The row's headline (or calculated answer for a calculator hero card).
    pub name: String,
    /// Secondary subtitle (or input expression for a calculator hero card).
    pub detail: String,
    /// Right-aligned pill badge (shortcut keycap or current setting value).
    pub shortcut: String,
    /// Fallback tinted glyph badge when no real app icon bitmap is available.
    pub icon: Option<Icon>,
    /// Optional section title; a header is rendered whenever this changes.
    pub section: String,
    /// Optional pill badge label (e.g. "Result", "Tip", "Unit" on calculator cards).
    pub badge: String,
    /// Render this row as a large split Calculator hero card.
    pub is_calculator: bool,
    /// Real application icon pixels (premultiplied RGBA) when extracted.
    pub bitmap: Option<Arc<IconBitmap>>,
    /// Stable cache key for GPU `ID2D1Bitmap` reuse across repaints.
    pub icon_key: Option<String>,
}

impl Row {
    /// A row with only a name.
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    /// A row with a name and a detail.
    pub fn detailed(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            detail: detail.into(),
            ..Self::default()
        }
    }

    /// A row with a badge in front of it.
    pub fn with_icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Attach a section header label.
    pub fn with_section(mut self, section: impl Into<String>) -> Self {
        self.section = section.into();
        self
    }

    /// Attach a right-aligned shortcut or value pill.
    pub fn with_shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = shortcut.into();
        self
    }

    /// Attach a real Windows icon bitmap and GPU cache key.
    pub fn with_bitmap(mut self, key: impl Into<String>, bitmap: Arc<IconBitmap>) -> Self {
        self.icon_key = Some(key.into());
        self.bitmap = Some(bitmap);
        self
    }
}

/// The whole of the palette's content.
#[derive(Clone, Debug, Default)]
pub struct Model {
    /// The query the user has typed.
    pub query: String,
    /// The results, best first.
    pub rows: Vec<Row>,
    /// Which result is selected.
    pub selected: usize,
    /// Which result the pointer is over, if any.
    pub hovered: Option<usize>,
    /// Fallback hint line at the bottom of the palette.
    pub footer: String,
    /// Cue banner placeholder displayed in the search box when empty.
    pub placeholder: String,
    /// Optional pill shown on the right of the header bar (e.g. "Settings").
    pub header_badge: Option<String>,
    /// Optional left-aligned footer status (e.g. "12 results").
    pub result_count: Option<String>,
    /// Structured right-aligned footer keycap hints `(keycap, label)`.
    pub footer_hints: Vec<(String, String)>,
    /// Vertical scroll offset inside the list viewport, in DIPs.
    pub scroll_offset: f32,
}

/// Positioned section header in content-space DIPs (relative to `list_top`).
#[derive(Clone, Debug, PartialEq)]
pub struct SectionLayout {
    pub y: f32,
    pub height: f32,
    pub title: String,
}

/// Positioned result row in content-space DIPs (relative to `list_top`).
#[derive(Clone, Debug, PartialEq)]
pub struct RowLayout {
    pub index: usize,
    pub y: f32,
    pub height: f32,
    pub is_calculator: bool,
}

/// Complete vertical layout geometry in DIPs, shared by paint and hit-testing.
#[derive(Clone, Debug)]
pub struct PaletteLayout {
    /// Y coordinate of the top of the scrollable list viewport.
    pub list_top: f32,
    /// Y coordinate of the bottom of the scrollable list viewport.
    pub list_bottom: f32,
    /// Total height of all section headers + rows in DIPs.
    pub content_height: f32,
    /// Total target window height in DIPs.
    pub window_height: f32,
    /// Positioned section headers (Y relative to `list_top`).
    pub sections: Vec<SectionLayout>,
    /// Positioned rows (Y relative to `list_top`).
    pub rows: Vec<RowLayout>,
}

impl PaletteLayout {
    /// Height of the visible list viewport in DIPs.
    pub fn viewport_height(&self) -> f32 {
        (self.list_bottom - self.list_top).max(0.0)
    }

    /// Maximum valid `scroll_offset` in DIPs.
    pub fn max_scroll(&self) -> f32 {
        (self.content_height - self.viewport_height()).max(0.0)
    }

    /// Hit-test a window-relative Y coordinate in DIPs, accounting for `scroll_offset`.
    pub fn row_at_y(&self, y_dip: f32, scroll_offset: f32) -> Option<usize> {
        if y_dip < self.list_top || y_dip >= self.list_bottom {
            return None;
        }
        let content_y = (y_dip - self.list_top) + scroll_offset;
        for row in &self.rows {
            if content_y >= row.y && content_y < row.y + row.height {
                return Some(row.index);
            }
        }
        None
    }

    /// Adjust `scroll_offset` so `selected` is fully inside the visible viewport.
    pub fn scroll_for_selected(&self, selected: usize, current_scroll: f32) -> f32 {
        let max_scroll = self.max_scroll();
        let mut scroll = current_scroll.clamp(0.0, max_scroll);
        if let Some(row) = self.rows.get(selected) {
            let viewport = self.viewport_height();
            // If this row sits immediately below a section header and we're scrolling up,
            // include the section header for visual polish.
            let row_top = if selected == 0 { 0.0 } else { row.y };
            let row_bottom = row.y + row.height;
            if row_top < scroll {
                scroll = row_top;
            } else if row_bottom > scroll + viewport {
                scroll = (row_bottom - viewport).max(0.0);
            }
        }
        scroll.clamp(0.0, max_scroll)
    }
}

impl Model {
    /// The selected row, if there is one.
    pub fn selected_row(&self) -> Option<&Row> {
        self.rows.get(self.selected)
    }

    /// Moves the selection down, stopping at the last row.
    pub fn select_next(&mut self) {
        if self.selected + 1 < self.rows.len() {
            self.selected += 1;
        }
    }

    /// Moves the selection up, stopping at the first row.
    pub fn select_previous(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    /// Compute the shared DIP layout geometry used by both painting and hit-testing.
    pub fn layout(&self, metrics: Metrics) -> PaletteLayout {
        let list_top = metrics.field_height + 1.0 + metrics.list_gap;
        let mut sections = Vec::new();
        let mut rows = Vec::with_capacity(self.rows.len());

        let mut cursor_y = 0.0_f32;
        let mut last_section: Option<&str> = None;

        for (index, row) in self.rows.iter().enumerate() {
            if !row.section.is_empty() && last_section != Some(row.section.as_str()) {
                if index > 0 {
                    cursor_y += 4.0;
                }
                sections.push(SectionLayout {
                    y: cursor_y,
                    height: SECTION_HEADER_HEIGHT,
                    title: row.section.clone(),
                });
                cursor_y += SECTION_HEADER_HEIGHT;
                last_section = Some(row.section.as_str());
            }

            let h = if row.is_calculator {
                CALC_HERO_HEIGHT
            } else {
                metrics.row_height
            };
            rows.push(RowLayout {
                index,
                y: cursor_y,
                height: h,
                is_calculator: row.is_calculator,
            });
            cursor_y += h;
            if row.is_calculator {
                cursor_y += 6.0;
            }
        }

        let content_height = if self.rows.is_empty() {
            // Compact empty-state breathing room.
            56.0
        } else {
            cursor_y + 4.0
        };

        let max_list_height = metrics.max_rows as f32 * metrics.row_height + SECTION_HEADER_HEIGHT;
        let visible_list_height = content_height.min(max_list_height);
        let list_bottom = list_top + visible_list_height;
        let window_height = list_bottom + metrics.list_gap + metrics.footer_height;

        PaletteLayout {
            list_top,
            list_bottom,
            content_height,
            window_height,
            sections,
            rows,
        }
    }

    /// Clamp `scroll_offset` and ensure the selected row is visible.
    pub fn ensure_selected_visible(&mut self, metrics: Metrics) {
        let layout = self.layout(metrics);
        self.scroll_offset = layout.scroll_for_selected(self.selected, self.scroll_offset);
    }

    /// Apply a mouse-wheel delta (in DIPs; positive scrolls down).
    pub fn scroll_by(&mut self, delta_y: f32, metrics: Metrics) {
        let layout = self.layout(metrics);
        self.scroll_offset = (self.scroll_offset + delta_y).clamp(0.0, layout.max_scroll());
    }

    /// Legacy flat hit-test helper retained for uniform-row unit tests.
    pub fn row_at(&self, y: f32, list_top: f32, row_height: f32) -> Option<usize> {
        if row_height <= 0.0 || y < list_top {
            return None;
        }
        let index = ((y - list_top) / row_height).floor() as usize;
        if index < self.rows.len() {
            Some(index)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_selection_stays_inside_the_list() {
        let mut model = Model {
            rows: vec![Row::named("one"), Row::named("two")],
            ..Model::default()
        };
        model.select_previous();
        assert_eq!(model.selected, 0);
        model.select_next();
        assert_eq!(model.selected, 1);
        model.select_next();
        assert_eq!(model.selected, 1);
        assert_eq!(
            model.selected_row().map(|row| row.name.as_str()),
            Some("two")
        );
    }

    #[test]
    fn an_empty_list_has_no_selected_row() {
        let model = Model::default();
        assert!(model.selected_row().is_none());
    }

    #[test]
    fn a_row_can_carry_a_tile() {
        let row = Row::named("Calculator").with_icon(Icon::new("C", Color::rgb(0, 0, 0)));
        assert_eq!(row.icon.map(|icon| icon.letter), Some("C".to_string()));
    }

    #[test]
    fn a_point_finds_its_row() {
        let model = Model {
            rows: vec![Row::named("one"), Row::named("two"), Row::named("three")],
            ..Model::default()
        };
        assert_eq!(model.row_at(100.0, 100.0, 40.0), Some(0));
        assert_eq!(model.row_at(139.0, 100.0, 40.0), Some(0));
        assert_eq!(model.row_at(140.0, 100.0, 40.0), Some(1));
        assert_eq!(model.row_at(219.0, 100.0, 40.0), Some(2));
        assert_eq!(model.row_at(220.0, 100.0, 40.0), None);
    }

    #[test]
    fn a_point_outside_the_list_finds_nothing() {
        let model = Model {
            rows: vec![Row::named("one"), Row::named("two")],
            ..Model::default()
        };
        assert_eq!(model.row_at(10.0, 100.0, 40.0), None);
        assert_eq!(model.row_at(400.0, 100.0, 40.0), None);
        assert_eq!(model.row_at(110.0, 100.0, 0.0), None);
    }

    #[test]
    fn layout_hit_test_matches_painted_rows_with_section_headers_and_scroll() {
        let metrics = Metrics::default();
        let rows: Vec<Row> = (0..12)
            .map(|i| {
                let section = if i < 2 { "Settings" } else { "Applications" };
                Row::named(format!("Row {i}")).with_section(section)
            })
            .collect();
        let mut model = Model {
            rows,
            ..Model::default()
        };
        let layout = model.layout(metrics);
        assert_eq!(layout.sections.len(), 2);

        // Clicking inside section header 0 hits no row.
        assert_eq!(layout.row_at_y(layout.list_top + 5.0, 0.0), None);

        // Clicking the midpoint of row 0 hits row 0, and row 3 hits row 3.
        let r0 = &layout.rows[0];
        assert_eq!(
            layout.row_at_y(layout.list_top + r0.y + r0.height * 0.5, 0.0),
            Some(0)
        );
        let r3 = &layout.rows[3];
        assert_eq!(
            layout.row_at_y(layout.list_top + r3.y + r3.height * 0.5, 0.0),
            Some(3)
        );

        // Scrolling down shifts hit-testing by exact scroll_offset.
        model.scroll_by(88.0, metrics);
        let r5 = &layout.rows[5];
        let screen_y = layout.list_top + r5.y - model.scroll_offset + 10.0;
        assert_eq!(layout.row_at_y(screen_y, model.scroll_offset), Some(5));
    }

    #[test]
    fn ensure_selected_visible_scrolls_to_bottom_and_back_to_top() {
        let metrics = Metrics::default();
        let rows: Vec<Row> = (0..15)
            .map(|i| Row::named(format!("App {i}")).with_section("Applications"))
            .collect();
        let mut model = Model {
            rows,
            selected: 14,
            ..Model::default()
        };
        model.ensure_selected_visible(metrics);
        assert!(model.scroll_offset > 0.0);

        model.selected = 0;
        model.ensure_selected_visible(metrics);
        assert_eq!(model.scroll_offset, 0.0);
    }
}
