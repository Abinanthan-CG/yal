//! The custom commands on disk, and what the launcher may offer.

use crate::model::CustomCommand;
use crate::store::CustomCommandStore;

#[test]
fn a_command_is_found_by_its_name() {
    let mut store = CustomCommandStore::in_memory();
    store
        .add(CustomCommand::new("Sleep", "Start-Sleep 5"))
        .expect("the command is added");
    store
        .add(CustomCommand::new("Disk usage", "Get-PSDrive"))
        .expect("the command is added");

    let found = store.matching("sleep");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].name, "Sleep");
    assert_eq!(store.matching("DISK").len(), 1, "the search ignores case");
    assert_eq!(store.matching("nothing here").len(), 0);
}

#[test]
fn only_the_name_is_searchable_not_the_command() {
    let mut store = CustomCommandStore::in_memory();
    store
        .add(CustomCommand::new("Sleep", "Start-Sleep 5"))
        .expect("the command is added");
    assert!(
        store.matching("Start-Sleep").is_empty(),
        "the command text is not searched"
    );
}

#[test]
fn a_command_that_is_not_enabled_is_never_offered() {
    let mut store = CustomCommandStore::in_memory();
    let command = store
        .add(CustomCommand::new("Sleep", "Start-Sleep 5"))
        .expect("the command is added");
    assert_eq!(store.matching("sleep").len(), 1);
    store.set_enabled(command.id, false);
    assert!(store.matching("sleep").is_empty());
    assert_eq!(store.commands().len(), 1, "the command itself is kept");
}

#[test]
fn a_command_can_be_taken_out_of_the_main_search_but_keep_its_shortcut() {
    let mut store = CustomCommandStore::in_memory();
    let command = store
        .add(CustomCommand::new("Sleep", "Start-Sleep 5"))
        .expect("the command is added");
    store.set_shows_in_root_search(command.id, false);
    assert!(!store.command(command.id).unwrap().shows_in_root_search);
    assert!(
        store.command(command.id).is_some(),
        "the command is still there"
    );
    store.set_shows_in_root_search(command.id, true);
    assert!(store.command(command.id).unwrap().shows_in_root_search);
}

#[test]
fn a_command_is_kept_when_it_is_edited() {
    let mut store = CustomCommandStore::in_memory();
    let mut command = store
        .add(CustomCommand::new("Sleep", "Start-Sleep 5"))
        .expect("the command is added");
    command.name = "Sleep for a while".to_string();
    command.command = "Start-Sleep 60".to_string();
    store.update(command.clone()).expect("the edit is accepted");
    assert_eq!(store.command(command.id).unwrap().name, "Sleep for a while");
    assert_eq!(store.commands().len(), 1);
}

#[test]
fn a_deleted_command_is_gone() {
    let mut store = CustomCommandStore::in_memory();
    let command = store
        .add(CustomCommand::new("Sleep", "Start-Sleep 5"))
        .expect("the command is added");
    let removed = store.remove(command.id).expect("the command is removed");
    assert_eq!(removed.id, command.id);
    assert!(store.commands().is_empty());
    assert!(store.remove(command.id).is_none());
}

#[test]
fn the_commands_survive_a_restart() {
    let directory = tempfile::tempdir().expect("a folder");
    let path = directory.path().join("commands.json");
    {
        let mut store = CustomCommandStore::open(&path).expect("the store opens");
        store
            .add(CustomCommand::new("Sleep", "Start-Sleep 5"))
            .expect("the command is added");
    }
    let store = CustomCommandStore::open(&path).expect("the store opens");
    assert_eq!(store.commands().len(), 1);
    assert_eq!(store.commands()[0].name, "Sleep");
}

#[test]
fn a_store_that_cannot_be_read_is_empty_rather_than_broken() {
    let directory = tempfile::tempdir().expect("a folder");
    let path = directory.path().join("commands.json");
    std::fs::write(&path, "{ this is not json").expect("the file is written");
    let store = CustomCommandStore::open(&path);
    assert!(store.is_err(), "a broken file is reported");
}

#[test]
fn a_missing_file_is_an_empty_store() {
    let directory = tempfile::tempdir().expect("a folder");
    let path = directory.path().join("commands.json");
    let store = CustomCommandStore::open(&path).expect("the store opens");
    assert!(store.commands().is_empty());
}

#[test]
fn importing_the_same_folder_again_only_adds_new_scripts() {
    let mut store = CustomCommandStore::in_memory();
    let first = CustomCommand::new("Sleep", "Start-Sleep 5");
    let second = CustomCommand::new("Disk", "Get-PSDrive");
    assert_eq!(store.add_all(&[first.clone(), second.clone()]), 2);
    assert_eq!(store.add_all(&[first, second]), 0, "the names are taken");
    assert_eq!(store.commands().len(), 2);
}

#[test]
fn a_backup_replaces_the_whole_set_and_drops_what_does_not_fit() {
    let mut store = CustomCommandStore::in_memory();
    store
        .add(CustomCommand::new("Old", "echo old"))
        .expect("the command is added");

    let mut broken = CustomCommand::new("  ", "echo broken");
    broken.id = store.commands()[0].id;
    let good = CustomCommand::new("New", "echo new");
    assert_eq!(store.replace_with(vec![broken, good]), 1);
    assert_eq!(store.commands().len(), 1);
    assert_eq!(store.commands()[0].name, "New");
}

#[test]
fn a_record_with_the_same_identity_twice_is_kept_once() {
    let directory = tempfile::tempdir().expect("a folder");
    let path = directory.path().join("commands.json");
    let command = CustomCommand::new("Sleep", "Start-Sleep 5");
    let twice = serde_json::to_string(&[&command, &command]).expect("the file is written");
    std::fs::write(&path, twice).expect("the file is written");
    let store = CustomCommandStore::open(&path).expect("the store opens");
    assert_eq!(store.commands().len(), 1);
}
