//! Pure Direct2D screen model builders for every native YAL sub-screen:
//! Clipboard History, 2D Color Emoji Grid, Snippets Browser, Create Snippet
//! Wizard, Quick Notes, Quicklinks & Inline Arguments, File Search, Window
//! Switcher, Calculator History, and Shell Runner.

use std::io::Read;
use std::path::Path;

use tc_clipboard::{ClipboardFilter, ClipboardItem, ItemKind, TextForm};
use tc_palette::FileType;
use tc_platform_win::window_control::WindowInfo;
use tc_snippets::StoredSnippet;
use tc_ui::{
    Color, DetailPane, GridCell, GridModel, Icon, IconBitmap, InlineArgBanner, Model, Row,
};

use crate::features::{CalcHistoryEntry, NoteItem, Quicklink};
use crate::launcher::{ResultRow, CALCULATOR_SECTION};

/// Prefix for fallback web-search rows surfaced when root Launcher has no local matches.
pub const FALLBACK_URL_PREFIX: &str = "fallback-url:";

/// Maximum file size (bytes) read for inline UTF-8 text preview in File Search.
const MAX_FILE_PREVIEW_BYTES: u64 = 32 * 1024;
/// Maximum number of lines shown in a file preview pane.
const MAX_FILE_PREVIEW_LINES: usize = 40;
/// Maximum number of characters kept in any detail pane preview body.
const MAX_DETAIL_BODY_CHARS: usize = 3600;

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

/// Build the native Clipboard History screen model with Master-Detail preview.
pub fn clipboard_model(
    query: &str,
    items: &[ClipboardItem],
    filter: ClipboardFilter,
    selected: usize,
    show_detail: bool,
) -> Model {
    let clamped_sel = selected.min(items.len().saturating_sub(1));
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
                        let sw = item.color_value().map(color_value_to_rgba);
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
                row = row.with_shortcut("★");
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

    let detail = if show_detail {
        items.get(clamped_sel).map(clipboard_detail)
    } else {
        None
    };

    Model {
        query: query.to_string(),
        rows,
        selected: clamped_sel,
        hovered: None,
        footer: count_label.clone(),
        placeholder: "Search clipboard history…".to_string(),
        header_badge: Some(badge),
        result_count: Some(count_label),
        footer_hints: vec![
            ("↵".to_string(), "Paste".to_string()),
            ("Ctrl+K".to_string(), "Actions".to_string()),
            ("Ctrl+I".to_string(), "Preview".to_string()),
        ],
        scroll_offset: 0.0,
        grid: None,
        back_button: true,
        detail,
        primary_action: Some("Paste".to_string()),
        ..Model::default()
    }
}

/// Build the Master-Detail pane for a single Clipboard History entry.
pub fn clipboard_detail(item: &ClipboardItem) -> DetailPane {
    let (type_label, swatch) = match item.kind {
        ItemKind::Image => ("Image", None),
        ItemKind::File => ("File", None),
        ItemKind::Text => match item.text_form() {
            Some(TextForm::Color) => ("Color", item.color_value().map(color_value_to_rgba)),
            Some(TextForm::Link) => ("Link", None),
            Some(TextForm::Email) => ("Email", None),
            _ => ("Text", None),
        },
    };

    let raw_body = item.text.as_deref().unwrap_or("");
    let body = if raw_body.is_empty() {
        match item.kind {
            ItemKind::Image => "Captured clipboard image".to_string(),
            ItemKind::File => "Copied file reference".to_string(),
            ItemKind::Text => "(Empty)".to_string(),
        }
    } else {
        truncate_chars(raw_body, MAX_DETAIL_BODY_CHARS)
    };

    let chars = raw_body.chars().count();
    let words = raw_body.split_whitespace().count();
    let lines = if raw_body.is_empty() {
        0
    } else {
        raw_body.lines().count().max(1)
    };

    let title = if swatch.is_some() {
        format!("Color · {}", raw_body.trim())
    } else {
        type_label.to_string()
    };

    let mut metadata = vec![
        ("Type".to_string(), type_label.to_string()),
        ("Characters".to_string(), chars.to_string()),
        ("Words".to_string(), words.to_string()),
        ("Lines".to_string(), lines.to_string()),
        (
            "Pinned".to_string(),
            if item.is_pinned() { "Yes" } else { "No" }.to_string(),
        ),
        ("Captured".to_string(), relative_time_ms(item.created_at)),
    ];
    if let Some(src) = item.source_bundle_id.as_deref() {
        if !src.trim().is_empty() {
            metadata.push(("Source".to_string(), src.to_string()));
        }
    }

    DetailPane {
        title,
        body,
        color_swatch: swatch,
        image: None,
        image_key: None,
        metadata,
    }
}

fn color_value_to_rgba(c: tc_clipboard::color::ColorValue) -> [u8; 4] {
    [
        (c.red.clamp(0.0, 1.0) * 255.0).round() as u8,
        (c.green.clamp(0.0, 1.0) * 255.0).round() as u8,
        (c.blue.clamp(0.0, 1.0) * 255.0).round() as u8,
        (c.alpha.clamp(0.0, 1.0) * 255.0).round() as u8,
    ]
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
            ("Ctrl+P".to_string(), "Pin".to_string()),
            ("Ctrl+K".to_string(), "Actions".to_string()),
        ],
        scroll_offset: 0.0,
        grid: Some(GridModel {
            columns: 8,
            cells,
            selected: clamped,
            hovered: None,
        }),
        back_button: true,
        primary_action: Some("Paste".to_string()),
        ..Model::default()
    }
}

/// Whether the trailing "Create New Snippet" action row is shown for `query`.
pub fn shows_create_snippet_row(query: &str) -> bool {
    let q = query.trim().to_ascii_lowercase();
    q.is_empty() || "create new snippet".contains(&q)
}

/// Whether the trailing "Create New Note" action row is shown for `query`.
pub fn shows_create_note_row(query: &str) -> bool {
    let q = query.trim().to_ascii_lowercase();
    q.is_empty() || "create new note".contains(&q)
}

/// Whether the trailing "Create New Quicklink" action row is shown for `query`.
pub fn shows_create_quicklink_row(query: &str) -> bool {
    let q = query.trim().to_ascii_lowercase();
    q.is_empty() || "create quicklink".contains(&q)
}

/// Build the native Snippets browser screen model with live expanded Master-Detail preview.
pub fn snippets_model(
    query: &str,
    snippets: &[StoredSnippet],
    selected: usize,
    show_detail: bool,
    mut expand_fn: impl FnMut(&StoredSnippet) -> String,
) -> Model {
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

    if shows_create_snippet_row(query) {
        rows.push(
            Row::detailed(
                "Create New Snippet",
                "Supports {date}, {time}, {clipboard}, {uuid}, {snippet:Name}",
            )
            .with_section("Actions")
            .with_icon(Icon::new("+", Color::rgb(0x63, 0x66, 0xf1))),
        );
    }

    let count_label = match snippets.len() {
        0 => "No snippets yet — select Create New Snippet".to_string(),
        1 => "1 snippet".to_string(),
        n => format!("{n} snippets"),
    };

    let clamped = selected.min(rows.len().saturating_sub(1));
    let detail = if show_detail {
        snippets.get(clamped).map(|record| {
            let expanded = expand_fn(record);
            snippet_detail(record, &expanded)
        })
    } else {
        None
    };

    let primary = if clamped < snippets.len() {
        "Paste"
    } else {
        "Create"
    };

    Model {
        query: query.to_string(),
        rows,
        selected: clamped,
        hovered: None,
        footer: count_label.clone(),
        placeholder: "Search snippets or type !keyword…".to_string(),
        header_badge: Some("Snippets".to_string()),
        result_count: Some(count_label),
        footer_hints: vec![
            ("↵".to_string(), primary.to_string()),
            ("Ctrl+K".to_string(), "Actions".to_string()),
            ("Ctrl+I".to_string(), "Preview".to_string()),
        ],
        scroll_offset: 0.0,
        grid: None,
        back_button: true,
        detail,
        primary_action: Some(primary.to_string()),
        ..Model::default()
    }
}

/// Build the Master-Detail preview pane for a stored Snippet.
pub fn snippet_detail(record: &StoredSnippet, expanded: &str) -> DetailPane {
    let keyword = record
        .snippet
        .keyword
        .as_deref()
        .map(str::trim)
        .filter(|k| !k.is_empty())
        .map(|k| format!("!{}", k.trim_start_matches('!')))
        .unwrap_or_else(|| "None".to_string());

    let placeholders = extract_placeholders(&record.snippet.text);
    let placeholder_summary = if placeholders.is_empty() {
        "None".to_string()
    } else {
        placeholders.join(", ")
    };

    DetailPane {
        title: record.snippet.name.clone(),
        body: truncate_chars(expanded, MAX_DETAIL_BODY_CHARS),
        color_swatch: None,
        image: None,
        image_key: None,
        metadata: vec![
            ("Keyword".to_string(), keyword),
            ("Placeholders".to_string(), placeholder_summary),
            (
                "Characters".to_string(),
                expanded.chars().count().to_string(),
            ),
        ],
    }
}

/// Detect `{placeholder}` tokens inside a snippet template in order of appearance.
fn extract_placeholders(template: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        let after_open = &rest[open..];
        if let Some(close) = after_open.find('}') {
            let token = &after_open[..=close];
            if token.len() > 2 && token.len() <= 40 && !token.contains('\n') {
                let owned = token.to_string();
                if !found.contains(&owned) {
                    found.push(owned);
                }
            }
            rest = &after_open[close + 1..];
        } else {
            break;
        }
    }
    found
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
        back_button: true,
        primary_action: Some("Continue".to_string()),
        ..Model::default()
    }
}

/// Build the dedicated native File Search screen model with Master-Detail preview.
pub fn files_model(
    query: &str,
    results: &[ResultRow],
    filter: FileType,
    indexing: bool,
    selected: usize,
    show_detail: bool,
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

    let clamped = selected.min(results.len().saturating_sub(1));
    let detail = if show_detail {
        results.get(clamped).map(file_detail)
    } else {
        None
    };

    Model {
        query: query.to_string(),
        rows,
        selected: clamped,
        hovered: None,
        footer: status.clone(),
        placeholder: "Search files & folders…".to_string(),
        header_badge: Some(badge),
        result_count: Some(status),
        footer_hints: vec![
            ("↵".to_string(), "Open".to_string()),
            ("Ctrl+K".to_string(), "Actions".to_string()),
            ("Ctrl+I".to_string(), "Preview".to_string()),
        ],
        scroll_offset: 0.0,
        grid: None,
        back_button: true,
        detail,
        primary_action: Some("Open".to_string()),
        ..Model::default()
    }
}

/// Build the Master-Detail preview pane for a file or folder result.
pub fn file_detail(row: &ResultRow) -> DetailPane {
    let path = Path::new(&row.target);
    let meta = std::fs::metadata(path).ok();
    let is_dir = row
        .is_dir
        .unwrap_or_else(|| meta.as_ref().is_some_and(|m| m.is_dir()));

    let type_label = if is_dir {
        "Folder".to_string()
    } else {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| format!("{} File", e.to_ascii_uppercase()))
            .unwrap_or_else(|| "File".to_string())
    };

    let size_label = if is_dir {
        "Folder".to_string()
    } else if let Some(ref m) = meta {
        format_bytes(m.len())
    } else {
        "Unknown".to_string()
    };

    let modified_label = meta
        .as_ref()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| relative_time_ms(d.as_millis() as i64))
        .unwrap_or_else(|| "Unknown".to_string());

    let body = if is_dir {
        preview_folder_contents(path)
    } else {
        preview_text_file(path, meta.as_ref())
    };

    DetailPane {
        title: row.name.clone(),
        body,
        color_swatch: None,
        image: None,
        image_key: None,
        metadata: vec![
            ("Path".to_string(), row.target.clone()),
            ("Type".to_string(), type_label),
            ("Size".to_string(), size_label),
            ("Modified".to_string(), modified_label),
        ],
    }
}

fn preview_folder_contents(path: &Path) -> String {
    let Ok(entries) = std::fs::read_dir(path) else {
        return "Folder — press Enter to open in Explorer".to_string();
    };
    let mut names: Vec<String> = Vec::new();
    for entry in entries.flatten().take(24) {
        let name = entry.file_name().to_string_lossy().to_string();
        let suffix = if entry.file_type().is_ok_and(|t| t.is_dir()) {
            "/"
        } else {
            ""
        };
        names.push(format!("{name}{suffix}"));
    }
    if names.is_empty() {
        "Empty folder".to_string()
    } else {
        names.sort();
        names.join("\n")
    }
}

fn preview_text_file(path: &Path, meta: Option<&std::fs::Metadata>) -> String {
    let Some(m) = meta else {
        return "File unavailable on disk".to_string();
    };
    if m.len() == 0 {
        return "(Empty file)".to_string();
    }
    if m.len() > MAX_FILE_PREVIEW_BYTES {
        return format!(
            "File preview skipped ({} exceeds 32 KB limit).\nPress Enter to open.",
            format_bytes(m.len())
        );
    }
    let Ok(file) = std::fs::File::open(path) else {
        return "Unable to read file preview".to_string();
    };
    let mut buf = Vec::with_capacity(m.len().min(MAX_FILE_PREVIEW_BYTES) as usize);
    if file
        .take(MAX_FILE_PREVIEW_BYTES + 1)
        .read_to_end(&mut buf)
        .is_err()
    {
        return "Unable to read file preview".to_string();
    }
    if buf.len() as u64 > MAX_FILE_PREVIEW_BYTES || buf.contains(&0) {
        return "Binary file — press Enter to open".to_string();
    }
    let Ok(text) = std::str::from_utf8(&buf) else {
        return "Binary file — press Enter to open".to_string();
    };
    let mut lines: Vec<&str> = text.lines().take(MAX_FILE_PREVIEW_LINES + 1).collect();
    let truncated = lines.len() > MAX_FILE_PREVIEW_LINES;
    if truncated {
        lines.truncate(MAX_FILE_PREVIEW_LINES);
    }
    let mut preview = truncate_chars(&lines.join("\n"), MAX_DETAIL_BODY_CHARS);
    if truncated {
        preview.push_str("\n…");
    }
    preview
}

fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    const GB: f64 = 1024.0 * 1024.0 * 1024.0;
    let b = bytes as f64;
    if bytes < 1024 {
        format!("{bytes} B")
    } else if b < MB {
        format!("{:.1} KB", b / KB)
    } else if b < GB {
        format!("{:.1} MB", b / MB)
    } else {
        format!("{:.2} GB", b / GB)
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
            ("Ctrl+K".to_string(), "Actions".to_string()),
            ("Esc".to_string(), "Back".to_string()),
        ],
        scroll_offset: 0.0,
        grid: None,
        back_button: true,
        primary_action: Some("Focus".to_string()),
        ..Model::default()
    }
}

/// Build the Calculator History screen model with optional live `ans` hero card & Master-Detail pane.
pub fn calc_history_model(
    query: &str,
    live_calc: Option<&ResultRow>,
    entries: &[CalcHistoryEntry],
    selected: usize,
    show_detail: bool,
) -> Model {
    let mut rows: Vec<Row> = Vec::with_capacity(entries.len() + usize::from(live_calc.is_some()));

    if let Some(calc) = live_calc {
        let source_badge = calc
            .calc_source_badge
            .clone()
            .filter(|b| !b.is_empty())
            .unwrap_or_else(|| "Expression".to_string());
        let badge = calc
            .calc_badge
            .clone()
            .filter(|b| !b.is_empty())
            .unwrap_or_else(|| "Result".to_string());
        rows.push(Row {
            name: calc.name.clone(),
            detail: calc.detail.clone(),
            icon: Some(Icon::new("=", Color::rgb(0x60, 0xa5, 0xfa))),
            section: CALCULATOR_SECTION.to_string(),
            source_badge,
            badge,
            is_calculator: true,
            color_swatch: calc.color_swatch,
            ..Row::default()
        });
    }

    for entry in entries {
        rows.push(
            Row::detailed(&entry.display, &entry.expression)
                .with_section("Calculator History")
                .with_shortcut(&entry.badge)
                .with_icon(Icon::new("=", Color::rgb(0xf5, 0x9e, 0x0b))),
        );
    }

    let count_label = match entries.len() {
        0 if live_calc.is_some() => "Live calculation (ans supported)".to_string(),
        0 => "No calculations copied yet".to_string(),
        1 => "1 calculation".to_string(),
        n => format!("{n} calculations"),
    };

    let clamped = selected.min(rows.len().saturating_sub(1));
    let detail = if show_detail {
        if let Some(calc) = live_calc {
            if clamped == 0 {
                Some(DetailPane {
                    title: calc.name.clone(),
                    body: format!("{} = {}", calc.detail, calc.name),
                    color_swatch: calc.color_swatch,
                    image: None,
                    image_key: None,
                    metadata: vec![
                        ("Expression".to_string(), calc.detail.clone()),
                        ("Answer".to_string(), calc.target.clone()),
                        ("Saved".to_string(), "Live".to_string()),
                    ],
                })
            } else {
                entries.get(clamped - 1).map(calc_history_detail)
            }
        } else {
            entries.get(clamped).map(calc_history_detail)
        }
    } else {
        None
    };

    Model {
        query: query.to_string(),
        rows,
        selected: clamped,
        hovered: None,
        footer: count_label.clone(),
        placeholder: "Search history or calculate with `ans`…".to_string(),
        header_badge: Some("Calculator".to_string()),
        result_count: Some(count_label),
        footer_hints: vec![
            ("↵".to_string(), "Copy".to_string()),
            ("Ctrl+K".to_string(), "Actions".to_string()),
            ("Ctrl+I".to_string(), "Preview".to_string()),
        ],
        scroll_offset: 0.0,
        grid: None,
        back_button: true,
        detail,
        primary_action: Some("Copy".to_string()),
        ..Model::default()
    }
}

/// Build the Master-Detail pane for a saved Calculator History entry.
pub fn calc_history_detail(entry: &CalcHistoryEntry) -> DetailPane {
    DetailPane {
        title: entry.display.clone(),
        body: format!("{} = {}", entry.expression, entry.display),
        color_swatch: None,
        image: None,
        image_key: None,
        metadata: vec![
            ("Expression".to_string(), entry.expression.clone()),
            ("Answer".to_string(), entry.copy_text.clone()),
            ("Category".to_string(), entry.badge.clone()),
            (
                "Saved".to_string(),
                relative_time_ms(entry.created_at.saturating_mul(1000)),
            ),
        ],
    }
}

/// Build the Quick Notes browser screen model with optional Master-Detail preview.
pub fn notes_model(query: &str, notes: &[NoteItem], selected: usize, show_detail: bool) -> Model {
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

    if shows_create_note_row(query) {
        rows.push(
            Row::detailed(
                "Create New Note",
                "Saved as portable Markdown under %APPDATA%\\yal\\notes",
            )
            .with_section("Actions")
            .with_icon(Icon::new("+", Color::rgb(0x63, 0x66, 0xf1))),
        );
    }

    let count_label = match notes.len() {
        0 => "No notes yet — select Create New Note".to_string(),
        1 => "1 note".to_string(),
        n => format!("{n} notes"),
    };

    let clamped = selected.min(rows.len().saturating_sub(1));
    let detail = if show_detail {
        notes.get(clamped).map(note_detail)
    } else {
        None
    };

    let primary = if clamped < notes.len() {
        "Copy"
    } else {
        "Create"
    };

    Model {
        query: query.to_string(),
        rows,
        selected: clamped,
        hovered: None,
        footer: count_label.clone(),
        placeholder: "Search quick notes…".to_string(),
        header_badge: Some("Notes".to_string()),
        result_count: Some(count_label),
        footer_hints: vec![
            ("↵".to_string(), primary.to_string()),
            ("Ctrl+K".to_string(), "Actions".to_string()),
            ("Ctrl+I".to_string(), "Preview".to_string()),
        ],
        scroll_offset: 0.0,
        grid: None,
        back_button: true,
        detail,
        primary_action: Some(primary.to_string()),
        ..Model::default()
    }
}

/// Build the Master-Detail preview pane for a Quick Note.
pub fn note_detail(note: &NoteItem) -> DetailPane {
    let words = note.body.split_whitespace().count();
    let chars = note.body.chars().count();
    DetailPane {
        title: note.title.clone(),
        body: truncate_chars(&note.body, MAX_DETAIL_BODY_CHARS),
        color_swatch: None,
        image: None,
        image_key: None,
        metadata: vec![
            ("Characters".to_string(), chars.to_string()),
            ("Words".to_string(), words.to_string()),
            ("Modified".to_string(), relative_time_ms(note.modified_ms)),
            ("File".to_string(), note.path.to_string_lossy().to_string()),
        ],
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
        back_button: true,
        primary_action: Some("Continue".to_string()),
        ..Model::default()
    }
}

/// Build the Quicklinks browser screen model with Master-Detail preview.
pub fn quicklinks_model(
    query: &str,
    links: &[Quicklink],
    selected: usize,
    show_detail: bool,
) -> Model {
    let mut rows: Vec<Row> = links
        .iter()
        .map(|link| {
            let mut row = Row::detailed(&link.name, &link.url_template)
                .with_section("Quicklinks")
                .with_icon(Icon::new("↗", Color::rgb(0x38, 0xbd, 0xf8)));
            if link.needs_query() {
                row = row.with_shortcut("Tab Arg");
            }
            row
        })
        .collect();

    if shows_create_quicklink_row(query) {
        rows.push(
            Row::detailed(
                "Create New Quicklink",
                "Use {query} or {argument} in any URL for instant parameterized search",
            )
            .with_section("Actions")
            .with_icon(Icon::new("+", Color::rgb(0x63, 0x66, 0xf1))),
        );
    }

    let count_label = match links.len() {
        0 => "No matching quicklinks".to_string(),
        1 => "1 quicklink".to_string(),
        n => format!("{n} quicklinks"),
    };

    let clamped = selected.min(rows.len().saturating_sub(1));
    let detail = if show_detail {
        links.get(clamped).map(quicklink_detail)
    } else {
        None
    };
    let primary = if clamped < links.len() {
        "Open"
    } else {
        "Create"
    };

    Model {
        query: query.to_string(),
        rows,
        selected: clamped,
        hovered: None,
        footer: count_label.clone(),
        placeholder: "Search quicklinks…".to_string(),
        header_badge: Some("Quicklinks".to_string()),
        result_count: Some(count_label),
        footer_hints: vec![
            ("↵".to_string(), primary.to_string()),
            ("Tab".to_string(), "Fill Arg".to_string()),
            ("Ctrl+K".to_string(), "Actions".to_string()),
        ],
        scroll_offset: 0.0,
        grid: None,
        back_button: true,
        detail,
        primary_action: Some(primary.to_string()),
        ..Model::default()
    }
}

/// Build the Master-Detail pane for a Quicklink.
pub fn quicklink_detail(link: &Quicklink) -> DetailPane {
    let args_desc = if link.needs_query() {
        "Parameterized (press Tab to fill)".to_string()
    } else {
        "Direct URL (no arguments)".to_string()
    };
    let body = if link.needs_query() {
        format!(
            "{}\n\nPress Tab or Enter to type an argument and launch in your default browser.",
            link.url_template
        )
    } else {
        link.url_template.clone()
    };
    let mut metadata = vec![
        ("Arguments".to_string(), args_desc),
        ("URL".to_string(), link.url_template.clone()),
    ];
    if let Some(alias) = link.alias.as_deref() {
        if !alias.trim().is_empty() {
            metadata.insert(0, ("Alias".to_string(), alias.to_string()));
        }
    }
    DetailPane {
        title: link.name.clone(),
        body,
        color_swatch: None,
        image: None,
        image_key: None,
        metadata,
    }
}

/// Build the inline header argument entry prompt for a parameterized `{query}`/`{argument}` quicklink.
pub fn quicklink_arg_model(link: &Quicklink, input: &str) -> Model {
    let preview_url = link.expand_url(input);
    let label = if input.trim().is_empty() {
        format!("Type argument for {}", link.name)
    } else {
        format!("Open {} with \"{}\"", link.name, input.trim())
    };
    Model {
        query: input.to_string(),
        rows: vec![Row::detailed(label, preview_url.clone())
            .with_section("Quicklink")
            .with_shortcut("↵ Open")
            .with_icon(Icon::new("↗", Color::rgb(0x38, 0xbd, 0xf8)))],
        selected: 0,
        hovered: None,
        footer: link.name.clone(),
        placeholder: format!("Argument for {}…", link.name),
        header_badge: None,
        result_count: Some("Press Enter to open in browser".to_string()),
        footer_hints: vec![
            ("↵".to_string(), "Open".to_string()),
            ("Shift+Tab".to_string(), "Back".to_string()),
            ("Esc".to_string(), "Cancel".to_string()),
        ],
        scroll_offset: 0.0,
        grid: None,
        back_button: true,
        inline_arg: Some(InlineArgBanner {
            command_name: link.name.clone(),
            arg_label: "Argument".to_string(),
        }),
        detail: Some(DetailPane {
            title: link.name.clone(),
            body: preview_url.clone(),
            color_swatch: None,
            image: None,
            image_key: None,
            metadata: vec![
                ("Template".to_string(), link.url_template.clone()),
                ("Preview".to_string(), preview_url),
            ],
        }),
        primary_action: Some("Open".to_string()),
        ..Model::default()
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
            "Step 2/2 — URL (include {query} or {argument} for search)…",
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
        back_button: true,
        primary_action: Some("Continue".to_string()),
        ..Model::default()
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
        back_button: true,
        primary_action: Some("Run".to_string()),
        ..Model::default()
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
        back_button: true,
        primary_action: Some("Back".to_string()),
        ..Model::default()
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

fn truncate_chars(text: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (idx, ch) in text.chars().enumerate() {
        if idx >= max_chars {
            out.push('…');
            break;
        }
        out.push(ch);
    }
    out
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
        assert!(model.back_button);
    }

    #[test]
    fn clipboard_model_marks_pinned_color_swatches_and_detail_pane() {
        let mut color_item = ClipboardItem::text("#FF5733", Some("VS Code".to_string()));
        color_item.pinned_at = Some(1_700_000_000_000);
        let model = clipboard_model("", &[color_item], ClipboardFilter::All, 0, true);
        assert_eq!(model.rows.len(), 1);
        assert_eq!(model.rows[0].section, "Pinned");
        assert_eq!(model.rows[0].color_swatch, Some([0xff, 0x57, 0x33, 0xff]));
        let detail = model.detail.expect("detail pane");
        assert_eq!(detail.color_swatch, Some([0xff, 0x57, 0x33, 0xff]));
        assert!(detail
            .metadata
            .iter()
            .any(|(k, v)| k == "Pinned" && v == "Yes"));

        // Toggling detail off clears DetailPane cleanly.
        let hidden = clipboard_model(
            "",
            &[ClipboardItem::text("hello", None)],
            ClipboardFilter::All,
            0,
            false,
        );
        assert!(hidden.detail.is_none());
    }

    #[test]
    fn quicklink_arg_model_sets_inline_arg_banner_and_encoded_preview() {
        let link = Quicklink {
            name: "GitHub Search".to_string(),
            url_template: "https://github.com/search?q={query}".to_string(),
            alias: Some("gh".to_string()),
        };
        let model = quicklink_arg_model(&link, "rust win32");
        let banner = model.inline_arg.expect("inline arg banner");
        assert_eq!(banner.command_name, "GitHub Search");
        assert_eq!(banner.arg_label, "Argument");
        assert_eq!(
            model.rows[0].detail,
            "https://github.com/search?q=rust%20win32"
        );
        assert!(model.back_button);
    }

    #[test]
    fn snippet_detail_extracts_placeholders_and_expanded_body() {
        let body = "Best,\nDabin ({date}) — {clipboard}";
        let record = StoredSnippet::for_content(
            std::path::PathBuf::from("sig.md"),
            tc_snippets::Snippet::with_options(
                "Email Signature",
                body,
                Some("sig".to_string()),
                true,
                false,
            ),
            body,
        );
        let detail = snippet_detail(&record, "Best,\nDabin (2026-11-24) — hello");
        assert_eq!(detail.title, "Email Signature");
        assert!(detail.body.contains("2026-11-24"));
        assert!(detail
            .metadata
            .iter()
            .any(|(k, v)| k == "Keyword" && v == "!sig"));
        assert!(detail.metadata.iter().any(|(k, v)| k == "Placeholders"
            && v.contains("{date}")
            && v.contains("{clipboard}")));
    }

    #[test]
    fn calc_history_model_prepends_live_calc_and_builds_detail() {
        let entry = CalcHistoryEntry {
            expression: "20% off 500".to_string(),
            display: "400".to_string(),
            copy_text: "400".to_string(),
            badge: "Discount".to_string(),
            created_at: 1_700_000_000,
        };
        let model = calc_history_model("", None, &[entry], 0, true);
        assert_eq!(model.rows.len(), 1);
        let detail = model.detail.expect("calc detail");
        assert_eq!(detail.title, "400");
        assert!(detail
            .metadata
            .iter()
            .any(|(k, v)| k == "Expression" && v == "20% off 500"));
    }
}
