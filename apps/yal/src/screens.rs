//! Pure Direct2D screen model builders for every native YAL sub-screen:
//! Clipboard History, 2D Color Emoji Grid, Snippets Browser, Create Snippet
//! Wizard, Quick Notes, Quicklinks & Inline Arguments, File Search, Window
//! Switcher, Calculator History, and Shell Runner.

use std::io::Read;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use tc_clipboard::{
    color_from_rgb, contrast_report, tint_shade_ramp, ClipboardItem, ItemKind, OcrFormatMode,
    PickerColorFormat, TextForm,
};
use tc_palette::FileType;
use tc_platform_win::window_control::WindowInfo;
use tc_snippets::StoredSnippet;
use tc_ui::{
    Color, DetailPane, EditorModel, GridCell, GridModel, Icon, IconBitmap, IconKind,
    InlineArgBanner, Model, Row,
};

use crate::features::{
    CalcHistoryEntry, ClipboardViewFilter, ColorHistoryEntry, NoteItem, Quicklink,
};
use crate::launcher::{ResultRow, CALCULATOR_SECTION};
use crate::tools::OcrPreviewState;

/// Prefix for fallback web-search rows surfaced under `Use “<query>” with…`.
pub const FALLBACK_URL_PREFIX: &str = "fallback-url:";
/// Prefix for built-in fallback commands surfaced under `Use “<query>” with…`.
pub const FALLBACK_CMD_PREFIX: &str = "fallback-cmd:";

/// Maximum file size (bytes) read for inline UTF-8 text preview in File Search.
const MAX_FILE_PREVIEW_BYTES: u64 = 32 * 1024;
/// Maximum number of lines shown in a file preview pane.
const MAX_FILE_PREVIEW_LINES: usize = 40;
/// Maximum number of characters kept in any detail pane preview body.
const MAX_DETAIL_BODY_CHARS: usize = 32_000;

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

/// Parse newline-separated file paths stored in a `ItemKind::File` clipboard entry.
pub fn parse_clipboard_file_paths(item: &ClipboardItem) -> Vec<String> {
    item.text
        .as_deref()
        .unwrap_or("")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect()
}

/// Downsample straight RGBA8 pixels to fit within `(max_w, max_h)` while preserving
/// aspect ratio, returning a premultiplied RGBA8 `IconBitmap` ready for Direct2D.
pub fn downsample_rgba_premultiplied(
    width: u32,
    height: u32,
    rgba: &[u8],
    max_w: u32,
    max_h: u32,
) -> Option<IconBitmap> {
    if width == 0 || height == 0 || max_w == 0 || max_h == 0 {
        return None;
    }
    let expected = (width as usize)
        .checked_mul(height as usize)?
        .checked_mul(4)?;
    if rgba.len() != expected {
        return None;
    }

    let (dst_w, dst_h) = if width <= max_w && height <= max_h {
        (width, height)
    } else {
        let scale_w = f64::from(max_w) / f64::from(width);
        let scale_h = f64::from(max_h) / f64::from(height);
        let scale = scale_w.min(scale_h);
        let w = ((f64::from(width) * scale).round() as u32).clamp(1, max_w);
        let h = ((f64::from(height) * scale).round() as u32).clamp(1, max_h);
        (w, h)
    };

    let dst_len = (dst_w as usize)
        .checked_mul(dst_h as usize)?
        .checked_mul(4)?;
    let mut premultiplied_rgba = Vec::with_capacity(dst_len);
    let src_w = width as u64;
    let src_h = height as u64;
    let dw = dst_w as u64;
    let dh = dst_h as u64;
    let stride = width as usize * 4;

    for dy in 0..dh {
        let y0 = ((dy * src_h) / dh) as usize;
        let y1 = ((((dy + 1) * src_h) / dh) as usize)
            .max(y0 + 1)
            .min(height as usize);
        for dx in 0..dw {
            let x0 = ((dx * src_w) / dw) as usize;
            let x1 = ((((dx + 1) * src_w) / dw) as usize)
                .max(x0 + 1)
                .min(width as usize);
            let mut sum_r: u64 = 0;
            let mut sum_g: u64 = 0;
            let mut sum_b: u64 = 0;
            let mut sum_a: u64 = 0;
            let mut count: u64 = 0;
            for sy in y0..y1 {
                let row_off = sy * stride;
                for sx in x0..x1 {
                    let off = row_off + sx * 4;
                    sum_r += u64::from(rgba[off]);
                    sum_g += u64::from(rgba[off + 1]);
                    sum_b += u64::from(rgba[off + 2]);
                    sum_a += u64::from(rgba[off + 3]);
                    count += 1;
                }
            }
            let count = count.max(1);
            let r = (sum_r + count / 2) / count;
            let g = (sum_g + count / 2) / count;
            let b = (sum_b + count / 2) / count;
            let a = (sum_a + count / 2) / count;
            let pr = ((r * a + 127) / 255) as u8;
            let pg = ((g * a + 127) / 255) as u8;
            let pb = ((b * a + 127) / 255) as u8;
            premultiplied_rgba.push(pr);
            premultiplied_rgba.push(pg);
            premultiplied_rgba.push(pb);
            premultiplied_rgba.push(a as u8);
        }
    }

    Some(IconBitmap {
        width: dst_w,
        height: dst_h,
        premultiplied_rgba,
    })
}

/// Center-crop and downsample straight RGBA8 pixels into a crisp square
/// `side × side` premultiplied RGBA8 thumbnail for list rows (e.g. Clipboard History).
pub fn downsample_square_cover_premultiplied(
    width: u32,
    height: u32,
    rgba: &[u8],
    side: u32,
) -> Option<IconBitmap> {
    if width == 0 || height == 0 || side == 0 {
        return None;
    }
    let expected = (width as usize)
        .checked_mul(height as usize)?
        .checked_mul(4)?;
    if rgba.len() != expected {
        return None;
    }

    let crop = width.min(height);
    let ox = (width - crop) / 2;
    let oy = (height - crop) / 2;
    let dst = side.min(crop).max(1);
    let dst_len = (dst as usize).checked_mul(dst as usize)?.checked_mul(4)?;
    let mut premultiplied_rgba = Vec::with_capacity(dst_len);
    let crop_u = crop as u64;
    let dst_u = dst as u64;
    let stride = width as usize * 4;

    for dy in 0..dst_u {
        let y0 = oy as usize + ((dy * crop_u) / dst_u) as usize;
        let y1 = (oy as usize + (((dy + 1) * crop_u) / dst_u) as usize)
            .max(y0 + 1)
            .min(height as usize);
        for dx in 0..dst_u {
            let x0 = ox as usize + ((dx * crop_u) / dst_u) as usize;
            let x1 = (ox as usize + (((dx + 1) * crop_u) / dst_u) as usize)
                .max(x0 + 1)
                .min(width as usize);
            let mut sum_r: u64 = 0;
            let mut sum_g: u64 = 0;
            let mut sum_b: u64 = 0;
            let mut sum_a: u64 = 0;
            let mut count: u64 = 0;
            for sy in y0..y1 {
                let row_off = sy * stride;
                for sx in x0..x1 {
                    let off = row_off + sx * 4;
                    sum_r += u64::from(rgba[off]);
                    sum_g += u64::from(rgba[off + 1]);
                    sum_b += u64::from(rgba[off + 2]);
                    sum_a += u64::from(rgba[off + 3]);
                    count += 1;
                }
            }
            let count = count.max(1);
            let r = (sum_r + count / 2) / count;
            let g = (sum_g + count / 2) / count;
            let b = (sum_b + count / 2) / count;
            let a = (sum_a + count / 2) / count;
            let pr = ((r * a + 127) / 255) as u8;
            let pg = ((g * a + 127) / 255) as u8;
            let pb = ((b * a + 127) / 255) as u8;
            premultiplied_rgba.push(pr);
            premultiplied_rgba.push(pg);
            premultiplied_rgba.push(pb);
            premultiplied_rgba.push(a as u8);
        }
    }

    Some(IconBitmap {
        width: dst,
        height: dst,
        premultiplied_rgba,
    })
}

/// Format a recorded clipboard source (either an `.exe` path or an app label)
/// into a concise human-friendly app name for subtitles and metadata.
fn friendly_source_name(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let looks_like_path = trimmed.contains('\\')
        || trimmed.contains('/')
        || trimmed.to_ascii_lowercase().ends_with(".exe");
    if !looks_like_path {
        return truncate_chars(trimmed, 28);
    }
    let stem = Path::new(trimmed)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(trimmed);
    match stem.to_ascii_lowercase().as_str() {
        "chrome" => "Chrome".to_string(),
        "msedge" => "Edge".to_string(),
        "firefox" => "Firefox".to_string(),
        "code" => "VS Code".to_string(),
        "windowsterminal" => "Terminal".to_string(),
        "powershell" | "pwsh" => "PowerShell".to_string(),
        "cmd" => "Command Prompt".to_string(),
        "explorer" => "File Explorer".to_string(),
        "notepad" => "Notepad".to_string(),
        "opencode" => "OpenCode".to_string(),
        "discord" => "Discord".to_string(),
        "slack" => "Slack".to_string(),
        "whatsapp" => "WhatsApp".to_string(),
        "telegram" => "Telegram".to_string(),
        other => {
            let mut chars = other.chars();
            match chars.next() {
                Some(first) => {
                    let cap = format!("{}{}", first.to_uppercase(), &stem[first.len_utf8()..]);
                    truncate_chars(&cap, 28)
                }
                None => truncate_chars(stem, 28),
            }
        }
    }
}

/// Heuristic for code-like copied text so it gets a crisp `< >` code icon.
fn looks_like_code(text: &str) -> bool {
    let t = text.trim();
    if t.len() < 4 {
        return false;
    }
    (t.starts_with('{') && t.ends_with('}'))
        || (t.starts_with('[') && t.ends_with(']'))
        || t.starts_with("fn ")
        || t.starts_with("pub fn ")
        || t.starts_with("function ")
        || t.starts_with("const ")
        || t.starts_with("import ")
        || t.starts_with("SELECT ")
        || t.starts_with("git ")
        || t.starts_with("cargo ")
        || t.starts_with("npm ")
        || t.starts_with("```")
}

/// Build the native Clipboard History screen model with Master-Detail preview,
/// live image row thumbnails, real file/source-app icons, and vector badges.
#[allow(clippy::too_many_arguments)]
pub fn clipboard_model(
    query: &str,
    items: &[ClipboardItem],
    filter: ClipboardViewFilter,
    queue_len: usize,
    selected: usize,
    show_detail: bool,
    mut image_lookup: impl FnMut(
        &ClipboardItem,
        bool,
    ) -> Option<(u32, u32, Arc<IconBitmap>, Arc<IconBitmap>)>,
    mut shell_icon_lookup: impl FnMut(&str) -> Option<Arc<IconBitmap>>,
) -> Model {
    let clamped_sel = selected.min(items.len().saturating_sub(1));
    let mut selected_image_info: Option<(u32, u32, Arc<IconBitmap>)> = None;

    let rows: Vec<Row> = items
        .iter()
        .enumerate()
        .map(|(idx, item)| {
            let section = if item.is_pinned() {
                "Pinned"
            } else {
                "History"
            };
            let is_selected = idx == clamped_sel;
            let mut detail_parts = Vec::new();
            let mut row_bitmap: Option<(String, Arc<IconBitmap>)> = None;

            let title = match item.kind {
                ItemKind::Image => {
                    if let Some(src) = item.source_bundle_id.as_deref() {
                        let friendly = friendly_source_name(src);
                        if !friendly.is_empty() {
                            detail_parts.push(friendly);
                        }
                    }
                    detail_parts.push(relative_time_ms(item.created_at));
                    // Decode thumbnails for the first 32 list items plus the active selection.
                    let load_thumb = is_selected || idx < 32;
                    if let Some((w, h, detail_bmp, row_thumb)) = image_lookup(item, load_thumb) {
                        if is_selected {
                            selected_image_info = Some((w, h, detail_bmp));
                        }
                        row_bitmap = Some((format!("clip-row:{}", item.id), row_thumb));
                        format!("Image · {w}×{h}")
                    } else {
                        "Captured Image".to_string()
                    }
                }
                ItemKind::File => {
                    let paths = parse_clipboard_file_paths(item);
                    if let Some(first) = paths.first() {
                        if idx < 32 {
                            if let Some(bmp) = shell_icon_lookup(first) {
                                row_bitmap = Some((first.clone(), bmp));
                            }
                        }
                    }
                    match paths.len() {
                        0 => {
                            if let Some(src) = item.source_bundle_id.as_deref() {
                                let friendly = friendly_source_name(src);
                                if !friendly.is_empty() {
                                    detail_parts.push(friendly);
                                }
                            }
                            detail_parts.push(relative_time_ms(item.created_at));
                            "File".to_string()
                        }
                        1 => {
                            let path = &paths[0];
                            if let Some(dir) = shorten_parent_dir(path) {
                                detail_parts.push(dir);
                            }
                            detail_parts.push(relative_time_ms(item.created_at));
                            file_basename(path).to_string()
                        }
                        n => {
                            let first = file_basename(&paths[0]);
                            detail_parts.push(truncate_chars(first, 36));
                            detail_parts.push(relative_time_ms(item.created_at));
                            format!("{n} files")
                        }
                    }
                }
                ItemKind::Text => {
                    if let Some(src) = item.source_bundle_id.as_deref() {
                        let friendly = friendly_source_name(src);
                        if !friendly.is_empty() {
                            detail_parts.push(friendly);
                        }
                        let is_special = matches!(
                            item.text_form(),
                            Some(TextForm::Color | TextForm::Link | TextForm::Email)
                        );
                        if !is_special
                            && idx < 32
                            && (src.contains('\\') || src.to_ascii_lowercase().ends_with(".exe"))
                        {
                            if let Some(bmp) = shell_icon_lookup(src) {
                                row_bitmap = Some((src.to_string(), bmp));
                            }
                        }
                    }
                    detail_parts.push(relative_time_ms(item.created_at));
                    clipboard_preview_title(item)
                }
            };
            let detail = detail_parts.join(" · ");

            let (icon, swatch) = match item.kind {
                ItemKind::Image => (
                    Icon::vector(IconKind::Image, Color::rgb(0xec, 0x48, 0x99)),
                    None,
                ),
                ItemKind::File => (
                    Icon::vector(IconKind::File, Color::rgb(0x38, 0xbd, 0xf8)),
                    None,
                ),
                ItemKind::Text => match item.text_form() {
                    Some(TextForm::Color) => {
                        let sw = item.color_value().map(color_value_to_rgba);
                        (
                            Icon::vector(IconKind::Text, Color::rgb(0xa8, 0x55, 0xf7)),
                            sw,
                        )
                    }
                    Some(TextForm::Link) => (
                        Icon::vector(IconKind::Link, Color::rgb(0x38, 0xbd, 0xf8)),
                        None,
                    ),
                    Some(TextForm::Email) => (
                        Icon::vector(IconKind::Email, Color::rgb(0x34, 0xd3, 0x99)),
                        None,
                    ),
                    _ => {
                        let kind = if item.text.as_deref().is_some_and(looks_like_code) {
                            IconKind::Code
                        } else {
                            IconKind::Text
                        };
                        (Icon::vector(kind, Color::rgb(0x81, 0x8c, 0xf8)), None)
                    }
                },
            };

            let mut row = Row::detailed(title, detail)
                .with_section(section)
                .with_icon(icon);
            if let Some((key, bmp)) = row_bitmap {
                row = row.with_bitmap(key, bmp);
            }
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

    let mut badge = if filter == ClipboardViewFilter::All {
        "Clipboard".to_string()
    } else {
        format!("Clipboard · {}", filter.label())
    };
    if queue_len > 0 {
        badge.push_str(&format!(" · Queue ({queue_len})"));
    }

    let footer = if queue_len > 0 {
        format!("{count_label} · Queue: {queue_len}")
    } else {
        count_label.clone()
    };

    let detail = if show_detail {
        items
            .get(clamped_sel)
            .map(|item| clipboard_detail_with_image(item, selected_image_info))
    } else {
        None
    };

    Model {
        query: query.to_string(),
        rows,
        selected: clamped_sel,
        hovered: None,
        footer,
        placeholder: "Search clipboard history…".to_string(),
        header_badge: Some(badge),
        result_count: Some(count_label),
        footer_hints: vec![
            ("↵".to_string(), "Paste".to_string()),
            ("Ctrl+E".to_string(), "Edit".to_string()),
            ("Ctrl+D".to_string(), "Delete".to_string()),
            ("Tab".to_string(), "Filter".to_string()),
            ("Ctrl+K".to_string(), "Actions".to_string()),
        ],
        scroll_offset: 0.0,
        grid: None,
        back_button: true,
        detail,
        primary_action: Some("Paste".to_string()),
        click_selects_only: true,
        ..Model::default()
    }
}

/// Build the Master-Detail pane for a single Clipboard History entry with optional image preview.
pub fn clipboard_detail_with_image(
    item: &ClipboardItem,
    image_info: Option<(u32, u32, Arc<IconBitmap>)>,
) -> DetailPane {
    let pinned_str = if item.is_pinned() { "Yes" } else { "No" };
    let captured_str = relative_time_ms(item.created_at);
    let push_source = |metadata: &mut Vec<(String, String)>| {
        if let Some(src) = item.source_bundle_id.as_deref() {
            let friendly = friendly_source_name(src);
            if !friendly.is_empty() {
                metadata.push(("Source".to_string(), friendly));
            }
        }
    };

    match item.kind {
        ItemKind::Image => {
            if let Some((w, h, bmp)) = image_info {
                let mut metadata = vec![
                    ("Type".to_string(), "Image".to_string()),
                    ("Dimensions".to_string(), format!("{w} × {h}")),
                    ("Pinned".to_string(), pinned_str.to_string()),
                    ("Captured".to_string(), captured_str),
                ];
                push_source(&mut metadata);
                DetailPane {
                    title: format!("Image · {w}×{h}"),
                    body: format!("{w} × {h} px"),
                    color_swatch: None,
                    color_ramp: Vec::new(),
                    image: Some(bmp),
                    image_key: Some(format!("clip-img-{}", item.id)),
                    metadata,
                }
            } else {
                let mut metadata = vec![
                    ("Type".to_string(), "Image".to_string()),
                    ("Pinned".to_string(), pinned_str.to_string()),
                    ("Captured".to_string(), captured_str),
                ];
                push_source(&mut metadata);
                DetailPane {
                    title: "Image".to_string(),
                    body: "Captured clipboard image".to_string(),
                    color_swatch: None,
                    color_ramp: Vec::new(),
                    image: None,
                    image_key: None,
                    metadata,
                }
            }
        }
        ItemKind::File => {
            let paths = parse_clipboard_file_paths(item);
            let title = match paths.len() {
                0 => "File".to_string(),
                1 => file_basename(&paths[0]).to_string(),
                n => format!("{n} Files"),
            };
            let body = if paths.is_empty() {
                "Copied file reference".to_string()
            } else {
                truncate_chars(&paths.join("\n"), MAX_DETAIL_BODY_CHARS)
            };
            let mut metadata = vec![
                ("Type".to_string(), "Files".to_string()),
                ("Count".to_string(), paths.len().to_string()),
                ("Pinned".to_string(), pinned_str.to_string()),
                ("Captured".to_string(), captured_str),
            ];
            push_source(&mut metadata);
            DetailPane {
                title,
                body,
                color_swatch: None,
                color_ramp: Vec::new(),
                image: None,
                image_key: None,
                metadata,
            }
        }
        ItemKind::Text => {
            let (type_label, swatch) = match item.text_form() {
                Some(TextForm::Color) => ("Color", item.color_value().map(color_value_to_rgba)),
                Some(TextForm::Link) => ("Link", None),
                Some(TextForm::Email) => ("Email", None),
                _ => ("Text", None),
            };
            let color_ramp = item
                .color_value()
                .map(|cv| {
                    tint_shade_ramp(cv)
                        .into_iter()
                        .map(|s| (s.rgba, s.label.to_string()))
                        .collect()
                })
                .unwrap_or_default();

            let raw_body = item.text.as_deref().unwrap_or("");
            let body = if raw_body.is_empty() {
                "(Empty)".to_string()
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
                ("Pinned".to_string(), pinned_str.to_string()),
                ("Captured".to_string(), captured_str),
            ];
            push_source(&mut metadata);

            DetailPane {
                title,
                body,
                color_swatch: swatch,
                color_ramp,
                image: None,
                image_key: None,
                metadata,
            }
        }
    }
}

/// Build the native multi-line Edit Clipboard Entry screen model.
pub fn edit_clipboard_model(item: &ClipboardItem, initial_text: &str) -> Model {
    let char_count = initial_text.chars().count();
    let line_count = if initial_text.is_empty() {
        0
    } else {
        initial_text.lines().count().max(1)
    };
    let badge = if item.is_pinned() {
        "Edit Pinned Clip".to_string()
    } else {
        "Edit Clipboard Entry".to_string()
    };
    Model {
        header_badge: Some(badge),
        back_button: true,
        footer: format!("{char_count} chars · {line_count} lines"),
        footer_hints: vec![
            ("Ctrl+↵".to_string(), "Update Entry".to_string()),
            ("Ctrl+Shift+↵".to_string(), "Save as New Copy".to_string()),
            ("Esc".to_string(), "Cancel".to_string()),
        ],
        editor: Some(EditorModel {
            title: "Edit Clipboard Entry".to_string(),
            text: initial_text.to_string(),
            primary_label: "Update Entry".to_string(),
            secondary_label: "Save as New Copy".to_string(),
        }),
        ..Model::default()
    }
}

fn file_basename(path: &str) -> &str {
    path.rsplit(['\\', '/'])
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("File")
}

fn shorten_parent_dir(path: &str) -> Option<String> {
    let parent = Path::new(path).parent()?.to_str()?.trim();
    if parent.is_empty() {
        return None;
    }
    let count = parent.chars().count();
    if count <= 36 {
        Some(parent.to_string())
    } else {
        let tail: String = parent.chars().skip(count.saturating_sub(35)).collect();
        Some(format!("…{tail}"))
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
        color_ramp: Vec::new(),
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
#[allow(clippy::too_many_arguments)]
pub fn files_model(
    query: &str,
    results: &[ResultRow],
    filter: FileType,
    current_dir: Option<&Path>,
    indexing: bool,
    selected: usize,
    show_detail: bool,
    mut icon_lookup: impl FnMut(&str) -> Option<Arc<IconBitmap>>,
) -> Model {
    let section = if current_dir.is_some() {
        "Folder Contents"
    } else if query.trim().is_empty() {
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
            let display_detail = shorten_home_path(&r.detail);
            let mut row = Row::detailed(&r.name, &display_detail)
                .with_section(section)
                .with_accessory(if is_dir { "Folder" } else { "File" })
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

    let clamped = selected.min(results.len().saturating_sub(1));
    let detail = if show_detail {
        results.get(clamped).map(file_detail)
    } else {
        None
    };

    let (badge, placeholder, footer_hints) = if let Some(dir) = current_dir {
        let folder_name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| dir.to_str().unwrap_or("Folder"));
        let parent_name = dir
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .filter(|s| !s.is_empty());
        let raw_breadcrumb = match parent_name {
            Some(parent) => format!("{parent} / {folder_name}"),
            None => folder_name.to_string(),
        };
        let breadcrumb = truncate_chars(&raw_breadcrumb, 36);
        let badge = if filter == FileType::All {
            format!("📁 {breadcrumb}")
        } else {
            format!("📁 {breadcrumb} · {}", filter.label())
        };
        let placeholder = format!("Filter inside {folder_name}…");
        let hints = vec![
            ("↵".to_string(), "Open".to_string()),
            ("Tab".to_string(), "Open Folder".to_string()),
            ("Shift+Tab".to_string(), "Up".to_string()),
            ("Ctrl+K".to_string(), "Actions".to_string()),
        ];
        (badge, placeholder, hints)
    } else {
        let badge = if filter == FileType::All {
            "Files".to_string()
        } else {
            format!("Files · {}", filter.label())
        };
        let selected_is_dir = results.get(clamped).and_then(|r| r.is_dir) == Some(true);
        let mut hints = vec![("↵".to_string(), "Open".to_string())];
        if selected_is_dir {
            hints.push(("Tab".to_string(), "Browse Folder".to_string()));
        }
        hints.push(("Ctrl+K".to_string(), "Actions".to_string()));
        hints.push(("Ctrl+I".to_string(), "Preview".to_string()));
        (badge, "Search files & folders…".to_string(), hints)
    };

    Model {
        query: query.to_string(),
        rows,
        selected: clamped,
        hovered: None,
        footer: status.clone(),
        placeholder,
        header_badge: Some(badge),
        result_count: Some(status),
        footer_hints,
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
        color_ramp: Vec::new(),
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
                    color_ramp: Vec::new(),
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
        color_ramp: Vec::new(),
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
        color_ramp: Vec::new(),
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
        color_ramp: Vec::new(),
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
            color_ramp: Vec::new(),
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

/// Build the inline Set Alias entry prompt for a launcher item or snippet.
pub fn set_alias_model(target_title: &str, current_alias: Option<&str>, input: &str) -> Model {
    let trimmed = input.trim();
    let current_clean = current_alias.map(str::trim).filter(|s| !s.is_empty());
    let headline = if trimmed.is_empty() {
        match current_clean {
            Some(existing) => format!("Clear alias \"{existing}\" for {target_title}"),
            None => format!("Type a short alias for {target_title}"),
        }
    } else {
        format!("Save alias \"{trimmed}\" for {target_title}")
    };
    let detail = match current_clean {
        Some(existing) => {
            format!("Current alias: {existing} — leave empty and press Enter to clear")
        }
        None => "Type a short trigger word (e.g. gh, vs, notes) and press Enter".to_string(),
    };

    Model {
        query: input.to_string(),
        rows: vec![Row::detailed(headline, detail)
            .with_section("Alias")
            .with_shortcut("↵ Save")
            .with_icon(Icon::new("⌘", Color::rgb(0x81, 0x8c, 0xf8)))],
        selected: 0,
        hovered: None,
        footer: format!("Alias · {target_title}"),
        placeholder: "Type short alias (empty to clear)…".to_string(),
        header_badge: Some("Set Alias".to_string()),
        result_count: Some("Press Enter to save".to_string()),
        footer_hints: vec![
            ("↵".to_string(), "Save Alias".to_string()),
            ("Esc".to_string(), "Cancel".to_string()),
        ],
        scroll_offset: 0.0,
        grid: None,
        back_button: true,
        inline_arg: Some(InlineArgBanner {
            command_name: format!("Set Alias: {target_title}"),
            arg_label: "Alias".to_string(),
        }),
        primary_action: Some("Save Alias".to_string()),
        ..Model::default()
    }
}

/// Build the Color Inspector / Color Picker History screen model with Master-Detail
/// WCAG contrast report and 9-step tint/shade color ramp.
pub fn color_history_model(
    query: &str,
    entries: &[ColorHistoryEntry],
    selected: usize,
    show_detail: bool,
    default_format: PickerColorFormat,
) -> Model {
    let rows: Vec<Row> = if entries.is_empty() {
        if query.trim().is_empty() {
            vec![Row::detailed(
                "Pick a Color from Screen",
                "No picked colors yet — press Ctrl+Alt+X or Enter to pick a color",
            )
            .with_section("Color Inspector")
            .with_icon(Icon::new("⌖", Color::rgb(0x5e, 0x8b, 0xff)))
            .with_shortcut("Ctrl+Alt+X")]
        } else {
            Vec::new()
        }
    } else {
        entries
            .iter()
            .map(|e| {
                let cv = color_from_rgb(e.r, e.g, e.b);
                let rgb_str = PickerColorFormat::Rgb.format(cv);
                let hsl_str = PickerColorFormat::Hsl.format(cv);
                let title = if e.pinned {
                    format!("★ {}", e.hex)
                } else {
                    e.hex.clone()
                };
                let detail = format!("{rgb_str} · {hsl_str}");
                let section = if e.pinned {
                    "Pinned Colors"
                } else {
                    "Recent Colors"
                };
                let shortcut = if e.pinned {
                    "Pinned".to_string()
                } else {
                    default_format.label().to_string()
                };
                Row::detailed(title, detail)
                    .with_section(section)
                    .with_color_swatch([e.r, e.g, e.b, 255])
                    .with_shortcut(shortcut)
            })
            .collect()
    };

    let clamped = selected.min(rows.len().saturating_sub(1));
    let detail = if show_detail {
        entries.get(clamped).map(|entry| {
            let cv = color_from_rgb(entry.r, entry.g, entry.b);
            let report = contrast_report(cv);
            let ramp = tint_shade_ramp(cv)
                .into_iter()
                .map(|s| (s.rgba, s.label.to_string()))
                .collect();
            DetailPane {
                title: entry.hex.clone(),
                body: format!(
                    "Default copy ({}): {}",
                    default_format.label(),
                    default_format.format(cv)
                ),
                color_swatch: Some([entry.r, entry.g, entry.b, 255]),
                color_ramp: ramp,
                image: None,
                image_key: None,
                metadata: vec![
                    ("HEX".to_string(), PickerColorFormat::Hex.format(cv)),
                    ("RGB".to_string(), PickerColorFormat::Rgb.format(cv)),
                    ("HSL".to_string(), PickerColorFormat::Hsl.format(cv)),
                    ("OKLCH".to_string(), PickerColorFormat::Oklch.format(cv)),
                    (
                        "On White".to_string(),
                        format!(
                            "{:.2}:1 · {}",
                            report.vs_white,
                            report.vs_white_level.badge()
                        ),
                    ),
                    (
                        "On Black".to_string(),
                        format!(
                            "{:.2}:1 · {}",
                            report.vs_black,
                            report.vs_black_level.badge()
                        ),
                    ),
                ],
            }
        })
    } else {
        None
    };

    let count_label = if entries.is_empty() {
        if query.trim().is_empty() {
            "No picked colors yet".to_string()
        } else {
            "No matching colors".to_string()
        }
    } else if entries.len() == 1 {
        "1 color".to_string()
    } else {
        format!("{} colors", entries.len())
    };

    let primary_action = if entries.is_empty() {
        "Pick Color".to_string()
    } else {
        format!("Copy {}", default_format.label())
    };

    Model {
        query: query.to_string(),
        rows,
        selected: clamped,
        hovered: None,
        footer: count_label.clone(),
        placeholder: "Search picked colors (hex, rgb, hsl, oklch)…".to_string(),
        header_badge: Some(format!("Color Inspector · {}", default_format.label())),
        result_count: Some(count_label),
        footer_hints: vec![
            ("↵".to_string(), "Copy".to_string()),
            ("Tab".to_string(), "Format".to_string()),
            ("Ctrl+P".to_string(), "Pin".to_string()),
            ("Ctrl+D".to_string(), "Delete".to_string()),
            ("Ctrl+K".to_string(), "Actions".to_string()),
        ],
        scroll_offset: 0.0,
        grid: None,
        back_button: true,
        detail,
        primary_action: Some(primary_action),
        ..Model::default()
    }
}

/// Build the Screen OCR Preview screen model with Smart/Raw toggle and text metadata.
pub fn ocr_preview_model(state: &OcrPreviewState, selected: usize) -> Model {
    let rows = vec![
        Row::detailed(
            "Copy Extracted Text",
            format!(
                "{} chars · {} lines ({})",
                state.char_count,
                state.line_count,
                state.mode.label()
            ),
        )
        .with_section("OCR Preview")
        .with_icon(Icon::new("⧉", Color::rgb(0x34, 0xc7, 0x59)))
        .with_shortcut("↵"),
        Row::detailed(
            format!(
                "Toggle Mode: {} → {}",
                state.mode.label(),
                state.mode.toggle().label()
            ),
            "Compare Smart (NormCap) reflow vs Raw line breaks",
        )
        .with_section("OCR Preview")
        .with_icon(Icon::new("⇄", Color::rgb(0x5e, 0x8b, 0xff)))
        .with_shortcut("Tab"),
        Row::detailed(
            "Capture Again (OCR)",
            "Hide YAL and select another screen region",
        )
        .with_section("OCR Preview")
        .with_icon(Icon::new("⌖", Color::rgb(0xff, 0x9f, 0x0a)))
        .with_shortcut("Ctrl+Alt+T"),
    ];

    let clamped = selected.min(rows.len().saturating_sub(1));
    let primary_action = match clamped {
        0 => "Copy Text",
        1 => "Toggle Mode",
        _ => "Capture",
    };
    let mode_meta = if state.mode == OcrFormatMode::Smart {
        "Smart (NormCap)"
    } else {
        "Raw"
    };
    let summary = format!("{} chars · {} lines", state.char_count, state.line_count);

    Model {
        query: String::new(),
        rows,
        selected: clamped,
        hovered: None,
        footer: summary.clone(),
        placeholder: "Review extracted text · Enter to Copy · Tab to toggle Smart/Raw".to_string(),
        header_badge: Some(format!("OCR Preview · {}", state.mode.label())),
        result_count: Some(summary),
        footer_hints: vec![
            ("↵".to_string(), "Run".to_string()),
            ("Tab".to_string(), "Smart/Raw".to_string()),
            ("Ctrl+K".to_string(), "Actions".to_string()),
            ("Esc".to_string(), "Back".to_string()),
        ],
        scroll_offset: 0.0,
        grid: None,
        back_button: true,
        detail: Some(DetailPane {
            title: format!("Extracted Text · {}", state.mode.label()),
            body: truncate_chars(&state.text, MAX_DETAIL_BODY_CHARS),
            color_swatch: None,
            color_ramp: Vec::new(),
            image: None,
            image_key: None,
            metadata: vec![
                ("Mode".to_string(), mode_meta.to_string()),
                ("Language".to_string(), state.language_tag.clone()),
                ("Characters".to_string(), state.char_count.to_string()),
                ("Lines".to_string(), state.line_count.to_string()),
            ],
        }),
        primary_action: Some(primary_action.to_string()),
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

/// Shorten a Windows path under the current user's profile folder with `~`
/// (e.g. `C:\Users\dabin\development\YAL` -> `~\development\YAL`), matching
/// Raycast's compact path display.
pub fn shorten_home_path(path: &str) -> String {
    static HOME_PREFIX: OnceLock<Option<String>> = OnceLock::new();
    let home = HOME_PREFIX.get_or_init(|| {
        std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(|v| {
                v.to_string_lossy()
                    .trim_end_matches(['\\', '/'])
                    .to_string()
            })
            .filter(|s| !s.is_empty())
    });
    shorten_path_with_home(path, home.as_deref())
}

fn shorten_path_with_home(path: &str, home: Option<&str>) -> String {
    let Some(home) = home else {
        return path.to_string();
    };
    if path.len() >= home.len()
        && path.is_char_boundary(home.len())
        && path[..home.len()].eq_ignore_ascii_case(home)
    {
        let rest = &path[home.len()..];
        if rest.is_empty() {
            return "~".to_string();
        }
        if rest.starts_with('\\') || rest.starts_with('/') {
            return format!("~{rest}");
        }
    }
    path.to_string()
}

/// Fallback command & web-search rows rendered in Raycast's `Use “<query>” with…`
/// section at the bottom of Root Search whenever `query` has 2+ characters.
pub fn fallback_search_rows(query: &str, quicklinks: &[Quicklink]) -> Vec<ResultRow> {
    let trimmed = query.trim();
    if trimmed.chars().count() < 2 {
        return Vec::new();
    }
    let section = format!("Use \u{201C}{trimmed}\u{201D} with\u{2026}");
    let mut rows = vec![
        ResultRow {
            id: format!("{FALLBACK_CMD_PREFIX}search-files"),
            name: "Search Files".to_string(),
            detail: "File Search".to_string(),
            kind: section.clone(),
            target: trimmed.to_string(),
            ..ResultRow::default()
        },
        ResultRow {
            id: format!("{FALLBACK_CMD_PREFIX}search-snippets"),
            name: "Search Snippets".to_string(),
            detail: "Snippets".to_string(),
            kind: section.clone(),
            target: trimmed.to_string(),
            ..ResultRow::default()
        },
    ];
    for link in quicklinks.iter().filter(|l| l.needs_query()).take(4) {
        let url = link.expand_url(trimmed);
        rows.push(ResultRow {
            id: format!("{FALLBACK_URL_PREFIX}{url}"),
            name: link.name.clone(),
            detail: "Web Searches".to_string(),
            kind: section.clone(),
            target: url,
            ..ResultRow::default()
        });
    }
    rows
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
        let model = clipboard_model(
            "",
            &[color_item],
            ClipboardViewFilter::All,
            2,
            0,
            true,
            |_, _| None,
            |_| None,
        );
        assert_eq!(model.rows.len(), 1);
        assert_eq!(model.rows[0].section, "Pinned");
        assert_eq!(model.rows[0].color_swatch, Some([0xff, 0x57, 0x33, 0xff]));
        assert_eq!(model.header_badge.as_deref(), Some("Clipboard · Queue (2)"));
        assert!(model.footer.contains("Queue: 2"));
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
            ClipboardViewFilter::All,
            0,
            0,
            false,
            |_, _| None,
            |_| None,
        );
        assert!(hidden.detail.is_none());
    }

    #[test]
    fn downsample_rgba_premultiplied_handles_aspect_premul_invalid_and_passthrough() {
        // Invalid dimensions or byte lengths return None.
        assert!(downsample_rgba_premultiplied(0, 4, &[], 10, 10).is_none());
        assert!(downsample_rgba_premultiplied(4, 0, &[], 10, 10).is_none());
        assert!(downsample_rgba_premultiplied(2, 2, &[0u8; 4], 10, 10).is_none());
        assert!(downsample_rgba_premultiplied(2, 2, &[0u8; 16], 0, 10).is_none());

        // Small image passthrough preserves size and premultiplies RGB by alpha.
        // Straight pixel (200, 100, 50, 128) -> pr = (200*128+127)/255 = 100, pg = 50, pb = 25, a = 128.
        let small = vec![200, 100, 50, 128, 255, 0, 0, 255];
        let bmp = downsample_rgba_premultiplied(2, 1, &small, 16, 16).expect("passthrough");
        assert_eq!(bmp.width, 2);
        assert_eq!(bmp.height, 1);
        assert_eq!(
            bmp.premultiplied_rgba,
            vec![100, 50, 25, 128, 255, 0, 0, 255]
        );

        // Large 8x4 image constrained to 4x4 preserves 2:1 aspect ratio -> 4x2.
        let mut wide = Vec::with_capacity(8 * 4 * 4);
        for _ in 0..(8 * 4) {
            wide.extend_from_slice(&[200, 100, 50, 255]);
        }
        let scaled = downsample_rgba_premultiplied(8, 4, &wide, 4, 4).expect("scaled");
        assert_eq!(scaled.width, 4);
        assert_eq!(scaled.height, 2);
        assert_eq!(scaled.premultiplied_rgba.len(), 4 * 2 * 4);
        assert_eq!(&scaled.premultiplied_rgba[0..4], &[200, 100, 50, 255]);
    }

    #[test]
    fn clipboard_detail_with_image_and_files_populates_fields() {
        let img_item = ClipboardItem::image("C:\\clip\\shot.png", Some("SnippingTool".to_string()));
        let bmp = Arc::new(IconBitmap {
            width: 2,
            height: 2,
            premultiplied_rgba: vec![255; 16],
        });
        let img_detail =
            clipboard_detail_with_image(&img_item, Some((1920, 1080, Arc::clone(&bmp))));
        assert_eq!(img_detail.title, "Image · 1920×1080");
        assert_eq!(img_detail.body, "1920 × 1080 px");
        assert_eq!(img_detail.image, Some(bmp));
        assert_eq!(
            img_detail.image_key,
            Some(format!("clip-img-{}", img_item.id))
        );
        assert!(img_detail
            .metadata
            .iter()
            .any(|(k, v)| k == "Dimensions" && v == "1920 × 1080"));
        assert!(img_detail
            .metadata
            .iter()
            .any(|(k, v)| k == "Type" && v == "Image"));

        let files_item =
            ClipboardItem::file("C:\\Users\\dabin\\a.txt\nC:\\Users\\dabin\\b.pdf\n", None);
        let files_detail = clipboard_detail_with_image(&files_item, None);
        assert_eq!(files_detail.title, "2 Files");
        assert!(files_detail.body.contains("a.txt"));
        assert!(files_detail.body.contains("b.pdf"));
        assert!(files_detail
            .metadata
            .iter()
            .any(|(k, v)| k == "Count" && v == "2"));
        assert!(files_detail
            .metadata
            .iter()
            .any(|(k, v)| k == "Type" && v == "Files"));
    }

    #[test]
    fn files_model_with_current_dir_sets_breadcrumb_and_placeholder() {
        let dir = Path::new("C:\\Users\\dabin\\Projects");
        let row = ResultRow {
            name: "main.rs".to_string(),
            detail: "C:\\Users\\dabin\\Projects\\main.rs".to_string(),
            target: "C:\\Users\\dabin\\Projects\\main.rs".to_string(),
            is_dir: Some(false),
            ..ResultRow::default()
        };
        let model = files_model(
            "",
            &[row],
            FileType::All,
            Some(dir),
            false,
            0,
            false,
            |_| None,
        );
        assert_eq!(model.header_badge.as_deref(), Some("📁 dabin / Projects"));
        assert_eq!(model.placeholder, "Filter inside Projects…");
        assert_eq!(model.rows[0].section, "Folder Contents");
        assert!(model
            .footer_hints
            .iter()
            .any(|(k, v)| k == "Shift+Tab" && v == "Up"));
    }

    #[test]
    fn set_alias_model_sets_inline_arg_banner_and_existing_alias_hint() {
        let model = set_alias_model("Visual Studio Code", Some("vsc"), "code");
        let banner = model.inline_arg.expect("inline arg banner");
        assert_eq!(banner.command_name, "Set Alias: Visual Studio Code");
        assert_eq!(banner.arg_label, "Alias");
        assert_eq!(model.header_badge.as_deref(), Some("Set Alias"));
        assert_eq!(model.placeholder, "Type short alias (empty to clear)…");
        assert_eq!(model.primary_action.as_deref(), Some("Save Alias"));
        assert!(model.back_button);
        assert!(model.rows[0].name.contains("code"));
        assert!(model.rows[0].detail.contains("vsc"));
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

    #[test]
    fn fallback_search_rows_uses_raycast_section_header_and_commands() {
        let links = vec![Quicklink {
            name: "Google Search".to_string(),
            url_template: "https://www.google.com/search?q={query}".to_string(),
            alias: Some("g".to_string()),
        }];
        let rows = fallback_search_rows("hours till 27 nov", &links);
        assert_eq!(rows.len(), 3);
        assert_eq!(
            rows[0].kind,
            "Use \u{201C}hours till 27 nov\u{201D} with\u{2026}"
        );
        assert_eq!(rows[0].name, "Search Files");
        assert_eq!(rows[1].name, "Search Snippets");
        assert_eq!(rows[2].name, "Google Search");
        assert_eq!(rows[2].detail, "Web Searches");
    }

    #[test]
    fn shorten_path_with_home_replaces_user_profile_prefix() {
        assert_eq!(
            shorten_path_with_home(
                r"C:\Users\dabin\development\opencode-stack\AIStudio2API",
                Some(r"C:\Users\dabin")
            ),
            r"~\development\opencode-stack\AIStudio2API"
        );
        assert_eq!(
            shorten_path_with_home(r"C:\Users\dabin", Some(r"C:\Users\dabin")),
            "~"
        );
        assert_eq!(
            shorten_path_with_home(r"C:\Users\dabin2\work", Some(r"C:\Users\dabin")),
            r"C:\Users\dabin2\work"
        );
    }

    #[test]
    fn color_history_model_builds_swatch_ramp_metadata_and_empty_states() {
        let empty = color_history_model("", &[], 0, true, PickerColorFormat::Hex);
        assert_eq!(empty.rows.len(), 1);
        assert_eq!(empty.rows[0].name, "Pick a Color from Screen");
        assert_eq!(empty.primary_action.as_deref(), Some("Pick Color"));
        assert!(empty.detail.is_none());

        let no_match = color_history_model("zzz", &[], 0, true, PickerColorFormat::Hex);
        assert!(no_match.rows.is_empty());
        assert_eq!(no_match.footer, "No matching colors");

        let entries = vec![ColorHistoryEntry {
            hex: "#3B82F6".to_string(),
            r: 0x3b,
            g: 0x82,
            b: 0xf6,
            pinned: true,
            picked_at: 1_700_000_000,
        }];
        let model = color_history_model("", &entries, 0, true, PickerColorFormat::Oklch);
        assert_eq!(model.rows.len(), 1);
        assert_eq!(model.rows[0].name, "★ #3B82F6");
        assert_eq!(model.rows[0].section, "Pinned Colors");
        assert_eq!(model.rows[0].color_swatch, Some([0x3b, 0x82, 0xf6, 0xff]));
        assert_eq!(
            model.header_badge.as_deref(),
            Some("Color Inspector · OKLCH")
        );
        assert_eq!(model.primary_action.as_deref(), Some("Copy OKLCH"));
        assert!(model.back_button);

        let detail = model.detail.expect("color detail pane");
        assert_eq!(detail.color_swatch, Some([0x3b, 0x82, 0xf6, 0xff]));
        assert_eq!(detail.color_ramp.len(), 9);
        assert_eq!(detail.color_ramp[0].1, "100");
        assert_eq!(detail.color_ramp[4].1, "500");
        assert_eq!(detail.color_ramp[8].1, "900");
        for key in ["HEX", "RGB", "HSL", "OKLCH", "On White", "On Black"] {
            assert!(
                detail.metadata.iter().any(|(k, _)| k == key),
                "missing metadata key {key}"
            );
        }
    }

    #[test]
    fn ocr_preview_model_builds_rows_detail_and_reflects_mode_toggle() {
        let mut state = OcrPreviewState::from_texts(
            "Hello world".to_string(),
            "Hello\nworld".to_string(),
            OcrFormatMode::Smart,
            "en-US".to_string(),
        );
        let smart = ocr_preview_model(&state, 0);
        assert_eq!(smart.rows.len(), 3);
        assert_eq!(smart.header_badge.as_deref(), Some("OCR Preview · Smart"));
        assert_eq!(smart.primary_action.as_deref(), Some("Copy Text"));
        assert!(smart.back_button);
        let smart_detail = smart.detail.expect("smart ocr detail");
        assert_eq!(smart_detail.body, "Hello world");
        assert!(smart_detail
            .metadata
            .iter()
            .any(|(k, v)| k == "Mode" && v == "Smart (NormCap)"));
        assert!(smart_detail
            .metadata
            .iter()
            .any(|(k, v)| k == "Language" && v == "en-US"));
        assert!(smart_detail
            .metadata
            .iter()
            .any(|(k, v)| k == "Lines" && v == "1"));

        state.toggle_mode();
        let raw = ocr_preview_model(&state, 1);
        assert_eq!(raw.header_badge.as_deref(), Some("OCR Preview · Raw"));
        assert_eq!(raw.primary_action.as_deref(), Some("Toggle Mode"));
        let raw_detail = raw.detail.expect("raw ocr detail");
        assert_eq!(raw_detail.body, "Hello\nworld");
        assert!(raw_detail
            .metadata
            .iter()
            .any(|(k, v)| k == "Mode" && v == "Raw"));
        assert!(raw_detail
            .metadata
            .iter()
            .any(|(k, v)| k == "Lines" && v == "2"));
    }

    #[test]
    fn edit_clipboard_model_populates_editor_badge_footer_and_hints() {
        let unpinned = ClipboardItem::text("hello\nworld", None);
        let model = edit_clipboard_model(&unpinned, "hello\nworld");
        assert_eq!(model.header_badge.as_deref(), Some("Edit Clipboard Entry"));
        assert!(model.back_button);
        assert_eq!(model.footer, "11 chars · 2 lines");
        assert_eq!(
            model.footer_hints,
            vec![
                ("Ctrl+↵".to_string(), "Update Entry".to_string()),
                ("Ctrl+Shift+↵".to_string(), "Save as New Copy".to_string()),
                ("Esc".to_string(), "Cancel".to_string()),
            ]
        );
        let editor = model.editor.expect("editor model");
        assert_eq!(editor.title, "Edit Clipboard Entry");
        assert_eq!(editor.text, "hello\nworld");
        assert_eq!(editor.primary_label, "Update Entry");
        assert_eq!(editor.secondary_label, "Save as New Copy");

        let mut pinned = ClipboardItem::text("pinned", None);
        pinned.pinned_at = Some(1_700_000_000_000);
        let pinned_empty = edit_clipboard_model(&pinned, "");
        assert_eq!(
            pinned_empty.header_badge.as_deref(),
            Some("Edit Pinned Clip")
        );
        assert_eq!(pinned_empty.footer, "0 chars · 0 lines");
        let pinned_editor = pinned_empty.editor.expect("pinned editor model");
        assert_eq!(pinned_editor.text, "");
    }

    #[test]
    fn clipboard_detail_preserves_long_text_up_to_32000_chars() {
        let long_text = "a".repeat(5_000);
        let item = ClipboardItem::text(&long_text, None);
        let detail = clipboard_detail_with_image(&item, None);
        assert_eq!(detail.body.chars().count(), 5_000);
        assert_eq!(detail.body, long_text);

        let max_text = "b".repeat(32_000);
        let max_item = ClipboardItem::text(&max_text, None);
        let max_detail = clipboard_detail_with_image(&max_item, None);
        assert_eq!(max_detail.body.chars().count(), 32_000);
        assert_eq!(max_detail.body, max_text);

        let over_text = "c".repeat(32_005);
        let over_item = ClipboardItem::text(&over_text, None);
        let over_detail = clipboard_detail_with_image(&over_item, None);
        assert_eq!(over_detail.body.chars().count(), 32_001);
        assert!(over_detail.body.ends_with('…'));
    }
}
