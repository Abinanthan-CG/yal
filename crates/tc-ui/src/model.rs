//! What the palette is showing.

use crate::theme::Color;

/// The little tile that leads a row.
#[derive(Clone, Debug, PartialEq)]
pub struct Icon {
    /// The letter drawn in the tile.
    pub letter: String,
    /// The tile's colour.
    pub color: Color,
}

impl Icon {
    /// A tile with a letter and a colour.
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
    /// The row's headline.
    pub name: String,
    /// The dim text at the row's right hand end.
    pub detail: String,
    /// The keyboard shortcut the row answers to.
    pub shortcut: String,
    /// The tile that leads the row.
    pub icon: Option<Icon>,
}

impl Row {
    /// A row with only a name.
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            detail: String::new(),
            shortcut: String::new(),
            icon: None,
        }
    }

    /// A row with a name and a detail.
    pub fn detailed(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            detail: detail.into(),
            shortcut: String::new(),
            icon: None,
        }
    }

    /// A row with a tile in front of it.
    pub fn with_icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
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
    /// The hint line at the bottom of the palette.
    pub footer: String,
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

    /// Which row lies under a point, if any.
    ///
    /// The point is in the same units as the measurements - device-independent
    /// pixels - and `list_top` is where the first row begins.
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
        // The bottom edge of the last row belongs to the row below it, which
        // is not there.
        assert_eq!(model.row_at(220.0, 100.0, 40.0), None);
    }

    #[test]
    fn a_point_outside_the_list_finds_nothing() {
        let model = Model {
            rows: vec![Row::named("one"), Row::named("two")],
            ..Model::default()
        };
        // Above the list.
        assert_eq!(model.row_at(10.0, 100.0, 40.0), None);
        // Below the last row.
        assert_eq!(model.row_at(400.0, 100.0, 40.0), None);
        // A row height of nothing divides nothing.
        assert_eq!(model.row_at(110.0, 100.0, 0.0), None);
    }
}
