//! The row of fields a command asks for before it runs.

use crate::arguments::{ArgumentSheet, Submission};
use crate::model::CustomCommandArgument;

fn sheet() -> ArgumentSheet {
    ArgumentSheet::new(&[
        CustomCommandArgument::new("From", false),
        CustomCommandArgument::new("To", true),
        CustomCommandArgument::new("Mode", false),
    ])
}

#[test]
fn a_command_with_no_arguments_has_no_fields() {
    let mut sheet = ArgumentSheet::empty();
    assert!(sheet.is_empty());
    assert_eq!(sheet.submit(), Submission::Ready(Vec::new()));
}

#[test]
fn tab_moves_through_the_fields_and_shift_tab_moves_back() {
    let mut sheet = sheet();
    assert_eq!(sheet.focused(), 0);
    sheet.tab();
    assert_eq!(sheet.focused(), 1);
    sheet.tab();
    assert_eq!(sheet.focused(), 2);
    sheet.tab();
    assert_eq!(sheet.focused(), 0, "the fields wrap around");
    sheet.shift_tab();
    assert_eq!(sheet.focused(), 2, "and back the other way");
}

#[test]
fn a_required_argument_cannot_be_left_empty() {
    let mut sheet = sheet();
    sheet.set_value(0, "notes.txt");
    sheet.set_value(2, "copy");
    assert!(sheet.can_submit());
    sheet.set_value(0, "");
    assert!(!sheet.can_submit());
    assert_eq!(sheet.first_missing(), Some(0));
}

#[test]
fn return_focuses_the_first_missing_field() {
    let mut sheet = sheet();
    sheet.set_value(0, "");
    sheet.set_value(2, "");
    match sheet.submit() {
        Submission::Missing(index) => {
            assert_eq!(index, 0);
            assert_eq!(sheet.focused(), 0, "the field takes the focus");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn return_runs_the_command_once_every_required_value_is_present() {
    let mut sheet = sheet();
    sheet.set_value(0, "notes.txt");
    sheet.set_value(1, "backup.txt");
    sheet.set_value(2, "copy");
    match sheet.submit() {
        Submission::Ready(values) => {
            assert_eq!(values.len(), 3);
            assert_eq!(values[0], "notes.txt");
            assert_eq!(values[1], "backup.txt");
            assert_eq!(values[2], "copy");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn an_empty_optional_value_keeps_its_position() {
    let mut sheet = sheet();
    sheet.set_value(0, "notes.txt");
    sheet.set_value(1, "");
    sheet.set_value(2, "copy");
    match sheet.submit() {
        Submission::Ready(values) => {
            assert_eq!(values.len(), 3);
            assert_eq!(values[1], "", "the second value is still the second");
            assert_eq!(values[2], "copy");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_command_stays_selected_while_the_values_are_filled_in() {
    let mut sheet = sheet();
    sheet.set_value(0, "notes.txt");
    sheet.focus(2);
    sheet.set_value(2, "move");
    assert_eq!(sheet.value(0), "notes.txt");
    assert_eq!(sheet.value(2), "move");
    assert_eq!(sheet.focused(), 2);
}
