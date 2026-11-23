//! Pure Direct2D screen model builders for every native YAL sub-screen:
//! Clipboard History, 2D Color Emoji Grid, Snippets Browser, Create Snippet
//! Wizard, File Search, Window Switcher, and Calculator History.

use tc_clipboard::{ClipboardFilter, ClipboardItem, ItemKind, TextForm};
use tc_palette::FileType;
use tc_platform_win::window_control::WindowInfo;
use tc_snippets::StoredSnippet;
use tc_ui::{Color, GridCell, GridModel, Icon, IconBitmap, Model, Row};

use crate::features::{CalcHistoryEntry, NoteItem, Quicklink};
use crate::launcher::ResultRow;

/// Prefix for fallback web-search rows surfaced when root Launcher has no local matches.
pub const FALLBACK_URL_PREFIX: &str = "fallback-url:";

/// Wizard step when creating a Quick Note inline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteDraft {
    pub step: NoteDraftStep,
    pub title: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteDraftStep {
    Title,
    Body,
}

impl NoteDraft {
    pub fn new() -> Self {
        Self {
            step: NoteDraftStep::Title,
            title: String::new(),
        }
    }
}

/// Wizard step when creating a Quicklink inline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuicklinkDraft {
    pub step: QuicklinkDraftStep,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuicklinkDraftStep {
    Name,
    Url,
}

impl QuicklinkDraft {
    pub fn new() -> Self {
        Self {
            step: QuicklinkDraftStep::Name,
            name: String::new(),
        }
    }
}

/// Wizard step when creating a snippet inline inside the palette.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnippetDraft {
    pub step: SnippetDraftStep,
    pub name: String,
    pub keyword: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnippetDraftStep {
    Name,
    Keyword,
    Content,
}

impl SnippetDraft {
    pub fn new() -> Self {
        Self {
            step: SnippetDraftStep::Name,
            name: String::new(),
            keyword: String::new(),
        }
    }
}

/// Build the native Clipboard History screen model.
pub fn clipboard_model(
    query: &str,
    items: &[ClipboardItem],
    filter: ClipboardFilter,
    selected: usize,
) -> Model {
    let rows: Vec<Row> = items
        .iter()
        .map(|item| {
            let section = if item.is_pinned() {
                "Pinned"
            } else {
                "History"
            };
            let title = clipboard_preview_title(item);
            let mut detail_parts = Vec::new();
            if let Some(src) = item.source_bundle_id.as_deref() {
                let compact: String = src.chars().take(28).collect();
                detail_parts.push(compact);
            }
            detail_parts.push(relative_time_ms(item.created_at));
            let detail = detail_parts.join(" · ");

            let (letter, tint, swatch) = match item.kind {
                ItemKind::Image => ("IMG", Color::rgb(0xec, 0x48, 0x99), None),
                ItemKind::File => ("F", Color::rgb(0x38, 0xbd, 0xf8), None),
                ItemKind::Text => match item.text_form() {
                    Some(TextForm::Color) => {
                        let sw = item.color_value().map(|c| {
                            [
                                (c.red.clamp(0.0, 1.0) * 255.0).round() as u8,
                                (c.green.clamp(0.0, 1.0) * 255.0).round() as u8,
                                (c.blue.clamp(0.0, 1.0) * 255.0).round() as u8,
                                (c.alpha.clamp(0.0, 1.0) * 255.0).round() as u8,
                            ]
                        });
                        ("#", Color::rgb(0xa8, 0x55, 0xf7), sw)
                    }
                    Some(TextForm::Link) => ("↗", Color::rgb(0x38, 0xbd, 0xf8), None),
                    Some(TextForm::Email) => ("@", Color::rgb(0x34, 0xd3, 0x99), None),
                    _ => ("T", Color::rgb(0x81, 0x8c, 0xf8), None),
                },
            };

            let mut row = Row::detailed(title, detail)
                .with_section(section)
                .with_icon(Icon::new(letter, tint));
            if item.is_pinned() {
                row = row.with_shortcut("Pinned");
            }
            if let Some(rgba) = swatch {
                row = row.with_color_swatch(rgba);
            }
            row
        })
        .collect();

    let count_label = match rows.len() {
        0 => "Clipboard empty — copy anything to capture".to_string(),
        1 => "1 clip".to_string(),
        n => format!("{n} clips"),
    };

    let badge = if filter == ClipboardFilter::All {
        "Clipboard".to_string()
    } else {
        format!("Clipboard · {}", clipboard_filter_label(filter))
    };

    Model {
        query: query.to_string(),
        rows,
        selected: selected.min(items.len().saturating_sub(1)),
        hovered: None,
        footer: count_label.clone(),
        placeholder: "Search clipboard history…".to_string(),
        header_badge: Some(badge),
        result_count: Some(count_label),
        footer_hints: vec![
            ("↵".to_string(), "Paste".to_string()),
            ("Ctrl+↵".to_string(), "Copy".to_string()),
            ("Ctrl+P".to_string(), "Pin".to_string()),
            ("Ctrl+K".to_string(), "Filter".to_string()),
            ("Esc".to_string(), "Back".to_string()),
        ],
        scroll_offset: 0.0,
        grid: None,
    }
}

/// Cycle the Clipboard filter cleanly.
pub fn next_clipboard_filter(current: ClipboardFilter) -> ClipboardFilter {
    let idx = ClipboardFilter::ALL
        .iter()
        .position(|&f| f == current)
        .unwrap_or(0);
    ClipboardFilter::ALL[(idx + 1) % ClipboardFilter::ALL.len()]
}

fn clipboard_filter_label(filter: ClipboardFilter) -> &'static str {
    match filter {
        ClipboardFilter::All => "All",
        ClipboardFilter::Text => "Text",
        ClipboardFilter::Image => "Images",
        ClipboardFilter::File => "Files",
        ClipboardFilter::Color => "Colors",
        ClipboardFilter::Link => "Links",
        ClipboardFilter::Email => "Email",
    }
}

fn clipboard_preview_title(item: &ClipboardItem) -> String {
    match item.kind {
        ItemKind::Image => "Captured Image".to_string(),
        ItemKind::File => item
            .text
            .as_deref()
            .and_then(|p| p.rsplit(['\\', '/']).next())
            .unwrap_or("File")
            .to_string(),
        ItemKind::Text => {
            let raw = item.text.as_deref().unwrap_or("");
            let collapsed: String = raw
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .chars()
                .take(90)
                .collect();
            if collapsed.is_empty() {
                "(Whitespace)".to_string()
            } else {
                collapsed
            }
        }
    }
}

/// Build the native 2D Color Emoji Picker screen model (8 columns).
pub fn emoji_model(query: &str, cells: Vec<GridCell>, selected: usize) -> Model {
    let clamped = selected.min(cells.len().saturating_sub(1));
    let count_label = match cells.len() {
        0 => "No matching emoji".to_string(),
        1 => "1 emoji".to_string(),
        n => format!("{n} emoji"),
    };
    Model {
        query: query.to_string(),
        rows: Vec::new(),
        selected: 0,
        hovered: None,
        footer: count_label.clone(),
        placeholder: "Search emoji & symbols…".to_string(),
        header_badge: Some("Emoji".to_string()),
        result_count: Some(count_label),
        footer_hints: vec![
            ("↵".to_string(), "Paste".to_string()),
            ("Ctrl+↵".to_string(), "Copy".to_string()),
            ("Ctrl+P".to_string(), "Pin".to_string()),
            ("Esc".to_string(), "Back".to_string()),
        ],
        scroll_offset: 0.0,
        grid: Some(GridModel {
            columns: 8,
            cells,
            selected: clamped,
            hovered: None,
        }),
    }
}

/// Build the native Snippets browser screen model.
pub fn snippets_model(query: &str, snippets: &[StoredSnippet], selected: usize) -> Model {
    let mut rows: Vec<Row> = snippets
        .iter()
        .map(|record| {
            let preview: String = record
                .snippet
                .text
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .chars()
                .take(72)
                .collect();
            let mut row = Row::detailed(&record.snippet.name, preview)
                .with_section("Snippets")
                .with_icon(Icon::new("✂", Color::rgb(0x34, 0xd3, 0x99)));
            if let Some(kw) = record.snippet.keyword.as_deref() {
                if !kw.trim().is_empty() {
                    row = row.with_shortcut(format!("!{}", kw.trim()));
                }
            }
            row
        })
        .collect();

    // Always append a quick "Create New Snippet" action at the bottom when query is empty
    // or matches "new"/"create", so users can create snippets straight from the list.
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() || "create new snippet".contains(&q) {
        rows.push(
            Row::detailed(
                "Create New Snippet",
                "Supports {date}, {time}, {clipboard}, {uuid}, {snippet:Name}",
            )
            .with_section("Actions")
            .with_shortcut("Ctrl+K")
            .with_icon(Icon::new("+", Color::rgb(0x63, 0x66, 0xf1))),
        );
    }

    let count_label = match snippets.len() {
        0 => "No snippets yet — press Ctrl+K to create".to_string(),
        1 => "1 snippet".to_string(),
        n => format!("{n} snippets"),
    };

    Model {
        query: query.to_string(),
        rows,
        selected: selected.min(snippets.len()),
        hovered: None,
        footer: count_label.clone(),
        placeholder: "Search snippets or type !keyword…".to_string(),
        header_badge: Some("Snippets".to_string()),
        result_count: Some(count_label),
        footer_hints: vec![
            ("↵".to_string(), "Paste".to_string()),
            ("Ctrl+↵".to_string(), "Copy".to_string()),
            ("Ctrl+K".to_string(), "New".to_string()),
            ("Esc".to_string(), "Back".to_string()),
        ],
        scroll_offset: 0.0,
        grid: None,
    }
}

/// Build the inline 3-step Create Snippet keyboard wizard model.
pub fn create_snippet_model(draft: &SnippetDraft, current_input: &str) -> Model {
    let (placeholder, step_badge, prompt_row, detail_row) = match draft.step {
        SnippetDraftStep::Name => (
            "Step 1/3 — Snippet name (e.g. Email Signature)…",
            "New Snippet · 1/3",
            if current_input.trim().is_empty() {
                "Type a name for this snippet and press Enter".to_string()
            } else {
                format!("Name: {}", current_input.trim())
            },
            "Required — identifies the snippet in search and {snippet:Name}".to_string(),
        ),
        SnippetDraftStep::Keyword => (
            "Step 2/3 — Optional trigger keyword (e.g. sig) or Enter to skip…",
            "New Snippet · 2/3",
            if current_input.trim().is_empty() {
                "No keyword (press Enter to skip)".to_string()
            } else {
                format!(
                    "Trigger keyword: !{}",
                    current_input.trim().trim_start_matches('!')
                )
            },
            format!("Snippet: {}", draft.name),
        ),
        SnippetDraftStep::Content => (
            "Step 3/3 — Snippet text (or press Enter empty to use Clipboard)…",
            "New Snippet · 3/3",
            if current_input.trim().is_empty() {
                "Press Enter to save current Clipboard text as snippet body".to_string()
            } else {
                format!("Save: {}", current_input.trim())
            },
            "Tokens: {date}, {time}, {datetime}, {day}, {uuid}, {clipboard}".to_string(),
        ),
    };

    let mut rows = vec![Row::detailed(prompt_row, detail_row)
        .with_section("Create Snippet")
        .with_shortcut("↵ Continue")
        .with_icon(Icon::new("✂", Color::rgb(0x34, 0xd3, 0x99)))];

    if !draft.name.is_empty() {
        let kw_label = if draft.keyword.is_empty() {
            "None".to_string()
        } else {
            format!("!{}", draft.keyword)
        };
        rows.push(
            Row::detailed(
                format!("Draft: {}", draft.name),
                format!("Keyword: {kw_label}"),
            )
            .with_section("Summary")
            .with_icon(Icon::new("✓", Color::rgb(0x81, 0x8c, 0xf8))),
        );
    }

    Model {
        query: current_input.to_string(),
        rows,
        selected: 0,
        hovered: None,
        footer: step_badge.to_string(),
        placeholder: placeholder.to_string(),
        header_badge: Some(step_badge.to_string()),
        result_count: Some(step_badge.to_string()),
        footer_hints: vec![
            ("↵".to_string(), "Next / Save".to_string()),
            ("Esc".to_string(), "Cancel".to_string()),
        ],
        scroll_offset: 0.0,
        grid: None,
    }
}

/// Build the dedicated native File Search screen model.
pub fn files_model(
    query: &str,
    results: &[ResultRow],
    filter: FileType,
    indexing: bool,
    mut icon_lookup: impl FnMut(&str) -> Option<std::sync::Arc<IconBitmap>>,
) -> Model {
    let section = if query.trim().is_empty() {
        "Recent Files"
    } else {
        "Matching Files"
    };
    let rows: Vec<Row> = results
        .iter()
        .enumerate()
        .map(|(idx, r)| {
            let is_dir = r.is_dir.unwrap_or(false);
            let icon = if is_dir {
                Icon::new("📁", Color::rgb(0xf5, 0x9e, 0x0b))
            } else {
                Icon::new("F", Color::rgb(0x38, 0xbd, 0xf8))
            };
            let mut row = Row::detailed(&r.name, &r.detail)
                .with_section(section)
                .with_icon(icon);
            if idx < 30 {
                if let Some(bmp) = icon_lookup(&r.target) {
                    row = row.with_bitmap(r.target.clone(), bmp);
                }
            }
            row
        })
        .collect();

    let status = if indexing && rows.is_empty() {
        "Indexing personal folders in background…".to_string()
    } else if indexing {
        format!("{} files (updating index…)", rows.len())
    } else {
        match rows.len() {
            0 => "No matching files".to_string(),
            1 => "1 file".to_string(),
            n => format!("{n} files"),
        }
    };

    let badge = if filter == FileType::All {
        "Files".to_string()
    } else {
        format!("Files · {}", filter.label())
    };

    Model {
        query: query.to_string(),
        rows,
        selected: 0,
        hovered: None,
        footer: status.clone(),
        placeholder: "Search files & folders…".to_string(),
        header_badge: Some(badge),
        result_count: Some(status),
        footer_hints: vec![
            ("↵".to_string(), "Open".to_string()),
            ("Ctrl+↵".to_string(), "Reveal".to_string()),
            ("Ctrl+P".to_string(), "Filter".to_string()),
            ("Esc".to_string(), "Back".to_string()),
        ],
        scroll_offset: 0.0,
        grid: None,
    }
}

/// Build the live Window Switcher screen model.
pub fn windows_model(query: &str, windows: &[WindowInfo], selected: usize) -> Model {
    let rows: Vec<Row> = windows
        .iter()
        .map(|w| {
            let state_tag = if w.minimized { "Minimized" } else { "Open" };
            let detail = if w.class.is_empty() {
                state_tag.to_string()
            } else {
                format!("{} · {}", w.class, state_tag)
            };
            let letter = w
                .title
                .chars()
                .find(|c| c.is_alphanumeric())
                .map(|c| c.to_ascii_uppercase().to_string())
                .unwrap_or_else(|| "W".to_string());
            let mut row = Row::detailed(&w.title, detail)
                .with_section("Open Windows")
                .with_icon(Icon::new(letter, Color::rgb(0x63, 0x66, 0xf1)));
            if w.minimized {
                row = row.with_shortcut("Restore");
            }
            row
        })
        .collect();

    let count_label = match rows.len() {
        0 => "No open windows found".to_string(),
        1 => "1 window".to_string(),
        n => format!("{n} windows"),
    };

    Model {
        query: query.to_string(),
        rows,
        selected: selected.min(windows.len().saturating_sub(1)),
        hovered: None,
        footer: count_label.clone(),
        placeholder: "Switch to open window…".to_string(),
        header_badge: Some("Windows".to_string()),
        result_count: Some(count_label),
        footer_hints: vec![
            ("↵".to_string(), "Focus".to_string()),
            ("↑↓".to_string(), "Select".to_string()),
            ("Esc".to_string(), "Back".to_string()),
        ],
        scroll_offset: 0.0,
        grid: None,
    }
}

/// Build the Calculator History screen model.
pub fn calc_history_model(query: &str, entries: &[CalcHistoryEntry], selected: usize) -> Model {
    let rows: Vec<Row> = entries
        .iter()
        .map(|entry| {
            Row::detailed(&entry.display, &entry.expression)
                .with_section("Calculator History")
                .with_shortcut(&entry.badge)
                .with_icon(Icon::new("=", Color::rgb(0xf5, 0x9e, 0x0b)))
        })
        .collect();

    let count_label = match rows.len() {
        0 => "No calculations copied yet".to_string(),
        1 => "1 calculation".to_string(),
        n => format!("{n} calculations"),
    };

    Model {
        query: query.to_string(),
        rows,
        selected: selected.min(entries.len().saturating_sub(1)),
        hovered: None,
        footer: count_label.clone(),
        placeholder: "Search calculator history…".to_string(),
        header_badge: Some("Calculator".to_string()),
        result_count: Some(count_label),
        footer_hints: vec![
            ("↵".to_string(), "Copy".to_string()),
            ("Ctrl+Del".to_string(), "Remove".to_string()),
            ("Esc".to_string(), "Back".to_string()),
        ],
        scroll_offset: 0.0,
        grid: None,
    }
}

/// Build the Quick Notes browser screen model.
pub fn notes_model(query: &str, notes: &[NoteItem], selected: usize) -> Model {
    let mut rows: Vec<Row> = notes
        .iter()
        .map(|note| {
            let preview: String = note
                .body
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .chars()
                .take(72)
                .collect();
            let detail = if preview.is_empty() {
                relative_time_ms(note.modified_ms)
            } else {
                format!("{preview} · {}", relative_time_ms(note.modified_ms))
            };
            Row::detailed(&note.title, detail)
                .with_section("Notes")
                .with_icon(Icon::new("✎", Color::rgb(0xfb, 0xbf, 0x24)))
        })
        .collect();

    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() || "create new note".contains(&q) {
        rows.push(
            Row::detailed(
                "Create New Note",
                "Saved as portable Markdown under %APPDATA%\\yal\\notes",
            )
            .with_section("Actions")
            .with_shortcut("Ctrl+K")
            .with_icon(Icon::new("+", Color::rgb(0x63, 0x66, 0xf1))),
        );
    }

    let count_label = match notes.len() {
        0 => "No notes yet — press Ctrl+K to write one".to_string(),
        1 => "1 note".to_string(),
        n => format!("{n} notes"),
    };

    Model {
        query: query.to_string(),
        rows,
        selected: selected.min(notes.len()),
        hovered: None,
        footer: count_label.clone(),
        placeholder: "Search quick notes…".to_string(),
        header_badge: Some("Notes".to_string()),
        result_count: Some(count_label),
        footer_hints: vec![
            ("↵".to_string(), "Copy".to_string()),
            ("Ctrl+↵".to_string(), "Open File".to_string()),
            ("Ctrl+K".to_string(), "New".to_string()),
            ("Esc".to_string(), "Back".to_string()),
        ],
        scroll_offset: 0.0,
        grid: None,
    }
}

/// Build the 2-step inline Create Note wizard model.
pub fn create_note_model(draft: &NoteDraft, input: &str) -> Model {
    let (placeholder, badge, prompt, detail) = match draft.step {
        NoteDraftStep::Title => (
            "Step 1/2 — Note title…",
            "New Note · 1/2",
            if input.trim().is_empty() {
                "Type a title for this note and press Enter".to_string()
            } else {
                format!("Title: {}", input.trim())
            },
            "Stored as a portable .md file included in YAL backups".to_string(),
        ),
        NoteDraftStep::Body => (
            "Step 2/2 — Note text (or Enter empty to save Clipboard)…",
            "New Note · 2/2",
            if input.trim().is_empty() {
                "Press Enter to save current Clipboard text into this note".to_string()
            } else {
                format!("Save note: {}", input.trim())
            },
            format!("Title: {}", draft.title),
        ),
    };

    Model {
        query: input.to_string(),
        rows: vec![Row::detailed(prompt, detail)
            .with_section("Create Note")
            .with_shortcut("↵ Continue")
            .with_icon(Icon::new("✎", Color::rgb(0xfb, 0xbf, 0x24)))],
        selected: 0,
        hovered: None,
        footer: badge.to_string(),
        placeholder: placeholder.to_string(),
        header_badge: Some(badge.to_string()),
        result_count: Some(badge.to_string()),
        footer_hints: vec![
            ("↵".to_string(), "Next / Save".to_string()),
            ("Esc".to_string(), "Cancel".to_string()),
        ],
        scroll_offset: 0.0,
        grid: None,
    }
}

/// Build the Quicklinks browser screen model.
pub fn quicklinks_model(query: &str, links: &[Quicklink], selected: usize) -> Model {
    let mut rows: Vec<Row> = links
        .iter()
        .map(|link| {
            let mut row = Row::detailed(&link.name, &link.url_template)
                .with_section("Quicklinks")
                .with_icon(Icon::new("↗", Color::rgb(0x38, 0xbd, 0xf8)));
            if link.needs_query() {
                row = row.with_shortcut("{query}");
            }
            row
        })
        .collect();

    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() || "create quicklink".contains(&q) {
        rows.push(
            Row::detailed(
                "Create New Quicklink",
                "Use {query} in any URL for instant parameterized web search",
            )
            .with_section("Actions")
            .with_shortcut("Ctrl+K")
            .with_icon(Icon::new("+", Color::rgb(0x63, 0x66, 0xf1))),
        );
    }

    let count_label = match links.len() {
        0 => "No matching quicklinks".to_string(),
        1 => "1 quicklink".to_string(),
        n => format!("{n} quicklinks"),
    };

    Model {
        query: query.to_string(),
        rows,
        selected: selected.min(links.len()),
        hovered: None,
        footer: count_label.clone(),
        placeholder: "Search quicklinks…".to_string(),
        header_badge: Some("Quicklinks".to_string()),
        result_count: Some(count_label),
        footer_hints: vec![
            ("↵".to_string(), "Open".to_string()),
            ("Ctrl+K".to_string(), "New".to_string()),
            ("Ctrl+Del".to_string(), "Remove".to_string()),
            ("Esc".to_string(), "Back".to_string()),
        ],
        scroll_offset: 0.0,
        grid: None,
    }
}

/// Build the inline argument entry prompt for a parameterized `{query}` quicklink.
pub fn quicklink_arg_model(link: &Quicklink, input: &str) -> Model {
    let preview_url = link.expand_url(input);
    let label = if input.trim().is_empty() {
        format!("Type search query for {}", link.name)
    } else {
        format!("Search {} for \"{}\"", link.name, input.trim())
    };
    Model {
        query: input.to_string(),
        rows: vec![Row::detailed(label, preview_url)
            .with_section("Quicklink")
            .with_shortcut("↵ Open")
            .with_icon(Icon::new("↗", Color::rgb(0x38, 0xbd, 0xf8)))],
        selected: 0,
        hovered: None,
        footer: link.name.clone(),
        placeholder: format!("{} query…", link.name),
        header_badge: Some(link.name.clone()),
        result_count: Some("Press Enter to open in browser".to_string()),
        footer_hints: vec![
            ("↵".to_string(), "Open Browser".to_string()),
            ("Esc".to_string(), "Back".to_string()),
        ],
        scroll_offset: 0.0,
        grid: None,
    }
}

/// Build the 2-step inline Create Quicklink wizard model.
pub fn create_quicklink_model(draft: &QuicklinkDraft, input: &str) -> Model {
    let (placeholder, badge, prompt, detail) = match draft.step {
        QuicklinkDraftStep::Name => (
            "Step 1/2 — Quicklink name (e.g. Rust Docs)…",
            "New Quicklink · 1/2",
            if input.trim().is_empty() {
                "Type a name for this quicklink and press Enter".to_string()
            } else {
                format!("Name: {}", input.trim())
            },
            "Appears in Quicklinks screen and YAL backups".to_string(),
        ),
        QuicklinkDraftStep::Url => (
            "Step 2/2 — URL (include {query} for search)…",
            "New Quicklink · 2/2",
            if input.trim().is_empty() {
                "Type URL e.g. https://docs.rs/releases/search?query={query}".to_string()
            } else {
                format!("Save URL: {}", input.trim())
            },
            format!("Quicklink: {}", draft.name),
        ),
    };

    Model {
        query: input.to_string(),
        rows: vec![Row::detailed(prompt, detail)
            .with_section("Create Quicklink")
            .with_shortcut("↵ Continue")
            .with_icon(Icon::new("↗", Color::rgb(0x38, 0xbd, 0xf8)))],
        selected: 0,
        hovered: None,
        footer: badge.to_string(),
        placeholder: placeholder.to_string(),
        header_badge: Some(badge.to_string()),
        result_count: Some(badge.to_string()),
        footer_hints: vec![
            ("↵".to_string(), "Next / Save".to_string()),
            ("Esc".to_string(), "Cancel".to_string()),
        ],
        scroll_offset: 0.0,
        grid: None,
    }
}

/// Build the inline Run Shell Command screen model.
pub fn run_shell_model(input: &str, last_result: Option<&str>) -> Model {
    let mut rows = vec![Row::detailed(
        if input.trim().is_empty() {
            "Type a PowerShell command and press Enter".to_string()
        } else {
            format!("Run: {}", input.trim())
        },
        "Runs in PowerShell and copies output to clipboard",
    )
    .with_section("Shell")
    .with_shortcut("↵ Run")
    .with_icon(Icon::new(">_", Color::rgb(0xa7, 0x8b, 0xfa)))];

    if let Some(output) = last_result {
        let preview: String = output
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .take(110)
            .collect();
        rows.push(
            Row::detailed(preview, "Copied to clipboard")
                .with_section("Last Output")
                .with_icon(Icon::new("✓", Color::rgb(0x34, 0xd3, 0x99))),
        );
    }

    Model {
        query: input.to_string(),
        rows,
        selected: 0,
        hovered: None,
        footer: "PowerShell".to_string(),
        placeholder: "Run PowerShell command…".to_string(),
        header_badge: Some("Shell".to_string()),
        result_count: Some("Output copied to clipboard".to_string()),
        footer_hints: vec![
            ("↵".to_string(), "Run".to_string()),
            ("Esc".to_string(), "Back".to_string()),
        ],
        scroll_offset: 0.0,
        grid: None,
    }
}

/// Build the About YAL screen (or toast/status feedback view).
pub fn status_screen_model(badge: &str, title: &str, detail: &str, extra_rows: Vec<Row>) -> Model {
    let mut rows = vec![Row::detailed(title, detail)
        .with_section(badge)
        .with_icon(Icon::new("✦", Color::rgb(0x63, 0x66, 0xf1)))];
    rows.extend(extra_rows);
    Model {
        query: String::new(),
        rows,
        selected: 0,
        hovered: None,
        footer: badge.to_string(),
        placeholder: "Press Esc to return to Launcher…".to_string(),
        header_badge: Some(badge.to_string()),
        result_count: Some(badge.to_string()),
        footer_hints: vec![("Esc".to_string(), "Back".to_string())],
        scroll_offset: 0.0,
        grid: None,
    }
}

/// Fallback web search rows offered at root Launcher when no local result matched `query`.
pub fn fallback_search_rows(query: &str, quicklinks: &[Quicklink]) -> Vec<ResultRow> {
    let trimmed = query.trim();
    if trimmed.chars().count() < 2 {
        return Vec::new();
    }
    quicklinks
        .iter()
        .filter(|l| l.needs_query())
        .take(4)
        .map(|link| {
            let url = link.expand_url(trimmed);
            ResultRow {
                id: format!("{FALLBACK_URL_PREFIX}{url}"),
                name: format!("Search {} for \"{trimmed}\"", link.name),
                detail: url.clone(),
                kind: "Fallback Search".to_string(),
                target: url,
                ..ResultRow::default()
            }
        })
        .collect()
}

/// Format a millisecond Unix timestamp as a crisp relative age label (`Just now`, `4m ago`, `2h ago`, `3d ago`).
fn relative_time_ms(ts_ms: i64) -> String {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(ts_ms);
    let secs = ((now_ms - ts_ms) / 1000).max(0);
    if secs < 45 {
        "Just now".to_string()
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86_400 {
        format!("{}h ago", secs / 3600)
    } else {
        format!("{}d ago", secs / 86_400)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emoji_model_populates_8_column_grid() {
        let cells = vec![
            GridCell {
                glyph: "🔥".to_string(),
                name: "fire".to_string(),
                pinned: true,
            },
            GridCell {
                glyph: "🚀".to_string(),
                name: "rocket".to_string(),
                pinned: false,
            },
        ];
        let model = emoji_model("fire", cells, 0);
        let grid = model.grid.expect("emoji grid");
        assert_eq!(grid.columns, 8);
        assert_eq!(grid.cells.len(), 2);
        assert_eq!(model.header_badge.as_deref(), Some("Emoji"));
    }

    #[test]
    fn clipboard_model_marks_pinned_and_color_swatches() {
        let mut color_item = ClipboardItem::text("#FF5733", Some("VS Code".to_string()));
        color_item.pinned_at = Some(1_700_000_000_000);
        let model = clipboard_model("", &[color_item], ClipboardFilter::All, 0);
        assert_eq!(model.rows.len(), 1);
        assert_eq!(model.rows[0].section, "Pinned");
        assert_eq!(model.rows[0].color_swatch, Some([0xff, 0x57, 0x33, 0xff]));
    }
}
