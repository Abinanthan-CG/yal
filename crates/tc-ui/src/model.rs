//! What the palette is showing, and the single-source-of-truth layout geometry
//! shared by both Direct2D painting and mouse hit-testing.

use std::sync::Arc;

use crate::theme::{Color, Metrics};

/// Height of a section divider header in DIPs.
pub const SECTION_HEADER_HEIGHT: f32 = 24.0;

/// Height of a Calculator hero card in DIPs (exact Raycast split-card height).
pub const CALC_HERO_HEIGHT: f32 = 124.0;

/// Height of one cell row in 2D Emoji Grid mode (in DIPs).
pub const GRID_CELL_HEIGHT: f32 = 62.0;

/// Left master list ratio in Master-Detail split views (exact Raycast 285/750 split).
pub const MASTER_SPLIT_RATIO: f32 = 0.38;

/// Minimum list viewport height when Master-Detail split view is active.
pub const SPLIT_VIEW_MIN_HEIGHT: f32 = 368.0;

/// Right-side Master-Detail preview & metadata pane (used by Clipboard, Files, Snippets, Notes).
#[derive(Clone, Debug, Default)]
pub struct DetailPane {
    /// Preview headline (file name, clip type, note title, snippet name).
    pub title: String,
    /// Multi-line wrapped preview text (clipboard body, file text snippet, note markdown).
    pub body: String,
    /// Optional large color preview swatch banner (RGBA).
    pub color_swatch: Option<[u8; 4]>,
    /// Optional horizontal tint/shade color ramp swatches `(rgba, label)` rendered below `color_swatch`.
    pub color_ramp: Vec<([u8; 4], String)>,
    /// Optional preview bitmap (clipboard image thumbnail or large shell icon).
    pub image: Option<Arc<IconBitmap>>,
    /// Stable GPU cache key for `image`.
    pub image_key: Option<String>,
    /// Bottom Raycast-style key-value metadata table `(Label, Value)`.
    pub metadata: Vec<(String, String)>,
}

/// Native multi-line text editor surface (e.g. Edit Clipboard Entry).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EditorModel {
    /// Header title displayed in the top bar while editing.
    pub title: String,
    /// Initial text loaded into the multi-line editor when entering edit mode.
    pub text: String,
    /// Label for primary in-place save (`Ctrl+Enter` / `Ctrl+S`).
    pub primary_label: String,
    /// Label for secondary save-as-new (`Ctrl+Shift+Enter` / `Ctrl+Shift+S`).
    pub secondary_label: String,
}

/// One selectable command inside the floating `Ctrl+K` Action Panel popover.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ActionMenuItem {
    pub id: String,
    pub title: String,
    pub section: String,
    pub shortcut: String,
    pub glyph: String,
    pub destructive: bool,
}

impl ActionMenuItem {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        section: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            section: section.into(),
            shortcut: String::new(),
            glyph: "›".to_string(),
            destructive: false,
        }
    }

    pub fn with_shortcut(mut self, s: impl Into<String>) -> Self {
        self.shortcut = s.into();
        self
    }

    pub fn with_glyph(mut self, g: impl Into<String>) -> Self {
        self.glyph = g.into();
        self
    }

    pub fn destructive(mut self) -> Self {
        self.destructive = true;
        self
    }
}

/// Floating bottom-right Raycast-style Action Panel (`Ctrl+K`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ActionMenuModel {
    pub title: String,
    pub query: String,
    pub items: Vec<ActionMenuItem>,
    pub selected: usize,
    pub hovered: Option<usize>,
}

/// Action Panel layout constants shared by paint and hit-testing.
pub const ACTION_MENU_WIDTH: f32 = 304.0;
pub const ACTION_MENU_HEADER_H: f32 = 32.0;
pub const ACTION_MENU_ROW_H: f32 = 32.0;
pub const ACTION_MENU_MAX_VISIBLE: usize = 8;

impl ActionMenuModel {
    pub fn filtered_indices(&self) -> Vec<usize> {
        let q = self.query.trim().to_ascii_lowercase();
        if q.is_empty() {
            return (0..self.items.len()).collect();
        }
        self.items
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                item.title.to_ascii_lowercase().contains(&q)
                    || item.section.to_ascii_lowercase().contains(&q)
            })
            .map(|(idx, _)| idx)
            .collect()
    }

    pub fn selected_item(&self) -> Option<&ActionMenuItem> {
        let indices = self.filtered_indices();
        let raw_idx = *indices.get(self.selected)?;
        self.items.get(raw_idx)
    }

    pub fn select_next(&mut self) {
        let len = self.filtered_indices().len();
        if self.selected + 1 < len {
            self.selected += 1;
        }
    }

    pub fn select_previous(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    /// Total desired height of the floating Action Panel in DIPs for its current filtered rows.
    pub fn desired_height(&self) -> f32 {
        let count = self
            .filtered_indices()
            .len()
            .clamp(1, ACTION_MENU_MAX_VISIBLE);
        ACTION_MENU_HEADER_H + count as f32 * ACTION_MENU_ROW_H + 12.0
    }

    /// Bounding rectangle `(left, top, right, bottom)` in DIPs anchored bottom-right above footer.
    pub fn rect(&self, metrics: Metrics, window_height: f32) -> (f32, f32, f32, f32) {
        let h = self.desired_height();
        let right = metrics.width - 10.0;
        let left = (right - ACTION_MENU_WIDTH).max(10.0);
        let bottom = (window_height - metrics.footer_height - 8.0).max(metrics.field_height + 40.0);
        let top = (bottom - h).max(metrics.field_height + 8.0);
        (left, top, right, bottom)
    }

    /// Number of action rows that physically fit inside the menu card for the given window height.
    pub fn visible_slots(&self, metrics: Metrics, window_height: f32) -> usize {
        let (_, top, _, bottom) = self.rect(metrics, window_height);
        let rows_top = top + ACTION_MENU_HEADER_H + 4.0;
        let avail = (bottom - rows_top).max(0.0);
        ((avail / ACTION_MENU_ROW_H).floor() as usize).clamp(1, ACTION_MENU_MAX_VISIBLE)
    }

    /// First filtered row index visible when `max_slots` rows fit in the menu card.
    pub fn scroll_start_for_slots(&self, total: usize, max_slots: usize) -> usize {
        let slots = max_slots.max(1);
        if total <= slots || self.selected < slots {
            0
        } else {
            (self.selected + 1 - slots).min(total - slots)
        }
    }

    /// Hit-test a DIP point against the Action Panel rows; returns filtered row index.
    pub fn hit_test_dip(
        &self,
        x_dip: f32,
        y_dip: f32,
        metrics: Metrics,
        window_height: f32,
    ) -> Option<usize> {
        let (left, top, right, bottom) = self.rect(metrics, window_height);
        if x_dip < left || x_dip > right || y_dip < top || y_dip > bottom {
            return None;
        }
        let rows_top = top + ACTION_MENU_HEADER_H + 4.0;
        if y_dip < rows_top {
            return None;
        }
        let idx = ((y_dip - rows_top) / ACTION_MENU_ROW_H).floor() as usize;
        let slots = self.visible_slots(metrics, window_height);
        if idx >= slots {
            return None;
        }
        let filtered = self.filtered_indices();
        let scroll = self.scroll_start_for_slots(filtered.len(), slots);
        let target = scroll + idx;
        if target < filtered.len() {
            Some(target)
        } else {
            None
        }
    }

    pub fn scroll_start(&self, total: usize) -> usize {
        self.scroll_start_for_slots(total, ACTION_MENU_MAX_VISIBLE)
    }
}

/// Visual tone for an inline bottom footer toast.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastKind {
    Success,
    Info,
    Error,
}

/// Inline status toast displayed in the left slot of the footer bar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToastModel {
    pub kind: ToastKind,
    pub message: String,
}

/// Active inline header argument pill (e.g. `GitHub Search ▸`) while typing a command argument.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InlineArgBanner {
    pub command_name: String,
    pub arg_label: String,
}

/// One tile in a 2D Emoji / Symbol grid.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GridCell {
    pub glyph: String,
    pub name: String,
    pub pinned: bool,
}

/// 2D grid content rendered instead of list rows (used by the Emoji Picker).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GridModel {
    pub columns: usize,
    pub cells: Vec<GridCell>,
    pub selected: usize,
    pub hovered: Option<usize>,
}

impl GridModel {
    pub fn cols(&self) -> usize {
        self.columns.max(1)
    }

    pub fn row_count(&self) -> usize {
        if self.cells.is_empty() {
            0
        } else {
            self.cells.len().div_ceil(self.cols())
        }
    }

    pub fn select_left(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn select_right(&mut self) {
        if self.selected + 1 < self.cells.len() {
            self.selected += 1;
        }
    }

    pub fn select_up(&mut self) {
        let cols = self.cols();
        if self.selected >= cols {
            self.selected -= cols;
        }
    }

    pub fn select_down(&mut self) {
        let cols = self.cols();
        if self.selected + cols < self.cells.len() {
            self.selected += cols;
        } else if !self.cells.is_empty() {
            let cur_row = self.selected / cols;
            let last_row = (self.cells.len() - 1) / cols;
            if cur_row < last_row {
                self.selected = self.cells.len() - 1;
            }
        }
    }

    pub fn active_cell(&self) -> Option<&GridCell> {
        self.hovered
            .and_then(|i| self.cells.get(i))
            .or_else(|| self.cells.get(self.selected))
    }
}

/// Premultiplied top-down RGBA bitmap for a real application icon.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IconBitmap {
    pub width: u32,
    pub height: u32,
    pub premultiplied_rgba: Vec<u8>,
}

/// Built-in crisp Direct2D vector icon drawn inside a row's leading tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconKind {
    /// Document / plain text lines icon.
    Text,
    /// Chain-link / URL icon.
    Link,
    /// Envelope / email icon.
    Email,
    /// Landscape photo frame icon (fallback for images while thumbnail decodes).
    Image,
    /// Folded document / file icon.
    File,
    /// Angle-brackets code icon.
    Code,
}

/// The badge/tile that leads a row when no bitmap icon is attached.
#[derive(Clone, Debug, PartialEq)]
pub struct Icon {
    /// The letter or symbol drawn in the badge.
    pub letter: String,
    /// The badge's tint colour.
    pub color: Color,
    /// Optional vector glyph painted with Direct2D geometry instead of `letter`.
    pub kind: Option<IconKind>,
}

impl Icon {
    /// A badge with a symbol/letter and a tint colour.
    pub fn new(letter: impl Into<String>, color: Color) -> Self {
        Self {
            letter: letter.into(),
            color,
            kind: None,
        }
    }

    /// A badge with a crisp Direct2D vector icon and a tint colour.
    pub fn vector(kind: IconKind, color: Color) -> Self {
        Self {
            letter: String::new(),
            color,
            kind: Some(kind),
        }
    }
}

/// One result in the list.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Row {
    /// The row's headline (or calculated answer for a calculator hero card).
    pub name: String,
    /// Secondary inline subtitle (or input expression for a calculator hero card).
    pub detail: String,
    /// Optional right-aligned muted type/category accessory (e.g. "Command", "Application", "File").
    pub accessory: String,
    /// Right-aligned pill badge (shortcut keycap or current setting value).
    pub shortcut: String,
    /// Fallback tinted glyph badge when no real app icon bitmap is available.
    pub icon: Option<Icon>,
    /// Optional section title; a header is rendered whenever this changes.
    pub section: String,
    /// Optional left-side pill badge on a Calculator hero card (e.g. "Expression", "US Dollar").
    pub source_badge: String,
    /// Optional right-side pill badge label (e.g. "Days", "Tip", "Indian Rupee", "OKLCH").
    pub badge: String,
    /// Render this row as a large split Calculator hero card.
    pub is_calculator: bool,
    /// Optional RGBA preview swatch painted inside the Calculator hero card (for colors).
    pub color_swatch: Option<[u8; 4]>,
    /// Real application icon pixels (premultiplied RGBA) when extracted.
    pub bitmap: Option<Arc<IconBitmap>>,
    /// Stable cache key for GPU `ID2D1Bitmap` reuse across repaints.
    pub icon_key: Option<String>,
    /// True when this row is actively recording a live global hotkey shortcut.
    pub recording: bool,
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

    /// Attach a right-aligned muted type/category accessory (e.g. "Command", "File").
    pub fn with_accessory(mut self, accessory: impl Into<String>) -> Self {
        self.accessory = accessory.into();
        self
    }

    /// Attach a real Windows icon bitmap and GPU cache key.
    pub fn with_bitmap(mut self, key: impl Into<String>, bitmap: Arc<IconBitmap>) -> Self {
        self.icon_key = Some(key.into());
        self.bitmap = Some(bitmap);
        self
    }

    /// Attach a live RGBA color preview swatch.
    pub fn with_color_swatch(mut self, rgba: [u8; 4]) -> Self {
        self.color_swatch = Some(rgba);
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
    /// Optional 2D Emoji / Symbol grid; when `Some`, rendered instead of `rows`.
    pub grid: Option<GridModel>,
    /// Show a clickable `←` back button in the header left slot (sub-screens).
    pub back_button: bool,
    /// Optional Master-Detail split preview/metadata pane on the right (465 DIPs).
    pub detail: Option<DetailPane>,
    /// Optional floating bottom-right `Ctrl+K` Action Panel popover.
    pub action_menu: Option<ActionMenuModel>,
    /// Optional inline toast banner rendered in the left footer slot.
    pub toast: Option<ToastModel>,
    /// Active inline `Tab` argument pill inside the search header.
    pub inline_arg: Option<InlineArgBanner>,
    /// Primary action verb shown in the right footer slot (e.g. "Open", "Paste", "Copy").
    pub primary_action: Option<String>,
    /// Render as a single compact search bar (hiding list & footer until typing or expanding).
    pub compact: bool,
    /// When `true` (e.g. Clipboard History), single-clicking a row only selects and
    /// previews it instead of immediately activating/pasting; `Enter` activates.
    pub click_selects_only: bool,
    /// Optional native multi-line text editor surface (e.g. Edit Clipboard Entry).
    pub editor: Option<EditorModel>,
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
        if self.compact
            && self.action_menu.is_none()
            && self.detail.is_none()
            && self.grid.is_none()
            && self.editor.is_none()
        {
            return PaletteLayout {
                list_top: metrics.field_height,
                list_bottom: metrics.field_height,
                content_height: 0.0,
                window_height: metrics.field_height,
                sections: Vec::new(),
                rows: Vec::new(),
            };
        }

        let list_top = metrics.field_height + 1.0 + metrics.list_gap;

        if self.editor.is_some() {
            let visible_height = SPLIT_VIEW_MIN_HEIGHT;
            let list_bottom = list_top + visible_height;
            let window_height = list_bottom + metrics.list_gap + metrics.footer_height;
            return PaletteLayout {
                list_top,
                list_bottom,
                content_height: visible_height,
                window_height,
                sections: Vec::new(),
                rows: Vec::new(),
            };
        }

        if let Some(grid) = self.grid.as_ref() {
            let row_count = grid.row_count();
            let content_height = if row_count == 0 {
                56.0
            } else {
                row_count as f32 * GRID_CELL_HEIGHT + 8.0
            };
            let max_grid_height = 5.0 * GRID_CELL_HEIGHT + 8.0;
            let mut visible_height = content_height.min(max_grid_height);
            if let Some(menu) = self.action_menu.as_ref() {
                visible_height =
                    visible_height.max((menu.desired_height() + 16.0).min(max_grid_height));
            }
            let list_bottom = list_top + visible_height;
            let window_height = list_bottom + metrics.list_gap + metrics.footer_height;
            return PaletteLayout {
                list_top,
                list_bottom,
                content_height,
                window_height,
                sections: Vec::new(),
                rows: Vec::new(),
            };
        }

        let mut sections = Vec::new();
        let mut rows = Vec::with_capacity(self.rows.len());

        let mut cursor_y = 0.0_f32;
        let mut last_section: Option<&str> = None;

        for (index, row) in self.rows.iter().enumerate() {
            if !row.section.is_empty() && last_section != Some(row.section.as_str()) {
                if index > 0 {
                    cursor_y += 10.0;
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
                cursor_y += 2.0;
            }
        }

        let content_height = if self.rows.is_empty() {
            // Compact empty-state breathing room.
            56.0
        } else {
            cursor_y + 4.0
        };

        let expanded_viewport = (metrics.height
            - metrics.field_height
            - 1.0
            - metrics.list_gap * 2.0
            - metrics.footer_height)
            .max(metrics.max_rows as f32 * metrics.row_height);
        let max_list_height = expanded_viewport;
        let mut visible_list_height = if self.detail.is_some() {
            SPLIT_VIEW_MIN_HEIGHT.max(content_height.min(max_list_height))
        } else {
            content_height.min(max_list_height)
        };
        if let Some(menu) = self.action_menu.as_ref() {
            let min_menu_height = (menu.desired_height() + 16.0).min(max_list_height);
            visible_list_height = visible_list_height.max(min_menu_height);
        }
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

    /// Clamp `scroll_offset` and ensure the selected item (list row or grid cell) is visible.
    pub fn ensure_selected_visible(&mut self, metrics: Metrics) {
        let layout = self.layout(metrics);
        if let Some(grid) = self.grid.as_ref() {
            let max_scroll = layout.max_scroll();
            let mut scroll = self.scroll_offset.clamp(0.0, max_scroll);
            if grid.selected < grid.cells.len() {
                let r = grid.selected / grid.cols();
                let cell_top = r as f32 * GRID_CELL_HEIGHT;
                let cell_bottom = cell_top + GRID_CELL_HEIGHT;
                let viewport = layout.viewport_height();
                if cell_top < scroll {
                    scroll = cell_top;
                } else if cell_bottom > scroll + viewport {
                    scroll = (cell_bottom - viewport).max(0.0);
                }
            }
            self.scroll_offset = scroll.clamp(0.0, max_scroll);
        } else {
            self.scroll_offset = layout.scroll_for_selected(self.selected, self.scroll_offset);
        }
    }

    /// Hit-test a 2D point `(x_dip, y_dip)` against either the Emoji Grid or the row list.
    pub fn hit_test_dip(&self, x_dip: f32, y_dip: f32, metrics: Metrics) -> Option<usize> {
        if self.editor.is_some() {
            return None;
        }
        let layout = self.layout(metrics);
        if y_dip < layout.list_top || y_dip >= layout.list_bottom {
            return None;
        }
        if let Some(grid) = self.grid.as_ref() {
            let cols = grid.cols();
            let show_scrollbar = layout.max_scroll() > 1.0;
            let left = metrics.padding;
            let right = metrics.width
                - if show_scrollbar {
                    metrics.padding + 8.0
                } else {
                    metrics.padding
                };
            if x_dip < left || x_dip >= right {
                return None;
            }
            let cell_w = (right - left) / cols as f32;
            let col = ((x_dip - left) / cell_w).floor() as usize;
            let content_y = (y_dip - layout.list_top) + self.scroll_offset;
            if content_y < 0.0 {
                return None;
            }
            let row = (content_y / GRID_CELL_HEIGHT).floor() as usize;
            let idx = row * cols + col.min(cols - 1);
            return if idx < grid.cells.len() {
                Some(idx)
            } else {
                None
            };
        }
        // In Master-Detail split mode, only clicks in the left master list hit rows.
        if self.detail.is_some() {
            let split_x = (metrics.width * MASTER_SPLIT_RATIO).round();
            if x_dip >= split_x {
                return None;
            }
        }
        layout.row_at_y(y_dip, self.scroll_offset)
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

    #[test]
    fn compact_mode_collapses_window_to_header_height() {
        let metrics = Metrics::default();
        let model = Model {
            rows: vec![Row::named("OpenCode").with_section("Suggestions")],
            compact: true,
            ..Model::default()
        };
        let layout = model.layout(metrics);
        assert_eq!(layout.window_height, metrics.field_height);
        assert!(layout.rows.is_empty());
        assert!(layout.sections.is_empty());
    }

    #[test]
    fn detail_pane_color_ramp_defaults_empty_and_preserves_entries() {
        let default_pane = DetailPane::default();
        assert!(default_pane.color_ramp.is_empty());

        let ramp = vec![
            ([239, 246, 255, 255], "100".to_string()),
            ([59, 130, 246, 255], "500".to_string()),
            ([30, 58, 138, 255], "900".to_string()),
        ];
        let pane = DetailPane {
            color_ramp: ramp.clone(),
            ..DetailPane::default()
        };
        assert_eq!(pane.color_ramp, ramp);
    }

    fn sample_action_menu(count: usize) -> ActionMenuModel {
        ActionMenuModel {
            title: "Actions".to_string(),
            items: (0..count)
                .map(|i| ActionMenuItem::new(format!("act-{i}"), format!("Action {i}"), "General"))
                .collect(),
            ..ActionMenuModel::default()
        }
    }

    #[test]
    fn action_menu_expands_short_list_and_grid_then_contracts() {
        let metrics = Metrics::default();
        let menu = sample_action_menu(8);

        // Short list (1 row, no detail pane)
        let mut list_model = Model {
            rows: vec![Row::named("Single Row")],
            detail: None,
            ..Model::default()
        };
        let base_list_height = list_model.layout(metrics).window_height;

        list_model.action_menu = Some(menu.clone());
        let expanded_list_layout = list_model.layout(metrics);
        assert!(expanded_list_layout.window_height > base_list_height);
        assert_eq!(
            menu.visible_slots(metrics, expanded_list_layout.window_height),
            8
        );

        list_model.action_menu = None;
        assert_eq!(list_model.layout(metrics).window_height, base_list_height);

        // Short 2D grid (1 row of cells)
        let mut grid_model = Model {
            grid: Some(GridModel {
                columns: 6,
                cells: vec![
                    GridCell {
                        glyph: "a".to_string(),
                        name: "Alpha".to_string(),
                        pinned: false,
                    },
                    GridCell {
                        glyph: "b".to_string(),
                        name: "Beta".to_string(),
                        pinned: false,
                    },
                ],
                ..GridModel::default()
            }),
            ..Model::default()
        };
        let base_grid_height = grid_model.layout(metrics).window_height;

        grid_model.action_menu = Some(menu.clone());
        let expanded_grid_layout = grid_model.layout(metrics);
        assert!(expanded_grid_layout.window_height > base_grid_height);
        assert_eq!(
            menu.visible_slots(metrics, expanded_grid_layout.window_height),
            8
        );

        grid_model.action_menu = None;
        assert_eq!(grid_model.layout(metrics).window_height, base_grid_height);
    }

    #[test]
    fn action_menu_clamps_slots_and_hit_tests_accurately_in_constrained_window() {
        let metrics = Metrics::default();
        let constrained_h = 220.0;
        let mut menu = sample_action_menu(10);

        let slots = menu.visible_slots(metrics, constrained_h);
        assert!(slots < ACTION_MENU_MAX_VISIBLE);
        assert_eq!(slots, 2);

        menu.selected = 4;
        let scroll = menu.scroll_start_for_slots(10, slots);
        assert_eq!(scroll, 3);

        let (left, top, right, bottom) = menu.rect(metrics, constrained_h);
        let mid_x = (left + right) * 0.5;
        let rows_top = top + ACTION_MENU_HEADER_H + 4.0;

        // Slot 0 and Slot 1 map to scrolled filtered indices 3 and 4.
        assert_eq!(
            menu.hit_test_dip(mid_x, rows_top + 10.0, metrics, constrained_h),
            Some(3)
        );
        assert_eq!(
            menu.hit_test_dip(
                mid_x,
                rows_top + ACTION_MENU_ROW_H + 10.0,
                metrics,
                constrained_h
            ),
            Some(4)
        );

        // Click inside the menu card below the clamped visible slots is rejected.
        let below_slots_y = rows_top + slots as f32 * ACTION_MENU_ROW_H + 1.0;
        assert!(below_slots_y < bottom);
        assert_eq!(
            menu.hit_test_dip(mid_x, below_slots_y, metrics, constrained_h),
            None
        );
    }

    #[test]
    fn editor_model_uses_split_view_height_even_when_compact_and_blocks_hit_test() {
        let metrics = Metrics::default();
        let model = Model {
            rows: Vec::new(),
            compact: true,
            editor: Some(EditorModel {
                title: "Edit Clipboard Entry".to_string(),
                text: "line 1\nline 2".to_string(),
                primary_label: "Save".to_string(),
                secondary_label: "Save as New".to_string(),
            }),
            ..Model::default()
        };

        let layout = model.layout(metrics);
        let expected_list_top = metrics.field_height + 1.0 + metrics.list_gap;
        let expected_window_height =
            expected_list_top + SPLIT_VIEW_MIN_HEIGHT + metrics.list_gap + metrics.footer_height;

        assert_eq!(layout.viewport_height(), SPLIT_VIEW_MIN_HEIGHT);
        assert_eq!(layout.window_height, expected_window_height);
        assert!(layout.rows.is_empty());
        assert!(layout.sections.is_empty());
        assert_eq!(
            model.hit_test_dip(100.0, layout.list_top + 20.0, metrics),
            None
        );
    }
}
