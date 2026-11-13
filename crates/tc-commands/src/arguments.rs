//! The values a command asks for before it runs.
//!
//! "If the command has arguments, the launcher shows a row of fields before it
//! runs." This module is that row: which field has the focus, what the user
//! has typed, and whether the command may run yet.

use crate::model::CustomCommandArgument;

/// What the row of fields does when the user presses return.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Submission {
    /// Every required value is there; these are the values, in order.
    Ready(Vec<String>),
    /// A required value is missing; this is the field to focus.
    Missing(usize),
}

/// The row of fields for a command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgumentSheet {
    values: Vec<String>,
    optional: Vec<bool>,
    focused: usize,
}

impl ArgumentSheet {
    /// A sheet for a command with these arguments.
    pub fn new(arguments: &[CustomCommandArgument]) -> ArgumentSheet {
        ArgumentSheet {
            values: vec![String::new(); arguments.len()],
            optional: arguments
                .iter()
                .map(|argument| argument.is_optional)
                .collect(),
            focused: 0,
        }
    }

    /// A sheet for a command that takes no values.
    pub fn empty() -> ArgumentSheet {
        ArgumentSheet {
            values: Vec::new(),
            optional: Vec::new(),
            focused: 0,
        }
    }

    /// How many fields there are.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether there are no fields at all.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// The field that has the focus.
    pub fn focused(&self) -> usize {
        self.focused
    }

    /// The value in a field.
    pub fn value(&self, index: usize) -> &str {
        self.values.get(index).map(String::as_str).unwrap_or("")
    }

    /// Puts the focus in a field.
    pub fn focus(&mut self, index: usize) {
        if index < self.values.len() {
            self.focused = index;
        }
    }

    /// Types into a field.
    pub fn set_value(&mut self, index: usize, value: &str) {
        if index < self.values.len() {
            self.values[index] = value.to_string();
        }
    }

    /// Moves to the next field, wrapping around.
    pub fn tab(&mut self) {
        if self.values.is_empty() {
            return;
        }
        self.focused = (self.focused + 1) % self.values.len();
    }

    /// Moves to the previous field, wrapping around.
    pub fn shift_tab(&mut self) {
        if self.values.is_empty() {
            return;
        }
        self.focused = (self.focused + self.values.len() - 1) % self.values.len();
    }

    /// The first required field that is still empty.
    pub fn first_missing(&self) -> Option<usize> {
        self.optional
            .iter()
            .enumerate()
            .find(|(index, optional)| !*optional && self.values[*index].is_empty())
            .map(|(index, _)| index)
    }

    /// Whether the command may run.
    pub fn can_submit(&self) -> bool {
        self.first_missing().is_none()
    }

    /// What happens when the user presses return.
    ///
    /// "return runs the command once every required value is present.
    /// Otherwise it focuses the first missing field."
    pub fn submit(&mut self) -> Submission {
        match self.first_missing() {
            Some(index) => {
                self.focused = index;
                Submission::Missing(index)
            }
            None => Submission::Ready(self.values.clone()),
        }
    }

    /// The values, in order.
    ///
    /// "An empty optional argument still keeps its position, so `$2` never
    /// shifts into `$3`."
    pub fn values(&self) -> &[String] {
        &self.values
    }
}
