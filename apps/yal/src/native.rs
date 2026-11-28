//! The native palette: the Direct2D window the user types into.
//!
//! Pure Win32 + Direct2D + DirectWrite — no WebView2, no browser runtime.
//! Routes every native screen cleanly inside one always-ready window:
//! Launcher (with Calculator `ans`, Quicklinks, Snippets, Windows Settings),
//! Clipboard History, 2D Color Emoji Grid, Snippets Browser & Wizard, Quick
//! Notes, Quicklinks & `Tab` Inline Arguments, File Search, Window Switcher,
//! Calculator History, Shell Runner, floating `Ctrl+K` Action Panel,
//! Master-Detail preview panes, Pop-to-Root timer, and live-themed Settings.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use tc_clipboard::{ClipboardItem, ItemKind, OcrFormatMode, PickerColorFormat};
use tc_core::CommandId;
use tc_palette::FileType;
use tc_platform::{Clipboard, FileSystem, Settings};
use tc_platform_win::window_control::WindowInfo;
use tc_platform_win::{HotkeyTrigger, WindowsClipboard, WindowsSettings};
use tc_snippets::StoredSnippet;
use tc_ui::{
    ActionKey, ActionMenuModel, Color, GridCell, Icon, IconBitmap, Model, Palette, PaletteEvent,
    PaletteHandle, PaletteReply, Row, Theme, ToastKind, ToastModel, Tokens,
};

use crate::actions::{self, LauncherActionMeta};
use crate::features::{
    CalcHistoryEntry, ClipboardViewFilter, ColorHistoryEntry, FeatureStores, NoteItem, PasteNext,
    Quicklink,
};
use crate::hotkeys::{
    self, HotkeyController, BINDING_LAUNCHER_TOGGLE, KEY_HOTKEY_HYPERKEY, KEY_HOTKEY_LAUNCHER,
    KEY_SNIPPETS_AUTO_EXPAND,
};
use crate::launcher::{
    filter_directory_entries, is_power_tool_command, parse_numeric_answer, LauncherState,
    ResultRow, CALCULATOR_PREFIX, CALCULATOR_SECTION, CMD_COLOR_HISTORY, CMD_COLOR_PICKER,
    CMD_SCREEN_OCR, COMMAND_SECTION, CUSTOM_COMMAND_SECTION, FILE_PREFIX, FILE_SECTION,
    RANKING_FILE, SYSTEM_ACTION_SECTION, WINDOW_COMMAND_SECTION,
};
use crate::open::Runner;
use crate::palette::{PaletteState, SettingRow};
use crate::screens::{
    calc_history_model, clipboard_model, color_history_model, create_note_model,
    create_quicklink_model, create_snippet_model, downsample_rgba_premultiplied,
    downsample_square_cover_premultiplied, edit_clipboard_model, emoji_model, fallback_search_rows,
    files_model, notes_model, ocr_preview_model, parse_clipboard_file_paths, quicklink_arg_model,
    quicklinks_model, run_shell_model, set_alias_model, shorten_home_path, shows_create_note_row,
    shows_create_quicklink_row, shows_create_snippet_row, snippets_model, status_screen_model,
    windows_model, NoteDraft, NoteDraftStep, QuicklinkDraft, QuicklinkDraftStep, SnippetDraft,
    SnippetDraftStep, FALLBACK_CMD_PREFIX, FALLBACK_URL_PREFIX,
};
use crate::shell::{spawn_rates_refresh, FILE_SEARCH_ENABLED, FILE_SEARCH_SHOW, RATES_FILE};
use crate::tools::{
    self, OcrPreviewState, ToolsBridge, COMMAND_OPEN_COLOR_INSPECTOR, COMMAND_OPEN_OCR_PREVIEW,
    KEY_COLOR_PICKER_FORMAT, KEY_OCR_LANGUAGE, KEY_OCR_MODE,
};
use crate::win_settings::{self, WIN_SETTING_PREFIX, WIN_SETTING_SECTION};

/// Cross-thread command code asking the native palette to open Settings.
pub const COMMAND_OPEN_SETTINGS: u32 = 1;
/// Cross-thread command code asking the palette to re-show with a pending status banner.
const COMMAND_SHOW_STATUS: u32 = 2;
/// Cross-thread command code fired when live hotkey capture finishes.
pub const COMMAND_HOTKEY_CAPTURED: u32 = 3;
/// Cross-thread command code fired when a per-command global hotkey opens a palette screen.
pub const COMMAND_DISPATCH_HOTKEY: u32 = 4;

/// Setting key storing the launcher's legacy global hotkey in `settings.json`.
pub const KEY_LAUNCHER_HOTKEY: &str = hotkeys::KEY_LEGACY_LAUNCHER_HOTKEY;
/// Virtual action key in Settings for launching live hotkey recording.
const KEY_RECORD_LAUNCHER_HOTKEY: &str = "hotkey.launcher.record";
/// Prefix for virtual action keys in Settings that record a built-in command hotkey.
const KEY_RECORD_CMD_HOTKEY_PREFIX: &str = "hotkey.record.cmd.";
/// Persisted custom dragged palette X coordinate (physical screen pixels).
const KEY_POSITION_X: &str = "palette.positionX";
/// Persisted custom dragged palette Y coordinate (physical screen pixels).
const KEY_POSITION_Y: &str = "palette.positionY";
/// Virtual action key in Settings to reset custom dragged position back to center.
const KEY_RESET_WINDOW_POSITION: &str = "palette.resetPosition";
/// Setting key toggling Window Management commands in the launcher (default: ON).
pub const KEY_WINDOW_MANAGEMENT_ENABLED: &str = "windowManagement.enabled";
/// Setting key storing the Pop-to-Root timer policy (`0`, `30`, `60`, `300`, `never`).
pub const KEY_POP_TO_ROOT_SECS: &str = "palette.popToRootSecs";

/// Header badge displayed while recording a global hotkey.
const RECORDING_HOTKEY_BADGE: &str = "● Recording…";

/// Maximum number of decoded clipboard image thumbnails cached in memory.
const CLIPBOARD_THUMB_CACHE_CAPACITY: usize = 48;

/// Prefix for an inline or root-search snippet row.
pub const INLINE_SNIPPET_PREFIX: &str = "snippet-inline:";
const SNIPPET_SECTION: &str = "Snippets";

/// Prefix for a root-search quicklink row.
pub const ROOT_QUICKLINK_PREFIX: &str = "quicklink:";
const QUICKLINK_SECTION: &str = "Quicklinks";

/// Built-in hotkey presets cycled in the native Settings pane.
const HOTKEY_PRESETS: [&str; 4] = ["Alt+Space", "Ctrl+Space", "Double Ctrl", "Double Alt"];

/// The section an application belongs to.
const APPLICATIONS: &str = "Applications";

/// Pop-to-Root reset timeout policy cycled in Settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopToRootPolicy {
    Immediately,
    AfterSecs(u64),
    Never,
}

impl PopToRootPolicy {
    /// Default reset timeout: return immediately to Root Search when reopened after closing.
    pub const DEFAULT: PopToRootPolicy = PopToRootPolicy::Immediately;

    /// Parse stored value from `settings.json` (defaults to 1 minute).
    pub fn parse(raw: Option<&str>) -> Self {
        let Some(value) = raw.map(str::trim).filter(|v| !v.is_empty()) else {
            return Self::DEFAULT;
        };
        let lower = value.to_ascii_lowercase();
        match lower.as_str() {
            "0" | "immediately" | "immediately (0s)" => Self::Immediately,
            "30" | "30 seconds" => Self::AfterSecs(30),
            "60" | "1 minute" | "60 seconds" => Self::AfterSecs(60),
            "300" | "5 minutes" => Self::AfterSecs(300),
            "never" => Self::Never,
            other => {
                let digits = other
                    .strip_suffix(" seconds")
                    .or_else(|| other.strip_suffix(" second"))
                    .unwrap_or(other);
                match digits.parse::<u64>() {
                    Ok(0) => Self::Immediately,
                    Ok(secs) => Self::AfterSecs(secs),
                    Err(_) => Self::DEFAULT,
                }
            }
        }
    }

    /// Human-readable label shown in the Settings screen.
    pub fn label(self) -> &'static str {
        match self {
            Self::Immediately => "Immediately (0s)",
            Self::AfterSecs(30) => "30 seconds",
            Self::AfterSecs(60) => "1 minute",
            Self::AfterSecs(300) => "5 minutes",
            Self::AfterSecs(_) => "1 minute",
            Self::Never => "Never",
        }
    }

    /// Canonical value written to `settings.json`.
    pub fn stored_value(self) -> &'static str {
        match self {
            Self::Immediately => "0",
            Self::AfterSecs(30) => "30",
            Self::AfterSecs(60) => "60",
            Self::AfterSecs(300) => "300",
            Self::AfterSecs(_) => "60",
            Self::Never => "never",
        }
    }

    /// Cycle cleanly through all five presets.
    pub fn next(self) -> Self {
        match self {
            Self::Immediately => Self::AfterSecs(30),
            Self::AfterSecs(30) => Self::AfterSecs(60),
            Self::AfterSecs(60) => Self::AfterSecs(300),
            Self::AfterSecs(300) => Self::Never,
            Self::Never => Self::Immediately,
            Self::AfterSecs(_) => Self::AfterSecs(60),
        }
    }

    /// Whether the palette should automatically pop back to Root Search on `Shown`.
    pub fn should_reset(self, last_dismissed_at: Option<Instant>, now: Instant) -> bool {
        match self {
            Self::Immediately => true,
            Self::AfterSecs(secs) => last_dismissed_at
                .is_some_and(|at| now.saturating_duration_since(at) >= Duration::from_secs(secs)),
            Self::Never => false,
        }
    }
}

/// Application context shared with the native Direct2D palette.
pub struct NativeContext {
    pub palette: Arc<PaletteState>,
    pub settings: Arc<WindowsSettings>,
    pub features: Arc<FeatureStores>,
    pub hotkeys: Arc<HotkeyController>,
    pub pending_hotkey_command: Arc<Mutex<Option<String>>>,
    pub tools: Arc<ToolsBridge>,
}

/// The native palette window.
pub struct NativePalette {
    palette: Palette,
}

impl NativePalette {
    /// Opens the palette on the current thread (standalone mode: exits on dismiss/activate).
    pub fn open(
        launcher: Arc<LauncherState>,
        runner: Arc<Runner>,
        theme: Theme,
    ) -> Result<NativePalette, String> {
        let palette = build(launcher, runner, None, true, theme)?;
        Ok(NativePalette { palette })
    }

    /// Opens the palette on the current thread with full application context.
    pub fn open_with_context(
        launcher: Arc<LauncherState>,
        runner: Arc<Runner>,
        context: NativeContext,
        theme: Theme,
    ) -> Result<NativePalette, String> {
        let palette = build(launcher, runner, Some(context), false, theme)?;
        Ok(NativePalette { palette })
    }

    pub fn handle(&self) -> PaletteHandle {
        self.palette.handle()
    }

    pub fn show(&self) {
        self.palette.show();
    }

    pub fn run(&self) {
        self.palette.run();
    }
}

#[derive(Clone, PartialEq, Eq)]
enum RecordingTarget {
    Launcher,
    Command { entry_id: String, title: String },
}

#[derive(Clone, PartialEq, Eq)]
enum CapturedHotkeyEvent {
    Trigger(HotkeyTrigger),
    Cancelled,
}

#[derive(Default, PartialEq, Eq, Clone)]
enum Mode {
    #[default]
    Launcher,
    Clipboard,
    EditClipboard {
        item: ClipboardItem,
        previous_query: String,
        previous_selected: usize,
    },
    Emoji,
    Snippets,
    CreateSnippet(SnippetDraft),
    Notes,
    CreateNote(NoteDraft),
    Quicklinks,
    QuicklinkArg {
        link: Quicklink,
        from_launcher: bool,
        previous_query: String,
    },
    SetAlias {
        entry_id: String,
        title: String,
        current_alias: Option<String>,
        previous_query: String,
    },
    CreateQuicklink(QuicklinkDraft),
    RunShell,
    Status,
    Files,
    SwitchWindows,
    CalcHistory,
    ColorHistory,
    OcrPreview,
    Settings,
}

struct Shown {
    mode: Mode,
    visible: bool,
    query: String,
    selected: usize,
    show_detail: bool,
    show_hidden_files: bool,
    action_menu_open: bool,
    compact_expanded: bool,
    last_dismissed_at: Option<Instant>,
    recording_hotkey: Option<RecordingTarget>,
    captured_hotkey: Arc<Mutex<Option<CapturedHotkeyEvent>>>,
    rows: Vec<ResultRow>,
    clipboard_items: Vec<ClipboardItem>,
    clipboard_filter: ClipboardViewFilter,
    clipboard_thumbs: HashMap<String, (u32, u32, Arc<IconBitmap>, Arc<IconBitmap>)>,
    emoji_cells: Vec<GridCell>,
    snippets: Vec<StoredSnippet>,
    notes: Vec<NoteItem>,
    quicklinks: Vec<Quicklink>,
    last_shell_output: Option<String>,
    pending_status: Option<(String, String, String, bool)>,
    file_rows: Vec<ResultRow>,
    file_filter: FileType,
    file_dir: Option<PathBuf>,
    file_dir_cache: Option<(PathBuf, bool, Instant, Vec<tc_files::index::Entry>)>,
    windows: Vec<WindowInfo>,
    calc_live: Option<ResultRow>,
    calc_entries: Vec<CalcHistoryEntry>,
    color_entries: Vec<ColorHistoryEntry>,
    ocr_preview: Option<OcrPreviewState>,
    settings_rows: Vec<SettingRow>,
    icons: Arc<Mutex<HashMap<String, Option<Arc<IconBitmap>>>>>,
    queued: HashSet<String>,
    icon_tx: Option<mpsc::Sender<String>>,
}

impl Shown {
    fn new(
        icons: Arc<Mutex<HashMap<String, Option<Arc<IconBitmap>>>>>,
        icon_tx: mpsc::Sender<String>,
    ) -> Self {
        Self {
            mode: Mode::Launcher,
            visible: false,
            query: String::new(),
            selected: 0,
            show_detail: true,
            show_hidden_files: false,
            action_menu_open: false,
            compact_expanded: false,
            last_dismissed_at: None,
            recording_hotkey: None,
            captured_hotkey: Arc::new(Mutex::new(None)),
            rows: Vec::new(),
            clipboard_items: Vec::new(),
            clipboard_filter: ClipboardViewFilter::All,
            clipboard_thumbs: HashMap::new(),
            emoji_cells: Vec::new(),
            snippets: Vec::new(),
            notes: Vec::new(),
            quicklinks: Vec::new(),
            last_shell_output: None,
            pending_status: None,
            file_rows: Vec::new(),
            file_filter: FileType::All,
            file_dir: None,
            file_dir_cache: None,
            windows: Vec::new(),
            calc_live: None,
            calc_entries: Vec::new(),
            color_entries: Vec::new(),
            ocr_preview: None,
            settings_rows: Vec::new(),
            icons,
            queued: HashSet::new(),
            icon_tx: Some(icon_tx),
        }
    }

    /// Bounded O(1) cache lookup for decoded & downsampled clipboard image
    /// previews `(width, height, detail_preview, square_row_thumb)`.
    fn lookup_clipboard_thumb(
        &mut self,
        features: &FeatureStores,
        item: &ClipboardItem,
        load_if_missing: bool,
    ) -> Option<(u32, u32, Arc<IconBitmap>, Arc<IconBitmap>)> {
        if item.kind != ItemKind::Image {
            return None;
        }
        if let Some(cached) = self.clipboard_thumbs.get(&item.id) {
            return Some(cached.clone());
        }
        if !load_if_missing {
            return None;
        }
        let (width, height, rgba) = features.load_clipboard_image_rgba(item)?;
        let detail_bitmap = Arc::new(downsample_rgba_premultiplied(
            width, height, &rgba, 440, 260,
        )?);
        let row_thumb = Arc::new(downsample_square_cover_premultiplied(
            width, height, &rgba, 44,
        )?);
        if self.clipboard_thumbs.len() >= CLIPBOARD_THUMB_CACHE_CAPACITY {
            self.clipboard_thumbs.clear();
        }
        let entry = (width, height, detail_bitmap, row_thumb);
        self.clipboard_thumbs.insert(item.id.clone(), entry.clone());
        Some(entry)
    }

    /// Filter the active File Search view (indexed search or drilled folder)
    /// without re-reading `std::fs::read_dir` on every keystroke.
    fn filtered_file_rows(&mut self, launcher: &LauncherState, query: &str) -> Vec<ResultRow> {
        let filter = self.file_filter;
        let show_hidden = self.show_hidden_files;
        if let Some(dir) = self.file_dir.as_deref() {
            let cache_hit =
                self.file_dir_cache
                    .as_ref()
                    .is_some_and(|(cached_dir, cached_hidden, _, _)| {
                        cached_dir == dir && *cached_hidden == show_hidden
                    });
            if !cache_hit {
                let entries = tc_files::list_directory(dir, show_hidden);
                self.file_dir_cache =
                    Some((dir.to_path_buf(), show_hidden, Instant::now(), entries));
            }
            let entries = self
                .file_dir_cache
                .as_ref()
                .map(|(_, _, _, e)| e.as_slice())
                .unwrap_or(&[]);
            return filter_directory_entries(entries, query, filter);
        }
        let mut rows = launcher.file_screen_rows(query, filter);
        if !show_hidden {
            rows.retain(|r| !r.name.starts_with('.'));
        }
        rows
    }

    /// Drop the cached directory listing if it is older than `ttl`, avoiding
    /// repeated `read_dir` calls when background icon loads trigger `Refresh`.
    fn invalidate_file_dir_cache_if_stale(&mut self, ttl: Duration) {
        if let Some((_, _, loaded_at, _)) = self.file_dir_cache.as_ref() {
            if loaded_at.elapsed() >= ttl {
                self.file_dir_cache = None;
            }
        }
    }

    /// Non-blocking O(1) cache lookup. If an icon is not yet decoded, queues it
    /// to the background `yal-icons` worker so the UI thread never stalls.
    fn lookup_or_queue_icon(&mut self, target: &str) -> Option<Arc<IconBitmap>> {
        if target.trim().is_empty() {
            return None;
        }
        if let Ok(cache) = self.icons.lock() {
            if let Some(cached) = cache.get(target) {
                return cached.clone();
            }
        }
        if self.queued.insert(target.to_string()) {
            if let Some(tx) = self.icon_tx.as_ref() {
                let _ = tx.send(target.to_string());
            }
        }
        None
    }
}

fn lock_or_recover<T>(lock: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    lock.lock().unwrap_or_else(|err| err.into_inner())
}

fn apply_recording_overlay(model: &mut Model, shown: &Shown) {
    if shown.recording_hotkey.is_some() {
        model.header_badge = Some(RECORDING_HOTKEY_BADGE.to_string());
        if let Some(row) = model.rows.get_mut(model.selected) {
            row.recording = true;
            row.shortcut = RECORDING_HOTKEY_BADGE.to_string();
            row.detail = "Press any key combination now · Esc to cancel".to_string();
        }
    }
}

fn build_clipboard_screen_model(
    ctx: &NativeContext,
    shown: &mut Shown,
    query: &str,
    items: &[ClipboardItem],
    selected: usize,
) -> Model {
    let filter = shown.clipboard_filter;
    let queue_len = ctx.features.paste_queue_len();
    let show_detail = shown.show_detail;
    let features = &ctx.features;
    // Two-pass borrow split: resolve image thumbs & file/app icons cleanly through `shown`.
    let mut preloaded_icons: HashMap<String, Arc<IconBitmap>> = HashMap::new();
    for (idx, item) in items.iter().enumerate() {
        if idx >= 32 {
            break;
        }
        match item.kind {
            ItemKind::File => {
                if let Some(first) = parse_clipboard_file_paths(item).into_iter().next() {
                    if let Some(bmp) = shown.lookup_or_queue_icon(&first) {
                        preloaded_icons.insert(first, bmp);
                    }
                }
            }
            ItemKind::Text => {
                if let Some(src) = item.source_bundle_id.as_deref() {
                    if src.contains('\\') || src.to_ascii_lowercase().ends_with(".exe") {
                        if let Some(bmp) = shown.lookup_or_queue_icon(src) {
                            preloaded_icons.insert(src.to_string(), bmp);
                        }
                    }
                }
            }
            ItemKind::Image => {}
        }
    }
    let mut model = clipboard_model(
        query,
        items,
        filter,
        queue_len,
        selected,
        show_detail,
        |item, load| shown.lookup_clipboard_thumb(features, item, load),
        |target| preloaded_icons.get(target).cloned(),
    );
    let primary = if tc_platform_win::has_paste_target() {
        "Paste"
    } else {
        "Copy"
    };
    model.primary_action = Some(primary.to_string());
    if let Some(first_hint) = model.footer_hints.first_mut() {
        first_hint.1 = primary.to_string();
    }
    model
}

fn start_hotkey_capture(
    ctx: &NativeContext,
    handle_slot: &Arc<Mutex<Option<PaletteHandle>>>,
    shown: &mut Shown,
    target: RecordingTarget,
) -> PaletteReply {
    let label = match &target {
        RecordingTarget::Launcher => "Launcher Hotkey".to_string(),
        RecordingTarget::Command { title, .. } => title.clone(),
    };
    let slot = Arc::clone(&shown.captured_hotkey);
    let h_slot = Arc::clone(handle_slot);
    let started = ctx.hotkeys.begin_capture(move |result| {
        let event = match result {
            Some(trigger) => CapturedHotkeyEvent::Trigger(trigger),
            None => CapturedHotkeyEvent::Cancelled,
        };
        *lock_or_recover(&slot) = Some(event);
        if let Some(handle) = *lock_or_recover(&h_slot) {
            tc_ui::post_command(handle, COMMAND_HOTKEY_CAPTURED);
        }
    });
    if !started {
        let toast = ToastModel {
            kind: ToastKind::Error,
            message: "Global hotkey hook is not active".to_string(),
        };
        return match rebuild_current_model(Some(ctx), shown, Some(toast)) {
            Some(model) => PaletteReply::Update(model),
            None => PaletteReply::Keep,
        };
    }
    shown.recording_hotkey = Some(target);
    let toast = ToastModel {
        kind: ToastKind::Info,
        message: format!("Press shortcut for {label} (Esc to cancel)"),
    };
    match rebuild_current_model(Some(ctx), shown, Some(toast)) {
        Some(model) => PaletteReply::Update(model),
        None => PaletteReply::Keep,
    }
}

fn execute_clipboard_item(
    ctx: &NativeContext,
    item: &ClipboardItem,
    paste: bool,
    close_with_hud: &dyn Fn(String) -> PaletteReply,
) -> PaletteReply {
    ctx.features.promote_clipboard_item(item);
    // Only inject Ctrl+V if there is an actual visible, pasteable target app;
    // otherwise keep it as an active copy on the system clipboard.
    let should_paste = paste && tc_platform_win::has_paste_target();
    match item.kind {
        ItemKind::Image => {
            let Some((w, h, rgba)) = ctx.features.load_clipboard_image_rgba(item) else {
                return close_with_hud("Image unavailable".to_string());
            };
            ctx.hotkeys.suppress_clipboard_for_ms(700);
            if should_paste {
                let _ = tc_platform_win::paste_image_async(w, h, rgba);
                close_with_hud("Pasted Image".to_string())
            } else {
                let _ = WindowsClipboard::new().write_dib_rgba(w, h, &rgba);
                close_with_hud("Copied Image".to_string())
            }
        }
        ItemKind::File => {
            let paths = parse_clipboard_file_paths(item);
            if paths.is_empty() {
                return close_with_hud("No files in entry".to_string());
            }
            let count = paths.len();
            ctx.hotkeys.suppress_clipboard_for_ms(700);
            if should_paste {
                let _ = tc_platform_win::paste_files_async(paths);
                if count == 1 {
                    close_with_hud("Pasted File".to_string())
                } else {
                    close_with_hud(format!("Pasted {count} Files"))
                }
            } else {
                let _ = WindowsClipboard::new().write_files(&paths);
                if count == 1 {
                    close_with_hud("Copied File".to_string())
                } else {
                    close_with_hud(format!("Copied {count} Files"))
                }
            }
        }
        ItemKind::Text => {
            let Some(text) = item.text.clone() else {
                return PaletteReply::Keep;
            };
            ctx.hotkeys.suppress_clipboard_for_ms(700);
            if should_paste {
                let _ = tc_platform_win::copy_and_paste_async(text);
                close_with_hud("Pasted".to_string())
            } else {
                let _ = tc_platform_win::clipboard().write_text(&text);
                close_with_hud("Copied to Clipboard".to_string())
            }
        }
    }
}

fn paste_next_from_queue(
    ctx: &NativeContext,
    close_with_hud: &dyn Fn(String) -> PaletteReply,
) -> PaletteReply {
    let (item, remaining) = match ctx.features.paste_queue_pop_next() {
        PasteNext::Item { item, remaining } => (item, remaining),
        PasteNext::Settling => return PaletteReply::Keep,
        PasteNext::Empty => return close_with_hud("Paste Queue Empty".to_string()),
    };
    let hud = if remaining == 0 {
        "Paste Queue Complete".to_string()
    } else {
        format!("Pasted from Queue ({remaining} left)")
    };
    match item.kind {
        ItemKind::Image => {
            if let Some((w, h, rgba)) = ctx.features.load_clipboard_image_rgba(&item) {
                ctx.hotkeys.suppress_clipboard_for_ms(700);
                let _ = tc_platform_win::paste_image_async(w, h, rgba);
            }
        }
        ItemKind::File => {
            let paths = parse_clipboard_file_paths(&item);
            if !paths.is_empty() {
                ctx.hotkeys.suppress_clipboard_for_ms(700);
                let _ = tc_platform_win::paste_files_async(paths);
            }
        }
        ItemKind::Text => {
            if let Some(text) = item.text.clone() {
                ctx.hotkeys.suppress_clipboard_for_ms(700);
                let _ = tc_platform_win::copy_and_paste_async(text);
            }
        }
    }
    close_with_hud(hud)
}

fn lock_shown(lock: &Arc<Mutex<Shown>>) -> std::sync::MutexGuard<'_, Shown> {
    lock.lock().unwrap_or_else(|err| err.into_inner())
}

fn spawn_icon_worker(
    icons: Arc<Mutex<HashMap<String, Option<Arc<IconBitmap>>>>>,
    handle_slot: Arc<Mutex<Option<PaletteHandle>>>,
) -> mpsc::Sender<String> {
    let (tx, rx) = mpsc::channel::<String>();
    let _ = std::thread::Builder::new()
        .name("yal-icons".to_string())
        .spawn(move || {
            while let Ok(first) = rx.recv() {
                let mut batch = vec![first];
                while let Ok(next) = rx.recv_timeout(Duration::from_millis(12)) {
                    batch.push(next);
                    if batch.len() >= 12 {
                        break;
                    }
                }
                let mut decoded_any = false;
                for target in batch {
                    let already = icons
                        .lock()
                        .ok()
                        .is_some_and(|map| map.contains_key(&target));
                    if already {
                        continue;
                    }
                    let extracted = tc_platform_win::extract_icon(&target).map(|img| {
                        Arc::new(IconBitmap {
                            width: img.width,
                            height: img.height,
                            premultiplied_rgba: img.premultiplied_rgba,
                        })
                    });
                    if extracted.is_some() {
                        decoded_any = true;
                    }
                    if let Ok(mut map) = icons.lock() {
                        if map.len() >= 512 {
                            map.clear();
                        }
                        map.insert(target, extracted);
                    }
                }
                if decoded_any {
                    if let Ok(guard) = handle_slot.lock() {
                        if let Some(handle) = *guard {
                            tc_ui::post_refresh(handle);
                        }
                    }
                }
            }
        });
    tx
}

fn build(
    launcher: Arc<LauncherState>,
    runner: Arc<Runner>,
    context: Option<NativeContext>,
    quit_on_close: bool,
    theme: Theme,
) -> Result<Palette, String> {
    let icons: Arc<Mutex<HashMap<String, Option<Arc<IconBitmap>>>>> =
        Arc::new(Mutex::new(HashMap::new()));
    let handle_slot: Arc<Mutex<Option<PaletteHandle>>> = Arc::new(Mutex::new(None));
    let icon_tx = spawn_icon_worker(Arc::clone(&icons), Arc::clone(&handle_slot));
    let shown: Arc<Mutex<Shown>> = Arc::new(Mutex::new(Shown::new(icons, icon_tx)));
    let context = context.map(Arc::new);

    let handler_shown = Arc::clone(&shown);
    let handler_launcher = Arc::clone(&launcher);
    let handler_runner = Arc::clone(&runner);
    let handler_context = context.clone();
    let handler_handle = Arc::clone(&handle_slot);

    let close_reply = move || {
        if quit_on_close {
            PaletteReply::Quit
        } else {
            PaletteReply::Close
        }
    };

    let close_with_hud = move |msg: String| {
        if quit_on_close {
            PaletteReply::Quit
        } else {
            PaletteReply::CloseWithHud(msg)
        }
    };

    let handler = move |event: PaletteEvent| -> PaletteReply {
        let reply = (|| -> PaletteReply {
            match event {
                PaletteEvent::Shown => {
                    let mut shown = lock_shown(&handler_shown);
                    shown.visible = true;
                    shown.action_menu_open = false;
                    let policy = handler_context
                        .as_deref()
                        .map(|ctx| pop_to_root_policy(&ctx.settings))
                        .unwrap_or(PopToRootPolicy::DEFAULT);
                    let should_reset = policy.should_reset(shown.last_dismissed_at, Instant::now());

                    if shown.mode != Mode::Launcher {
                        if should_reset {
                            if let Some(ctx) = handler_context.as_ref() {
                                ctx.palette.back_to_root();
                            }
                            return enter_launcher(
                                &handler_launcher,
                                handler_context.as_deref(),
                                &mut shown,
                                true,
                            );
                        }
                        if matches!(shown.mode, Mode::EditClipboard { .. }) {
                            return PaletteReply::Keep;
                        }
                        let saved_query = shown.query.clone();
                        if let Some(model) =
                            rebuild_current_model(handler_context.as_deref(), &mut shown, None)
                        {
                            return PaletteReply::UpdateWithEdit(saved_query, model);
                        }
                    }

                    if !should_reset && !shown.query.is_empty() {
                        let saved_query = shown.query.clone();
                        let rows = build_root_search_rows(
                            &handler_launcher,
                            handler_context.as_deref(),
                            &saved_query,
                        );
                        shown.rows = rows.clone();
                        let sel = shown.selected.min(rows.len().saturating_sub(1));
                        shown.selected = sel;
                        let model = model_for_with_icons(
                            &saved_query,
                            &rows,
                            false,
                            sel,
                            handler_context.as_deref(),
                            &mut shown,
                        );
                        return PaletteReply::UpdateWithEdit(saved_query, model);
                    }

                    if let Some(ctx) = handler_context.as_ref() {
                        ctx.palette.back_to_root();
                    }
                    enter_launcher(
                        &handler_launcher,
                        handler_context.as_deref(),
                        &mut shown,
                        false,
                    )
                }
                PaletteEvent::Refresh => {
                    let mut shown = lock_shown(&handler_shown);
                    match shown.mode {
                        Mode::Launcher => {
                            let query = shown.query.clone();
                            let is_suggestions = query.trim().is_empty();
                            let rows = if is_suggestions {
                                shown.rows.clone()
                            } else {
                                let fresh = build_root_search_rows(
                                    &handler_launcher,
                                    handler_context.as_deref(),
                                    &query,
                                );
                                shown.rows = fresh.clone();
                                fresh
                            };
                            let sel = shown.selected.min(rows.len().saturating_sub(1));
                            shown.selected = sel;
                            PaletteReply::Update(model_for_with_icons(
                                &query,
                                &rows,
                                is_suggestions,
                                sel,
                                handler_context.as_deref(),
                                &mut shown,
                            ))
                        }
                        Mode::Files => {
                            shown.invalidate_file_dir_cache_if_stale(Duration::from_millis(1500));
                            let query = shown.query.clone();
                            let filter = shown.file_filter;
                            let current_dir = shown.file_dir.clone();
                            let rows = shown.filtered_file_rows(&handler_launcher, &query);
                            let indexing = handler_context
                                .as_ref()
                                .is_some_and(|c| c.features.is_file_indexing());
                            let sel = shown.selected.min(rows.len().saturating_sub(1));
                            let show_detail = shown.show_detail;
                            shown.file_rows = rows.clone();
                            shown.selected = sel;
                            PaletteReply::Update(files_model(
                                &query,
                                &rows,
                                filter,
                                current_dir.as_deref(),
                                indexing,
                                sel,
                                show_detail,
                                |target| shown.lookup_or_queue_icon(target),
                            ))
                        }
                        Mode::Clipboard => {
                            let Some(ctx) = handler_context.as_ref() else {
                                return PaletteReply::Keep;
                            };
                            let query = shown.query.clone();
                            let filter = shown.clipboard_filter;
                            let items = ctx.features.search_clipboard_view(&query, filter);
                            let sel = shown.selected.min(items.len().saturating_sub(1));
                            shown.clipboard_items = items.clone();
                            shown.selected = sel;
                            let mut model =
                                build_clipboard_screen_model(ctx, &mut shown, &query, &items, sel);
                            apply_recording_overlay(&mut model, &shown);
                            PaletteReply::Update(model)
                        }
                        Mode::RunShell => {
                            let query = shown.query.clone();
                            let last = shown.last_shell_output.clone();
                            PaletteReply::Update(run_shell_model(&query, last.as_deref()))
                        }
                        Mode::ColorHistory => {
                            let Some(ctx) = handler_context.as_ref() else {
                                return PaletteReply::Keep;
                            };
                            let query = shown.query.clone();
                            let entries = ctx.features.color_history_items(&query);
                            let sel = shown.selected.min(entries.len().saturating_sub(1));
                            shown.color_entries = entries.clone();
                            shown.selected = sel;
                            let fmt = tools::configured_color_format(&*ctx.settings);
                            PaletteReply::Update(color_history_model(
                                &query,
                                &entries,
                                sel,
                                shown.show_detail,
                                fmt,
                            ))
                        }
                        Mode::EditClipboard { .. } => PaletteReply::Keep,
                        _ => PaletteReply::Keep,
                    }
                }
                PaletteEvent::SelectionChanged(idx) => {
                    let mut shown = lock_shown(&handler_shown);
                    if matches!(shown.mode, Mode::EditClipboard { .. }) {
                        return PaletteReply::Keep;
                    }
                    shown.selected = idx;
                    if shown.action_menu_open {
                        return PaletteReply::Keep;
                    }
                    match rebuild_current_model(handler_context.as_deref(), &mut shown, None) {
                        Some(model) => PaletteReply::Update(model),
                        None => PaletteReply::Keep,
                    }
                }
                PaletteEvent::Command(COMMAND_OPEN_SETTINGS) | PaletteEvent::OpenSettings => {
                    let Some(ctx) = handler_context.as_ref() else {
                        return PaletteReply::Keep;
                    };
                    let mut shown = lock_shown(&handler_shown);
                    shown.visible = true;
                    enter_settings(ctx, &mut shown)
                }
                PaletteEvent::Command(COMMAND_OPEN_OCR_PREVIEW) => {
                    let Some(ctx) = handler_context.as_ref() else {
                        return PaletteReply::Keep;
                    };
                    let Some(state) = ctx.tools.take_pending_ocr_preview() else {
                        return PaletteReply::Keep;
                    };
                    let mut shown = lock_shown(&handler_shown);
                    shown.visible = true;
                    shown.mode = Mode::OcrPreview;
                    shown.query.clear();
                    shown.selected = 0;
                    shown.action_menu_open = false;
                    let model = ocr_preview_model(&state, 0);
                    shown.ocr_preview = Some(state);
                    PaletteReply::Navigate(model)
                }
                PaletteEvent::Command(COMMAND_OPEN_COLOR_INSPECTOR) => {
                    let Some(ctx) = handler_context.as_ref() else {
                        return PaletteReply::Keep;
                    };
                    let pending_hex = ctx.tools.take_pending_color_hex();
                    let mut shown = lock_shown(&handler_shown);
                    shown.visible = true;
                    enter_color_history(ctx, &mut shown, pending_hex.as_deref())
                }
                PaletteEvent::Command(COMMAND_SHOW_STATUS) => {
                    let mut shown = lock_shown(&handler_shown);
                    let Some((badge, title, detail, refresh_theme)) = shown.pending_status.take()
                    else {
                        return PaletteReply::Keep;
                    };
                    shown.visible = true;
                    shown.mode = Mode::Status;
                    shown.query.clear();
                    shown.selected = 0;
                    shown.action_menu_open = false;
                    let model = status_screen_model(&badge, &title, &detail, Vec::new());
                    if refresh_theme {
                        if let Some(ctx) = handler_context.as_ref() {
                            return PaletteReply::UpdateTheme(
                                Box::new(theme_from_settings(&ctx.settings)),
                                model,
                            );
                        }
                    }
                    PaletteReply::Navigate(model)
                }
                PaletteEvent::Command(COMMAND_HOTKEY_CAPTURED) => {
                    let Some(ctx) = handler_context.as_ref() else {
                        return PaletteReply::Keep;
                    };
                    let mut shown = lock_shown(&handler_shown);
                    let Some(event) = lock_or_recover(&shown.captured_hotkey).take() else {
                        shown.recording_hotkey = None;
                        return PaletteReply::Keep;
                    };
                    let trigger = match event {
                        CapturedHotkeyEvent::Trigger(t) => t,
                        CapturedHotkeyEvent::Cancelled => {
                            shown.recording_hotkey = None;
                            let toast = ToastModel {
                                kind: ToastKind::Info,
                                message: "Hotkey recording cancelled".to_string(),
                            };
                            return match rebuild_current_model(Some(ctx), &mut shown, Some(toast)) {
                                Some(model) => PaletteReply::Update(model),
                                None => PaletteReply::Keep,
                            };
                        }
                    };
                    let Some(target) = shown.recording_hotkey.take() else {
                        return PaletteReply::Keep;
                    };
                    let formatted = trigger.format();
                    let toast = match target {
                        RecordingTarget::Launcher => {
                            if let Some(conflict) = hotkeys::check_hotkey_conflict(
                                &*ctx.settings,
                                trigger,
                                Some(BINDING_LAUNCHER_TOGGLE),
                            ) {
                                ToastModel {
                                    kind: ToastKind::Error,
                                    message: hotkeys::conflict_message(conflict),
                                }
                            } else {
                                let _ = hotkeys::save_launcher_hotkey(&*ctx.settings, trigger);
                                ctx.hotkeys.reload_bindings(&ctx.settings);
                                if shown.mode == Mode::Settings {
                                    let q = shown.query.clone();
                                    shown.settings_rows = collect_settings_rows(ctx, &q);
                                }
                                ToastModel {
                                    kind: ToastKind::Success,
                                    message: format!("Launcher hotkey set to {formatted}"),
                                }
                            }
                        }
                        RecordingTarget::Command { entry_id, title } => {
                            let binding_id = hotkeys::command_binding_id(&entry_id);
                            if let Some(conflict) = hotkeys::check_hotkey_conflict(
                                &*ctx.settings,
                                trigger,
                                Some(&binding_id),
                            ) {
                                ToastModel {
                                    kind: ToastKind::Error,
                                    message: hotkeys::conflict_message(conflict),
                                }
                            } else {
                                let _ = hotkeys::set_command_hotkey(
                                    &*ctx.settings,
                                    &entry_id,
                                    Some(trigger),
                                );
                                ctx.hotkeys.reload_bindings(&ctx.settings);
                                if shown.mode == Mode::Settings {
                                    let q = shown.query.clone();
                                    shown.settings_rows = collect_settings_rows(ctx, &q);
                                }
                                ToastModel {
                                    kind: ToastKind::Success,
                                    message: format!("Hotkey {formatted} bound to {title}"),
                                }
                            }
                        }
                    };
                    match rebuild_current_model(Some(ctx), &mut shown, Some(toast)) {
                        Some(model) => PaletteReply::Update(model),
                        None => PaletteReply::Keep,
                    }
                }
                PaletteEvent::Command(COMMAND_DISPATCH_HOTKEY) => {
                    let Some(ctx) = handler_context.as_ref() else {
                        return PaletteReply::Keep;
                    };
                    let Some(entry_id) = lock_or_recover(&ctx.pending_hotkey_command).take() else {
                        return PaletteReply::Keep;
                    };
                    let mut shown = lock_shown(&handler_shown);
                    if let Some(ql_name) = entry_id.strip_prefix(ROOT_QUICKLINK_PREFIX) {
                        if let Some(link) = ctx
                            .features
                            .search_quicklinks("")
                            .into_iter()
                            .find(|l| l.name.eq_ignore_ascii_case(ql_name))
                        {
                            shown.visible = true;
                            return enter_quicklink_arg(link, true, String::new(), &mut shown);
                        }
                        return PaletteReply::Keep;
                    }
                    if is_power_tool_command(&entry_id) {
                        let actually_visible = lock_or_recover(&handler_handle)
                            .map(tc_ui::is_visible)
                            .unwrap_or(shown.visible);
                        match entry_id.as_str() {
                            CMD_COLOR_HISTORY => {
                                if actually_visible && shown.mode == Mode::ColorHistory {
                                    shown.visible = false;
                                    shown.last_dismissed_at = Some(Instant::now());
                                    drop(shown);
                                    return close_reply();
                                }
                                shown.visible = true;
                                return enter_color_history(ctx, &mut shown, None);
                            }
                            CMD_SCREEN_OCR => {
                                let delay = if actually_visible { 110 } else { 0 };
                                shown.visible = false;
                                shown.last_dismissed_at = Some(Instant::now());
                                drop(shown);
                                if let Some(handle) = *lock_or_recover(&handler_handle) {
                                    tools::launch_screen_ocr(
                                        handle,
                                        Arc::clone(&ctx.settings),
                                        Arc::clone(&ctx.tools),
                                        delay,
                                    );
                                }
                                return close_reply();
                            }
                            CMD_COLOR_PICKER => {
                                let delay = if actually_visible { 110 } else { 0 };
                                shown.visible = false;
                                shown.last_dismissed_at = Some(Instant::now());
                                drop(shown);
                                if let Some(handle) = *lock_or_recover(&handler_handle) {
                                    tools::launch_color_picker(
                                        handle,
                                        Arc::clone(&ctx.features),
                                        Arc::clone(&ctx.settings),
                                        Arc::clone(&ctx.tools),
                                        delay,
                                    );
                                }
                                return close_reply();
                            }
                            _ => return PaletteReply::Keep,
                        }
                    }
                    if let Some(cmd) = LauncherState::command_of(&entry_id) {
                        // Pressing an action's global shortcut while YAL is already showing
                        // that exact screen toggles it closed cleanly (just like Raycast).
                        // Check live Win32 visibility so stale state can never prevent opening.
                        let actually_visible = lock_or_recover(&handler_handle)
                            .map(tc_ui::is_visible)
                            .unwrap_or(shown.visible);
                        let already_on_target = actually_visible
                            && match cmd {
                                CommandId::ClipboardHistory => shown.mode == Mode::Clipboard,
                                CommandId::SearchEmoji => shown.mode == Mode::Emoji,
                                CommandId::SearchSnippets => shown.mode == Mode::Snippets,
                                CommandId::SearchFiles => shown.mode == Mode::Files,
                                CommandId::SwitchWindows => shown.mode == Mode::SwitchWindows,
                                CommandId::ShowNotes | CommandId::SearchNotes => {
                                    shown.mode == Mode::Notes
                                }
                                CommandId::SearchQuicklinks => shown.mode == Mode::Quicklinks,
                                CommandId::CalculatorHistory => shown.mode == Mode::CalcHistory,
                                CommandId::Settings => shown.mode == Mode::Settings,
                                _ => false,
                            };
                        if already_on_target {
                            shown.visible = false;
                            shown.last_dismissed_at = Some(Instant::now());
                            drop(shown);
                            return close_reply();
                        }
                        shown.visible = true;
                        return match cmd {
                            CommandId::ClipboardHistory => enter_clipboard(ctx, &mut shown),
                            CommandId::PasteSequentially => {
                                shown.last_dismissed_at = Some(Instant::now());
                                drop(shown);
                                paste_next_from_queue(ctx, &close_with_hud)
                            }
                            CommandId::SearchEmoji => enter_emoji(ctx, &mut shown),
                            CommandId::SearchSnippets => enter_snippets(ctx, &mut shown),
                            CommandId::CreateSnippet => enter_create_snippet(&mut shown),
                            CommandId::ShowNotes | CommandId::SearchNotes => {
                                enter_notes(ctx, &mut shown)
                            }
                            CommandId::CreateNote => enter_create_note(&mut shown),
                            CommandId::SearchQuicklinks => enter_quicklinks(ctx, &mut shown),
                            CommandId::CreateQuicklink => enter_create_quicklink(&mut shown),
                            CommandId::RunShellCommand => enter_run_shell(&mut shown),
                            CommandId::ExportSettings => {
                                shown.mode = Mode::Status;
                                shown.query.clear();
                                shown.selected = 0;
                                shown.action_menu_open = false;
                                drop(shown);
                                run_export_backup(ctx)
                            }
                            CommandId::ImportSettings => {
                                shown.last_dismissed_at = Some(Instant::now());
                                drop(shown);
                                spawn_import_backup_dialog(
                                    Arc::clone(ctx),
                                    Arc::clone(&handler_launcher),
                                    Arc::clone(&handler_shown),
                                    Arc::clone(&handler_handle),
                                );
                                close_reply()
                            }
                            CommandId::ImportFromRaycast => {
                                shown.last_dismissed_at = Some(Instant::now());
                                drop(shown);
                                spawn_import_raycast_dialog(
                                    Arc::clone(ctx),
                                    Arc::clone(&handler_shown),
                                    Arc::clone(&handler_handle),
                                );
                                close_reply()
                            }
                            CommandId::SearchFiles => {
                                enter_files(&handler_launcher, ctx, &handler_handle, &mut shown)
                            }
                            CommandId::SwitchWindows => enter_switch_windows(&mut shown),
                            CommandId::CalculatorHistory => {
                                enter_calc_history(ctx, &handler_launcher, &mut shown)
                            }
                            CommandId::Settings => enter_settings(ctx, &mut shown),
                            CommandId::About => enter_about(&handler_launcher, &mut shown),
                            CommandId::Quit => PaletteReply::Quit,
                            _ => enter_launcher(&handler_launcher, Some(ctx), &mut shown, true),
                        };
                    }
                    PaletteReply::Keep
                }
                PaletteEvent::Command(_) => PaletteReply::Keep,
                PaletteEvent::QueryChanged(query) => {
                    let mut shown = lock_shown(&handler_shown);
                    if matches!(shown.mode, Mode::EditClipboard { .. }) {
                        return PaletteReply::Keep;
                    }
                    shown.query = query.clone();
                    shown.selected = 0;
                    shown.action_menu_open = false;
                    match shown.mode.clone() {
                        Mode::Launcher => {
                            let is_suggestions = query.trim().is_empty();
                            if is_suggestions {
                                shown.compact_expanded = false;
                            }
                            let rows = if is_suggestions {
                                suggestions_with_favorites(
                                    &handler_launcher,
                                    handler_context.as_deref(),
                                )
                            } else {
                                build_root_search_rows(
                                    &handler_launcher,
                                    handler_context.as_deref(),
                                    &query,
                                )
                            };
                            shown.rows = rows.clone();
                            PaletteReply::Update(model_for_with_icons(
                                &query,
                                &rows,
                                is_suggestions,
                                0,
                                handler_context.as_deref(),
                                &mut shown,
                            ))
                        }
                        Mode::Clipboard => {
                            let Some(ctx) = handler_context.as_ref() else {
                                return PaletteReply::Keep;
                            };
                            let filter = shown.clipboard_filter;
                            let items = ctx.features.search_clipboard_view(&query, filter);
                            shown.clipboard_items = items.clone();
                            PaletteReply::Update(build_clipboard_screen_model(
                                ctx, &mut shown, &query, &items, 0,
                            ))
                        }
                        Mode::EditClipboard { .. } => PaletteReply::Keep,
                        Mode::Emoji => {
                            let Some(ctx) = handler_context.as_ref() else {
                                return PaletteReply::Keep;
                            };
                            let cells = ctx.features.emoji_cells(&query);
                            shown.emoji_cells = cells.clone();
                            PaletteReply::Update(emoji_model(&query, cells, 0))
                        }
                        Mode::Snippets => {
                            let Some(ctx) = handler_context.as_ref() else {
                                return PaletteReply::Keep;
                            };
                            let list = ctx.features.search_snippets(&query);
                            shown.snippets = list.clone();
                            PaletteReply::Update(snippets_model(
                                &query,
                                &list,
                                0,
                                shown.show_detail,
                                |rec| ctx.features.expand_snippet(rec),
                            ))
                        }
                        Mode::CreateSnippet(draft) => {
                            PaletteReply::Update(create_snippet_model(&draft, &query))
                        }
                        Mode::Notes => {
                            let Some(ctx) = handler_context.as_ref() else {
                                return PaletteReply::Keep;
                            };
                            let list = ctx.features.search_notes(&query);
                            shown.notes = list.clone();
                            PaletteReply::Update(notes_model(&query, &list, 0, shown.show_detail))
                        }
                        Mode::CreateNote(draft) => {
                            PaletteReply::Update(create_note_model(&draft, &query))
                        }
                        Mode::Quicklinks => {
                            let Some(ctx) = handler_context.as_ref() else {
                                return PaletteReply::Keep;
                            };
                            let list = ctx.features.search_quicklinks(&query);
                            shown.quicklinks = list.clone();
                            PaletteReply::Update(quicklinks_model(
                                &query,
                                &list,
                                0,
                                shown.show_detail,
                            ))
                        }
                        Mode::QuicklinkArg { link, .. } => {
                            PaletteReply::Update(quicklink_arg_model(&link, &query))
                        }
                        Mode::SetAlias {
                            title,
                            current_alias,
                            ..
                        } => PaletteReply::Update(set_alias_model(
                            &title,
                            current_alias.as_deref(),
                            &query,
                        )),
                        Mode::CreateQuicklink(draft) => {
                            PaletteReply::Update(create_quicklink_model(&draft, &query))
                        }
                        Mode::RunShell => {
                            let last = shown.last_shell_output.clone();
                            PaletteReply::Update(run_shell_model(&query, last.as_deref()))
                        }
                        Mode::Status => PaletteReply::Keep,
                        Mode::Files => {
                            let filter = shown.file_filter;
                            let current_dir = shown.file_dir.clone();
                            let rows = shown.filtered_file_rows(&handler_launcher, &query);
                            let indexing = handler_context
                                .as_ref()
                                .is_some_and(|c| c.features.is_file_indexing());
                            let show_detail = shown.show_detail;
                            shown.file_rows = rows.clone();
                            PaletteReply::Update(files_model(
                                &query,
                                &rows,
                                filter,
                                current_dir.as_deref(),
                                indexing,
                                0,
                                show_detail,
                                |target| shown.lookup_or_queue_icon(target),
                            ))
                        }
                        Mode::SwitchWindows => {
                            let filtered = filter_windows(&query);
                            shown.windows = filtered.clone();
                            PaletteReply::Update(windows_model(&query, &filtered, 0))
                        }
                        Mode::CalcHistory => {
                            let Some(ctx) = handler_context.as_ref() else {
                                return PaletteReply::Keep;
                            };
                            let live = handler_launcher.evaluate_calc(&query);
                            let entries = ctx.features.search_calc_history(&query);
                            shown.calc_live = live.clone();
                            shown.calc_entries = entries.clone();
                            PaletteReply::Update(calc_history_model(
                                &query,
                                live.as_ref(),
                                &entries,
                                0,
                                shown.show_detail,
                            ))
                        }
                        Mode::ColorHistory => {
                            let Some(ctx) = handler_context.as_ref() else {
                                return PaletteReply::Keep;
                            };
                            let entries = ctx.features.color_history_items(&query);
                            shown.color_entries = entries.clone();
                            let fmt = tools::configured_color_format(&*ctx.settings);
                            PaletteReply::Update(color_history_model(
                                &query,
                                &entries,
                                0,
                                shown.show_detail,
                                fmt,
                            ))
                        }
                        Mode::OcrPreview => PaletteReply::Keep,
                        Mode::Settings => {
                            let Some(ctx) = handler_context.as_ref() else {
                                return PaletteReply::Keep;
                            };
                            let rows = collect_settings_rows(ctx, &query);
                            shown.settings_rows = rows.clone();
                            PaletteReply::Update(settings_model_for(&query, &rows, 0))
                        }
                    }
                }
                PaletteEvent::Activated(index) => handle_activate(
                    index,
                    false,
                    &handler_shown,
                    &handler_launcher,
                    &handler_runner,
                    handler_context.as_ref(),
                    &handler_handle,
                    &close_reply,
                    &close_with_hud,
                ),
                PaletteEvent::QuickSelect(index) => handle_activate(
                    index,
                    false,
                    &handler_shown,
                    &handler_launcher,
                    &handler_runner,
                    handler_context.as_ref(),
                    &handler_handle,
                    &close_reply,
                    &close_with_hud,
                ),
                PaletteEvent::SecondaryActivated(index) => handle_activate(
                    index,
                    true,
                    &handler_shown,
                    &handler_launcher,
                    &handler_runner,
                    handler_context.as_ref(),
                    &handler_handle,
                    &close_reply,
                    &close_with_hud,
                ),
                PaletteEvent::TertiaryActivated(index) => {
                    handle_tertiary(index, &handler_shown, &close_with_hud)
                }
                PaletteEvent::QuaternaryActivated(index) => {
                    handle_quaternary(index, &handler_shown, &close_with_hud)
                }
                PaletteEvent::CopyChord(index) => handle_copy_chord(
                    index,
                    &handler_shown,
                    &handler_launcher,
                    handler_context.as_deref(),
                    &close_with_hud,
                ),
                PaletteEvent::CopySelection(text) => {
                    if text.is_empty() {
                        return PaletteReply::Keep;
                    }
                    let dark = handler_context
                        .as_deref()
                        .map(|c| tools::settings_prefers_dark(&*c.settings))
                        .unwrap_or_else(tc_platform_win::apps_use_dark_theme);
                    if let Some(ctx) = handler_context.as_deref() {
                        ctx.hotkeys.suppress_clipboard_for_ms(600);
                    }
                    match tc_platform_win::clipboard().write_text(&text) {
                        Ok(()) => tc_ui::show_hud(
                            format!("Copied selection ({} chars)", text.chars().count()),
                            dark,
                        ),
                        Err(_) => tc_ui::show_hud("Failed to copy selection", dark),
                    }
                    PaletteReply::Keep
                }
                PaletteEvent::EditorSubmit { text, as_new } => {
                    handle_editor_submit(text, as_new, &handler_shown, handler_context.as_deref())
                }
                PaletteEvent::Action(action, selected) => handle_action(
                    action,
                    selected,
                    &handler_shown,
                    &handler_launcher,
                    handler_context.as_deref(),
                    &handler_handle,
                    &close_with_hud,
                ),
                PaletteEvent::ActionMenuDismissed => {
                    let mut shown = lock_shown(&handler_shown);
                    shown.action_menu_open = false;
                    PaletteReply::Keep
                }
                PaletteEvent::ActionMenuActivated(action_id) => handle_action_menu_choice(
                    &action_id,
                    &handler_shown,
                    &handler_launcher,
                    &handler_runner,
                    handler_context.as_ref(),
                    &handler_handle,
                    &close_reply,
                    &close_with_hud,
                ),
                PaletteEvent::Escape => handle_escape_or_back(
                    &handler_shown,
                    &handler_launcher,
                    handler_context.as_deref(),
                    true,
                    &close_reply,
                ),
                PaletteEvent::WindowMoved(origin) => {
                    if let Some(ctx) = handler_context.as_ref() {
                        match origin {
                            Some((x, y)) => {
                                let _ = ctx.settings.set(KEY_POSITION_X, &x.to_string());
                                let _ = ctx.settings.set(KEY_POSITION_Y, &y.to_string());
                            }
                            None => {
                                let _ = ctx.settings.set(KEY_POSITION_X, "");
                                let _ = ctx.settings.set(KEY_POSITION_Y, "");
                            }
                        }
                        let mut shown = lock_shown(&handler_shown);
                        if shown.mode == Mode::Settings {
                            let q = shown.query.clone();
                            let updated = collect_settings_rows(ctx, &q);
                            let clamped = shown.selected.min(updated.len().saturating_sub(1));
                            shown.settings_rows = updated.clone();
                            shown.selected = clamped;
                            let mut model = settings_model_for(&q, &updated, clamped);
                            if origin.is_none() {
                                model.toast = Some(ToastModel {
                                    kind: ToastKind::Info,
                                    message: "Window centered".to_string(),
                                });
                            }
                            return PaletteReply::Update(model);
                        }
                    }
                    PaletteReply::Keep
                }
                PaletteEvent::Dismissed => {
                    let mut shown = lock_shown(&handler_shown);
                    shown.visible = false;
                    shown.last_dismissed_at = Some(Instant::now());
                    shown.action_menu_open = false;
                    shown.compact_expanded = false;
                    if shown.recording_hotkey.take().is_some() {
                        if let Some(ctx) = handler_context.as_ref() {
                            ctx.hotkeys.cancel_capture();
                        }
                    }
                    if quit_on_close {
                        PaletteReply::Quit
                    } else {
                        PaletteReply::Keep
                    }
                }
            }
        })();
        if matches!(
            reply,
            PaletteReply::Close | PaletteReply::CloseWithHud(_) | PaletteReply::Quit
        ) {
            let mut shown = lock_shown(&handler_shown);
            shown.visible = false;
            shown.last_dismissed_at = Some(Instant::now());
            shown.action_menu_open = false;
            shown.compact_expanded = false;
            if shown.recording_hotkey.take().is_some() {
                if let Some(ctx) = handler_context.as_ref() {
                    ctx.hotkeys.cancel_capture();
                }
            }
        }
        reply
    };

    let rows = suggestions_with_favorites(&launcher, context.as_deref());
    let initial_model = {
        let mut guard = lock_shown(&shown);
        guard.rows = rows.clone();
        model_for_with_icons("", &rows, true, 0, context.as_deref(), &mut guard)
    };

    let palette = Palette::with_handler(theme, initial_model, Box::new(handler))?;
    if let Some(ctx) = context.as_ref() {
        let saved = ctx
            .settings
            .get(KEY_POSITION_X)
            .and_then(|s| s.trim().parse::<i32>().ok())
            .zip(
                ctx.settings
                    .get(KEY_POSITION_Y)
                    .and_then(|s| s.trim().parse::<i32>().ok()),
            );
        if saved.is_some() {
            palette.set_saved_position(saved);
        }
    }
    if let Ok(mut slot) = handle_slot.lock() {
        *slot = Some(palette.handle());
    }
    Ok(palette)
}

fn pop_to_root_policy(settings: &WindowsSettings) -> PopToRootPolicy {
    let raw = settings
        .get(KEY_POP_TO_ROOT_SECS)
        .or_else(|| settings.get(tc_palette::KEY_POP_TO_ROOT));
    PopToRootPolicy::parse(raw.as_deref())
}

fn handle_escape_or_back(
    shown_lock: &Arc<Mutex<Shown>>,
    launcher: &Arc<LauncherState>,
    ctx: Option<&NativeContext>,
    close_at_root: bool,
    close_reply: &dyn Fn() -> PaletteReply,
) -> PaletteReply {
    let mut shown = lock_shown(shown_lock);
    shown.action_menu_open = false;
    if shown.recording_hotkey.take().is_some() {
        if let Some(ctx) = ctx {
            ctx.hotkeys.cancel_capture();
        }
        let toast = ToastModel {
            kind: ToastKind::Info,
            message: "Hotkey recording cancelled".to_string(),
        };
        return match rebuild_current_model(ctx, &mut shown, Some(toast)) {
            Some(model) => PaletteReply::Update(model),
            None => PaletteReply::Keep,
        };
    }
    match shown.mode.clone() {
        Mode::Launcher => {
            if close_at_root {
                shown.visible = false;
                shown.last_dismissed_at = Some(Instant::now());
                close_reply()
            } else {
                PaletteReply::Keep
            }
        }
        Mode::Files => {
            if let Some(dir) = shown.file_dir.clone() {
                let next_dir = dir
                    .parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .map(Path::to_path_buf);
                shown.file_dir = next_dir.clone();
                shown.file_dir_cache = None;
                shown.query.clear();
                shown.selected = 0;
                let filter = shown.file_filter;
                let rows = shown.filtered_file_rows(launcher, "");
                let indexing = ctx.is_some_and(|c| c.features.is_file_indexing());
                let show_detail = shown.show_detail;
                shown.file_rows = rows.clone();
                let model = files_model(
                    "",
                    &rows,
                    filter,
                    next_dir.as_deref(),
                    indexing,
                    0,
                    show_detail,
                    |target| shown.lookup_or_queue_icon(target),
                );
                return PaletteReply::UpdateWithEdit(String::new(), model);
            }
            if let Some(ctx) = ctx {
                ctx.palette.back_to_root();
            }
            enter_launcher(launcher, ctx, &mut shown, true)
        }
        Mode::SetAlias { previous_query, .. } => {
            restore_launcher_query(launcher, ctx, &mut shown, previous_query)
        }
        Mode::CreateSnippet(_) => {
            if let Some(ctx) = ctx {
                enter_snippets(ctx, &mut shown)
            } else {
                enter_launcher(launcher, ctx, &mut shown, true)
            }
        }
        Mode::CreateNote(_) => {
            if let Some(ctx) = ctx {
                enter_notes(ctx, &mut shown)
            } else {
                enter_launcher(launcher, ctx, &mut shown, true)
            }
        }
        Mode::QuicklinkArg {
            from_launcher,
            previous_query,
            ..
        } => {
            if from_launcher {
                restore_launcher_query(launcher, ctx, &mut shown, previous_query)
            } else if let Some(ctx) = ctx {
                enter_quicklinks(ctx, &mut shown)
            } else {
                enter_launcher(launcher, ctx, &mut shown, true)
            }
        }
        Mode::CreateQuicklink(_) => {
            if let Some(ctx) = ctx {
                enter_quicklinks(ctx, &mut shown)
            } else {
                enter_launcher(launcher, ctx, &mut shown, true)
            }
        }
        Mode::EditClipboard {
            previous_query,
            previous_selected,
            ..
        } => {
            if let Some(ctx) = ctx {
                shown.mode = Mode::Clipboard;
                shown.action_menu_open = false;
                shown.query = previous_query.clone();
                let filter = shown.clipboard_filter;
                let items = ctx.features.search_clipboard_view(&previous_query, filter);
                let idx = previous_selected.min(items.len().saturating_sub(1));
                shown.clipboard_items = items.clone();
                shown.selected = idx;
                let mut model =
                    build_clipboard_screen_model(ctx, &mut shown, &previous_query, &items, idx);
                apply_recording_overlay(&mut model, &shown);
                PaletteReply::UpdateWithEdit(previous_query, model)
            } else {
                enter_launcher(launcher, ctx, &mut shown, true)
            }
        }
        _ => {
            if let Some(ctx) = ctx {
                ctx.palette.back_to_root();
            }
            enter_launcher(launcher, ctx, &mut shown, true)
        }
    }
}

fn restore_launcher_query(
    launcher: &LauncherState,
    ctx: Option<&NativeContext>,
    shown: &mut Shown,
    query: String,
) -> PaletteReply {
    shown.mode = Mode::Launcher;
    shown.query = query.clone();
    shown.selected = 0;
    shown.action_menu_open = false;
    let is_suggestions = query.trim().is_empty();
    let rows = if is_suggestions {
        suggestions_with_favorites(launcher, ctx)
    } else {
        build_root_search_rows(launcher, ctx, &query)
    };
    shown.rows = rows.clone();
    let model = model_for_with_icons(&query, &rows, is_suggestions, 0, ctx, shown);
    PaletteReply::UpdateWithEdit(query, model)
}

#[allow(clippy::too_many_arguments)]
fn handle_activate(
    index: usize,
    secondary: bool,
    shown_lock: &Arc<Mutex<Shown>>,
    launcher: &Arc<LauncherState>,
    runner: &Arc<Runner>,
    ctx_arc: Option<&Arc<NativeContext>>,
    handle_slot: &Arc<Mutex<Option<PaletteHandle>>>,
    close_reply: &dyn Fn() -> PaletteReply,
    close_with_hud: &dyn Fn(String) -> PaletteReply,
) -> PaletteReply {
    let ctx = ctx_arc.map(Arc::as_ref);
    let mut shown = lock_shown(shown_lock);
    shown.action_menu_open = false;
    match shown.mode.clone() {
        Mode::Launcher => {
            if shown.query.trim().is_empty()
                && !shown.compact_expanded
                && is_compact_window_mode(ctx)
            {
                return PaletteReply::Keep;
            }
            let Some(row) = shown.rows.get(index).cloned() else {
                return PaletteReply::Keep;
            };

            // 1. Inline / Root Search snippet -> expand & paste (or copy on Ctrl+Enter).
            if let Some(snippet_id) = row.id.strip_prefix(INLINE_SNIPPET_PREFIX) {
                if let Some(ctx) = ctx {
                    let record = ctx
                        .features
                        .snippets
                        .lock()
                        .ok()
                        .and_then(|g| g.record(snippet_id).cloned());
                    if let Some(record) = record {
                        let expanded = ctx.features.expand_snippet(&record);
                        let name = record.snippet.name.clone();
                        shown.last_dismissed_at = Some(Instant::now());
                        drop(shown);
                        if secondary {
                            let _ = tc_platform_win::clipboard().write_text(&expanded);
                            return close_with_hud(format!("Copied {name}"));
                        }
                        let _ = tc_platform_win::copy_and_paste_async(expanded);
                        return close_with_hud(format!("Pasted {name}"));
                    }
                }
                return PaletteReply::Keep;
            }

            // 1b. Root Search Quicklink -> inline argument mode if parameterized, else open!
            if let Some(ql_name) = row.id.strip_prefix(ROOT_QUICKLINK_PREFIX) {
                if let Some(ctx) = ctx {
                    let link = ctx
                        .features
                        .search_quicklinks("")
                        .into_iter()
                        .find(|l| l.name == ql_name);
                    if let Some(link) = link {
                        if link.needs_query() {
                            return enter_quicklink_arg(
                                link,
                                true,
                                shown.query.clone(),
                                &mut shown,
                            );
                        }
                        let url = link.expand_url("");
                        let name = link.name;
                        shown.last_dismissed_at = Some(Instant::now());
                        drop(shown);
                        let _ = tc_platform_win::open(&url);
                        return close_with_hud(format!("Opened {name}"));
                    }
                }
                return PaletteReply::Keep;
            }

            // 1c. Windows Settings & Admin Tool -> launch ms-settings: URI or control applet.
            if let Some(setting_id) = row.id.strip_prefix(WIN_SETTING_PREFIX) {
                let target = win_settings::find_by_id(setting_id)
                    .map(|item| item.target)
                    .unwrap_or(&row.target)
                    .to_string();
                let name = row.name.clone();
                shown.last_dismissed_at = Some(Instant::now());
                drop(shown);
                if secondary {
                    let _ = tc_platform_win::clipboard().write_text(&target);
                    return close_with_hud("Copied URI".to_string());
                }
                let _ = win_settings::launch(&target);
                return close_with_hud(format!("Opened {name}"));
            }

            // 1d. Fallback command (`Use “<query>” with…` -> Search Files / Search Snippets pre-filled!).
            if let Some(cmd_key) = row.id.strip_prefix(FALLBACK_CMD_PREFIX) {
                let prefill = row.target.clone();
                if let Some(ctx) = ctx {
                    match cmd_key {
                        "search-files" => {
                            ctx.palette.open_screen("File Search");
                            shown.mode = Mode::Files;
                            shown.query = prefill.clone();
                            shown.selected = 0;
                            shown.action_menu_open = false;
                            shown.file_filter = FileType::All;
                            shown.file_dir = None;
                            shown.file_dir_cache = None;
                            if !launcher.has_file_search() {
                                let _ = ctx.settings.set(FILE_SEARCH_ENABLED, "true");
                                ctx.features.trigger_file_index(
                                    Arc::clone(launcher),
                                    Some(Arc::clone(handle_slot)),
                                );
                            }
                            let rows = shown.filtered_file_rows(launcher, &prefill);
                            let indexing = ctx.features.is_file_indexing();
                            let show_detail = shown.show_detail;
                            shown.file_rows = rows.clone();
                            let model = files_model(
                                &prefill,
                                &rows,
                                FileType::All,
                                None,
                                indexing,
                                0,
                                show_detail,
                                |target| shown.lookup_or_queue_icon(target),
                            );
                            return PaletteReply::UpdateWithEdit(prefill, model);
                        }
                        "search-snippets" => {
                            ctx.palette.open_screen("Snippets");
                            shown.mode = Mode::Snippets;
                            shown.query = prefill.clone();
                            shown.selected = 0;
                            shown.action_menu_open = false;
                            let list = ctx.features.search_snippets(&prefill);
                            shown.snippets = list.clone();
                            let model =
                                snippets_model(&prefill, &list, 0, shown.show_detail, |rec| {
                                    ctx.features.expand_snippet(rec)
                                });
                            return PaletteReply::UpdateWithEdit(prefill, model);
                        }
                        _ => {}
                    }
                }
                return PaletteReply::Keep;
            }

            // 1e. Fallback web search URL -> open in default browser immediately.
            if let Some(url) = row.id.strip_prefix(FALLBACK_URL_PREFIX) {
                let url = url.to_string();
                shown.last_dismissed_at = Some(Instant::now());
                drop(shown);
                if secondary {
                    let _ = tc_platform_win::clipboard().write_text(&url);
                    return close_with_hud("Copied URL".to_string());
                }
                let _ = tc_platform_win::open(&url);
                return close_with_hud("Opened in Browser".to_string());
            }

            // 1f. Native Screen OCR, Color Picker, and Color Inspector power tools.
            if is_power_tool_command(&row.id) {
                launcher.remember(&row.id, &shown.query);
                match row.id.as_str() {
                    CMD_SCREEN_OCR => {
                        if let Some(ctx_shared) = ctx_arc {
                            shown.visible = false;
                            shown.last_dismissed_at = Some(Instant::now());
                            drop(shown);
                            if let Some(handle) = *lock_or_recover(handle_slot) {
                                tools::launch_screen_ocr(
                                    handle,
                                    Arc::clone(&ctx_shared.settings),
                                    Arc::clone(&ctx_shared.tools),
                                    110,
                                );
                            }
                            return close_reply();
                        }
                        return PaletteReply::Keep;
                    }
                    CMD_COLOR_PICKER => {
                        if let Some(ctx_shared) = ctx_arc {
                            shown.visible = false;
                            shown.last_dismissed_at = Some(Instant::now());
                            drop(shown);
                            if let Some(handle) = *lock_or_recover(handle_slot) {
                                tools::launch_color_picker(
                                    handle,
                                    Arc::clone(&ctx_shared.features),
                                    Arc::clone(&ctx_shared.settings),
                                    Arc::clone(&ctx_shared.tools),
                                    110,
                                );
                            }
                            return close_reply();
                        }
                        return PaletteReply::Keep;
                    }
                    CMD_COLOR_HISTORY => {
                        if let Some(ctx) = ctx {
                            return enter_color_history(ctx, &mut shown, None);
                        }
                        return PaletteReply::Keep;
                    }
                    _ => {}
                }
            }

            // 2. Built-in screen commands navigate cleanly inside the same palette window!
            if let Some(cmd) = LauncherState::command_of(&row.id) {
                if let Some(ctx) = ctx {
                    match cmd {
                        CommandId::ClipboardHistory => return enter_clipboard(ctx, &mut shown),
                        CommandId::PasteSequentially => {
                            shown.last_dismissed_at = Some(Instant::now());
                            drop(shown);
                            return paste_next_from_queue(ctx, close_with_hud);
                        }
                        CommandId::SearchEmoji => return enter_emoji(ctx, &mut shown),
                        CommandId::SearchSnippets => return enter_snippets(ctx, &mut shown),
                        CommandId::CreateSnippet => return enter_create_snippet(&mut shown),
                        CommandId::ShowNotes | CommandId::SearchNotes => {
                            return enter_notes(ctx, &mut shown);
                        }
                        CommandId::CreateNote => return enter_create_note(&mut shown),
                        CommandId::SearchQuicklinks => return enter_quicklinks(ctx, &mut shown),
                        CommandId::CreateQuicklink => return enter_create_quicklink(&mut shown),
                        CommandId::RunShellCommand => return enter_run_shell(&mut shown),
                        CommandId::ExportSettings => {
                            shown.mode = Mode::Status;
                            shown.query.clear();
                            shown.selected = 0;
                            shown.action_menu_open = false;
                            drop(shown);
                            return run_export_backup(ctx);
                        }
                        CommandId::ImportSettings => {
                            if let Some(ctx_shared) = ctx_arc {
                                shown.last_dismissed_at = Some(Instant::now());
                                drop(shown);
                                spawn_import_backup_dialog(
                                    Arc::clone(ctx_shared),
                                    Arc::clone(launcher),
                                    Arc::clone(shown_lock),
                                    Arc::clone(handle_slot),
                                );
                                return close_reply();
                            }
                            return PaletteReply::Keep;
                        }
                        CommandId::ImportFromRaycast => {
                            if let Some(ctx_shared) = ctx_arc {
                                shown.last_dismissed_at = Some(Instant::now());
                                drop(shown);
                                spawn_import_raycast_dialog(
                                    Arc::clone(ctx_shared),
                                    Arc::clone(shown_lock),
                                    Arc::clone(handle_slot),
                                );
                                return close_reply();
                            }
                            return PaletteReply::Keep;
                        }
                        CommandId::About => return enter_about(launcher, &mut shown),
                        CommandId::SearchFiles => {
                            return enter_files(launcher, ctx, handle_slot, &mut shown);
                        }
                        CommandId::SwitchWindows => return enter_switch_windows(&mut shown),
                        CommandId::CalculatorHistory => {
                            return enter_calc_history(ctx, launcher, &mut shown);
                        }
                        CommandId::Settings => return enter_settings(ctx, &mut shown),
                        _ => {}
                    }
                }
            }

            // 3. Calculator answer -> remember in persistent Calculator History AND update `ans`!
            if row.id.starts_with(CALCULATOR_PREFIX) {
                if secondary {
                    let expr = row.detail.clone();
                    shown.last_dismissed_at = Some(Instant::now());
                    drop(shown);
                    let _ = tc_platform_win::clipboard().write_text(&expr);
                    return close_with_hud("Copied Expression".to_string());
                }
                if let Some(ctx) = ctx {
                    let badge = row.calc_badge.as_deref().unwrap_or("Result");
                    ctx.features
                        .record_calc(&row.detail, &row.name, &row.target, badge);
                }
                if let Some(val) = parse_numeric_answer(&row.target) {
                    launcher.set_last_calc_answer(Some(val));
                }
                let answer = row.target.clone();
                let display = row.name.clone();
                shown.last_dismissed_at = Some(Instant::now());
                drop(shown);
                let _ = tc_platform_win::clipboard().write_text(&answer);
                return close_with_hud(format!("Copied {display}"));
            }

            // 4. Secondary action (`Ctrl+Enter`) on a File or App row -> Reveal in File Explorer.
            if secondary {
                if let Some(path) = row.id.strip_prefix(FILE_PREFIX) {
                    let path = path.to_string();
                    shown.last_dismissed_at = Some(Instant::now());
                    drop(shown);
                    let _ = tc_platform_win::reveal(&path);
                    return close_with_hud("Opened in Explorer".to_string());
                }
            }

            let is_window_cmd = LauncherState::window_command_of(&row.id).is_some();
            let id = row.id.clone();
            let name = row.name.clone();
            let query = shown.query.clone();
            shown.last_dismissed_at = Some(Instant::now());
            drop(shown);
            match runner.open_with_feedback(&id, &query, false) {
                Ok(Some(feedback)) => close_with_hud(feedback),
                Ok(None) if is_window_cmd => close_with_hud(format!("Applied {name}")),
                _ => close_reply(),
            }
        }
        Mode::Clipboard => {
            let Some(ctx) = ctx else {
                return PaletteReply::Keep;
            };
            let Some(item) = shown.clipboard_items.get(index).cloned() else {
                return PaletteReply::Keep;
            };
            shown.last_dismissed_at = Some(Instant::now());
            drop(shown);
            execute_clipboard_item(ctx, &item, !secondary, close_with_hud)
        }
        Mode::EditClipboard { .. } => PaletteReply::Keep,
        Mode::Emoji => {
            let Some(ctx) = ctx else {
                return PaletteReply::Keep;
            };
            let Some(cell) = shown.emoji_cells.get(index).cloned() else {
                return PaletteReply::Keep;
            };
            ctx.features.record_emoji_use(&cell.glyph);
            let glyph = cell.glyph.clone();
            shown.last_dismissed_at = Some(Instant::now());
            drop(shown);
            if secondary {
                let _ = tc_platform_win::clipboard().write_text(&glyph);
                close_with_hud(format!("Copied {glyph}"))
            } else {
                let _ = tc_platform_win::copy_and_paste_async(glyph.clone());
                close_with_hud(format!("Pasted {glyph}"))
            }
        }
        Mode::Snippets => {
            let Some(ctx) = ctx else {
                return PaletteReply::Keep;
            };
            if index == shown.snippets.len() && shows_create_snippet_row(&shown.query) {
                return enter_create_snippet(&mut shown);
            }
            let Some(record) = shown.snippets.get(index).cloned() else {
                return PaletteReply::Keep;
            };
            let expanded = ctx.features.expand_snippet(&record);
            let name = record.snippet.name.clone();
            shown.last_dismissed_at = Some(Instant::now());
            drop(shown);
            if secondary {
                let _ = tc_platform_win::clipboard().write_text(&expanded);
                close_with_hud(format!("Copied {name}"))
            } else {
                let _ = tc_platform_win::copy_and_paste_async(expanded);
                close_with_hud(format!("Pasted {name}"))
            }
        }
        Mode::CreateSnippet(draft) => {
            let Some(ctx) = ctx else {
                return PaletteReply::Keep;
            };
            let mut next_draft = draft;
            let input = shown.query.clone();
            match next_draft.step {
                SnippetDraftStep::Name => {
                    let trimmed = input.trim();
                    if trimmed.is_empty() {
                        return PaletteReply::Keep;
                    }
                    next_draft.name = trimmed.to_string();
                    next_draft.step = SnippetDraftStep::Keyword;
                    shown.mode = Mode::CreateSnippet(next_draft.clone());
                    shown.query.clear();
                    PaletteReply::Navigate(create_snippet_model(&next_draft, ""))
                }
                SnippetDraftStep::Keyword => {
                    next_draft.keyword = input.trim().trim_start_matches('!').trim().to_string();
                    next_draft.step = SnippetDraftStep::Content;
                    shown.mode = Mode::CreateSnippet(next_draft.clone());
                    shown.query.clear();
                    PaletteReply::Navigate(create_snippet_model(&next_draft, ""))
                }
                SnippetDraftStep::Content => {
                    let body = if input.trim().is_empty() {
                        tc_platform_win::clipboard().read_text().unwrap_or_default()
                    } else {
                        input
                    };
                    if body.trim().is_empty() {
                        return PaletteReply::Keep;
                    }
                    let kw = if next_draft.keyword.is_empty() {
                        None
                    } else {
                        Some(next_draft.keyword.clone())
                    };
                    let _ = ctx.features.create_snippet(next_draft.name, kw, body);
                    ctx.hotkeys
                        .sync_snippet_keywords(&ctx.settings, Arc::clone(&ctx.features));
                    enter_snippets(ctx, &mut shown)
                }
            }
        }
        Mode::Notes => {
            if index == shown.notes.len() && shows_create_note_row(&shown.query) {
                return enter_create_note(&mut shown);
            }
            let Some(note) = shown.notes.get(index).cloned() else {
                return PaletteReply::Keep;
            };
            shown.last_dismissed_at = Some(Instant::now());
            drop(shown);
            if secondary {
                let _ = tc_platform_win::open(&note.path.to_string_lossy());
                close_with_hud("Opened Note".to_string())
            } else {
                let _ = tc_platform_win::clipboard().write_text(&note.body);
                close_with_hud("Copied Note".to_string())
            }
        }
        Mode::CreateNote(draft) => {
            let Some(ctx) = ctx else {
                return PaletteReply::Keep;
            };
            let mut next_draft = draft;
            let input = shown.query.clone();
            match next_draft.step {
                NoteDraftStep::Title => {
                    let trimmed = input.trim();
                    if trimmed.is_empty() {
                        return PaletteReply::Keep;
                    }
                    next_draft.title = trimmed.to_string();
                    next_draft.step = NoteDraftStep::Body;
                    shown.mode = Mode::CreateNote(next_draft.clone());
                    shown.query.clear();
                    PaletteReply::Navigate(create_note_model(&next_draft, ""))
                }
                NoteDraftStep::Body => {
                    let body = if input.trim().is_empty() {
                        tc_platform_win::clipboard().read_text().unwrap_or_default()
                    } else {
                        input
                    };
                    if body.trim().is_empty() {
                        return PaletteReply::Keep;
                    }
                    let _ = ctx.features.create_note(&next_draft.title, &body);
                    enter_notes(ctx, &mut shown)
                }
            }
        }
        Mode::Quicklinks => {
            if index == shown.quicklinks.len() && shows_create_quicklink_row(&shown.query) {
                return enter_create_quicklink(&mut shown);
            }
            let Some(link) = shown.quicklinks.get(index).cloned() else {
                return PaletteReply::Keep;
            };
            if link.needs_query() {
                return enter_quicklink_arg(link, false, shown.query.clone(), &mut shown);
            }
            let url = link.expand_url("");
            let name = link.name;
            shown.last_dismissed_at = Some(Instant::now());
            drop(shown);
            let _ = tc_platform_win::open(&url);
            close_with_hud(format!("Opened {name}"))
        }
        Mode::QuicklinkArg { link, .. } => {
            let url = link.expand_url(&shown.query);
            let name = link.name;
            shown.last_dismissed_at = Some(Instant::now());
            drop(shown);
            let _ = tc_platform_win::open(&url);
            close_with_hud(format!("Opened {name}"))
        }
        Mode::SetAlias {
            entry_id,
            title,
            previous_query,
            ..
        } => {
            let Some(ctx) = ctx else {
                return PaletteReply::Keep;
            };
            let trimmed = shown.query.trim().to_string();
            let value = if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.as_str())
            };
            let _ = hotkeys::set_alias(&*ctx.settings, &entry_id, value);
            let reply = restore_launcher_query(launcher, Some(ctx), &mut shown, previous_query);
            if let PaletteReply::UpdateWithEdit(q, mut model) = reply {
                model.toast = Some(ToastModel {
                    kind: ToastKind::Success,
                    message: if trimmed.is_empty() {
                        format!("Cleared alias for {title}")
                    } else {
                        format!("Alias `{trimmed}` set for {title}")
                    },
                });
                return PaletteReply::UpdateWithEdit(q, model);
            }
            reply
        }
        Mode::CreateQuicklink(draft) => {
            let Some(ctx) = ctx else {
                return PaletteReply::Keep;
            };
            let mut next_draft = draft;
            let input = shown.query.clone();
            match next_draft.step {
                QuicklinkDraftStep::Name => {
                    let trimmed = input.trim();
                    if trimmed.is_empty() {
                        return PaletteReply::Keep;
                    }
                    next_draft.name = trimmed.to_string();
                    next_draft.step = QuicklinkDraftStep::Url;
                    shown.mode = Mode::CreateQuicklink(next_draft.clone());
                    shown.query.clear();
                    PaletteReply::Navigate(create_quicklink_model(&next_draft, ""))
                }
                QuicklinkDraftStep::Url => {
                    let trimmed = input.trim();
                    if trimmed.is_empty() {
                        return PaletteReply::Keep;
                    }
                    ctx.features
                        .add_quicklink(next_draft.name, trimmed.to_string());
                    enter_quicklinks(ctx, &mut shown)
                }
            }
        }
        Mode::RunShell => {
            let cmd = shown.query.trim().to_string();
            if cmd.is_empty() {
                return PaletteReply::Keep;
            }
            let running_msg = format!("Running `{cmd}`…");
            shown.last_shell_output = Some(running_msg.clone());
            shown.query.clear();
            drop(shown);

            let shown_bg = Arc::clone(shown_lock);
            let handle_bg = Arc::clone(handle_slot);
            let _ = std::thread::Builder::new()
                .name("yal-shell-cmd".to_string())
                .spawn(move || {
                    let output = match FeatureStores::run_shell_command(&cmd) {
                        Ok(out) => {
                            let _ = tc_platform_win::clipboard().write_text(&out);
                            out
                        }
                        Err(err) => format!("Error: {err}"),
                    };
                    if let Ok(mut guard) = shown_bg.lock() {
                        guard.last_shell_output = Some(output);
                    }
                    if let Ok(guard) = handle_bg.lock() {
                        if let Some(handle) = *guard {
                            tc_ui::post_refresh(handle);
                        }
                    }
                });
            PaletteReply::Navigate(run_shell_model("", Some(&running_msg)))
        }
        Mode::Status => enter_launcher(launcher, ctx, &mut shown, true),
        Mode::Files => {
            let Some(row) = shown.file_rows.get(index).cloned() else {
                return PaletteReply::Keep;
            };
            let path = row.target.clone();
            shown.last_dismissed_at = Some(Instant::now());
            drop(shown);
            if secondary {
                let _ = tc_platform_win::reveal(&path);
                close_with_hud("Opened in Explorer".to_string())
            } else {
                let _ = tc_platform_win::open(&path);
                close_reply()
            }
        }
        Mode::SwitchWindows => {
            let Some(win) = shown.windows.get(index).cloned() else {
                return PaletteReply::Keep;
            };
            shown.last_dismissed_at = Some(Instant::now());
            drop(shown);
            let _ = tc_platform_win::window_control::focus_window(win.handle);
            close_reply()
        }
        Mode::CalcHistory => {
            // Index 0 may be a live `ans` calculation hero card.
            if let Some(live) = shown.calc_live.clone() {
                if index == 0 {
                    if let Some(ctx) = ctx {
                        let badge = live.calc_badge.as_deref().unwrap_or("Result");
                        ctx.features
                            .record_calc(&live.detail, &live.name, &live.target, badge);
                    }
                    if let Some(val) = parse_numeric_answer(&live.target) {
                        launcher.set_last_calc_answer(Some(val));
                    }
                    shown.last_dismissed_at = Some(Instant::now());
                    drop(shown);
                    if secondary {
                        let _ = tc_platform_win::copy_and_paste_async(live.target);
                        return close_with_hud(format!("Pasted {}", live.name));
                    }
                    let _ = tc_platform_win::clipboard().write_text(&live.target);
                    return close_with_hud(format!("Copied {}", live.name));
                }
            }
            let entry_idx = if shown.calc_live.is_some() {
                index.saturating_sub(1)
            } else {
                index
            };
            let Some(entry) = shown.calc_entries.get(entry_idx).cloned() else {
                return PaletteReply::Keep;
            };
            if let Some(val) = parse_numeric_answer(&entry.copy_text) {
                launcher.set_last_calc_answer(Some(val));
            }
            shown.last_dismissed_at = Some(Instant::now());
            drop(shown);
            if secondary {
                let _ = tc_platform_win::copy_and_paste_async(entry.copy_text);
                close_with_hud(format!("Pasted {}", entry.display))
            } else {
                let _ = tc_platform_win::clipboard().write_text(&entry.copy_text);
                close_with_hud(format!("Copied {}", entry.display))
            }
        }
        Mode::ColorHistory => {
            let Some(ctx) = ctx else {
                return PaletteReply::Keep;
            };
            if shown.color_entries.is_empty() && shown.query.trim().is_empty() {
                if let Some(ctx_shared) = ctx_arc {
                    shown.visible = false;
                    shown.last_dismissed_at = Some(Instant::now());
                    drop(shown);
                    if let Some(handle) = *lock_or_recover(handle_slot) {
                        tools::launch_color_picker(
                            handle,
                            Arc::clone(&ctx_shared.features),
                            Arc::clone(&ctx_shared.settings),
                            Arc::clone(&ctx_shared.tools),
                            110,
                        );
                    }
                    return close_reply();
                }
                return PaletteReply::Keep;
            }
            let Some(entry) = shown.color_entries.get(index).cloned() else {
                return PaletteReply::Keep;
            };
            let fmt = tools::configured_color_format(&*ctx.settings);
            let fmt = if secondary { fmt.next() } else { fmt };
            let formatted = fmt.format(tc_clipboard::color_from_rgb(entry.r, entry.g, entry.b));
            shown.last_dismissed_at = Some(Instant::now());
            drop(shown);
            let _ = tc_platform_win::clipboard().write_text(&formatted);
            close_with_hud(format!("Copied {formatted}"))
        }
        Mode::OcrPreview => match index {
            0 => {
                let Some(state) = shown.ocr_preview.clone() else {
                    return PaletteReply::Keep;
                };
                if state.text.trim().is_empty() {
                    return PaletteReply::Keep;
                }
                shown.last_dismissed_at = Some(Instant::now());
                drop(shown);
                let _ = tc_platform_win::clipboard().write_text(&state.text);
                close_with_hud(format!(
                    "Copied {} chars ({})",
                    state.char_count,
                    state.mode.label()
                ))
            }
            1 => {
                let Some(state) = shown.ocr_preview.as_mut() else {
                    return PaletteReply::Keep;
                };
                state.toggle_mode();
                let mode_label = state.mode.label();
                let mut model = ocr_preview_model(state, 1);
                shown.selected = 1;
                model.toast = Some(ToastModel {
                    kind: ToastKind::Info,
                    message: format!("Switched to {mode_label} mode"),
                });
                PaletteReply::Update(model)
            }
            2 => {
                if let Some(ctx_shared) = ctx_arc {
                    shown.visible = false;
                    shown.last_dismissed_at = Some(Instant::now());
                    drop(shown);
                    if let Some(handle) = *lock_or_recover(handle_slot) {
                        tools::launch_screen_ocr(
                            handle,
                            Arc::clone(&ctx_shared.settings),
                            Arc::clone(&ctx_shared.tools),
                            110,
                        );
                    }
                    return close_reply();
                }
                PaletteReply::Keep
            }
            _ => PaletteReply::Keep,
        },
        Mode::Settings => {
            let Some(ctx) = ctx else {
                return PaletteReply::Keep;
            };
            let Some(setting) = shown.settings_rows.get(index) else {
                return PaletteReply::Keep;
            };
            let key = setting.key.clone();
            let label = setting.label.clone();
            shown.selected = index;
            if key == KEY_RECORD_LAUNCHER_HOTKEY {
                return start_hotkey_capture(
                    ctx,
                    handle_slot,
                    &mut shown,
                    RecordingTarget::Launcher,
                );
            }
            if let Some(entry_id) = key.strip_prefix(KEY_RECORD_CMD_HOTKEY_PREFIX) {
                return start_hotkey_capture(
                    ctx,
                    handle_slot,
                    &mut shown,
                    RecordingTarget::Command {
                        entry_id: entry_id.to_string(),
                        title: label,
                    },
                );
            }
            if key == KEY_RESET_WINDOW_POSITION {
                if let Some(handle) = *lock_or_recover(handle_slot) {
                    tc_ui::post_reset_position(handle);
                }
                return PaletteReply::Keep;
            }
            let query = shown.query.clone();
            let toast = advance_native_setting(ctx, launcher, handle_slot, &key);
            let updated = collect_settings_rows(ctx, &query);
            let clamped = index.min(updated.len().saturating_sub(1));
            shown.settings_rows = updated.clone();
            shown.selected = clamped;
            let mut model = settings_model_for(&query, &updated, clamped);
            model.toast = toast;
            if key == tc_palette::KEY_APPEARANCE {
                PaletteReply::UpdateTheme(Box::new(theme_from_settings(&ctx.settings)), model)
            } else {
                PaletteReply::Update(model)
            }
        }
    }
}

/// Tertiary action (`Alt+Enter`): Run as Administrator (App) / Properties (File) / Reveal (Note).
fn handle_tertiary(
    index: usize,
    shown_lock: &Arc<Mutex<Shown>>,
    close_with_hud: &dyn Fn(String) -> PaletteReply,
) -> PaletteReply {
    let mut shown = lock_shown(shown_lock);
    match shown.mode {
        Mode::Launcher => {
            let Some(row) = shown.rows.get(index).cloned() else {
                return PaletteReply::Keep;
            };
            shown.last_dismissed_at = Some(Instant::now());
            drop(shown);
            match row.kind.as_str() {
                APPLICATIONS => {
                    let _ = tc_platform_win::run_as_admin(&row.target);
                    close_with_hud(format!("Running {} as Admin", row.name))
                }
                FILE_SECTION => {
                    let _ = tc_platform_win::show_properties(&row.target);
                    close_with_hud("Opened Properties".to_string())
                }
                CALCULATOR_SECTION => {
                    let _ = tc_platform_win::clipboard().write_text(&row.detail);
                    close_with_hud("Copied Expression".to_string())
                }
                WIN_SETTING_SECTION => {
                    let _ = tc_platform_win::clipboard().write_text(&row.target);
                    close_with_hud("Copied URI".to_string())
                }
                QUICKLINK_SECTION => {
                    let _ = tc_platform_win::clipboard().write_text(&row.target);
                    close_with_hud("Copied URL".to_string())
                }
                _ if row.id.starts_with(FALLBACK_URL_PREFIX) => {
                    let _ = tc_platform_win::clipboard().write_text(&row.target);
                    close_with_hud("Copied URL".to_string())
                }
                _ => PaletteReply::Keep,
            }
        }
        Mode::Files => {
            let Some(row) = shown.file_rows.get(index).cloned() else {
                return PaletteReply::Keep;
            };
            shown.last_dismissed_at = Some(Instant::now());
            drop(shown);
            let _ = tc_platform_win::show_properties(&row.target);
            close_with_hud("Opened Properties".to_string())
        }
        Mode::Notes => {
            let Some(note) = shown.notes.get(index).cloned() else {
                return PaletteReply::Keep;
            };
            shown.last_dismissed_at = Some(Instant::now());
            drop(shown);
            let _ = tc_platform_win::reveal(&note.path.to_string_lossy());
            close_with_hud("Opened in Explorer".to_string())
        }
        _ => PaletteReply::Keep,
    }
}

/// Quaternary action (`Shift+Enter`): Open File Location / Show in Explorer.
fn handle_quaternary(
    index: usize,
    shown_lock: &Arc<Mutex<Shown>>,
    close_with_hud: &dyn Fn(String) -> PaletteReply,
) -> PaletteReply {
    let mut shown = lock_shown(shown_lock);
    match shown.mode {
        Mode::Launcher => {
            let Some(row) = shown.rows.get(index).cloned() else {
                return PaletteReply::Keep;
            };
            if matches!(row.kind.as_str(), APPLICATIONS | FILE_SECTION) {
                shown.last_dismissed_at = Some(Instant::now());
                drop(shown);
                let _ = tc_platform_win::reveal(&row.target);
                return close_with_hud("Opened in Explorer".to_string());
            }
            PaletteReply::Keep
        }
        Mode::Files => {
            let Some(row) = shown.file_rows.get(index).cloned() else {
                return PaletteReply::Keep;
            };
            shown.last_dismissed_at = Some(Instant::now());
            drop(shown);
            let _ = tc_platform_win::reveal(&row.target);
            close_with_hud("Opened in Explorer".to_string())
        }
        Mode::Notes => {
            let Some(note) = shown.notes.get(index).cloned() else {
                return PaletteReply::Keep;
            };
            shown.last_dismissed_at = Some(Instant::now());
            drop(shown);
            let _ = tc_platform_win::reveal(&note.path.to_string_lossy());
            close_with_hud("Opened in Explorer".to_string())
        }
        _ => PaletteReply::Keep,
    }
}

/// Copy chord (`Alt+C`): Copy Path / Plain Text / Raw Template / URL / URI / Expression.
fn handle_copy_chord(
    index: usize,
    shown_lock: &Arc<Mutex<Shown>>,
    launcher: &Arc<LauncherState>,
    ctx: Option<&NativeContext>,
    close_with_hud: &dyn Fn(String) -> PaletteReply,
) -> PaletteReply {
    let mut shown = lock_shown(shown_lock);
    match shown.mode {
        Mode::Launcher => {
            let Some(row) = shown.rows.get(index).cloned() else {
                return PaletteReply::Keep;
            };
            let (text, hud) =
                match row.kind.as_str() {
                    APPLICATIONS | FILE_SECTION => (row.target.clone(), "Copied Path".to_string()),
                    CALCULATOR_SECTION => {
                        if let Some(val) = parse_numeric_answer(&row.target) {
                            launcher.set_last_calc_answer(Some(val));
                        }
                        (row.target.clone(), format!("Copied {}", row.name))
                    }
                    SNIPPET_SECTION => {
                        let raw =
                            row.id
                                .strip_prefix(INLINE_SNIPPET_PREFIX)
                                .and_then(|sid| {
                                    ctx.and_then(|c| {
                                        c.features.snippets.lock().ok().and_then(|g| {
                                            g.record(sid).map(|r| r.snippet.text.clone())
                                        })
                                    })
                                })
                                .unwrap_or_else(|| row.detail.clone());
                        (raw, "Copied Template".to_string())
                    }
                    QUICKLINK_SECTION => (row.target.clone(), "Copied URL".to_string()),
                    _ if row.id.starts_with(FALLBACK_URL_PREFIX) => {
                        (row.target.clone(), "Copied URL".to_string())
                    }
                    WIN_SETTING_SECTION => (row.target.clone(), "Copied URI".to_string()),
                    _ => (row.name.clone(), "Copied Name".to_string()),
                };
            shown.last_dismissed_at = Some(Instant::now());
            drop(shown);
            let _ = tc_platform_win::clipboard().write_text(&text);
            close_with_hud(hud)
        }
        Mode::Clipboard => {
            let Some(item) = shown.clipboard_items.get(index).cloned() else {
                return PaletteReply::Keep;
            };
            let text = item
                .plain_text()
                .map(str::to_string)
                .or(item.text)
                .unwrap_or_default();
            if text.is_empty() {
                return PaletteReply::Keep;
            }
            shown.last_dismissed_at = Some(Instant::now());
            drop(shown);
            let _ = tc_platform_win::clipboard().write_text(&text);
            close_with_hud("Copied Plain Text".to_string())
        }
        Mode::Snippets => {
            let Some(rec) = shown.snippets.get(index).cloned() else {
                return PaletteReply::Keep;
            };
            shown.last_dismissed_at = Some(Instant::now());
            drop(shown);
            let _ = tc_platform_win::clipboard().write_text(&rec.snippet.text);
            close_with_hud("Copied Template".to_string())
        }
        Mode::Quicklinks => {
            let Some(link) = shown.quicklinks.get(index).cloned() else {
                return PaletteReply::Keep;
            };
            shown.last_dismissed_at = Some(Instant::now());
            drop(shown);
            let _ = tc_platform_win::clipboard().write_text(&link.url_template);
            close_with_hud("Copied URL".to_string())
        }
        Mode::Files => {
            let Some(row) = shown.file_rows.get(index).cloned() else {
                return PaletteReply::Keep;
            };
            shown.last_dismissed_at = Some(Instant::now());
            drop(shown);
            let _ = tc_platform_win::clipboard().write_text(&row.target);
            close_with_hud("Copied Path".to_string())
        }
        Mode::CalcHistory => {
            let expr = if let Some(live) = shown.calc_live.as_ref() {
                if index == 0 {
                    live.detail.clone()
                } else {
                    match shown.calc_entries.get(index.saturating_sub(1)) {
                        Some(e) => e.expression.clone(),
                        None => return PaletteReply::Keep,
                    }
                }
            } else {
                match shown.calc_entries.get(index) {
                    Some(e) => e.expression.clone(),
                    None => return PaletteReply::Keep,
                }
            };
            shown.last_dismissed_at = Some(Instant::now());
            drop(shown);
            let _ = tc_platform_win::clipboard().write_text(&expr);
            close_with_hud("Copied Expression".to_string())
        }
        Mode::Notes => {
            let Some(note) = shown.notes.get(index).cloned() else {
                return PaletteReply::Keep;
            };
            shown.last_dismissed_at = Some(Instant::now());
            drop(shown);
            let _ = tc_platform_win::clipboard().write_text(&note.body);
            close_with_hud("Copied Note".to_string())
        }
        Mode::SwitchWindows => {
            let Some(win) = shown.windows.get(index).cloned() else {
                return PaletteReply::Keep;
            };
            shown.last_dismissed_at = Some(Instant::now());
            drop(shown);
            let _ = tc_platform_win::clipboard().write_text(&win.title);
            close_with_hud("Copied Window Title".to_string())
        }
        Mode::Emoji => {
            let Some(cell) = shown.emoji_cells.get(index).cloned() else {
                return PaletteReply::Keep;
            };
            shown.last_dismissed_at = Some(Instant::now());
            drop(shown);
            let _ = tc_platform_win::clipboard().write_text(&cell.name);
            close_with_hud("Copied Emoji Name".to_string())
        }
        Mode::ColorHistory => {
            let Some(entry) = shown.color_entries.get(index).cloned() else {
                return PaletteReply::Keep;
            };
            shown.last_dismissed_at = Some(Instant::now());
            drop(shown);
            let _ = tc_platform_win::clipboard().write_text(&entry.hex);
            close_with_hud(format!("Copied {}", entry.hex))
        }
        Mode::OcrPreview => {
            let Some(state) = shown.ocr_preview.clone() else {
                return PaletteReply::Keep;
            };
            if state.text.trim().is_empty() {
                return PaletteReply::Keep;
            }
            shown.last_dismissed_at = Some(Instant::now());
            drop(shown);
            let _ = tc_platform_win::clipboard().write_text(&state.text);
            close_with_hud(format!(
                "Copied {} chars ({})",
                state.char_count,
                state.mode.label()
            ))
        }
        _ => PaletteReply::Keep,
    }
}

fn enter_edit_clipboard(
    ctx: Option<&NativeContext>,
    shown: &mut Shown,
    selected: usize,
) -> PaletteReply {
    let Some(item) = shown.clipboard_items.get(selected).cloned() else {
        return PaletteReply::Keep;
    };
    if item.kind == ItemKind::Text {
        let text = item.text.clone().unwrap_or_default();
        let previous_query = shown.query.clone();
        shown.mode = Mode::EditClipboard {
            item: item.clone(),
            previous_query,
            previous_selected: selected,
        };
        shown.action_menu_open = false;
        PaletteReply::Navigate(edit_clipboard_model(&item, &text))
    } else {
        shown.action_menu_open = false;
        let toast = ToastModel {
            kind: ToastKind::Info,
            message: "Only text clipboard entries can be edited".to_string(),
        };
        match rebuild_current_model(ctx, shown, Some(toast)) {
            Some(model) => PaletteReply::Update(model),
            None => PaletteReply::Keep,
        }
    }
}

fn handle_editor_submit(
    text: String,
    as_new: bool,
    shown_lock: &Arc<Mutex<Shown>>,
    ctx: Option<&NativeContext>,
) -> PaletteReply {
    let (item, previous_query, previous_selected, filter) = {
        let shown = lock_shown(shown_lock);
        let Mode::EditClipboard {
            item,
            previous_query,
            previous_selected,
        } = shown.mode.clone()
        else {
            return PaletteReply::Keep;
        };
        (
            item,
            previous_query,
            previous_selected,
            shown.clipboard_filter,
        )
    };
    let dark = ctx
        .map(|c| tools::settings_prefers_dark(&*c.settings))
        .unwrap_or_else(tc_platform_win::apps_use_dark_theme);
    if text.trim().is_empty() {
        tc_ui::show_hud("Cannot save empty text", dark);
        return PaletteReply::Keep;
    }
    let Some(ctx) = ctx else {
        return PaletteReply::Keep;
    };
    if !as_new {
        match ctx.features.update_clipboard_text(&item, &text) {
            Ok(Some(updated)) => {
                let items = ctx.features.search_clipboard_view(&previous_query, filter);
                let idx = items
                    .iter()
                    .position(|c| c.id == updated.id)
                    .unwrap_or_else(|| previous_selected.min(items.len().saturating_sub(1)));
                let mut shown = lock_shown(shown_lock);
                shown.mode = Mode::Clipboard;
                shown.action_menu_open = false;
                shown.query = previous_query.clone();
                shown.clipboard_items = items.clone();
                shown.selected = idx;
                let mut model =
                    build_clipboard_screen_model(ctx, &mut shown, &previous_query, &items, idx);
                apply_recording_overlay(&mut model, &shown);
                model.toast = Some(ToastModel {
                    kind: ToastKind::Success,
                    message: "Updated clipboard entry".to_string(),
                });
                PaletteReply::UpdateWithEdit(previous_query, model)
            }
            Ok(None) => {
                let items = ctx.features.search_clipboard_view(&previous_query, filter);
                let idx = previous_selected.min(items.len().saturating_sub(1));
                let mut shown = lock_shown(shown_lock);
                shown.mode = Mode::Clipboard;
                shown.action_menu_open = false;
                shown.query = previous_query.clone();
                shown.clipboard_items = items.clone();
                shown.selected = idx;
                let mut model =
                    build_clipboard_screen_model(ctx, &mut shown, &previous_query, &items, idx);
                apply_recording_overlay(&mut model, &shown);
                model.toast = Some(ToastModel {
                    kind: ToastKind::Info,
                    message: "Entry no longer exists".to_string(),
                });
                PaletteReply::UpdateWithEdit(previous_query, model)
            }
            Err(err) => {
                tc_ui::show_hud(err, dark);
                PaletteReply::Keep
            }
        }
    } else {
        match ctx.features.add_clipboard_text(&text) {
            Ok(Some(created)) => {
                ctx.hotkeys.suppress_clipboard_for_ms(600);
                let copied = tc_platform_win::clipboard().write_text(&text).is_ok();
                let mut active_filter = filter;
                let mut items = ctx.features.search_clipboard_view("", active_filter);
                if !items.iter().any(|c| c.id == created.id) {
                    active_filter = ClipboardViewFilter::All;
                    items = ctx.features.search_clipboard_view("", active_filter);
                }
                let idx = items.iter().position(|c| c.id == created.id).unwrap_or(0);
                let mut shown = lock_shown(shown_lock);
                shown.mode = Mode::Clipboard;
                shown.action_menu_open = false;
                shown.query.clear();
                shown.clipboard_filter = active_filter;
                shown.clipboard_items = items.clone();
                shown.selected = idx;
                let mut model = build_clipboard_screen_model(ctx, &mut shown, "", &items, idx);
                apply_recording_overlay(&mut model, &shown);
                model.toast = Some(if copied {
                    ToastModel {
                        kind: ToastKind::Success,
                        message: "Saved as new copy & copied to clipboard".to_string(),
                    }
                } else {
                    ToastModel {
                        kind: ToastKind::Info,
                        message: "Saved as new copy (failed to copy to clipboard)".to_string(),
                    }
                });
                PaletteReply::Navigate(model)
            }
            Ok(None) => {
                tc_ui::show_hud(
                    "Entry could not be added (check clipboard length/policy)",
                    dark,
                );
                PaletteReply::Keep
            }
            Err(err) => {
                tc_ui::show_hud(err, dark);
                PaletteReply::Keep
            }
        }
    }
}

fn handle_action(
    action: ActionKey,
    selected: usize,
    shown_lock: &Arc<Mutex<Shown>>,
    launcher: &Arc<LauncherState>,
    ctx: Option<&NativeContext>,
    handle_slot: &Arc<Mutex<Option<PaletteHandle>>>,
    close_with_hud: &dyn Fn(String) -> PaletteReply,
) -> PaletteReply {
    // Back chord (`Alt+Left` / `Ctrl+[`) works even without NativeContext.
    if action == ActionKey::Back {
        return handle_escape_or_back(shown_lock, launcher, ctx, false, &|| PaletteReply::Keep);
    }

    let mut shown = lock_shown(shown_lock);
    if matches!(shown.mode, Mode::EditClipboard { .. }) {
        return PaletteReply::Keep;
    }
    shown.selected = selected;

    if action == ActionKey::CtrlE {
        if shown.mode == Mode::Clipboard {
            return enter_edit_clipboard(ctx, &mut shown, selected);
        }
        return PaletteReply::Keep;
    }

    // 0. `Down` while in Compact Mode: expand the single-bar palette into Suggestions.
    if action == ActionKey::Expand {
        if shown.mode == Mode::Launcher && !shown.compact_expanded {
            shown.compact_expanded = true;
            let q = shown.query.clone();
            let rows = shown.rows.clone();
            let is_suggestions = q.trim().is_empty();
            return PaletteReply::Update(model_for_with_icons(
                &q,
                &rows,
                is_suggestions,
                0,
                ctx,
                &mut shown,
            ));
        }
        return PaletteReply::Keep;
    }

    // 1. Floating `Ctrl+K` Action Panel toggle across ALL screens.
    if action == ActionKey::CtrlK {
        if shown.mode == Mode::Launcher
            && shown.query.trim().is_empty()
            && !shown.compact_expanded
            && is_compact_window_mode(ctx)
        {
            return PaletteReply::Keep;
        }
        if shown.action_menu_open {
            shown.action_menu_open = false;
            return match rebuild_current_model(ctx, &mut shown, None) {
                Some(model) => PaletteReply::Update(model),
                None => PaletteReply::Keep,
            };
        }
        if let Some(menu) = build_action_menu_for_current(ctx, &shown, selected) {
            shown.action_menu_open = true;
            if let Some(mut model) = rebuild_current_model(ctx, &mut shown, None) {
                model.action_menu = Some(menu);
                return PaletteReply::Update(model);
            }
        }
        return PaletteReply::Keep;
    }

    // 2. `Ctrl+I`: Toggle Master-Detail preview pane on/off cleanly.
    if action == ActionKey::CtrlI {
        shown.show_detail = !shown.show_detail;
        shown.action_menu_open = false;
        let msg = if shown.show_detail {
            "Preview pane shown"
        } else {
            "Preview pane hidden"
        };
        let toast = ToastModel {
            kind: ToastKind::Info,
            message: msg.to_string(),
        };
        return match rebuild_current_model(ctx, &mut shown, Some(toast)) {
            Some(model) => PaletteReply::Update(model),
            None => PaletteReply::Keep,
        };
    }

    // 3. `Shift+Tab`: step out of Quicklink inline argument mode, alias edit, or drilled folder.
    if action == ActionKey::ShiftTab {
        match shown.mode.clone() {
            Mode::QuicklinkArg {
                from_launcher,
                previous_query,
                ..
            } => {
                if from_launcher {
                    return restore_launcher_query(launcher, ctx, &mut shown, previous_query);
                }
                if let Some(ctx) = ctx {
                    return enter_quicklinks(ctx, &mut shown);
                }
                return enter_launcher(launcher, ctx, &mut shown, true);
            }
            Mode::SetAlias { previous_query, .. } => {
                return restore_launcher_query(launcher, ctx, &mut shown, previous_query);
            }
            Mode::Files => {
                if let Some(dir) = shown.file_dir.clone() {
                    let next_dir = dir
                        .parent()
                        .filter(|p| !p.as_os_str().is_empty())
                        .map(Path::to_path_buf);
                    shown.file_dir = next_dir.clone();
                    shown.file_dir_cache = None;
                    shown.query.clear();
                    shown.selected = 0;
                    let filter = shown.file_filter;
                    let rows = shown.filtered_file_rows(launcher, "");
                    let indexing = ctx.is_some_and(|c| c.features.is_file_indexing());
                    let show_detail = shown.show_detail;
                    shown.file_rows = rows.clone();
                    let model = files_model(
                        "",
                        &rows,
                        filter,
                        next_dir.as_deref(),
                        indexing,
                        0,
                        show_detail,
                        |target| shown.lookup_or_queue_icon(target),
                    );
                    return PaletteReply::UpdateWithEdit(String::new(), model);
                }
                return PaletteReply::Keep;
            }
            _ => return PaletteReply::Keep,
        }
    }

    // 4. `Tab`: Calculator answer into search bar, Quicklink inline argument pill,
    //    File Search folder drilling, Clipboard filter cycling, or empty-root flip to Clipboard.
    if action == ActionKey::Tab {
        match shown.mode.clone() {
            Mode::Launcher => {
                if let Some(row) = shown.rows.get(selected).cloned() {
                    if row.kind == CALCULATOR_SECTION {
                        if let Some(val) = parse_numeric_answer(&row.target) {
                            launcher.set_last_calc_answer(Some(val));
                        }
                        return restore_launcher_query(launcher, ctx, &mut shown, row.target);
                    }
                    if let Some(ql_name) = row.id.strip_prefix(ROOT_QUICKLINK_PREFIX) {
                        if let Some(ctx) = ctx {
                            if let Some(link) = ctx
                                .features
                                .search_quicklinks("")
                                .into_iter()
                                .find(|l| l.name == ql_name && l.needs_query())
                            {
                                return enter_quicklink_arg(
                                    link,
                                    true,
                                    shown.query.clone(),
                                    &mut shown,
                                );
                            }
                        }
                    }
                }
                if shown.query.trim().is_empty() {
                    if let Some(ctx) = ctx {
                        return enter_clipboard(ctx, &mut shown);
                    }
                }
                return PaletteReply::Keep;
            }
            Mode::Files => {
                if let Some(row) = shown.file_rows.get(selected).cloned() {
                    if row.is_dir == Some(true) {
                        let next_dir = PathBuf::from(&row.target);
                        shown.file_dir = Some(next_dir.clone());
                        shown.file_dir_cache = None;
                        shown.query.clear();
                        shown.selected = 0;
                        let filter = shown.file_filter;
                        let rows = shown.filtered_file_rows(launcher, "");
                        let indexing = ctx.is_some_and(|c| c.features.is_file_indexing());
                        let show_detail = shown.show_detail;
                        shown.file_rows = rows.clone();
                        let model = files_model(
                            "",
                            &rows,
                            filter,
                            Some(&next_dir),
                            indexing,
                            0,
                            show_detail,
                            |target| shown.lookup_or_queue_icon(target),
                        );
                        return PaletteReply::UpdateWithEdit(String::new(), model);
                    }
                }
                return PaletteReply::Keep;
            }
            Mode::Quicklinks => {
                if let Some(link) = shown.quicklinks.get(selected).cloned() {
                    if link.needs_query() {
                        return enter_quicklink_arg(link, false, shown.query.clone(), &mut shown);
                    }
                }
                return PaletteReply::Keep;
            }
            Mode::CalcHistory => {
                let copy_text = if let Some(live) = shown.calc_live.as_ref() {
                    if selected == 0 {
                        Some(live.target.clone())
                    } else {
                        shown
                            .calc_entries
                            .get(selected.saturating_sub(1))
                            .map(|e| e.copy_text.clone())
                    }
                } else {
                    shown
                        .calc_entries
                        .get(selected)
                        .map(|e| e.copy_text.clone())
                };
                if let Some(ans_text) = copy_text {
                    if let Some(val) = parse_numeric_answer(&ans_text) {
                        launcher.set_last_calc_answer(Some(val));
                    }
                    return restore_launcher_query(launcher, ctx, &mut shown, ans_text);
                }
                return PaletteReply::Keep;
            }
            Mode::Clipboard => {
                let Some(ctx) = ctx else {
                    return PaletteReply::Keep;
                };
                shown.clipboard_filter = shown.clipboard_filter.next();
                let q = shown.query.clone();
                let filter = shown.clipboard_filter;
                let items = ctx.features.search_clipboard_view(&q, filter);
                shown.clipboard_items = items.clone();
                shown.selected = 0;
                return PaletteReply::Update(build_clipboard_screen_model(
                    ctx, &mut shown, &q, &items, 0,
                ));
            }
            Mode::ColorHistory => {
                let Some(ctx) = ctx else {
                    return PaletteReply::Keep;
                };
                let next = tools::cycle_color_format(&*ctx.settings);
                let q = shown.query.clone();
                let entries = shown.color_entries.clone();
                let sel = selected.min(entries.len().saturating_sub(1));
                shown.selected = sel;
                let mut model = color_history_model(&q, &entries, sel, shown.show_detail, next);
                model.toast = Some(ToastModel {
                    kind: ToastKind::Info,
                    message: format!("Default format: {}", next.label()),
                });
                return PaletteReply::Update(model);
            }
            Mode::OcrPreview => {
                let Some(state) = shown.ocr_preview.as_mut() else {
                    return PaletteReply::Keep;
                };
                state.toggle_mode();
                let mode_label = state.mode.label();
                let mut model = ocr_preview_model(state, selected);
                model.toast = Some(ToastModel {
                    kind: ToastKind::Info,
                    message: format!("Switched to {mode_label} mode"),
                });
                return PaletteReply::Update(model);
            }
            _ => return PaletteReply::Keep,
        }
    }

    // 5. `Ctrl+H`: toggle hidden (dot-prefixed) files in File Search.
    if action == ActionKey::CtrlH && shown.mode == Mode::Files {
        shown.show_hidden_files = !shown.show_hidden_files;
        shown.file_dir_cache = None;
        let query = shown.query.clone();
        let filter = shown.file_filter;
        let current_dir = shown.file_dir.clone();
        let rows = shown.filtered_file_rows(launcher, &query);
        let indexing = ctx.is_some_and(|c| c.features.is_file_indexing());
        let show_detail = shown.show_detail;
        shown.file_rows = rows.clone();
        shown.selected = 0;
        let mut model = files_model(
            &query,
            &rows,
            filter,
            current_dir.as_deref(),
            indexing,
            0,
            show_detail,
            |target| shown.lookup_or_queue_icon(target),
        );
        model.toast = Some(ToastModel {
            kind: ToastKind::Info,
            message: if shown.show_hidden_files {
                "Showing hidden files".to_string()
            } else {
                "Hiding dot files".to_string()
            },
        });
        return PaletteReply::Update(model);
    }

    // 6. `Ctrl+R`: Refresh live FX rates (Launcher/Calc) or re-index files (Files).
    if action == ActionKey::CtrlR {
        if let Some(ctx) = ctx {
            match shown.mode {
                Mode::Launcher | Mode::CalcHistory => {
                    let rates_path = ctx.features.data_dir.join(RATES_FILE);
                    spawn_rates_refresh(Arc::clone(launcher), rates_path, true);
                    let toast = ToastModel {
                        kind: ToastKind::Info,
                        message: "Refreshing exchange rates…".to_string(),
                    };
                    if let Some(model) = rebuild_current_model(Some(ctx), &mut shown, Some(toast)) {
                        return PaletteReply::Update(model);
                    }
                }
                Mode::Files => {
                    ctx.features
                        .trigger_file_index(Arc::clone(launcher), Some(Arc::clone(handle_slot)));
                    let toast = ToastModel {
                        kind: ToastKind::Info,
                        message: "Re-indexing files in background…".to_string(),
                    };
                    if let Some(model) = rebuild_current_model(Some(ctx), &mut shown, Some(toast)) {
                        return PaletteReply::Update(model);
                    }
                }
                _ => {}
            }
        }
        return PaletteReply::Keep;
    }

    let Some(ctx) = ctx else {
        return PaletteReply::Keep;
    };

    match (&shown.mode, action) {
        // Launcher Ctrl+P: Pin / unpin selected app as a Favorite!
        (Mode::Launcher, ActionKey::CtrlP) => {
            if let Some(row) = shown.rows.get(selected).cloned() {
                if row.kind == APPLICATIONS {
                    let pinned = ctx.features.toggle_favorite(&row.id);
                    let q = shown.query.clone();
                    let is_suggestions = q.trim().is_empty();
                    let rows = if is_suggestions {
                        suggestions_with_favorites(launcher, Some(ctx))
                    } else {
                        shown.rows.clone()
                    };
                    shown.rows = rows.clone();
                    let sel = selected.min(rows.len().saturating_sub(1));
                    shown.selected = sel;
                    let mut model =
                        model_for_with_icons(&q, &rows, is_suggestions, sel, Some(ctx), &mut shown);
                    model.toast = Some(ToastModel {
                        kind: ToastKind::Success,
                        message: if pinned {
                            format!("Pinned {}", row.name)
                        } else {
                            format!("Unpinned {}", row.name)
                        },
                    });
                    return PaletteReply::Update(model);
                }
            }
            PaletteReply::Keep
        }

        // Launcher Ctrl+Delete: Move file to Recycle Bin or delete snippet/quicklink.
        (Mode::Launcher, ActionKey::CtrlDelete) => {
            if let Some(row) = shown.rows.get(selected).cloned() {
                if row.kind == FILE_SECTION {
                    shown.last_dismissed_at = Some(Instant::now());
                    drop(shown);
                    let _ = tc_platform_win::recycle(&row.target);
                    return close_with_hud("Moved to Recycle Bin".to_string());
                }
                if let Some(sid) = row.id.strip_prefix(INLINE_SNIPPET_PREFIX) {
                    ctx.features.delete_snippet(sid);
                    ctx.hotkeys
                        .sync_snippet_keywords(&ctx.settings, Arc::clone(&ctx.features));
                    let q = shown.query.clone();
                    let rows = build_root_search_rows(launcher, Some(ctx), &q);
                    shown.rows = rows.clone();
                    let sel = selected.min(rows.len().saturating_sub(1));
                    shown.selected = sel;
                    let mut model =
                        model_for_with_icons(&q, &rows, false, sel, Some(ctx), &mut shown);
                    model.toast = Some(ToastModel {
                        kind: ToastKind::Info,
                        message: "Deleted Snippet".to_string(),
                    });
                    return PaletteReply::Update(model);
                }
                if let Some(ql_name) = row.id.strip_prefix(ROOT_QUICKLINK_PREFIX) {
                    ctx.features.delete_quicklink(ql_name);
                    let q = shown.query.clone();
                    let rows = build_root_search_rows(launcher, Some(ctx), &q);
                    shown.rows = rows.clone();
                    let sel = selected.min(rows.len().saturating_sub(1));
                    shown.selected = sel;
                    let mut model =
                        model_for_with_icons(&q, &rows, false, sel, Some(ctx), &mut shown);
                    model.toast = Some(ToastModel {
                        kind: ToastKind::Info,
                        message: "Deleted Quicklink".to_string(),
                    });
                    return PaletteReply::Update(model);
                }
            }
            PaletteReply::Keep
        }

        // Notes Ctrl+Delete: Remove selected note.
        (Mode::Notes, ActionKey::CtrlDelete) => {
            if let Some(note) = shown.notes.get(selected).cloned() {
                ctx.features.delete_note(&note.path);
                let q = shown.query.clone();
                let list = ctx.features.search_notes(&q);
                let next_sel = selected.min(list.len().saturating_sub(1));
                shown.notes = list.clone();
                shown.selected = next_sel;
                let mut model = notes_model(&q, &list, next_sel, shown.show_detail);
                model.toast = Some(ToastModel {
                    kind: ToastKind::Info,
                    message: "Deleted Note".to_string(),
                });
                return PaletteReply::Update(model);
            }
            PaletteReply::Keep
        }

        // Quicklinks Ctrl+Delete: Remove selected quicklink.
        (Mode::Quicklinks, ActionKey::CtrlDelete) => {
            if let Some(link) = shown.quicklinks.get(selected).cloned() {
                ctx.features.delete_quicklink(&link.name);
                let q = shown.query.clone();
                let list = ctx.features.search_quicklinks(&q);
                let next_sel = selected.min(list.len().saturating_sub(1));
                shown.quicklinks = list.clone();
                shown.selected = next_sel;
                let mut model = quicklinks_model(&q, &list, next_sel, shown.show_detail);
                model.toast = Some(ToastModel {
                    kind: ToastKind::Info,
                    message: "Deleted Quicklink".to_string(),
                });
                return PaletteReply::Update(model);
            }
            PaletteReply::Keep
        }

        // Settings Ctrl+Delete: Clear a bound built-in Action Hotkey.
        (Mode::Settings, ActionKey::CtrlDelete) => {
            if let Some(setting) = shown.settings_rows.get(selected).cloned() {
                if let Some(entry_id) = setting.key.strip_prefix(KEY_RECORD_CMD_HOTKEY_PREFIX) {
                    let _ = hotkeys::set_command_hotkey(&*ctx.settings, entry_id, None);
                    ctx.hotkeys.reload_bindings(&ctx.settings);
                    let q = shown.query.clone();
                    let updated = collect_settings_rows(ctx, &q);
                    let clamped = selected.min(updated.len().saturating_sub(1));
                    shown.settings_rows = updated.clone();
                    shown.selected = clamped;
                    let mut model = settings_model_for(&q, &updated, clamped);
                    model.toast = Some(ToastModel {
                        kind: ToastKind::Info,
                        message: format!("Cleared hotkey for {}", setting.label),
                    });
                    return PaletteReply::Update(model);
                }
            }
            PaletteReply::Keep
        }

        // Clipboard actions: Ctrl+P Pin, Ctrl+Delete Remove.
        (Mode::Clipboard, ActionKey::CtrlP) => {
            if let Some(item) = shown.clipboard_items.get(selected).cloned() {
                ctx.features.toggle_clipboard_pin(&item);
                let q = shown.query.clone();
                let filter = shown.clipboard_filter;
                let items = ctx.features.search_clipboard_view(&q, filter);
                let next_sel = selected.min(items.len().saturating_sub(1));
                shown.clipboard_items = items.clone();
                shown.selected = next_sel;
                return PaletteReply::Update(build_clipboard_screen_model(
                    ctx, &mut shown, &q, &items, next_sel,
                ));
            }
            PaletteReply::Keep
        }
        (Mode::Clipboard, ActionKey::CtrlDelete) => {
            if let Some(item) = shown.clipboard_items.get(selected).cloned() {
                ctx.features.remove_clipboard_item(&item);
                shown.clipboard_thumbs.remove(&item.id);
                let q = shown.query.clone();
                let filter = shown.clipboard_filter;
                let items = ctx.features.search_clipboard_view(&q, filter);
                let next_sel = selected.min(items.len().saturating_sub(1));
                shown.clipboard_items = items.clone();
                shown.selected = next_sel;
                let mut model = build_clipboard_screen_model(ctx, &mut shown, &q, &items, next_sel);
                model.toast = Some(ToastModel {
                    kind: ToastKind::Info,
                    message: "Deleted Entry".to_string(),
                });
                return PaletteReply::Update(model);
            }
            PaletteReply::Keep
        }

        // Emoji actions: Ctrl+P Pin/Unpin selected emoji cell.
        (Mode::Emoji, ActionKey::CtrlP) => {
            if let Some(cell) = shown.emoji_cells.get(selected).cloned() {
                ctx.features.toggle_emoji_pin(&cell.glyph);
                let q = shown.query.clone();
                let cells = ctx.features.emoji_cells(&q);
                let next_sel = selected.min(cells.len().saturating_sub(1));
                shown.emoji_cells = cells.clone();
                shown.selected = next_sel;
                return PaletteReply::Update(emoji_model(&q, cells, next_sel));
            }
            PaletteReply::Keep
        }

        // Snippets Ctrl+Delete: Remove selected snippet.
        (Mode::Snippets, ActionKey::CtrlDelete) => {
            if let Some(rec) = shown.snippets.get(selected).cloned() {
                ctx.features.delete_snippet(&rec.id());
                ctx.hotkeys
                    .sync_snippet_keywords(&ctx.settings, Arc::clone(&ctx.features));
                let q = shown.query.clone();
                let list = ctx.features.search_snippets(&q);
                let next_sel = selected.min(list.len().saturating_sub(1));
                shown.snippets = list.clone();
                shown.selected = next_sel;
                let mut model = snippets_model(&q, &list, next_sel, shown.show_detail, |r| {
                    ctx.features.expand_snippet(r)
                });
                model.toast = Some(ToastModel {
                    kind: ToastKind::Info,
                    message: "Deleted Snippet".to_string(),
                });
                return PaletteReply::Update(model);
            }
            PaletteReply::Keep
        }

        // Files actions: Ctrl+P Cycle FileType filter, Ctrl+Delete Move to Recycle Bin.
        (Mode::Files, ActionKey::CtrlP) => {
            shown.file_filter = shown.file_filter.next();
            let q = shown.query.clone();
            let filter = shown.file_filter;
            let current_dir = shown.file_dir.clone();
            let rows = shown.filtered_file_rows(launcher, &q);
            let indexing = ctx.features.is_file_indexing();
            let show_detail = shown.show_detail;
            shown.file_rows = rows.clone();
            shown.selected = 0;
            PaletteReply::Update(files_model(
                &q,
                &rows,
                filter,
                current_dir.as_deref(),
                indexing,
                0,
                show_detail,
                |target| shown.lookup_or_queue_icon(target),
            ))
        }
        (Mode::Files, ActionKey::CtrlDelete) => {
            if let Some(row) = shown.file_rows.get(selected).cloned() {
                shown.file_dir_cache = None;
                shown.last_dismissed_at = Some(Instant::now());
                drop(shown);
                let _ = tc_platform_win::recycle(&row.target);
                return close_with_hud("Moved to Recycle Bin".to_string());
            }
            PaletteReply::Keep
        }

        // Calculator History: Ctrl+Delete Remove selected entry.
        (Mode::CalcHistory, ActionKey::CtrlDelete) => {
            let entry_idx = if shown.calc_live.is_some() {
                if selected == 0 {
                    return PaletteReply::Keep;
                }
                selected - 1
            } else {
                selected
            };
            if let Some(entry) = shown.calc_entries.get(entry_idx).cloned() {
                ctx.features.remove_calc_entry(&entry);
                let q = shown.query.clone();
                let entries = ctx.features.search_calc_history(&q);
                let live = shown.calc_live.clone();
                let total = entries.len() + usize::from(live.is_some());
                let next_sel = selected.min(total.saturating_sub(1));
                shown.calc_entries = entries.clone();
                shown.selected = next_sel;
                return PaletteReply::Update(calc_history_model(
                    &q,
                    live.as_ref(),
                    &entries,
                    next_sel,
                    shown.show_detail,
                ));
            }
            PaletteReply::Keep
        }

        // Color Inspector: Ctrl+P Pin/Unpin, Ctrl+Delete Remove.
        (Mode::ColorHistory, ActionKey::CtrlP) => {
            if let Some(entry) = shown.color_entries.get(selected).cloned() {
                if let Some(pinned) = ctx.features.toggle_pin_color(&entry.hex) {
                    let q = shown.query.clone();
                    let entries = ctx.features.color_history_items(&q);
                    let next_sel = entries
                        .iter()
                        .position(|e| e.hex.eq_ignore_ascii_case(&entry.hex))
                        .unwrap_or_else(|| selected.min(entries.len().saturating_sub(1)));
                    shown.color_entries = entries.clone();
                    shown.selected = next_sel;
                    let fmt = tools::configured_color_format(&*ctx.settings);
                    let mut model =
                        color_history_model(&q, &entries, next_sel, shown.show_detail, fmt);
                    model.toast = Some(ToastModel {
                        kind: ToastKind::Success,
                        message: if pinned {
                            format!("Pinned {}", entry.hex)
                        } else {
                            format!("Unpinned {}", entry.hex)
                        },
                    });
                    return PaletteReply::Update(model);
                }
            }
            PaletteReply::Keep
        }
        (Mode::ColorHistory, ActionKey::CtrlDelete) => {
            if let Some(entry) = shown.color_entries.get(selected).cloned() {
                if ctx.features.delete_color(&entry.hex) {
                    let q = shown.query.clone();
                    let entries = ctx.features.color_history_items(&q);
                    let next_sel = selected.min(entries.len().saturating_sub(1));
                    shown.color_entries = entries.clone();
                    shown.selected = next_sel;
                    let fmt = tools::configured_color_format(&*ctx.settings);
                    let mut model =
                        color_history_model(&q, &entries, next_sel, shown.show_detail, fmt);
                    model.toast = Some(ToastModel {
                        kind: ToastKind::Info,
                        message: format!("Deleted {}", entry.hex),
                    });
                    return PaletteReply::Update(model);
                }
            }
            PaletteReply::Keep
        }

        _ => PaletteReply::Keep,
    }
}

/// Build the contextual `ActionMenuModel` for whatever screen and row is currently selected.
fn build_action_menu_for_current(
    ctx: Option<&NativeContext>,
    shown: &Shown,
    selected: usize,
) -> Option<ActionMenuModel> {
    match &shown.mode {
        Mode::Launcher => {
            let row = shown.rows.get(selected)?;
            let is_favorite =
                row.kind == APPLICATIONS && ctx.is_some_and(|c| c.features.is_favorite(&row.id));
            let quicklink_needs_args =
                row.id
                    .strip_prefix(ROOT_QUICKLINK_PREFIX)
                    .is_some_and(|name| {
                        ctx.is_some_and(|c| {
                            c.features
                                .search_quicklinks("")
                                .into_iter()
                                .any(|l| l.name == name && l.needs_query())
                        })
                    });
            let has_hotkey =
                ctx.is_some_and(|c| hotkeys::get_command_hotkey(&*c.settings, &row.id).is_some());
            let has_alias =
                ctx.is_some_and(|c| hotkeys::get_alias(&*c.settings, &row.id).is_some());
            Some(actions::for_launcher_row(
                row,
                &shown.query,
                LauncherActionMeta {
                    is_favorite,
                    quicklink_needs_args,
                    has_hotkey,
                    has_alias,
                },
            ))
        }
        Mode::Files => shown.file_rows.get(selected).map(actions::for_file_row),
        Mode::Clipboard => {
            let queue_len = ctx.map_or(0, |c| c.features.paste_queue_len());
            shown
                .clipboard_items
                .get(selected)
                .map(|item| actions::for_clipboard_item(item, queue_len))
        }
        Mode::Snippets => shown.snippets.get(selected).map(|rec| {
            let entry_id = format!("{INLINE_SNIPPET_PREFIX}{}", rec.id());
            let has_hotkey =
                ctx.is_some_and(|c| hotkeys::get_command_hotkey(&*c.settings, &entry_id).is_some());
            let has_alias =
                ctx.is_some_and(|c| hotkeys::get_alias(&*c.settings, &entry_id).is_some());
            actions::for_snippet(rec, true, has_hotkey, has_alias)
        }),
        Mode::Quicklinks => shown
            .quicklinks
            .get(selected)
            .map(|link| actions::for_quicklink(link, true)),
        Mode::Notes => shown.notes.get(selected).map(actions::for_note),
        Mode::CalcHistory => {
            let idx = if shown.calc_live.is_some() {
                if selected == 0 {
                    return shown.calc_live.as_ref().map(|r| {
                        actions::for_launcher_row(r, &shown.query, LauncherActionMeta::default())
                    });
                }
                selected - 1
            } else {
                selected
            };
            shown.calc_entries.get(idx).map(actions::for_calc_history)
        }
        Mode::SwitchWindows => shown
            .windows
            .get(selected)
            .map(|w| actions::for_window(&w.title)),
        Mode::Emoji => shown
            .emoji_cells
            .get(selected)
            .map(|c| actions::for_emoji(&c.glyph, &c.name, c.pinned)),
        Mode::ColorHistory => {
            let fmt = ctx
                .map(|c| tools::configured_color_format(&*c.settings))
                .unwrap_or(PickerColorFormat::Hex);
            Some(actions::for_color_history(
                shown.color_entries.get(selected),
                fmt,
            ))
        }
        Mode::OcrPreview => shown.ocr_preview.as_ref().map(actions::for_ocr_preview),
        _ => None,
    }
}

fn shortcut_target_for_current(shown: &Shown, selected: usize) -> Option<(String, String)> {
    match &shown.mode {
        Mode::Launcher => shown
            .rows
            .get(selected)
            .map(|r| (r.id.clone(), r.name.clone())),
        Mode::Snippets => shown.snippets.get(selected).map(|rec| {
            (
                format!("{INLINE_SNIPPET_PREFIX}{}", rec.id()),
                rec.snippet.name.clone(),
            )
        }),
        _ => None,
    }
}

/// Execute an action chosen from the floating `Ctrl+K` Action Panel popover.
#[allow(clippy::too_many_arguments)]
fn handle_action_menu_choice(
    action_id: &str,
    shown_lock: &Arc<Mutex<Shown>>,
    launcher: &Arc<LauncherState>,
    runner: &Arc<Runner>,
    ctx_arc: Option<&Arc<NativeContext>>,
    handle_slot: &Arc<Mutex<Option<PaletteHandle>>>,
    close_reply: &dyn Fn() -> PaletteReply,
    close_with_hud: &dyn Fn(String) -> PaletteReply,
) -> PaletteReply {
    let ctx = ctx_arc.map(Arc::as_ref);
    let (mode, selected, query) = {
        let mut shown = lock_shown(shown_lock);
        shown.action_menu_open = false;
        (shown.mode.clone(), shown.selected, shown.query.clone())
    };
    if matches!(mode, Mode::EditClipboard { .. }) {
        return PaletteReply::Keep;
    }

    match action_id {
        // Common toggle / primary / copy actions
        actions::id::TOGGLE_DETAIL => handle_action(
            ActionKey::CtrlI,
            selected,
            shown_lock,
            launcher,
            ctx,
            handle_slot,
            close_with_hud,
        ),
        actions::id::OPEN | actions::id::RUN_PRIMARY | actions::id::WS_OPEN => handle_activate(
            selected,
            false,
            shown_lock,
            launcher,
            runner,
            ctx_arc,
            handle_slot,
            close_reply,
            close_with_hud,
        ),
        actions::id::RUN_ADMIN => handle_tertiary(selected, shown_lock, close_with_hud),
        actions::id::REVEAL | actions::id::NOTE_REVEAL => {
            handle_quaternary(selected, shown_lock, close_with_hud)
        }
        actions::id::OPEN_TERMINAL => {
            let target = {
                let mut shown = lock_shown(shown_lock);
                let t = match shown.mode {
                    Mode::Launcher => shown.rows.get(selected).map(|r| r.target.clone()),
                    Mode::Files => shown.file_rows.get(selected).map(|r| r.target.clone()),
                    _ => None,
                };
                if t.is_some() {
                    shown.last_dismissed_at = Some(Instant::now());
                }
                t
            };
            if let Some(path) = target {
                let _ = tc_platform_win::open_in_terminal(&path);
                close_with_hud("Opened in Terminal".to_string())
            } else {
                PaletteReply::Keep
            }
        }
        actions::id::COPY_PATH
        | actions::id::WS_COPY_URI
        | actions::id::QL_COPY_URL
        | actions::id::SNIPPET_COPY_RAW
        | actions::id::CLIP_COPY_PLAIN
        | actions::id::COPY_TEXT
        | actions::id::EMOJI_COPY_NAME => {
            handle_copy_chord(selected, shown_lock, launcher, ctx, close_with_hud)
        }
        actions::id::COPY_FILE => {
            let target = {
                let mut shown = lock_shown(shown_lock);
                let t = match shown.mode {
                    Mode::Launcher => shown.rows.get(selected).map(|r| r.target.clone()),
                    Mode::Files => shown.file_rows.get(selected).map(|r| r.target.clone()),
                    _ => None,
                };
                if t.is_some() {
                    shown.last_dismissed_at = Some(Instant::now());
                }
                t
            };
            if let Some(path) = target {
                let _ = tc_platform_win::clipboard().write_files(&[path]);
                close_with_hud("Copied File".to_string())
            } else {
                PaletteReply::Keep
            }
        }
        actions::id::PROPERTIES => {
            let target = {
                let mut shown = lock_shown(shown_lock);
                let t = match shown.mode {
                    Mode::Launcher => shown.rows.get(selected).map(|r| r.target.clone()),
                    Mode::Files => shown.file_rows.get(selected).map(|r| r.target.clone()),
                    _ => None,
                };
                if t.is_some() {
                    shown.last_dismissed_at = Some(Instant::now());
                }
                t
            };
            if let Some(path) = target {
                let _ = tc_platform_win::show_properties(&path);
                close_with_hud("Opened Properties".to_string())
            } else {
                PaletteReply::Keep
            }
        }
        actions::id::TOGGLE_PIN
        | actions::id::CLIP_TOGGLE_PIN
        | actions::id::EMOJI_TOGGLE_PIN
        | actions::id::COLOR_TOGGLE_PIN => handle_action(
            ActionKey::CtrlP,
            selected,
            shown_lock,
            launcher,
            ctx,
            handle_slot,
            close_with_hud,
        ),
        actions::id::RECYCLE
        | actions::id::CLIP_DELETE
        | actions::id::SNIPPET_DELETE
        | actions::id::QL_DELETE
        | actions::id::NOTE_DELETE
        | actions::id::CALCHIST_DELETE
        | actions::id::COLOR_DELETE => handle_action(
            ActionKey::CtrlDelete,
            selected,
            shown_lock,
            launcher,
            ctx,
            handle_slot,
            close_with_hud,
        ),

        // Calculator actions
        actions::id::CALC_COPY_ANSWER => handle_activate(
            selected,
            false,
            shown_lock,
            launcher,
            runner,
            ctx_arc,
            handle_slot,
            close_reply,
            close_with_hud,
        ),
        actions::id::CALC_COPY_EXPR => {
            if mode == Mode::CalcHistory {
                handle_copy_chord(selected, shown_lock, launcher, ctx, close_with_hud)
            } else {
                handle_activate(
                    selected,
                    true,
                    shown_lock,
                    launcher,
                    runner,
                    ctx_arc,
                    handle_slot,
                    close_reply,
                    close_with_hud,
                )
            }
        }
        actions::id::CALC_SWAP_UNITS => {
            if let Some(swapped) = tc_calc::swap_units_query(&query) {
                let mut shown = lock_shown(shown_lock);
                if mode == Mode::CalcHistory {
                    if let Some(ctx) = ctx {
                        shown.query = swapped.clone();
                        shown.selected = 0;
                        let live = launcher.evaluate_calc(&swapped);
                        let entries = ctx.features.search_calc_history(&swapped);
                        shown.calc_live = live.clone();
                        shown.calc_entries = entries.clone();
                        let model = calc_history_model(
                            &swapped,
                            live.as_ref(),
                            &entries,
                            0,
                            shown.show_detail,
                        );
                        return PaletteReply::UpdateWithEdit(swapped, model);
                    }
                }
                return restore_launcher_query(launcher, ctx, &mut shown, swapped);
            }
            PaletteReply::Keep
        }
        actions::id::CALC_PUT_IN_SEARCH | actions::id::CALCHIST_USE_IN_LAUNCHER => handle_action(
            ActionKey::Tab,
            selected,
            shown_lock,
            launcher,
            ctx,
            handle_slot,
            close_with_hud,
        ),
        actions::id::CALC_OPEN_HISTORY => {
            let Some(ctx) = ctx else {
                return PaletteReply::Keep;
            };
            let mut shown = lock_shown(shown_lock);
            enter_calc_history(ctx, launcher, &mut shown)
        }
        actions::id::CALC_REFRESH_FX => handle_action(
            ActionKey::CtrlR,
            selected,
            shown_lock,
            launcher,
            ctx,
            handle_slot,
            close_with_hud,
        ),

        // Clipboard actions
        actions::id::CLIP_PASTE | actions::id::SNIPPET_PASTE | actions::id::EMOJI_PASTE => {
            handle_activate(
                selected,
                false,
                shown_lock,
                launcher,
                runner,
                ctx_arc,
                handle_slot,
                close_reply,
                close_with_hud,
            )
        }
        actions::id::CLIP_COPY | actions::id::SNIPPET_COPY | actions::id::EMOJI_COPY => {
            handle_activate(
                selected,
                true,
                shown_lock,
                launcher,
                runner,
                ctx_arc,
                handle_slot,
                close_reply,
                close_with_hud,
            )
        }
        actions::id::CLIP_EDIT => {
            let mut shown = lock_shown(shown_lock);
            if shown.mode != Mode::Clipboard {
                return PaletteReply::Keep;
            }
            enter_edit_clipboard(ctx, &mut shown, selected)
        }
        actions::id::SET_HOTKEY => {
            let Some(ctx) = ctx else {
                return PaletteReply::Keep;
            };
            let mut shown = lock_shown(shown_lock);
            shown.selected = selected;
            let Some((entry_id, title)) = shortcut_target_for_current(&shown, selected) else {
                return PaletteReply::Keep;
            };
            start_hotkey_capture(
                ctx,
                handle_slot,
                &mut shown,
                RecordingTarget::Command { entry_id, title },
            )
        }
        actions::id::CLEAR_HOTKEY => {
            let Some(ctx) = ctx else {
                return PaletteReply::Keep;
            };
            let mut shown = lock_shown(shown_lock);
            let Some((entry_id, title)) = shortcut_target_for_current(&shown, selected) else {
                return PaletteReply::Keep;
            };
            let _ = hotkeys::set_command_hotkey(&*ctx.settings, &entry_id, None);
            ctx.hotkeys.reload_bindings(&ctx.settings);
            let toast = ToastModel {
                kind: ToastKind::Info,
                message: format!("Cleared hotkey for {title}"),
            };
            match rebuild_current_model(Some(ctx), &mut shown, Some(toast)) {
                Some(model) => PaletteReply::Update(model),
                None => PaletteReply::Keep,
            }
        }
        actions::id::SET_ALIAS => {
            let Some(ctx) = ctx else {
                return PaletteReply::Keep;
            };
            let mut shown = lock_shown(shown_lock);
            let Some((entry_id, title)) = shortcut_target_for_current(&shown, selected) else {
                return PaletteReply::Keep;
            };
            let current_alias = hotkeys::get_alias(&*ctx.settings, &entry_id);
            let initial_edit = current_alias.clone().unwrap_or_default();
            let previous_query = shown.query.clone();
            shown.mode = Mode::SetAlias {
                entry_id,
                title: title.clone(),
                current_alias: current_alias.clone(),
                previous_query,
            };
            shown.query = initial_edit.clone();
            shown.selected = 0;
            shown.action_menu_open = false;
            PaletteReply::UpdateWithEdit(
                initial_edit.clone(),
                set_alias_model(&title, current_alias.as_deref(), &initial_edit),
            )
        }
        actions::id::CLEAR_ALIAS => {
            let Some(ctx) = ctx else {
                return PaletteReply::Keep;
            };
            let mut shown = lock_shown(shown_lock);
            let Some((entry_id, title)) = shortcut_target_for_current(&shown, selected) else {
                return PaletteReply::Keep;
            };
            let _ = hotkeys::set_alias(&*ctx.settings, &entry_id, None);
            let toast = ToastModel {
                kind: ToastKind::Info,
                message: format!("Cleared alias for {title}"),
            };
            match rebuild_current_model(Some(ctx), &mut shown, Some(toast)) {
                Some(model) => PaletteReply::Update(model),
                None => PaletteReply::Keep,
            }
        }
        actions::id::CLIP_CYCLE_FILTER => {
            let Some(ctx) = ctx else {
                return PaletteReply::Keep;
            };
            let mut shown = lock_shown(shown_lock);
            shown.clipboard_filter = shown.clipboard_filter.next();
            let q = shown.query.clone();
            let filter = shown.clipboard_filter;
            let items = ctx.features.search_clipboard_view(&q, filter);
            shown.clipboard_items = items.clone();
            shown.selected = 0;
            PaletteReply::Update(build_clipboard_screen_model(ctx, &mut shown, &q, &items, 0))
        }
        actions::id::CLIP_QUEUE_ADD => {
            let Some(ctx) = ctx else {
                return PaletteReply::Keep;
            };
            let mut shown = lock_shown(shown_lock);
            let Some(item) = shown.clipboard_items.get(selected).cloned() else {
                return PaletteReply::Keep;
            };
            let len = ctx.features.paste_queue_add(item);
            let q = shown.query.clone();
            let items = shown.clipboard_items.clone();
            let mut model = build_clipboard_screen_model(ctx, &mut shown, &q, &items, selected);
            model.toast = Some(ToastModel {
                kind: ToastKind::Success,
                message: format!("Added to Paste Queue ({len} queued)"),
            });
            PaletteReply::Update(model)
        }
        actions::id::CLIP_QUEUE_PASTE_NEXT => {
            let Some(ctx) = ctx else {
                return PaletteReply::Keep;
            };
            {
                let mut shown = lock_shown(shown_lock);
                shown.last_dismissed_at = Some(Instant::now());
            }
            paste_next_from_queue(ctx, close_with_hud)
        }
        actions::id::CLIP_QUEUE_CLEAR => {
            let Some(ctx) = ctx else {
                return PaletteReply::Keep;
            };
            let cleared = ctx.features.paste_queue_clear();
            let mut shown = lock_shown(shown_lock);
            let q = shown.query.clone();
            let items = shown.clipboard_items.clone();
            let mut model = build_clipboard_screen_model(ctx, &mut shown, &q, &items, selected);
            model.toast = Some(ToastModel {
                kind: ToastKind::Info,
                message: format!("Cleared Paste Queue ({cleared} items)"),
            });
            PaletteReply::Update(model)
        }
        actions::id::CLIP_CLEAR_UNPINNED => {
            let Some(ctx) = ctx else {
                return PaletteReply::Keep;
            };
            let removed = ctx.features.clear_unpinned_clipboard();
            let mut shown = lock_shown(shown_lock);
            shown.clipboard_thumbs.clear();
            let q = shown.query.clone();
            let filter = shown.clipboard_filter;
            let items = ctx.features.search_clipboard_view(&q, filter);
            shown.clipboard_items = items.clone();
            shown.selected = 0;
            let mut model = build_clipboard_screen_model(ctx, &mut shown, &q, &items, 0);
            model.toast = Some(ToastModel {
                kind: ToastKind::Info,
                message: format!("Cleared {removed} unpinned clips"),
            });
            PaletteReply::Update(model)
        }

        // Snippet / Quicklink / Note creation & arguments
        actions::id::SNIPPET_CREATE => {
            let mut shown = lock_shown(shown_lock);
            enter_create_snippet(&mut shown)
        }
        actions::id::QL_OPEN => handle_activate(
            selected,
            false,
            shown_lock,
            launcher,
            runner,
            ctx_arc,
            handle_slot,
            close_reply,
            close_with_hud,
        ),
        actions::id::QL_FILL_ARGS => handle_action(
            ActionKey::Tab,
            selected,
            shown_lock,
            launcher,
            ctx,
            handle_slot,
            close_with_hud,
        ),
        actions::id::QL_CREATE => {
            let mut shown = lock_shown(shown_lock);
            enter_create_quicklink(&mut shown)
        }
        actions::id::NOTE_COPY => handle_activate(
            selected,
            false,
            shown_lock,
            launcher,
            runner,
            ctx_arc,
            handle_slot,
            close_reply,
            close_with_hud,
        ),
        actions::id::NOTE_OPEN_FILE => handle_activate(
            selected,
            true,
            shown_lock,
            launcher,
            runner,
            ctx_arc,
            handle_slot,
            close_reply,
            close_with_hud,
        ),
        actions::id::NOTE_CREATE => {
            let mut shown = lock_shown(shown_lock);
            enter_create_note(&mut shown)
        }

        // Calc history actions
        actions::id::CALCHIST_COPY => handle_activate(
            selected,
            false,
            shown_lock,
            launcher,
            runner,
            ctx_arc,
            handle_slot,
            close_reply,
            close_with_hud,
        ),
        actions::id::CALCHIST_PASTE => handle_activate(
            selected,
            true,
            shown_lock,
            launcher,
            runner,
            ctx_arc,
            handle_slot,
            close_reply,
            close_with_hud,
        ),
        actions::id::CALCHIST_COPY_EXPR => {
            handle_copy_chord(selected, shown_lock, launcher, ctx, close_with_hud)
        }

        // Clipboard Image OCR
        actions::id::CLIP_OCR => {
            let Some(ctx_shared) = ctx_arc else {
                return PaletteReply::Keep;
            };
            let item = {
                let shown = lock_shown(shown_lock);
                if shown.mode != Mode::Clipboard {
                    return PaletteReply::Keep;
                }
                shown.clipboard_items.get(selected).cloned()
            };
            let png_bytes = item
                .filter(|item| item.kind == ItemKind::Image)
                .and_then(|item| {
                    item.image_path
                        .as_deref()
                        .and_then(|path| std::fs::read(path).ok())
                        .or_else(|| {
                            ctx_shared
                                .features
                                .load_clipboard_image_rgba(&item)
                                .and_then(|(w, h, rgba)| tc_clipboard::encode_png_rgba(w, h, &rgba))
                        })
                });
            if let Some(png_bytes) = png_bytes {
                {
                    let mut shown = lock_shown(shown_lock);
                    shown.visible = false;
                    shown.last_dismissed_at = Some(Instant::now());
                }
                tools::run_clipboard_image_ocr(png_bytes, Arc::clone(&ctx_shared.settings));
                close_reply()
            } else {
                let mut shown = lock_shown(shown_lock);
                let toast = ToastModel {
                    kind: ToastKind::Error,
                    message: "Clipboard image unavailable for OCR".to_string(),
                };
                match rebuild_current_model(ctx, &mut shown, Some(toast)) {
                    Some(model) => PaletteReply::Update(model),
                    None => PaletteReply::Keep,
                }
            }
        }

        // Color Inspector actions
        actions::id::COLOR_COPY_DEFAULT => handle_activate(
            selected,
            false,
            shown_lock,
            launcher,
            runner,
            ctx_arc,
            handle_slot,
            close_reply,
            close_with_hud,
        ),
        actions::id::COLOR_COPY_HEX
        | actions::id::COLOR_COPY_RGB
        | actions::id::COLOR_COPY_HSL
        | actions::id::COLOR_COPY_OKLCH => {
            let fmt = match action_id {
                actions::id::COLOR_COPY_HEX => PickerColorFormat::Hex,
                actions::id::COLOR_COPY_RGB => PickerColorFormat::Rgb,
                actions::id::COLOR_COPY_HSL => PickerColorFormat::Hsl,
                _ => PickerColorFormat::Oklch,
            };
            let entry = {
                let mut shown = lock_shown(shown_lock);
                let entry = shown.color_entries.get(selected).cloned();
                if entry.is_some() {
                    shown.last_dismissed_at = Some(Instant::now());
                }
                entry
            };
            let Some(entry) = entry else {
                return PaletteReply::Keep;
            };
            let formatted = fmt.format(tc_clipboard::color_from_rgb(entry.r, entry.g, entry.b));
            let _ = tc_platform_win::clipboard().write_text(&formatted);
            close_with_hud(format!("Copied {formatted}"))
        }
        actions::id::COLOR_PICK_NEW => {
            let Some(ctx_shared) = ctx_arc else {
                return PaletteReply::Keep;
            };
            {
                let mut shown = lock_shown(shown_lock);
                shown.visible = false;
                shown.last_dismissed_at = Some(Instant::now());
            }
            if let Some(handle) = *lock_or_recover(handle_slot) {
                tools::launch_color_picker(
                    handle,
                    Arc::clone(&ctx_shared.features),
                    Arc::clone(&ctx_shared.settings),
                    Arc::clone(&ctx_shared.tools),
                    110,
                );
            }
            close_reply()
        }
        actions::id::COLOR_CLEAR_UNPINNED => {
            let Some(ctx) = ctx else {
                return PaletteReply::Keep;
            };
            let removed = ctx.features.clear_unpinned_colors();
            let mut shown = lock_shown(shown_lock);
            let q = shown.query.clone();
            let entries = ctx.features.color_history_items(&q);
            shown.color_entries = entries.clone();
            shown.selected = 0;
            let fmt = tools::configured_color_format(&*ctx.settings);
            let mut model = color_history_model(&q, &entries, 0, shown.show_detail, fmt);
            model.toast = Some(ToastModel {
                kind: ToastKind::Info,
                message: format!("Cleared {removed} unpinned colors"),
            });
            PaletteReply::Update(model)
        }

        // Screen OCR Preview actions
        actions::id::OCR_COPY => handle_activate(
            0,
            false,
            shown_lock,
            launcher,
            runner,
            ctx_arc,
            handle_slot,
            close_reply,
            close_with_hud,
        ),
        actions::id::OCR_TOGGLE_MODE => handle_action(
            ActionKey::Tab,
            selected,
            shown_lock,
            launcher,
            ctx,
            handle_slot,
            close_with_hud,
        ),
        actions::id::OCR_CAPTURE_AGAIN => handle_activate(
            2,
            false,
            shown_lock,
            launcher,
            runner,
            ctx_arc,
            handle_slot,
            close_reply,
            close_with_hud,
        ),

        _ => {
            // Dismiss menu cleanly if an unknown ID ever arrives.
            let mut shown = lock_shown(shown_lock);
            match rebuild_current_model(ctx, &mut shown, None) {
                Some(model) => PaletteReply::Update(model),
                None => PaletteReply::Keep,
            }
        }
    }
}

/// Rebuild the active screen's `Model` cleanly at `shown.selected` (used by live
/// selection updates, `Ctrl+I` detail toggle, `Ctrl+K` popover toggle, and `Shown` resume).
fn rebuild_current_model(
    ctx: Option<&NativeContext>,
    shown: &mut Shown,
    toast: Option<ToastModel>,
) -> Option<Model> {
    let query = shown.query.clone();
    let sel = shown.selected;
    let show_detail = shown.show_detail;
    let mut model = match shown.mode.clone() {
        Mode::Launcher => {
            let rows = shown.rows.clone();
            let is_suggestions = query.trim().is_empty();
            model_for_with_icons(&query, &rows, is_suggestions, sel, ctx, shown)
        }
        Mode::Clipboard => {
            let ctx = ctx?;
            let items = shown.clipboard_items.clone();
            build_clipboard_screen_model(ctx, shown, &query, &items, sel)
        }
        Mode::EditClipboard { .. } => return None,
        Mode::Emoji => emoji_model(&query, shown.emoji_cells.clone(), sel),
        Mode::Snippets => {
            let ctx = ctx?;
            snippets_model(&query, &shown.snippets, sel, show_detail, |rec| {
                ctx.features.expand_snippet(rec)
            })
        }
        Mode::CreateSnippet(draft) => create_snippet_model(&draft, &query),
        Mode::Notes => notes_model(&query, &shown.notes, sel, show_detail),
        Mode::CreateNote(draft) => create_note_model(&draft, &query),
        Mode::Quicklinks => quicklinks_model(&query, &shown.quicklinks, sel, show_detail),
        Mode::QuicklinkArg { link, .. } => quicklink_arg_model(&link, &query),
        Mode::SetAlias {
            title,
            current_alias,
            ..
        } => set_alias_model(&title, current_alias.as_deref(), &query),
        Mode::CreateQuicklink(draft) => create_quicklink_model(&draft, &query),
        Mode::RunShell => run_shell_model(&query, shown.last_shell_output.as_deref()),
        Mode::Files => {
            let rows = shown.file_rows.clone();
            let filter = shown.file_filter;
            let current_dir = shown.file_dir.clone();
            let indexing = ctx.is_some_and(|c| c.features.is_file_indexing());
            files_model(
                &query,
                &rows,
                filter,
                current_dir.as_deref(),
                indexing,
                sel,
                show_detail,
                |target| shown.lookup_or_queue_icon(target),
            )
        }
        Mode::SwitchWindows => windows_model(&query, &shown.windows, sel),
        Mode::CalcHistory => calc_history_model(
            &query,
            shown.calc_live.as_ref(),
            &shown.calc_entries,
            sel,
            show_detail,
        ),
        Mode::ColorHistory => {
            let ctx = ctx?;
            let fmt = tools::configured_color_format(&*ctx.settings);
            color_history_model(&query, &shown.color_entries, sel, show_detail, fmt)
        }
        Mode::OcrPreview => {
            let state = shown.ocr_preview.as_ref()?;
            ocr_preview_model(state, sel)
        }
        Mode::Settings => settings_model_for(&query, &shown.settings_rows, sel),
        Mode::Status => return None,
    };
    apply_recording_overlay(&mut model, shown);
    if toast.is_some() {
        model.toast = toast;
    }
    Some(model)
}

fn resolve_alias_row(
    launcher: &LauncherState,
    ctx: &NativeContext,
    entry_id: &str,
) -> Option<ResultRow> {
    if let Some(row) = launcher.row_for_entry_id(entry_id) {
        return Some(row);
    }
    if let Some(sid) = entry_id.strip_prefix(INLINE_SNIPPET_PREFIX) {
        return ctx
            .features
            .search_snippets("")
            .into_iter()
            .find(|s| s.id() == sid)
            .map(|s| inline_snippet_row(&s));
    }
    if let Some(ql_name) = entry_id.strip_prefix(ROOT_QUICKLINK_PREFIX) {
        return ctx
            .features
            .search_quicklinks("")
            .into_iter()
            .find(|l| l.name.eq_ignore_ascii_case(ql_name))
            .map(|l| root_quicklink_row(&l));
    }
    None
}

/// Compose Root Search results in Raycast priority order:
/// Calculator hero card → Exact Alias Match → Installed Applications →
/// Quicklinks & Snippets → Built-in & Custom Commands → Windows Settings →
/// Window/System Actions → Indexed Files → Fallback Web Search.
fn build_root_search_rows(
    launcher: &LauncherState,
    ctx: Option<&NativeContext>,
    query: &str,
) -> Vec<ResultRow> {
    let base = launcher.search(query);
    let mut calc_rows = Vec::new();
    let mut app_rows = Vec::new();
    let mut command_rows = Vec::new();
    let mut other_rows = Vec::new();
    let mut file_rows = Vec::new();

    for row in base {
        match row.kind.as_str() {
            CALCULATOR_SECTION => calc_rows.push(row),
            APPLICATIONS => app_rows.push(row),
            COMMAND_SECTION | CUSTOM_COMMAND_SECTION => command_rows.push(row),
            FILE_SECTION => file_rows.push(row),
            _ => other_rows.push(row),
        }
    }

    let mut exact_alias_rows = Vec::new();
    let mut prefix_alias_rows = Vec::new();
    let mut exact_snippet: Option<ResultRow> = None;
    let mut exact_snippet_id: Option<String> = None;
    let mut snippet_rows = Vec::new();
    let mut quicklink_rows = Vec::new();

    if let Some(ctx) = ctx {
        let (exact_ids, prefix_ids) = hotkeys::find_alias_matches(&*ctx.settings, query);
        for id in exact_ids {
            if let Some(row) = resolve_alias_row(launcher, ctx, &id) {
                exact_alias_rows.push(row);
            }
        }
        for id in prefix_ids {
            if let Some(row) = resolve_alias_row(launcher, ctx, &id) {
                prefix_alias_rows.push(row);
            }
        }

        if let Some(snippet) = ctx.features.find_snippet_by_keyword(query) {
            exact_snippet_id = Some(snippet.id());
            exact_snippet = Some(inline_snippet_row(&snippet));
        }
        let trimmed = query.trim();
        if trimmed.chars().count() >= 2 {
            for link in ctx.features.search_quicklinks(trimmed).into_iter().take(5) {
                quicklink_rows.push(root_quicklink_row(&link));
            }
            for snippet in ctx.features.search_snippets(trimmed).into_iter().take(4) {
                if exact_snippet_id.as_deref() == Some(snippet.id().as_str()) {
                    continue;
                }
                snippet_rows.push(inline_snippet_row(&snippet));
            }
        }
    }

    let win_setting_rows: Vec<ResultRow> = win_settings::search(query, 8)
        .into_iter()
        .map(win_settings::to_result_row)
        .collect();

    let mut seen: HashSet<String> = HashSet::new();
    let mut out = Vec::with_capacity(
        calc_rows.len()
            + exact_alias_rows.len()
            + prefix_alias_rows.len()
            + app_rows.len()
            + quicklink_rows.len()
            + snippet_rows.len()
            + command_rows.len()
            + win_setting_rows.len()
            + other_rows.len()
            + file_rows.len()
            + 1,
    );

    let mut push_unique = |row: ResultRow| {
        if row.kind == CALCULATOR_SECTION || seen.insert(row.id.clone()) {
            out.push(row);
        }
    };

    for row in calc_rows {
        push_unique(row);
    }
    for row in exact_alias_rows {
        push_unique(row);
    }
    let bang_snippet = query.trim_start().starts_with('!');
    if bang_snippet {
        if let Some(s) = exact_snippet.take() {
            push_unique(s);
        }
    }
    for row in prefix_alias_rows {
        push_unique(row);
    }
    for row in app_rows {
        push_unique(row);
    }
    if let Some(s) = exact_snippet {
        push_unique(s);
    }
    for row in quicklink_rows {
        push_unique(row);
    }
    for row in snippet_rows {
        push_unique(row);
    }
    for row in command_rows {
        push_unique(row);
    }
    for row in win_setting_rows {
        push_unique(row);
    }
    for row in other_rows {
        push_unique(row);
    }
    for row in file_rows {
        push_unique(row);
    }
    drop(push_unique);

    // Raycast's `Use “<query>” with…` fallback commands section always appears
    // at the bottom of Root Search whenever the user has typed 2+ characters.
    if query.trim().chars().count() >= 2 {
        if let Some(ctx) = ctx {
            let links = ctx.features.search_quicklinks("");
            let files_fb_id = format!("{FALLBACK_CMD_PREFIX}search-files");
            let snippets_fb_id = format!("{FALLBACK_CMD_PREFIX}search-snippets");
            let has_files_cmd = seen.contains(CommandId::SearchFiles.entry_id());
            let has_snippets_cmd = seen.contains(CommandId::SearchSnippets.entry_id());
            for fb in fallback_search_rows(query, &links) {
                if fb.id == files_fb_id && has_files_cmd {
                    continue;
                }
                if fb.id == snippets_fb_id && has_snippets_cmd {
                    continue;
                }
                if seen.insert(fb.id.clone()) {
                    out.push(fb);
                }
            }
        }
    }

    out
}

fn enter_launcher(
    launcher: &LauncherState,
    ctx: Option<&NativeContext>,
    shown: &mut Shown,
    clear_edit: bool,
) -> PaletteReply {
    let rows = suggestions_with_favorites(launcher, ctx);
    shown.mode = Mode::Launcher;
    shown.query.clear();
    shown.selected = 0;
    shown.action_menu_open = false;
    shown.compact_expanded = false;
    shown.rows = rows.clone();
    shown.settings_rows.clear();
    let model = model_for_with_icons("", &rows, true, 0, ctx, shown);
    if clear_edit {
        PaletteReply::Navigate(model)
    } else {
        PaletteReply::Update(model)
    }
}

fn enter_quicklink_arg(
    link: Quicklink,
    from_launcher: bool,
    previous_query: String,
    shown: &mut Shown,
) -> PaletteReply {
    shown.mode = Mode::QuicklinkArg {
        link: link.clone(),
        from_launcher,
        previous_query,
    };
    shown.query.clear();
    shown.selected = 0;
    shown.action_menu_open = false;
    PaletteReply::UpdateWithEdit(String::new(), quicklink_arg_model(&link, ""))
}

fn suggestions_with_favorites(
    launcher: &LauncherState,
    ctx: Option<&NativeContext>,
) -> Vec<ResultRow> {
    let mut base = launcher.suggestions();
    let Some(ctx) = ctx else {
        return base;
    };
    let fav_ids = ctx.features.favorite_ids();
    if fav_ids.is_empty() {
        return base;
    }
    let mut favs = Vec::new();
    for id in &fav_ids {
        if let Some(pos) = base.iter().position(|r| r.id == *id) {
            favs.push(base.remove(pos));
        }
    }
    favs.extend(base);
    favs
}

fn enter_notes(ctx: &NativeContext, shown: &mut Shown) -> PaletteReply {
    shown.mode = Mode::Notes;
    shown.query.clear();
    shown.selected = 0;
    shown.action_menu_open = false;
    let list = ctx.features.search_notes("");
    shown.notes = list.clone();
    PaletteReply::Navigate(notes_model("", &list, 0, shown.show_detail))
}

fn enter_create_note(shown: &mut Shown) -> PaletteReply {
    let draft = NoteDraft::new();
    shown.mode = Mode::CreateNote(draft.clone());
    shown.query.clear();
    shown.selected = 0;
    shown.action_menu_open = false;
    PaletteReply::Navigate(create_note_model(&draft, ""))
}

fn enter_quicklinks(ctx: &NativeContext, shown: &mut Shown) -> PaletteReply {
    shown.mode = Mode::Quicklinks;
    shown.query.clear();
    shown.selected = 0;
    shown.action_menu_open = false;
    let list = ctx.features.search_quicklinks("");
    shown.quicklinks = list.clone();
    PaletteReply::Navigate(quicklinks_model("", &list, 0, shown.show_detail))
}

fn enter_create_quicklink(shown: &mut Shown) -> PaletteReply {
    let draft = QuicklinkDraft::new();
    shown.mode = Mode::CreateQuicklink(draft.clone());
    shown.query.clear();
    shown.selected = 0;
    shown.action_menu_open = false;
    PaletteReply::Navigate(create_quicklink_model(&draft, ""))
}

fn enter_run_shell(shown: &mut Shown) -> PaletteReply {
    shown.mode = Mode::RunShell;
    shown.query.clear();
    shown.selected = 0;
    shown.action_menu_open = false;
    let last = shown.last_shell_output.clone();
    PaletteReply::Navigate(run_shell_model("", last.as_deref()))
}

fn run_export_backup(ctx: &NativeContext) -> PaletteReply {
    match ctx.features.export_backup_to_desktop() {
        Ok((path, count)) => {
            let path_str = path.to_string_lossy().to_string();
            let _ = tc_platform_win::reveal(&path_str);
            PaletteReply::Navigate(status_screen_model(
                "Backup Exported",
                &format!("Saved {count} files with SHA-256 checksum"),
                &path_str,
                Vec::new(),
            ))
        }
        Err(err) => PaletteReply::Navigate(status_screen_model(
            "Backup Failed",
            "Could not export backup",
            &err,
            Vec::new(),
        )),
    }
}

/// Show the Windows 11 native file picker on a dedicated STA thread (never inside
/// `WndProc`), restore & hot-reload all stores, then re-open the palette with status.
fn spawn_import_backup_dialog(
    ctx: Arc<NativeContext>,
    launcher: Arc<LauncherState>,
    shown: Arc<Mutex<Shown>>,
    handle_slot: Arc<Mutex<Option<PaletteHandle>>>,
) {
    let _ = std::thread::Builder::new()
        .name("yal-file-dialog".to_string())
        .spawn(move || {
            let picked = tc_platform_win::pick_file(
                "Select YAL Backup (.yalbackup.json)",
                "YAL Backup (*.json)",
                "*.json",
            );
            let status = match picked {
                None => (
                    "Import Cancelled".to_string(),
                    "No backup file selected".to_string(),
                    "Press Esc to return to Launcher".to_string(),
                    false,
                ),
                Some(path) => match ctx
                    .features
                    .import_backup_from(&path, &ctx.settings, &launcher)
                {
                    Ok(count) => {
                        let wm_on = ctx.settings.get(KEY_WINDOW_MANAGEMENT_ENABLED).as_deref()
                            != Some("false");
                        launcher.set_show_window_commands(wm_on);
                        let fs_show =
                            ctx.settings.get(FILE_SEARCH_SHOW).as_deref() != Some("false");
                        launcher.set_show_files(fs_show);
                        ctx.hotkeys.reload_bindings(&ctx.settings);
                        ctx.hotkeys
                            .set_hyperkey_enabled(hotkeys::hyperkey_enabled(&*ctx.settings));
                        ctx.hotkeys
                            .sync_snippet_keywords(&ctx.settings, Arc::clone(&ctx.features));
                        (
                            "Backup Restored".to_string(),
                            format!("Verified SHA-256 & restored {count} files"),
                            path.to_string_lossy().to_string(),
                            true,
                        )
                    }
                    Err(err) => (
                        "Import Failed".to_string(),
                        "Backup verification or restore failed".to_string(),
                        err,
                        false,
                    ),
                },
            };
            if let Ok(mut guard) = shown.lock() {
                guard.pending_status = Some(status);
            }
            if let Ok(guard) = handle_slot.lock() {
                if let Some(handle) = *guard {
                    tc_ui::post_command(handle, COMMAND_SHOW_STATUS);
                }
            }
        });
}

/// Show the Windows 11 native file picker on a dedicated STA thread for Raycast snippets.
fn spawn_import_raycast_dialog(
    ctx: Arc<NativeContext>,
    shown: Arc<Mutex<Shown>>,
    handle_slot: Arc<Mutex<Option<PaletteHandle>>>,
) {
    let _ = std::thread::Builder::new()
        .name("yal-file-dialog".to_string())
        .spawn(move || {
            let picked = tc_platform_win::pick_file(
                "Select Raycast Snippets JSON Export",
                "Raycast Export (*.json)",
                "*.json",
            );
            let status = match picked {
                None => (
                    "Import Cancelled".to_string(),
                    "No Raycast file selected".to_string(),
                    "Press Esc to return to Launcher".to_string(),
                    false,
                ),
                Some(path) => match ctx.features.import_raycast_snippets_from(&path) {
                    Ok(count) => {
                        ctx.hotkeys
                            .sync_snippet_keywords(&ctx.settings, Arc::clone(&ctx.features));
                        (
                            "Raycast Import".to_string(),
                            format!("Imported {count} Raycast snippets"),
                            "Open Search Snippets to browse & expand them".to_string(),
                            false,
                        )
                    }
                    Err(err) => (
                        "Import Failed".to_string(),
                        "Could not import Raycast snippets".to_string(),
                        err,
                        false,
                    ),
                },
            };
            if let Ok(mut guard) = shown.lock() {
                guard.pending_status = Some(status);
            }
            if let Ok(guard) = handle_slot.lock() {
                if let Some(handle) = *guard {
                    tc_ui::post_command(handle, COMMAND_SHOW_STATUS);
                }
            }
        });
}

fn enter_about(launcher: &LauncherState, shown: &mut Shown) -> PaletteReply {
    shown.mode = Mode::Status;
    shown.query.clear();
    shown.selected = 0;
    shown.action_menu_open = false;
    let extra = vec![
        Row::detailed(
            "Pure Win32 + Direct2D + DirectWrite",
            "Zero WebView2, zero Node, 0% idle CPU",
        )
        .with_section("Runtime")
        .with_icon(Icon::new("⚡", Color::rgb(0x34, 0xd3, 0x99))),
        Row::detailed(
            format!("{} Indexed Applications", launcher.program_count()),
            "Start Menu, Microsoft Store/MSIX, Registry & App Paths",
        )
        .with_section("Runtime")
        .with_icon(Icon::new("⊞", Color::rgb(0x38, 0xbd, 0xf8))),
        Row::detailed(
            format!(
                "{} Windows Settings & Admin Tools",
                win_settings::all().len()
            ),
            "Instant ms-settings: URIs and classic Control Panel applets",
        )
        .with_section("Runtime")
        .with_icon(Icon::new("⚙", Color::rgb(0x0e, 0xa5, 0xe9))),
    ];
    PaletteReply::Navigate(status_screen_model(
        "About YAL",
        &format!("YAL v{}", env!("CARGO_PKG_VERSION")),
        "Yet Another Launcher — Fast Native Windows Launcher",
        extra,
    ))
}

fn enter_clipboard(ctx: &NativeContext, shown: &mut Shown) -> PaletteReply {
    ctx.palette.open_screen("Clipboard History");
    shown.mode = Mode::Clipboard;
    shown.query.clear();
    shown.selected = 0;
    shown.action_menu_open = false;
    shown.clipboard_filter = ClipboardViewFilter::All;
    let items = ctx
        .features
        .search_clipboard_view("", ClipboardViewFilter::All);
    shown.clipboard_items = items.clone();
    PaletteReply::Navigate(build_clipboard_screen_model(ctx, shown, "", &items, 0))
}

fn enter_emoji(ctx: &NativeContext, shown: &mut Shown) -> PaletteReply {
    ctx.palette.open_screen("Emoji & Symbols");
    shown.mode = Mode::Emoji;
    shown.query.clear();
    shown.selected = 0;
    shown.action_menu_open = false;
    let cells = ctx.features.emoji_cells("");
    shown.emoji_cells = cells.clone();
    PaletteReply::Navigate(emoji_model("", cells, 0))
}

fn enter_snippets(ctx: &NativeContext, shown: &mut Shown) -> PaletteReply {
    ctx.palette.open_screen("Snippets");
    shown.mode = Mode::Snippets;
    shown.query.clear();
    shown.selected = 0;
    shown.action_menu_open = false;
    let list = ctx.features.search_snippets("");
    shown.snippets = list.clone();
    PaletteReply::Navigate(snippets_model("", &list, 0, shown.show_detail, |rec| {
        ctx.features.expand_snippet(rec)
    }))
}

fn enter_create_snippet(shown: &mut Shown) -> PaletteReply {
    let draft = SnippetDraft::new();
    shown.mode = Mode::CreateSnippet(draft.clone());
    shown.query.clear();
    shown.selected = 0;
    shown.action_menu_open = false;
    PaletteReply::Navigate(create_snippet_model(&draft, ""))
}

fn enter_files(
    launcher: &Arc<LauncherState>,
    ctx: &NativeContext,
    handle_slot: &Arc<Mutex<Option<PaletteHandle>>>,
    shown: &mut Shown,
) -> PaletteReply {
    ctx.palette.open_screen("File Search");
    shown.mode = Mode::Files;
    shown.query.clear();
    shown.selected = 0;
    shown.action_menu_open = false;
    shown.file_filter = FileType::All;
    shown.file_dir = None;
    shown.file_dir_cache = None;
    if !launcher.has_file_search() {
        let _ = ctx.settings.set(FILE_SEARCH_ENABLED, "true");
        ctx.features
            .trigger_file_index(Arc::clone(launcher), Some(Arc::clone(handle_slot)));
    }
    let rows = shown.filtered_file_rows(launcher, "");
    let indexing = ctx.features.is_file_indexing();
    let show_detail = shown.show_detail;
    shown.file_rows = rows.clone();
    PaletteReply::Navigate(files_model(
        "",
        &rows,
        FileType::All,
        None,
        indexing,
        0,
        show_detail,
        |target| shown.lookup_or_queue_icon(target),
    ))
}

fn enter_switch_windows(shown: &mut Shown) -> PaletteReply {
    shown.mode = Mode::SwitchWindows;
    shown.query.clear();
    shown.selected = 0;
    shown.action_menu_open = false;
    let windows = filter_windows("");
    shown.windows = windows.clone();
    PaletteReply::Navigate(windows_model("", &windows, 0))
}

fn enter_calc_history(
    ctx: &NativeContext,
    launcher: &LauncherState,
    shown: &mut Shown,
) -> PaletteReply {
    shown.mode = Mode::CalcHistory;
    shown.query.clear();
    shown.selected = 0;
    shown.action_menu_open = false;
    shown.calc_live = None;
    let entries = ctx.features.search_calc_history("");
    if launcher.last_calc_answer().is_none() {
        if let Some(ans) = entries
            .iter()
            .find_map(|e| parse_numeric_answer(&e.copy_text))
        {
            launcher.set_last_calc_answer(Some(ans));
        }
    }
    shown.calc_entries = entries.clone();
    PaletteReply::Navigate(calc_history_model("", None, &entries, 0, shown.show_detail))
}

fn enter_color_history(
    ctx: &NativeContext,
    shown: &mut Shown,
    select_hex: Option<&str>,
) -> PaletteReply {
    ctx.palette.open_screen("Color Inspector");
    shown.mode = Mode::ColorHistory;
    shown.query.clear();
    shown.action_menu_open = false;
    let entries = ctx.features.color_history_items("");
    let selected = select_hex
        .and_then(|hex| {
            entries
                .iter()
                .position(|entry| entry.hex.eq_ignore_ascii_case(hex))
        })
        .unwrap_or(0);
    shown.selected = selected;
    shown.color_entries = entries.clone();
    let fmt = tools::configured_color_format(&*ctx.settings);
    PaletteReply::Navigate(color_history_model(
        "",
        &entries,
        selected,
        shown.show_detail,
        fmt,
    ))
}

fn enter_settings(ctx: &NativeContext, shown: &mut Shown) -> PaletteReply {
    ctx.palette.open_screen("Settings");
    let rows = collect_settings_rows(ctx, "");
    shown.mode = Mode::Settings;
    shown.query.clear();
    shown.selected = 0;
    shown.action_menu_open = false;
    shown.rows.clear();
    shown.settings_rows = rows.clone();
    PaletteReply::Navigate(settings_model_for("", &rows, 0))
}

fn filter_windows(query: &str) -> Vec<WindowInfo> {
    let q = query.trim().to_ascii_lowercase();
    let all = tc_platform_win::window_control::list_switchable_windows();
    if q.is_empty() {
        return all;
    }
    all.into_iter()
        .filter(|w| {
            w.title.to_ascii_lowercase().contains(&q) || w.class.to_ascii_lowercase().contains(&q)
        })
        .collect()
}

fn inline_snippet_row(record: &StoredSnippet) -> ResultRow {
    let name = match record
        .snippet
        .keyword
        .as_deref()
        .map(str::trim)
        .filter(|k| !k.is_empty())
    {
        Some(k) => format!("{}  (!{})", record.snippet.name, k.trim_start_matches('!')),
        None => record.snippet.name.clone(),
    };
    let preview: String = record
        .snippet
        .text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(64)
        .collect();
    ResultRow {
        id: format!("{INLINE_SNIPPET_PREFIX}{}", record.id()),
        name,
        detail: preview,
        kind: SNIPPET_SECTION.to_string(),
        target: record.id().to_string(),
        ..ResultRow::default()
    }
}

fn root_quicklink_row(link: &Quicklink) -> ResultRow {
    ResultRow {
        id: format!("{ROOT_QUICKLINK_PREFIX}{}", link.name),
        name: link.name.clone(),
        detail: link.url_template.clone(),
        kind: QUICKLINK_SECTION.to_string(),
        target: link.url_template.clone(),
        ..ResultRow::default()
    }
}

fn theme_from_settings(settings: &WindowsSettings) -> Theme {
    let appearance = settings
        .get(tc_palette::KEY_APPEARANCE)
        .unwrap_or_else(|| "System".to_string());
    match appearance.trim().to_ascii_lowercase().as_str() {
        "light" => Theme::light(),
        "dark" => Theme::dark(),
        _ => {
            if tc_platform_win::apps_use_dark_theme() {
                Theme::dark()
            } else {
                Theme::light()
            }
        }
    }
}

/// Collect all native Settings rows (Hotkey + Feature Toggles + General), filtered by query.
fn collect_settings_rows(ctx: &NativeContext, query: &str) -> Vec<SettingRow> {
    let mut rows: Vec<SettingRow> = ctx
        .palette
        .settings_rows()
        .into_iter()
        .filter(|r| r.key != tc_palette::KEY_POP_TO_ROOT)
        .collect();

    let current_hotkey = hotkeys::configured_launcher_hotkey(&*ctx.settings).format();
    let hyperkey_on = hotkeys::hyperkey_enabled(&*ctx.settings);
    let snippets_expand_on = hotkeys::snippets_auto_expand_enabled(&*ctx.settings);
    let wm_on = ctx.settings.get(KEY_WINDOW_MANAGEMENT_ENABLED).as_deref() != Some("false");
    let fs_on = ctx.settings.get(FILE_SEARCH_ENABLED).as_deref() != Some("false");
    let fs_show = ctx.settings.get(FILE_SEARCH_SHOW).as_deref() != Some("false");
    let pop_policy = pop_to_root_policy(&ctx.settings);

    let has_custom_pos = ctx
        .settings
        .get(KEY_POSITION_X)
        .and_then(|s| s.trim().parse::<i32>().ok())
        .is_some()
        && ctx
            .settings
            .get(KEY_POSITION_Y)
            .and_then(|s| s.trim().parse::<i32>().ok())
            .is_some();

    let ocr_language_value = tools::ocr_language_display(
        &tools::configured_ocr_language(&*ctx.settings),
        &tc_platform_win::available_ocr_languages(),
    );
    let ocr_mode_value = match tools::configured_ocr_mode(&*ctx.settings) {
        OcrFormatMode::Smart => "Smart (NormCap)",
        OcrFormatMode::Raw => "Raw",
    }
    .to_string();
    let color_picker_format_value = tools::configured_color_format(&*ctx.settings)
        .label()
        .to_string();

    let mut extra = vec![
        SettingRow {
            key: KEY_RECORD_LAUNCHER_HOTKEY.to_string(),
            label: "Record Launcher Hotkey".to_string(),
            value: current_hotkey.clone(),
            options: Vec::new(),
        },
        SettingRow {
            key: KEY_HOTKEY_LAUNCHER.to_string(),
            label: "Launcher Hotkey Preset".to_string(),
            value: current_hotkey,
            options: HOTKEY_PRESETS.iter().map(|s| (*s).to_string()).collect(),
        },
        SettingRow {
            key: KEY_HOTKEY_HYPERKEY.to_string(),
            label: "CapsLock Hyperkey".to_string(),
            value: if hyperkey_on { "true" } else { "false" }.to_string(),
            options: vec!["true".to_string(), "false".to_string()],
        },
        SettingRow {
            key: KEY_OCR_LANGUAGE.to_string(),
            label: "OCR Language".to_string(),
            value: ocr_language_value,
            options: Vec::new(),
        },
        SettingRow {
            key: KEY_OCR_MODE.to_string(),
            label: "OCR Formatting Mode".to_string(),
            value: ocr_mode_value,
            options: vec!["Smart (NormCap)".to_string(), "Raw".to_string()],
        },
        SettingRow {
            key: KEY_COLOR_PICKER_FORMAT.to_string(),
            label: "Color Picker Default Format".to_string(),
            value: color_picker_format_value,
            options: vec![
                "HEX".to_string(),
                "RGB".to_string(),
                "HSL".to_string(),
                "OKLCH".to_string(),
            ],
        },
    ];

    for &(entry_id, label, _) in hotkeys::DEFAULT_COMMAND_HOTKEYS {
        let current = hotkeys::get_command_hotkey(&*ctx.settings, entry_id)
            .map(|t| t.format())
            .unwrap_or_else(|| "Not Set".to_string());
        extra.push(SettingRow {
            key: format!("{KEY_RECORD_CMD_HOTKEY_PREFIX}{entry_id}"),
            label: format!("{label} Hotkey"),
            value: current,
            options: Vec::new(),
        });
    }

    extra.extend([
        SettingRow {
            key: KEY_SNIPPETS_AUTO_EXPAND.to_string(),
            label: "Auto-Expand Snippets".to_string(),
            value: if snippets_expand_on { "true" } else { "false" }.to_string(),
            options: vec!["true".to_string(), "false".to_string()],
        },
        SettingRow {
            key: KEY_WINDOW_MANAGEMENT_ENABLED.to_string(),
            label: "Window Management".to_string(),
            value: if wm_on { "true" } else { "false" }.to_string(),
            options: vec!["true".to_string(), "false".to_string()],
        },
        SettingRow {
            key: FILE_SEARCH_ENABLED.to_string(),
            label: "File Search Indexing".to_string(),
            value: if fs_on { "true" } else { "false" }.to_string(),
            options: vec!["true".to_string(), "false".to_string()],
        },
        SettingRow {
            key: FILE_SEARCH_SHOW.to_string(),
            label: "Show Files in Launcher".to_string(),
            value: if fs_show { "true" } else { "false" }.to_string(),
            options: vec!["true".to_string(), "false".to_string()],
        },
    ]);
    for (idx, item) in extra.into_iter().enumerate() {
        rows.insert(idx, item);
    }

    rows.push(SettingRow {
        key: KEY_RESET_WINDOW_POSITION.to_string(),
        label: "Reset Window Position".to_string(),
        value: if has_custom_pos { "Custom" } else { "Centered" }.to_string(),
        options: Vec::new(),
    });

    rows.push(SettingRow {
        key: KEY_POP_TO_ROOT_SECS.to_string(),
        label: "Pop to Root Search".to_string(),
        value: pop_policy.label().to_string(),
        options: vec![
            "Immediately (0s)".to_string(),
            "30 seconds".to_string(),
            "1 minute".to_string(),
            "5 minutes".to_string(),
            "Never".to_string(),
        ],
    });

    let trimmed = query.trim().to_ascii_lowercase();
    if trimmed.is_empty() {
        return rows;
    }
    rows.into_iter()
        .filter(|row| {
            row.label.to_ascii_lowercase().contains(&trimmed)
                || row.value.to_ascii_lowercase().contains(&trimmed)
        })
        .collect()
}

/// Cycle one setting in native Settings, persist it, and apply it live.
fn advance_native_setting(
    ctx: &NativeContext,
    launcher: &Arc<LauncherState>,
    handle_slot: &Arc<Mutex<Option<PaletteHandle>>>,
    key: &str,
) -> Option<ToastModel> {
    if key == KEY_HOTKEY_LAUNCHER || key == KEY_LAUNCHER_HOTKEY {
        let current = hotkeys::configured_launcher_hotkey(&*ctx.settings).format();
        let pos = HOTKEY_PRESETS
            .iter()
            .position(|preset| preset.eq_ignore_ascii_case(&current))
            .map(|i| (i + 1) % HOTKEY_PRESETS.len())
            .unwrap_or(0);
        let next = HOTKEY_PRESETS[pos];
        if let Some(trigger) = HotkeyTrigger::parse(next) {
            let _ = hotkeys::save_launcher_hotkey(&*ctx.settings, trigger);
            ctx.hotkeys.reload_bindings(&ctx.settings);
        }
        return None;
    }

    if key == KEY_HOTKEY_HYPERKEY {
        let now_on = !hotkeys::hyperkey_enabled(&*ctx.settings);
        let _ = hotkeys::set_hyperkey_enabled_setting(&*ctx.settings, now_on);
        ctx.hotkeys.set_hyperkey_enabled(now_on);
        return None;
    }

    if key == KEY_OCR_LANGUAGE {
        return match tools::cycle_ocr_language(&*ctx.settings) {
            Ok(label) => Some(ToastModel {
                kind: ToastKind::Success,
                message: format!("OCR language: {label}"),
            }),
            Err(msg) => {
                tc_ui::show_hud(msg, tools::settings_prefers_dark(&*ctx.settings));
                Some(ToastModel {
                    kind: ToastKind::Info,
                    message: msg.to_string(),
                })
            }
        };
    }

    if key == KEY_OCR_MODE {
        let next = tools::toggle_ocr_mode(&*ctx.settings);
        return Some(ToastModel {
            kind: ToastKind::Success,
            message: format!("OCR mode: {}", next.label()),
        });
    }

    if key == KEY_COLOR_PICKER_FORMAT {
        let next = tools::cycle_color_format(&*ctx.settings);
        return Some(ToastModel {
            kind: ToastKind::Success,
            message: format!("Color format: {}", next.label()),
        });
    }

    if key == KEY_SNIPPETS_AUTO_EXPAND {
        let now_on = !hotkeys::snippets_auto_expand_enabled(&*ctx.settings);
        let _ = hotkeys::set_snippets_auto_expand_setting(&*ctx.settings, now_on);
        ctx.hotkeys
            .sync_snippet_keywords(&ctx.settings, Arc::clone(&ctx.features));
        return None;
    }

    if key == KEY_WINDOW_MANAGEMENT_ENABLED {
        let currently_on =
            ctx.settings.get(KEY_WINDOW_MANAGEMENT_ENABLED).as_deref() != Some("false");
        let now_on = !currently_on;
        let _ = ctx.settings.set(
            KEY_WINDOW_MANAGEMENT_ENABLED,
            if now_on { "true" } else { "false" },
        );
        launcher.set_show_window_commands(now_on);
        return None;
    }

    if key == KEY_POP_TO_ROOT_SECS {
        let next = pop_to_root_policy(&ctx.settings).next();
        let _ = ctx.settings.set(KEY_POP_TO_ROOT_SECS, next.stored_value());
        return None;
    }

    if key == FILE_SEARCH_ENABLED {
        let currently_on = ctx.settings.get(FILE_SEARCH_ENABLED).as_deref() != Some("false");
        let now_on = !currently_on;
        let _ = ctx
            .settings
            .set(FILE_SEARCH_ENABLED, if now_on { "true" } else { "false" });
        if now_on {
            ctx.features
                .trigger_file_index(Arc::clone(launcher), Some(Arc::clone(handle_slot)));
        }
        return None;
    }

    if key == FILE_SEARCH_SHOW {
        let currently_on = ctx.settings.get(FILE_SEARCH_SHOW).as_deref() != Some("false");
        let now_on = !currently_on;
        let _ = ctx
            .settings
            .set(FILE_SEARCH_SHOW, if now_on { "true" } else { "false" });
        launcher.set_show_files(now_on);
        if now_on && !launcher.has_file_search() {
            let _ = ctx.settings.set(FILE_SEARCH_ENABLED, "true");
            ctx.features
                .trigger_file_index(Arc::clone(launcher), Some(Arc::clone(handle_slot)));
        }
        return None;
    }

    if let Some(next) = ctx.palette.advance_setting(key) {
        let _ = ctx.settings.set(key, &next);
    }
    None
}

fn primary_action_for_row(row: Option<&ResultRow>) -> &'static str {
    let Some(row) = row else {
        return "Open";
    };
    match row.kind.as_str() {
        CALCULATOR_SECTION => "Copy Answer",
        SNIPPET_SECTION => "Paste",
        WINDOW_COMMAND_SECTION => "Apply",
        SYSTEM_ACTION_SECTION | CUSTOM_COMMAND_SECTION => "Run",
        WIN_SETTING_SECTION => "Open Setting",
        _ => "Open",
    }
}

fn is_compact_window_mode(ctx: Option<&NativeContext>) -> bool {
    let Some(ctx) = ctx else {
        return false;
    };
    !ctx.settings
        .get(tc_palette::KEY_WINDOW_MODE)
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("expanded"))
}

fn model_for_with_icons(
    query: &str,
    rows: &[ResultRow],
    is_suggestions: bool,
    selected: usize,
    ctx: Option<&NativeContext>,
    shown: &mut Shown,
) -> Model {
    let clamped_sel = selected.min(rows.len().saturating_sub(1));
    let ui_rows: Vec<Row> = rows
        .iter()
        .enumerate()
        .map(|(idx, result)| {
            let mut row = row_of(result, query, is_suggestions);
            if result.kind == APPLICATIONS
                && ctx.is_some_and(|c| c.features.is_favorite(&result.id))
            {
                if is_suggestions {
                    row.section = "Favorites".to_string();
                }
                row.shortcut = "★".to_string();
            }
            if let Some(ctx) = ctx {
                if let Some(alias) = hotkeys::get_alias(&*ctx.settings, &result.id) {
                    if row.detail.is_empty() {
                        row.detail = format!("alias: {alias}");
                    } else {
                        row.detail = format!("{} · alias: {alias}", row.detail);
                    }
                }
                if let Some(hk) = hotkeys::get_command_hotkey(&*ctx.settings, &result.id) {
                    row.shortcut = hk.format();
                }
            }
            if idx < 30 && matches!(result.kind.as_str(), APPLICATIONS | FILE_SECTION) {
                if let Some(bitmap) = shown.lookup_or_queue_icon(&result.target) {
                    row = row.with_bitmap(result.target.clone(), bitmap);
                }
            }
            row
        })
        .collect();

    let count_label = match ui_rows.len() {
        0 => "No results".to_string(),
        1 => "1 result".to_string(),
        n => format!("{n} results"),
    };

    let primary = primary_action_for_row(rows.get(clamped_sel));
    let hints = vec![
        ("↵".to_string(), primary.to_string()),
        ("Ctrl+K".to_string(), "Actions".to_string()),
    ];

    let compact = is_suggestions && !shown.compact_expanded && is_compact_window_mode(ctx);

    Model {
        query: query.to_string(),
        rows: ui_rows,
        selected: clamped_sel,
        hovered: None,
        footer: count_label.clone(),
        placeholder: "Search for apps or commands…".to_string(),
        header_badge: None,
        result_count: Some(count_label),
        footer_hints: hints,
        scroll_offset: 0.0,
        grid: None,
        back_button: false,
        primary_action: Some(primary.to_string()),
        compact,
        ..Model::default()
    }
}

/// Pure helper without live shell icon lookup (used by unit tests).
#[cfg(test)]
fn model_for(query: &str, rows: &[ResultRow]) -> Model {
    let ui_rows: Vec<Row> = rows
        .iter()
        .map(|r| row_of(r, query, query.trim().is_empty()))
        .collect();
    let primary = primary_action_for_row(rows.first());
    Model {
        query: query.to_string(),
        rows: ui_rows,
        selected: 0,
        hovered: None,
        footer: "Enter to open".to_string(),
        placeholder: "Search apps, settings, commands, or calculate…".to_string(),
        header_badge: None,
        result_count: None,
        footer_hints: vec![
            ("↵".to_string(), primary.to_string()),
            ("Ctrl+K".to_string(), "Actions".to_string()),
        ],
        scroll_offset: 0.0,
        grid: None,
        back_button: false,
        primary_action: Some(primary.to_string()),
        ..Model::default()
    }
}

/// The palette's content for the native Settings screen.
fn settings_model_for(query: &str, rows: &[SettingRow], selected: usize) -> Model {
    let ui_rows: Vec<Row> = rows
        .iter()
        .map(|row| {
            let is_cmd_hotkey = row.key.starts_with(KEY_RECORD_CMD_HOTKEY_PREFIX);
            let section = if is_cmd_hotkey {
                "Action Hotkeys"
            } else {
                match row.key.as_str() {
                    KEY_RECORD_LAUNCHER_HOTKEY
                    | KEY_HOTKEY_LAUNCHER
                    | KEY_LAUNCHER_HOTKEY
                    | KEY_HOTKEY_HYPERKEY => "Hotkeys",
                    KEY_OCR_LANGUAGE | KEY_OCR_MODE | KEY_COLOR_PICKER_FORMAT => {
                        "Screen OCR & Color Picker"
                    }
                    KEY_SNIPPETS_AUTO_EXPAND
                    | KEY_WINDOW_MANAGEMENT_ENABLED
                    | FILE_SEARCH_ENABLED
                    | FILE_SEARCH_SHOW => "Features",
                    _ => "General",
                }
            };
            let glyph = if is_cmd_hotkey {
                "⌨"
            } else {
                match row.key.as_str() {
                    KEY_RECORD_LAUNCHER_HOTKEY => "⏺",
                    KEY_HOTKEY_LAUNCHER | KEY_LAUNCHER_HOTKEY => "⌘",
                    KEY_HOTKEY_HYPERKEY => "✦",
                    KEY_OCR_LANGUAGE => "A",
                    KEY_OCR_MODE => "¶",
                    KEY_COLOR_PICKER_FORMAT => "◉",
                    KEY_SNIPPETS_AUTO_EXPAND => "✂",
                    KEY_WINDOW_MANAGEMENT_ENABLED => "⊞",
                    FILE_SEARCH_ENABLED | FILE_SEARCH_SHOW => "F",
                    KEY_RESET_WINDOW_POSITION => "⌖",
                    KEY_POP_TO_ROOT_SECS => "◷",
                    _ => "⚙",
                }
            };
            let display_value = match row.value.trim().to_ascii_lowercase().as_str() {
                "true" => "On".to_string(),
                "false" => "Off".to_string(),
                _ => row.value.clone(),
            };
            let description = if is_cmd_hotkey {
                "Press Enter to record shortcut · Ctrl+D to clear"
            } else {
                match row.key.as_str() {
                    KEY_RECORD_LAUNCHER_HOTKEY => "Press Enter, then press any chord or double-tap",
                    KEY_HOTKEY_LAUNCHER | KEY_LAUNCHER_HOTKEY => {
                        "Cycle built-in global shortcut presets"
                    }
                    KEY_HOTKEY_HYPERKEY => "Hold CapsLock as Ctrl+Alt+Shift+Win Hyperkey",
                    KEY_OCR_LANGUAGE => {
                        "Cycle Auto (System) and installed Windows OCR language packs"
                    }
                    KEY_OCR_MODE => {
                        "Smart (NormCap) reflows wrapped paragraphs · Raw keeps exact line breaks"
                    }
                    KEY_COLOR_PICKER_FORMAT => {
                        "Default copy notation for Color Picker & Color Inspector (HEX, RGB, HSL, OKLCH)"
                    }
                    KEY_SNIPPETS_AUTO_EXPAND => "Auto-expand snippet keywords globally as you type",
                    KEY_WINDOW_MANAGEMENT_ENABLED => {
                        "Snap windows with Left/Right Half, Maximize, Always on Top"
                    }
                    FILE_SEARCH_ENABLED => "Index your user folders live in background",
                    FILE_SEARCH_SHOW => "Include indexed files inside root Launcher results",
                    KEY_RESET_WINDOW_POSITION => {
                        "Drag header/footer to move · Enter or double-click header to re-center"
                    }
                    KEY_POP_TO_ROOT_SECS => {
                        "Automatically reset sub-screens to Root Search after hiding"
                    }
                    tc_palette::KEY_APPEARANCE => "Color theme",
                    tc_palette::KEY_ANIMATIONS => "Motion & transitions",
                    tc_palette::KEY_INTERFACE_SIZE => "Palette text scale",
                    tc_palette::KEY_WINDOW_MODE => "Open as compact search bar or expanded list",
                    tc_palette::KEY_ESCAPE_BEHAVIOUR => "Action on Esc key",
                    tc_palette::KEY_POP_TO_ROOT => "Reset screen after hiding",
                    _ => "",
                }
            };
            Row {
                name: row.label.clone(),
                detail: description.to_string(),
                shortcut: display_value,
                icon: Some(Icon::new(glyph, Color::rgb(0x5e, 0x8b, 0xff))),
                section: section.to_string(),
                ..Row::default()
            }
        })
        .collect();

    let count_label = match ui_rows.len() {
        1 => "1 setting".to_string(),
        n => format!("{n} settings"),
    };

    Model {
        query: query.to_string(),
        rows: ui_rows,
        selected,
        hovered: None,
        footer: count_label.clone(),
        placeholder: "Filter settings…".to_string(),
        header_badge: Some("Settings".to_string()),
        result_count: Some(count_label),
        footer_hints: vec![
            ("↵".to_string(), "Toggle / Cycle".to_string()),
            ("↑↓".to_string(), "Move".to_string()),
            ("Esc".to_string(), "Back".to_string()),
        ],
        scroll_offset: 0.0,
        grid: None,
        back_button: true,
        primary_action: Some("Toggle".to_string()),
        ..Model::default()
    }
}

/// A result, as the palette draws it.
fn row_of(result: &ResultRow, query: &str, is_suggestions: bool) -> Row {
    if result.kind == CALCULATOR_SECTION {
        let expr = if result.detail.is_empty() {
            query.trim().to_string()
        } else {
            result.detail.clone()
        };
        let source_badge = result
            .calc_source_badge
            .clone()
            .filter(|b| !b.is_empty())
            .unwrap_or_else(|| "Expression".to_string());
        let badge = result
            .calc_badge
            .clone()
            .filter(|b| !b.is_empty())
            .unwrap_or_else(|| "Result".to_string());
        return Row {
            name: result.name.clone(),
            detail: expr,
            shortcut: String::new(),
            icon: Some(Icon::new("=", color_of(&result.kind))),
            section: CALCULATOR_SECTION.to_string(),
            source_badge,
            badge,
            is_calculator: true,
            color_swatch: result.color_swatch,
            ..Row::default()
        };
    }

    let section = if is_suggestions && result.kind == APPLICATIONS {
        "Suggestions".to_string()
    } else {
        result.kind.clone()
    };

    let is_fallback =
        result.id.starts_with(FALLBACK_URL_PREFIX) || result.id.starts_with(FALLBACK_CMD_PREFIX);

    let detail = if result.kind == FILE_SECTION {
        shorten_home_path(&result.detail)
    } else if result.kind == APPLICATIONS
        && matches!(
            result.detail.trim().to_ascii_lowercase().as_str(),
            "start menu"
                | "application path"
                | "desktop"
                | "store app"
                | "microsoft store"
                | "registry"
                | "uninstall"
        )
    {
        String::new()
    } else {
        result.detail.clone()
    };

    let accessory = if is_fallback {
        "Command".to_string()
    } else {
        match result.kind.as_str() {
            APPLICATIONS => "Application".to_string(),
            FILE_SECTION => {
                if result.is_dir == Some(true) {
                    "Folder".to_string()
                } else {
                    "File".to_string()
                }
            }
            COMMAND_SECTION | CUSTOM_COMMAND_SECTION => "Command".to_string(),
            QUICKLINK_SECTION => "Quicklink".to_string(),
            SNIPPET_SECTION => "Snippet".to_string(),
            WIN_SETTING_SECTION | SYSTEM_ACTION_SECTION => "System".to_string(),
            WINDOW_COMMAND_SECTION => "Window".to_string(),
            _ => String::new(),
        }
    };

    let shortcut = if result.kind == QUICKLINK_SECTION
        && (result.target.contains("{query}") || result.target.contains("{argument}"))
    {
        "Tab".to_string()
    } else {
        String::new()
    };

    Row {
        name: result.name.clone(),
        detail,
        accessory,
        shortcut,
        icon: Some(icon_of(result)),
        section,
        ..Row::default()
    }
}

fn icon_of(result: &ResultRow) -> Icon {
    if result.id == format!("{FALLBACK_CMD_PREFIX}search-files") {
        return Icon::new("F", Color::rgb(0x56, 0xc2, 0xff));
    }
    if result.id == format!("{FALLBACK_CMD_PREFIX}search-snippets") {
        return Icon::new("✂", Color::rgb(0x59, 0xd4, 0x99));
    }
    if result.id.starts_with(FALLBACK_URL_PREFIX) {
        return Icon::new("↗", Color::rgb(0x56, 0xc2, 0xff));
    }
    let glyph = match result.kind.as_str() {
        CALCULATOR_SECTION => "=".to_string(),
        COMMAND_SECTION => "›".to_string(),
        CUSTOM_COMMAND_SECTION => ">_".to_string(),
        WINDOW_COMMAND_SECTION => "⊞".to_string(),
        SYSTEM_ACTION_SECTION => "⚡".to_string(),
        FILE_SECTION => {
            if result.is_dir == Some(true) {
                "📁".to_string()
            } else {
                "📄".to_string()
            }
        }
        SNIPPET_SECTION => "✂".to_string(),
        QUICKLINK_SECTION => "↗".to_string(),
        WIN_SETTING_SECTION => "⚙".to_string(),
        _ => letter_of(&result.name),
    };
    Icon::new(glyph, color_of(&result.kind))
}

fn letter_of(name: &str) -> String {
    match name.chars().next() {
        Some(letter) => letter.to_uppercase().to_string(),
        None => "·".to_string(),
    }
}

fn color_of(kind: &str) -> Color {
    match kind {
        APPLICATIONS => Color::rgb(0x56, 0xc2, 0xff),
        FILE_SECTION => Color::rgb(0xff, 0xc5, 0x31),
        COMMAND_SECTION | SNIPPET_SECTION => Color::rgb(0x59, 0xd4, 0x99),
        QUICKLINK_SECTION => Color::rgb(0x56, 0xc2, 0xff),
        WIN_SETTING_SECTION => Color::rgb(0x56, 0xc2, 0xff),
        CUSTOM_COMMAND_SECTION => Color::rgb(0xa4, 0x85, 0xff),
        WINDOW_COMMAND_SECTION => Color::rgb(0xff, 0x92, 0x17),
        SYSTEM_ACTION_SECTION => Color::rgb(0xff, 0x63, 0x63),
        CALCULATOR_SECTION => Color::rgb(0x56, 0xc2, 0xff),
        _ => Color::rgb(0x9a, 0x9a, 0x9a),
    }
}

/// Runs the native palette on its own (`--native-palette`), without tray or global hotkey.
pub fn run(arguments: &[String]) -> Result<(), String> {
    let paths = tc_platform_win::windows_paths();
    let data = tc_platform_win::WindowsFileSystem::new();
    let _ = data.ensure_dir(&paths.data_dir);
    let _ = data.ensure_dir(&paths.cache_dir);

    let launcher = Arc::new(LauncherState::new(paths.data_dir.join(RANKING_FILE)));
    launcher.set_show_window_commands(true);
    let runner = Arc::new(Runner::new(Arc::clone(&launcher)));

    let theme = theme(arguments)?;
    let palette = NativePalette::open(launcher, runner, theme)?;
    palette.show();
    palette.run();
    Ok(())
}

fn theme(arguments: &[String]) -> Result<Theme, String> {
    let position = arguments
        .iter()
        .position(|argument| argument == "--style-pack");
    match position.and_then(|at| arguments.get(at + 1)) {
        Some(path) => {
            let tokens = Tokens::load(std::path::Path::new(path))
                .map_err(|error| format!("the style pack could not be worn: {error}"))?;
            Ok(tokens.theme())
        }
        None => Ok(Theme::dark()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tc_apps::{Program, Source};

    fn result(name: &str, detail: &str, kind: &str) -> ResultRow {
        ResultRow {
            id: format!("app:{name}"),
            name: name.to_string(),
            detail: detail.to_string(),
            kind: kind.to_string(),
            target: name.to_string(),
            needs_confirm: false,
            arguments: Vec::new(),
            is_dir: None,
            calc_source_badge: None,
            calc_badge: None,
            color_swatch: None,
        }
    }

    #[test]
    fn a_result_becomes_a_row_with_its_name_and_detail() {
        let row = row_of(&result("Notepad", "Documents", APPLICATIONS), "no", false);
        assert_eq!(row.name, "Notepad");
        assert_eq!(row.detail, "Documents");
        assert_eq!(row.icon.map(|icon| icon.letter), Some("N".to_string()));
        assert_eq!(row.section, APPLICATIONS);
    }

    #[test]
    fn calculator_result_renders_as_hero_card_with_badge() {
        let mut calc = result("17.00 USD", "20% tip on $85", CALCULATOR_SECTION);
        calc.calc_badge = Some("Tip".to_string());
        let row = row_of(&calc, "20% tip on $85", false);
        assert!(row.is_calculator);
        assert_eq!(row.name, "17.00 USD");
        assert_eq!(row.detail, "20% tip on $85");
        assert_eq!(row.badge, "Tip");
    }

    #[test]
    fn the_letter_is_the_first_one_and_it_is_upper_case() {
        assert_eq!(letter_of("notepad"), "N");
        assert_eq!(letter_of("Éclair"), "É");
        assert_eq!(letter_of("7-Zip"), "7");
    }

    #[test]
    fn a_row_with_no_name_still_has_a_tile() {
        assert_eq!(letter_of(""), "·");
    }

    #[test]
    fn each_section_has_its_own_colour() {
        let applications = color_of(APPLICATIONS);
        let files = color_of(FILE_SECTION);
        let commands = color_of(COMMAND_SECTION);
        assert_ne!(applications, files);
        assert_ne!(files, commands);
        assert_ne!(applications, commands);
    }

    #[test]
    fn a_section_nobody_heard_of_gets_the_neutral_colour() {
        assert_eq!(color_of("Something Else"), Color::rgb(0x9a, 0x9a, 0x9a));
    }

    #[test]
    fn the_model_carries_the_query_and_the_rows() {
        let rows = vec![
            result("Notepad", "Documents", APPLICATIONS),
            result("Notes.txt", "Desktop", FILE_SECTION),
        ];
        let model = model_for("no", &rows);
        assert_eq!(model.query, "no");
        assert_eq!(model.rows.len(), 2);
        assert_eq!(model.rows[0].name, "Notepad");
        assert_eq!(model.rows[1].name, "Notes.txt");
        assert_eq!(model.selected, 0);
        assert!(!model.footer_hints.is_empty());
    }

    #[test]
    fn an_empty_result_set_is_an_empty_list() {
        let model = model_for("nothing", &[]);
        assert!(model.rows.is_empty());
        assert_eq!(model.query, "nothing");
    }

    #[test]
    fn settings_model_displays_current_values_in_shortcut_column() {
        let rows = vec![SettingRow {
            key: KEY_LAUNCHER_HOTKEY.to_string(),
            label: "Launcher Hotkey".to_string(),
            value: "Alt+Space".to_string(),
            options: vec!["Alt+Space".to_string(), "Ctrl+Space".to_string()],
        }];
        let model = settings_model_for("", &rows, 0);
        assert_eq!(model.rows.len(), 1);
        assert_eq!(model.rows[0].name, "Launcher Hotkey");
        assert_eq!(model.rows[0].shortcut, "Alt+Space");
        assert_eq!(model.header_badge.as_deref(), Some("Settings"));
    }

    #[test]
    fn pop_to_root_policy_cycles_and_evaluates_timer_cleanly() {
        assert_eq!(PopToRootPolicy::parse(None), PopToRootPolicy::Immediately);
        assert_eq!(PopToRootPolicy::parse(None).label(), "Immediately (0s)");

        let mut p = PopToRootPolicy::Immediately;
        p = p.next();
        assert_eq!(p, PopToRootPolicy::AfterSecs(30));
        p = p.next();
        assert_eq!(p, PopToRootPolicy::AfterSecs(60));
        p = p.next();
        assert_eq!(p, PopToRootPolicy::AfterSecs(300));
        p = p.next();
        assert_eq!(p, PopToRootPolicy::Never);
        p = p.next();
        assert_eq!(p, PopToRootPolicy::Immediately);

        let now = Instant::now();
        let ten_secs_ago = now.checked_sub(Duration::from_secs(10));
        let two_mins_ago = now.checked_sub(Duration::from_secs(120));

        assert!(PopToRootPolicy::Immediately.should_reset(ten_secs_ago, now));
        assert!(!PopToRootPolicy::AfterSecs(60).should_reset(ten_secs_ago, now));
        if two_mins_ago.is_some() {
            assert!(PopToRootPolicy::AfterSecs(60).should_reset(two_mins_ago, now));
        }
        assert!(!PopToRootPolicy::Never.should_reset(two_mins_ago, now));
    }

    #[test]
    fn root_search_ranks_apps_and_calc_above_windows_settings() {
        let launcher = LauncherState::from_programs(
            vec![Program::new(
                "DisplayCal",
                "C:\\DisplayCal.exe",
                Source::Registry,
            )],
            std::path::PathBuf::from("unused-ranking.json"),
        );
        let rows = build_root_search_rows(&launcher, None, "display");
        assert_eq!(rows[0].kind, APPLICATIONS);
        assert_eq!(rows[0].name, "DisplayCal");
        assert!(rows
            .iter()
            .skip(1)
            .any(|r| r.kind == WIN_SETTING_SECTION && r.target == "ms-settings:display"));
    }

    #[test]
    fn root_search_ranks_builtin_commands_above_windows_settings() {
        let launcher = LauncherState::from_programs(
            Vec::new(),
            std::path::PathBuf::from("unused-ranking.json"),
        );
        let rows = build_root_search_rows(&launcher, None, "clipboard history");
        assert!(!rows.is_empty());
        assert_eq!(rows[0].kind, COMMAND_SECTION);
        assert_eq!(rows[0].name, "Clipboard History");
        assert!(rows
            .iter()
            .skip(1)
            .any(|r| r.kind == WIN_SETTING_SECTION && r.name == "Clipboard Settings"));
    }

    #[test]
    fn settings_rows_and_model_include_ocr_color_picker_and_action_hotkeys() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let base = std::env::temp_dir().join(format!(
            "yal-native-settings-test-{}-{unique}",
            std::process::id()
        ));
        let paths = tc_platform::Paths {
            data_dir: base.join("data"),
            cache_dir: base.join("cache"),
            home_dir: base.join("home"),
            settings_file: base.join("settings.json"),
        };
        let ctx = NativeContext {
            palette: Arc::new(PaletteState::new()),
            settings: Arc::new(WindowsSettings::load(paths.settings_file.clone())),
            features: Arc::new(FeatureStores::open(&paths)),
            hotkeys: Arc::new(HotkeyController::new()),
            pending_hotkey_command: Arc::new(Mutex::new(None)),
            tools: Arc::new(ToolsBridge::new()),
        };

        let rows = collect_settings_rows(&ctx, "");
        let ocr_lang_pos = rows.iter().position(|r| r.key == KEY_OCR_LANGUAGE);
        let ocr_mode_pos = rows.iter().position(|r| r.key == KEY_OCR_MODE);
        let color_fmt_pos = rows.iter().position(|r| r.key == KEY_COLOR_PICKER_FORMAT);
        let ocr_hk_key = format!("{KEY_RECORD_CMD_HOTKEY_PREFIX}{CMD_SCREEN_OCR}");
        let color_hk_key = format!("{KEY_RECORD_CMD_HOTKEY_PREFIX}{CMD_COLOR_PICKER}");
        let first_action_hk_pos = rows
            .iter()
            .position(|r| r.key.starts_with(KEY_RECORD_CMD_HOTKEY_PREFIX));
        let ocr_hk_row = rows.iter().find(|r| r.key == ocr_hk_key);
        let color_hk_row = rows.iter().find(|r| r.key == color_hk_key);

        assert!(ocr_lang_pos.is_some());
        assert!(ocr_mode_pos.is_some());
        assert!(color_fmt_pos.is_some());
        assert!(first_action_hk_pos.is_some());
        assert!(ocr_lang_pos < ocr_mode_pos);
        assert!(ocr_mode_pos < color_fmt_pos);
        assert!(color_fmt_pos < first_action_hk_pos);

        assert_eq!(
            ocr_hk_row.map(|r| r.label.as_str()),
            Some("Extract Text (OCR) Hotkey")
        );
        assert_eq!(
            color_hk_row.map(|r| r.label.as_str()),
            Some("Pick Color Hotkey")
        );

        let model = settings_model_for("", &rows, 0);
        for (label, glyph) in [
            ("OCR Language", "A"),
            ("OCR Formatting Mode", "¶"),
            ("Color Picker Default Format", "◉"),
        ] {
            let ui_row = model.rows.iter().find(|r| r.name == label);
            assert!(ui_row.is_some(), "missing settings row `{label}`");
            if let Some(ui_row) = ui_row {
                assert_eq!(ui_row.section, "Screen OCR & Color Picker");
                assert_eq!(ui_row.icon.as_ref().map(|i| i.letter.as_str()), Some(glyph));
                assert!(!ui_row.detail.is_empty());
            }
        }
        for label in ["Extract Text (OCR) Hotkey", "Pick Color Hotkey"] {
            let ui_row = model.rows.iter().find(|r| r.name == label);
            assert!(ui_row.is_some(), "missing action hotkey row `{label}`");
            if let Some(ui_row) = ui_row {
                assert_eq!(ui_row.section, "Action Hotkeys");
            }
        }

        let _ = std::fs::remove_dir_all(&base);
    }
}
