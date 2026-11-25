//! Context-sensitive `Ctrl+K` Action Panel builders for every YAL screen and
//! launcher row kind.

use tc_clipboard::ClipboardItem;
use tc_snippets::StoredSnippet;
use tc_ui::{ActionMenuItem, ActionMenuModel};

use crate::features::{CalcHistoryEntry, NoteItem, Quicklink};
use crate::launcher::{
    ResultRow, CALCULATOR_SECTION, COMMAND_SECTION, CUSTOM_COMMAND_SECTION, FILE_SECTION,
    SYSTEM_ACTION_SECTION, WINDOW_COMMAND_SECTION,
};
use crate::win_settings::WIN_SETTING_SECTION;

/// Stable action identifiers dispatched by the `Ctrl+K` Action Panel.
pub mod id {
    pub const OPEN: &str = "open";
    pub const RUN_ADMIN: &str = "run_admin";
    pub const REVEAL: &str = "reveal";
    pub const OPEN_TERMINAL: &str = "open_terminal";
    pub const COPY_PATH: &str = "copy_path";
    pub const COPY_FILE: &str = "copy_file";
    pub const PROPERTIES: &str = "properties";
    pub const TOGGLE_PIN: &str = "toggle_pin";
    pub const RECYCLE: &str = "recycle";

    pub const CALC_COPY_ANSWER: &str = "calc_copy_answer";
    pub const CALC_COPY_EXPR: &str = "calc_copy_expr";
    pub const CALC_SWAP_UNITS: &str = "calc_swap_units";
    pub const CALC_PUT_IN_SEARCH: &str = "calc_put_in_search";
    pub const CALC_OPEN_HISTORY: &str = "calc_open_history";
    pub const CALC_REFRESH_FX: &str = "calc_refresh_fx";

    pub const CLIP_PASTE: &str = "clip_paste";
    pub const CLIP_COPY: &str = "clip_copy";
    pub const CLIP_COPY_PLAIN: &str = "clip_copy_plain";
    pub const CLIP_TOGGLE_PIN: &str = "clip_toggle_pin";
    pub const CLIP_CYCLE_FILTER: &str = "clip_cycle_filter";
    pub const CLIP_QUEUE_ADD: &str = "clip_queue_add";
    pub const CLIP_QUEUE_PASTE_NEXT: &str = "clip_queue_paste_next";
    pub const CLIP_QUEUE_CLEAR: &str = "clip_queue_clear";
    pub const CLIP_DELETE: &str = "clip_delete";
    pub const CLIP_CLEAR_UNPINNED: &str = "clip_clear_unpinned";

    pub const SET_HOTKEY: &str = "set_hotkey";
    pub const CLEAR_HOTKEY: &str = "clear_hotkey";
    pub const SET_ALIAS: &str = "set_alias";
    pub const CLEAR_ALIAS: &str = "clear_alias";

    pub const SNIPPET_PASTE: &str = "snippet_paste";
    pub const SNIPPET_COPY: &str = "snippet_copy";
    pub const SNIPPET_COPY_RAW: &str = "snippet_copy_raw";
    pub const SNIPPET_CREATE: &str = "snippet_create";
    pub const SNIPPET_DELETE: &str = "snippet_delete";

    pub const QL_OPEN: &str = "ql_open";
    pub const QL_FILL_ARGS: &str = "ql_fill_args";
    pub const QL_COPY_URL: &str = "ql_copy_url";
    pub const QL_CREATE: &str = "ql_create";
    pub const QL_DELETE: &str = "ql_delete";

    pub const WS_OPEN: &str = "ws_open";
    pub const WS_COPY_URI: &str = "ws_copy_uri";

    pub const NOTE_COPY: &str = "note_copy";
    pub const NOTE_OPEN_FILE: &str = "note_open_file";
    pub const NOTE_REVEAL: &str = "note_reveal";
    pub const NOTE_CREATE: &str = "note_create";
    pub const NOTE_DELETE: &str = "note_delete";

    pub const CALCHIST_COPY: &str = "calchist_copy";
    pub const CALCHIST_PASTE: &str = "calchist_paste";
    pub const CALCHIST_COPY_EXPR: &str = "calchist_copy_expr";
    pub const CALCHIST_USE_IN_LAUNCHER: &str = "calchist_use_in_launcher";
    pub const CALCHIST_DELETE: &str = "calchist_delete";

    pub const RUN_PRIMARY: &str = "run_primary";
    pub const COPY_TEXT: &str = "copy_text";
    pub const TOGGLE_DETAIL: &str = "toggle_detail";
    pub const EMOJI_PASTE: &str = "emoji_paste";
    pub const EMOJI_COPY: &str = "emoji_copy";
    pub const EMOJI_COPY_NAME: &str = "emoji_copy_name";
    pub const EMOJI_TOGGLE_PIN: &str = "emoji_toggle_pin";
}

fn build_menu(title: impl Into<String>, items: Vec<ActionMenuItem>) -> ActionMenuModel {
    ActionMenuModel {
        title: title.into(),
        query: String::new(),
        items,
        selected: 0,
        hovered: None,
    }
}

fn non_empty_title(primary: &str, fallback: &str) -> String {
    if primary.trim().is_empty() {
        fallback.to_string()
    } else {
        primary.to_string()
    }
}

/// Metadata describing contextual launcher row capabilities for the Action Panel.
#[derive(Debug, Clone, Copy, Default)]
pub struct LauncherActionMeta {
    pub is_favorite: bool,
    pub quicklink_needs_args: bool,
    pub has_hotkey: bool,
    pub has_alias: bool,
}

fn push_shortcut_actions(items: &mut Vec<ActionMenuItem>, has_hotkey: bool, has_alias: bool) {
    let hotkey_label = if has_hotkey {
        "Change Hotkey"
    } else {
        "Assign Hotkey"
    };
    items.push(ActionMenuItem::new(id::SET_HOTKEY, hotkey_label, "Shortcuts").with_glyph("⌨"));
    if has_hotkey {
        items.push(
            ActionMenuItem::new(id::CLEAR_HOTKEY, "Clear Hotkey", "Shortcuts").with_glyph("✕"),
        );
    }
    let alias_label = if has_alias {
        "Change Alias"
    } else {
        "Set Alias"
    };
    items.push(ActionMenuItem::new(id::SET_ALIAS, alias_label, "Shortcuts").with_glyph("⌘"));
    if has_alias {
        items
            .push(ActionMenuItem::new(id::CLEAR_ALIAS, "Clear Alias", "Shortcuts").with_glyph("✕"));
    }
}

/// Build the context-sensitive Action Panel for a row in the main Launcher.
pub fn for_launcher_row(row: &ResultRow, query: &str, meta: LauncherActionMeta) -> ActionMenuModel {
    let title = non_empty_title(&row.name, &row.kind);
    match row.kind.as_str() {
        "Applications" => {
            let (pin_label, pin_glyph) = if meta.is_favorite {
                ("Unpin Favorite", "★")
            } else {
                ("Pin Favorite", "☆")
            };
            let mut items = vec![
                ActionMenuItem::new(id::OPEN, "Open", "Primary")
                    .with_shortcut("↵")
                    .with_glyph("↗"),
                ActionMenuItem::new(id::RUN_ADMIN, "Run as Administrator", "Primary")
                    .with_shortcut("Alt+↵")
                    .with_glyph("⚡"),
                ActionMenuItem::new(id::REVEAL, "Open File Location", "File")
                    .with_shortcut("Shift+↵")
                    .with_glyph("▤"),
                ActionMenuItem::new(id::OPEN_TERMINAL, "Open Parent in Terminal", "File")
                    .with_glyph(">"),
                ActionMenuItem::new(id::COPY_PATH, "Copy Path", "File")
                    .with_shortcut("Alt+C")
                    .with_glyph("⧉"),
                ActionMenuItem::new(id::PROPERTIES, "Properties", "File").with_glyph("ℹ"),
                ActionMenuItem::new(id::TOGGLE_PIN, pin_label, "Organize")
                    .with_shortcut("Ctrl+P")
                    .with_glyph(pin_glyph),
            ];
            push_shortcut_actions(&mut items, meta.has_hotkey, meta.has_alias);
            build_menu(title, items)
        }
        FILE_SECTION => for_file_row(row),
        CALCULATOR_SECTION => {
            let mut items = vec![
                ActionMenuItem::new(id::CALC_COPY_ANSWER, "Copy Answer", "Calculator")
                    .with_shortcut("↵")
                    .with_glyph("⧉"),
                ActionMenuItem::new(id::CALC_COPY_EXPR, "Copy Expression", "Calculator")
                    .with_shortcut("Ctrl+↵")
                    .with_glyph("≡"),
            ];
            if tc_calc::swap_units_query(query).is_some() {
                items.push(
                    ActionMenuItem::new(id::CALC_SWAP_UNITS, "Swap Units", "Calculator")
                        .with_shortcut("Ctrl+Shift+S")
                        .with_glyph("⇄"),
                );
            }
            items.push(
                ActionMenuItem::new(
                    id::CALC_PUT_IN_SEARCH,
                    "Put Answer in Search Bar",
                    "Calculator",
                )
                .with_shortcut("Tab")
                .with_glyph("↰"),
            );
            items.push(
                ActionMenuItem::new(id::CALC_OPEN_HISTORY, "Open Calculator History", "Tools")
                    .with_glyph("◷"),
            );
            items.push(
                ActionMenuItem::new(id::CALC_REFRESH_FX, "Refresh Exchange Rates", "Tools")
                    .with_shortcut("Ctrl+R")
                    .with_glyph("↻"),
            );
            build_menu(title, items)
        }
        "Snippets" => {
            let mut items = vec![
                ActionMenuItem::new(id::SNIPPET_PASTE, "Paste Snippet", "Primary")
                    .with_shortcut("↵")
                    .with_glyph("↳"),
                ActionMenuItem::new(id::SNIPPET_COPY, "Copy Expanded", "Copy")
                    .with_shortcut("Ctrl+↵")
                    .with_glyph("⧉"),
                ActionMenuItem::new(id::SNIPPET_COPY_RAW, "Copy Template", "Copy")
                    .with_shortcut("Alt+C")
                    .with_glyph("≡"),
            ];
            push_shortcut_actions(&mut items, meta.has_hotkey, meta.has_alias);
            items.push(
                ActionMenuItem::new(id::SNIPPET_DELETE, "Delete Snippet", "Danger")
                    .with_shortcut("Ctrl+Del")
                    .with_glyph("✕")
                    .destructive(),
            );
            build_menu(title, items)
        }
        "Quicklinks" => {
            let mut items = vec![
                ActionMenuItem::new(id::QL_OPEN, "Open Quicklink", "Primary")
                    .with_shortcut("↵")
                    .with_glyph("↗"),
            ];
            if meta.quicklink_needs_args {
                items.push(
                    ActionMenuItem::new(id::QL_FILL_ARGS, "Fill Arguments", "Primary")
                        .with_shortcut("Tab")
                        .with_glyph("✎"),
                );
            }
            items.push(
                ActionMenuItem::new(id::QL_COPY_URL, "Copy URL", "Copy")
                    .with_shortcut("Alt+C")
                    .with_glyph("⧉"),
            );
            push_shortcut_actions(&mut items, meta.has_hotkey, meta.has_alias);
            items.push(
                ActionMenuItem::new(id::QL_DELETE, "Delete Quicklink", "Danger")
                    .with_shortcut("Ctrl+Del")
                    .with_glyph("✕")
                    .destructive(),
            );
            build_menu(title, items)
        }
        WIN_SETTING_SECTION => {
            let mut items = vec![
                ActionMenuItem::new(id::WS_OPEN, "Open Setting", "Primary")
                    .with_shortcut("↵")
                    .with_glyph("⚙"),
                ActionMenuItem::new(id::WS_COPY_URI, "Copy URI", "Copy")
                    .with_shortcut("Alt+C")
                    .with_glyph("⧉"),
            ];
            push_shortcut_actions(&mut items, meta.has_hotkey, meta.has_alias);
            build_menu(title, items)
        }
        WINDOW_COMMAND_SECTION => {
            let mut items = vec![
                ActionMenuItem::new(id::RUN_PRIMARY, "Apply Window Layout", "Primary")
                    .with_shortcut("↵")
                    .with_glyph("⊞"),
                ActionMenuItem::new(id::COPY_TEXT, "Copy Command Name", "Copy")
                    .with_shortcut("Alt+C")
                    .with_glyph("⧉"),
            ];
            push_shortcut_actions(&mut items, meta.has_hotkey, meta.has_alias);
            build_menu(title, items)
        }
        SYSTEM_ACTION_SECTION => {
            let mut items = vec![
                ActionMenuItem::new(id::RUN_PRIMARY, "Run System Action", "Primary")
                    .with_shortcut("↵")
                    .with_glyph("⚡"),
                ActionMenuItem::new(id::COPY_TEXT, "Copy Action Name", "Copy")
                    .with_shortcut("Alt+C")
                    .with_glyph("⧉"),
            ];
            push_shortcut_actions(&mut items, meta.has_hotkey, meta.has_alias);
            build_menu(title, items)
        }
        COMMAND_SECTION | CUSTOM_COMMAND_SECTION => {
            let mut items = vec![
                ActionMenuItem::new(id::RUN_PRIMARY, "Run Command", "Primary")
                    .with_shortcut("↵")
                    .with_glyph("›"),
                ActionMenuItem::new(id::COPY_TEXT, "Copy Name", "Copy")
                    .with_shortcut("Alt+C")
                    .with_glyph("⧉"),
            ];
            push_shortcut_actions(&mut items, meta.has_hotkey, meta.has_alias);
            build_menu(title, items)
        }
        "Fallback Search" => build_menu(
            title,
            vec![
                ActionMenuItem::new(id::OPEN, "Open in Browser", "Primary")
                    .with_shortcut("↵")
                    .with_glyph("↗"),
                ActionMenuItem::new(id::QL_COPY_URL, "Copy URL", "Copy")
                    .with_shortcut("Alt+C")
                    .with_glyph("⧉"),
            ],
        ),
        _ => build_menu(
            title,
            vec![
                ActionMenuItem::new(id::RUN_PRIMARY, "Open", "Primary")
                    .with_shortcut("↵")
                    .with_glyph("↗"),
                ActionMenuItem::new(id::COPY_TEXT, "Copy Name", "Copy")
                    .with_shortcut("Alt+C")
                    .with_glyph("⧉"),
            ],
        ),
    }
}

/// Build the Action Panel for a file or folder row (Launcher or File Search screen).
pub fn for_file_row(row: &ResultRow) -> ActionMenuModel {
    let title = non_empty_title(&row.name, FILE_SECTION);
    build_menu(
        title,
        vec![
            ActionMenuItem::new(id::OPEN, "Open", "Primary")
                .with_shortcut("↵")
                .with_glyph("↗"),
            ActionMenuItem::new(id::REVEAL, "Show in Explorer", "File")
                .with_shortcut("Shift+↵")
                .with_glyph("▤"),
            ActionMenuItem::new(id::OPEN_TERMINAL, "Open in Terminal", "File").with_glyph(">"),
            ActionMenuItem::new(id::COPY_PATH, "Copy Path", "Copy")
                .with_shortcut("Alt+C")
                .with_glyph("⧉"),
            ActionMenuItem::new(id::COPY_FILE, "Copy File to Clipboard", "Copy").with_glyph("◫"),
            ActionMenuItem::new(id::PROPERTIES, "Properties", "File")
                .with_shortcut("Alt+↵")
                .with_glyph("ℹ"),
            ActionMenuItem::new(id::TOGGLE_DETAIL, "Toggle Preview Pane", "View")
                .with_shortcut("Ctrl+I")
                .with_glyph("◧"),
            ActionMenuItem::new(id::RECYCLE, "Move to Recycle Bin", "Danger")
                .with_shortcut("Ctrl+Del")
                .with_glyph("✕")
                .destructive(),
        ],
    )
}

/// Build the Action Panel for a Clipboard History entry.
pub fn for_clipboard_item(item: &ClipboardItem, queue_len: usize) -> ActionMenuModel {
    let (pin_label, pin_glyph) = if item.is_pinned() {
        ("Unpin Entry", "★")
    } else {
        ("Pin Entry", "☆")
    };
    let paste_next_label = if queue_len > 0 {
        format!("Paste Next in Queue ({queue_len})")
    } else {
        "Paste Next in Queue".to_string()
    };
    let mut items = vec![
        ActionMenuItem::new(id::CLIP_PASTE, "Paste to Active App", "Primary")
            .with_shortcut("↵")
            .with_glyph("↳"),
        ActionMenuItem::new(id::CLIP_COPY, "Copy to Clipboard", "Copy")
            .with_shortcut("Ctrl+↵")
            .with_glyph("⧉"),
        ActionMenuItem::new(id::CLIP_COPY_PLAIN, "Copy Plain Text", "Copy")
            .with_shortcut("Alt+C")
            .with_glyph("≡"),
        ActionMenuItem::new(id::CLIP_TOGGLE_PIN, pin_label, "Organize")
            .with_shortcut("Ctrl+P")
            .with_glyph(pin_glyph),
        ActionMenuItem::new(id::CLIP_CYCLE_FILTER, "Cycle Filter", "View")
            .with_shortcut("Tab")
            .with_glyph("⏷"),
        ActionMenuItem::new(id::TOGGLE_DETAIL, "Toggle Preview Pane", "View")
            .with_shortcut("Ctrl+I")
            .with_glyph("◧"),
        ActionMenuItem::new(id::CLIP_QUEUE_ADD, "Add to Paste Queue", "Paste Queue")
            .with_glyph("+"),
        ActionMenuItem::new(id::CLIP_QUEUE_PASTE_NEXT, paste_next_label, "Paste Queue")
            .with_glyph("⏭"),
    ];
    if queue_len > 0 {
        items.push(
            ActionMenuItem::new(id::CLIP_QUEUE_CLEAR, "Clear Paste Queue", "Paste Queue")
                .with_glyph("⊘")
                .destructive(),
        );
    }
    items.push(
        ActionMenuItem::new(id::CLIP_DELETE, "Delete Entry", "Danger")
            .with_shortcut("Ctrl+Del")
            .with_glyph("✕")
            .destructive(),
    );
    items.push(
        ActionMenuItem::new(id::CLIP_CLEAR_UNPINNED, "Clear Unpinned History", "Danger")
            .with_glyph("⊘")
            .destructive(),
    );
    build_menu("Clipboard Entry", items)
}

/// Build the Action Panel for a Snippet entry.
pub fn for_snippet(
    record: &StoredSnippet,
    in_snippets_screen: bool,
    has_hotkey: bool,
    has_alias: bool,
) -> ActionMenuModel {
    let title = non_empty_title(&record.snippet.name, "Snippet");
    let mut items = vec![
        ActionMenuItem::new(id::SNIPPET_PASTE, "Paste Snippet", "Primary")
            .with_shortcut("↵")
            .with_glyph("↳"),
        ActionMenuItem::new(id::SNIPPET_COPY, "Copy Expanded", "Copy")
            .with_shortcut("Ctrl+↵")
            .with_glyph("⧉"),
        ActionMenuItem::new(id::SNIPPET_COPY_RAW, "Copy Template", "Copy")
            .with_shortcut("Alt+C")
            .with_glyph("≡"),
    ];
    if in_snippets_screen {
        items.push(
            ActionMenuItem::new(id::SNIPPET_CREATE, "Create New Snippet", "Manage").with_glyph("+"),
        );
        items.push(
            ActionMenuItem::new(id::TOGGLE_DETAIL, "Toggle Preview Pane", "View")
                .with_shortcut("Ctrl+I")
                .with_glyph("◧"),
        );
    }
    push_shortcut_actions(&mut items, has_hotkey, has_alias);
    items.push(
        ActionMenuItem::new(id::SNIPPET_DELETE, "Delete Snippet", "Danger")
            .with_shortcut("Ctrl+Del")
            .with_glyph("✕")
            .destructive(),
    );
    build_menu(title, items)
}

/// Build the Action Panel for a Quicklink entry.
pub fn for_quicklink(link: &Quicklink, in_quicklinks_screen: bool) -> ActionMenuModel {
    let title = non_empty_title(&link.name, "Quicklink");
    let mut items = vec![
        ActionMenuItem::new(id::QL_OPEN, "Open Quicklink", "Primary")
            .with_shortcut("↵")
            .with_glyph("↗"),
    ];
    if link.needs_query() {
        items.push(
            ActionMenuItem::new(id::QL_FILL_ARGS, "Fill Arguments", "Primary")
                .with_shortcut("Tab")
                .with_glyph("✎"),
        );
    }
    items.push(
        ActionMenuItem::new(id::QL_COPY_URL, "Copy URL", "Copy")
            .with_shortcut("Alt+C")
            .with_glyph("⧉"),
    );
    if in_quicklinks_screen {
        items.push(
            ActionMenuItem::new(id::QL_CREATE, "Create New Quicklink", "Manage").with_glyph("+"),
        );
        items.push(
            ActionMenuItem::new(id::TOGGLE_DETAIL, "Toggle Preview Pane", "View")
                .with_shortcut("Ctrl+I")
                .with_glyph("◧"),
        );
    }
    items.push(
        ActionMenuItem::new(id::QL_DELETE, "Delete Quicklink", "Danger")
            .with_shortcut("Ctrl+Del")
            .with_glyph("✕")
            .destructive(),
    );
    build_menu(title, items)
}

/// Build the Action Panel for a Quick Note entry.
pub fn for_note(note: &NoteItem) -> ActionMenuModel {
    let title = non_empty_title(&note.title, "Note");
    build_menu(
        title,
        vec![
            ActionMenuItem::new(id::NOTE_COPY, "Copy Note Text", "Primary")
                .with_shortcut("↵")
                .with_glyph("⧉"),
            ActionMenuItem::new(id::NOTE_OPEN_FILE, "Open Markdown File", "File")
                .with_shortcut("Ctrl+↵")
                .with_glyph("↗"),
            ActionMenuItem::new(id::NOTE_REVEAL, "Show in Explorer", "File")
                .with_shortcut("Shift+↵")
                .with_glyph("▤"),
            ActionMenuItem::new(id::NOTE_CREATE, "Create New Note", "Manage").with_glyph("+"),
            ActionMenuItem::new(id::TOGGLE_DETAIL, "Toggle Preview Pane", "View")
                .with_shortcut("Ctrl+I")
                .with_glyph("◧"),
            ActionMenuItem::new(id::NOTE_DELETE, "Delete Note", "Danger")
                .with_shortcut("Ctrl+Del")
                .with_glyph("✕")
                .destructive(),
        ],
    )
}

/// Build the Action Panel for a Calculator History entry.
pub fn for_calc_history(entry: &CalcHistoryEntry) -> ActionMenuModel {
    let title = non_empty_title(&entry.display, &entry.expression);
    build_menu(
        title,
        vec![
            ActionMenuItem::new(id::CALCHIST_COPY, "Copy Answer", "Primary")
                .with_shortcut("↵")
                .with_glyph("⧉"),
            ActionMenuItem::new(id::CALCHIST_PASTE, "Paste Answer", "Primary")
                .with_shortcut("Ctrl+↵")
                .with_glyph("↳"),
            ActionMenuItem::new(id::CALCHIST_COPY_EXPR, "Copy Expression", "Copy")
                .with_shortcut("Alt+C")
                .with_glyph("≡"),
            ActionMenuItem::new(
                id::CALCHIST_USE_IN_LAUNCHER,
                "Put Answer in Search Bar",
                "Calculator",
            )
            .with_shortcut("Tab")
            .with_glyph("↰"),
            ActionMenuItem::new(id::TOGGLE_DETAIL, "Toggle Preview Pane", "View")
                .with_shortcut("Ctrl+I")
                .with_glyph("◧"),
            ActionMenuItem::new(id::CALCHIST_DELETE, "Remove Entry", "Danger")
                .with_shortcut("Ctrl+Del")
                .with_glyph("✕")
                .destructive(),
        ],
    )
}

/// Build the Action Panel for a Switch Windows entry.
pub fn for_window(title: &str) -> ActionMenuModel {
    build_menu(
        non_empty_title(title, "Window"),
        vec![
            ActionMenuItem::new(id::RUN_PRIMARY, "Focus Window", "Primary")
                .with_shortcut("↵")
                .with_glyph("↗"),
            ActionMenuItem::new(id::COPY_TEXT, "Copy Window Title", "Copy")
                .with_shortcut("Alt+C")
                .with_glyph("⧉"),
        ],
    )
}

/// Build the Action Panel for an Emoji Picker cell.
pub fn for_emoji(glyph: &str, name: &str, pinned: bool) -> ActionMenuModel {
    let (pin_label, pin_glyph) = if pinned {
        ("Unpin Emoji", "★")
    } else {
        ("Pin Emoji", "☆")
    };
    let title = match (glyph.trim().is_empty(), name.trim().is_empty()) {
        (false, false) => format!("{glyph} {name}"),
        (false, true) => glyph.to_string(),
        (true, false) => name.to_string(),
        (true, true) => "Emoji".to_string(),
    };
    build_menu(
        title,
        vec![
            ActionMenuItem::new(id::EMOJI_PASTE, "Paste Emoji", "Primary")
                .with_shortcut("↵")
                .with_glyph("↳"),
            ActionMenuItem::new(id::EMOJI_COPY, "Copy Emoji", "Copy")
                .with_shortcut("Ctrl+↵")
                .with_glyph("⧉"),
            ActionMenuItem::new(id::EMOJI_COPY_NAME, "Copy Name", "Copy")
                .with_shortcut("Alt+C")
                .with_glyph("≡"),
            ActionMenuItem::new(id::EMOJI_TOGGLE_PIN, pin_label, "Organize")
                .with_shortcut("Ctrl+P")
                .with_glyph(pin_glyph),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row_with_kind(name: &str, kind: &str) -> ResultRow {
        ResultRow {
            name: name.to_string(),
            kind: kind.to_string(),
            ..ResultRow::default()
        }
    }

    fn item_ids(menu: &ActionMenuModel) -> Vec<&str> {
        menu.items.iter().map(|i| i.id.as_str()).collect()
    }

    fn find_item<'a>(menu: &'a ActionMenuModel, action_id: &str) -> Option<&'a ActionMenuItem> {
        menu.items.iter().find(|i| i.id == action_id)
    }

    #[test]
    fn application_menu_includes_core_actions_and_flips_favorite_label() {
        let row = row_with_kind("Notepad", "Applications");
        let unpinned = for_launcher_row(&row, "note", LauncherActionMeta::default());
        assert_eq!(
            item_ids(&unpinned),
            vec![
                id::OPEN,
                id::RUN_ADMIN,
                id::REVEAL,
                id::OPEN_TERMINAL,
                id::COPY_PATH,
                id::PROPERTIES,
                id::TOGGLE_PIN,
                id::SET_HOTKEY,
                id::SET_ALIAS,
            ]
        );
        assert_eq!(
            find_item(&unpinned, id::TOGGLE_PIN).map(|i| i.title.as_str()),
            Some("Pin Favorite")
        );

        let pinned = for_launcher_row(
            &row,
            "note",
            LauncherActionMeta {
                is_favorite: true,
                has_hotkey: true,
                has_alias: true,
                ..LauncherActionMeta::default()
            },
        );
        assert_eq!(
            find_item(&pinned, id::TOGGLE_PIN).map(|i| i.title.as_str()),
            Some("Unpin Favorite")
        );
        assert!(find_item(&pinned, id::SET_HOTKEY).is_some());
        assert!(find_item(&pinned, id::CLEAR_HOTKEY).is_some());
        assert!(find_item(&pinned, id::SET_ALIAS).is_some());
        assert!(find_item(&pinned, id::CLEAR_ALIAS).is_some());
    }

    #[test]
    fn file_menu_includes_copy_file_and_destructive_recycle() {
        let row = row_with_kind("report.pdf", FILE_SECTION);
        let via_launcher = for_launcher_row(&row, "report", LauncherActionMeta::default());
        let direct = for_file_row(&row);
        assert_eq!(via_launcher, direct);

        assert!(find_item(&direct, id::COPY_FILE).is_some());
        let recycle = find_item(&direct, id::RECYCLE);
        assert!(recycle.is_some());
        assert!(recycle.is_some_and(|item| item.destructive));
    }

    #[test]
    fn calculator_menu_toggles_swap_units_based_on_query() {
        let row = row_with_kind("6.2137 mi", CALCULATOR_SECTION);
        let conversion = for_launcher_row(&row, "10 km to mi", LauncherActionMeta::default());
        assert!(find_item(&conversion, id::CALC_SWAP_UNITS).is_some());

        let arithmetic = for_launcher_row(&row, "2 + 2", LauncherActionMeta::default());
        assert!(find_item(&arithmetic, id::CALC_SWAP_UNITS).is_none());
    }

    #[test]
    fn clipboard_menu_includes_queue_actions_and_destructive_items() {
        let item = ClipboardItem::text("hello world", None);
        let empty_queue = for_clipboard_item(&item, 0);

        assert_eq!(
            find_item(&empty_queue, id::CLIP_CYCLE_FILTER).map(|i| i.shortcut.as_str()),
            Some("Tab")
        );
        assert!(find_item(&empty_queue, id::CLIP_QUEUE_ADD).is_some());
        assert_eq!(
            find_item(&empty_queue, id::CLIP_QUEUE_PASTE_NEXT).map(|i| i.title.as_str()),
            Some("Paste Next in Queue")
        );
        assert!(find_item(&empty_queue, id::CLIP_QUEUE_CLEAR).is_none());

        let delete = find_item(&empty_queue, id::CLIP_DELETE);
        let clear = find_item(&empty_queue, id::CLIP_CLEAR_UNPINNED);
        assert!(delete.is_some_and(|entry| entry.destructive));
        assert!(clear.is_some_and(|entry| entry.destructive));

        let populated_queue = for_clipboard_item(&item, 3);
        assert_eq!(
            find_item(&populated_queue, id::CLIP_QUEUE_PASTE_NEXT).map(|i| i.title.as_str()),
            Some("Paste Next in Queue (3)")
        );
        let clear_queue = find_item(&populated_queue, id::CLIP_QUEUE_CLEAR);
        assert!(clear_queue.is_some_and(|entry| entry.destructive));
    }

    #[test]
    fn snippet_menu_includes_shortcuts_before_danger_delete() {
        let record = StoredSnippet::for_content(
            std::path::PathBuf::from("sig.md"),
            tc_snippets::Snippet::with_options(
                "Signature",
                "Best regards",
                Some("sig".to_string()),
                true,
                false,
            ),
            "Best regards",
        );
        let menu = for_snippet(&record, true, true, true);
        let ids = item_ids(&menu);
        let hotkey_pos = ids
            .iter()
            .position(|&x| x == id::SET_HOTKEY)
            .expect("set hotkey");
        let clear_hotkey_pos = ids
            .iter()
            .position(|&x| x == id::CLEAR_HOTKEY)
            .expect("clear hotkey");
        let alias_pos = ids
            .iter()
            .position(|&x| x == id::SET_ALIAS)
            .expect("set alias");
        let clear_alias_pos = ids
            .iter()
            .position(|&x| x == id::CLEAR_ALIAS)
            .expect("clear alias");
        let delete_pos = ids
            .iter()
            .position(|&x| x == id::SNIPPET_DELETE)
            .expect("delete");
        assert!(hotkey_pos < clear_hotkey_pos);
        assert!(clear_hotkey_pos < alias_pos);
        assert!(alias_pos < clear_alias_pos);
        assert!(clear_alias_pos < delete_pos);
    }

    #[test]
    fn quicklink_menu_includes_fill_args_only_for_templated_urls() {
        let with_query = Quicklink {
            name: "Google Search".to_string(),
            url_template: "https://www.google.com/search?q={query}".to_string(),
            alias: Some("g".to_string()),
        };
        let with_arg = Quicklink {
            name: "GitHub Repo".to_string(),
            url_template: "https://github.com/{argument}".to_string(),
            alias: None,
        };
        let direct = Quicklink {
            name: "Docs".to_string(),
            url_template: "https://docs.rs".to_string(),
            alias: None,
        };

        assert!(find_item(&for_quicklink(&with_query, false), id::QL_FILL_ARGS).is_some());
        assert!(find_item(&for_quicklink(&with_arg, true), id::QL_FILL_ARGS).is_some());
        assert!(find_item(&for_quicklink(&direct, false), id::QL_FILL_ARGS).is_none());
    }

    #[test]
    fn windows_settings_menu_includes_open_and_copy_uri_and_shortcuts() {
        let row = row_with_kind("Display", WIN_SETTING_SECTION);
        let menu = for_launcher_row(&row, "display", LauncherActionMeta::default());
        assert_eq!(
            item_ids(&menu),
            vec![id::WS_OPEN, id::WS_COPY_URI, id::SET_HOTKEY, id::SET_ALIAS]
        );
    }
}
