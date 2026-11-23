//! The native palette: the Direct2D window the user types into.
//!
//! Pure Win32 + Direct2D + DirectWrite — no WebView2, no browser runtime.
//! Routes every native screen cleanly inside one always-ready window:
//! Launcher, Clipboard History, 2D Color Emoji Grid, Snippets Browser & Wizard,
//! File Search, Window Switcher, Calculator History, and live-themed Settings.

use std::collections::{HashMap, HashSet};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use tc_clipboard::{ClipboardFilter, ClipboardItem};
use tc_core::CommandId;
use tc_palette::FileType;
use tc_platform::{Clipboard, FileSystem, Settings};
use tc_platform_win::window_control::WindowInfo;
use tc_platform_win::{HotkeyTrigger, WindowsSettings};
use tc_snippets::StoredSnippet;
use tc_ui::{
    ActionKey, Color, GridCell, Icon, IconBitmap, Model, Palette, PaletteEvent, PaletteHandle,
    PaletteReply, Row, Theme, Tokens,
};

use crate::features::{CalcHistoryEntry, FeatureStores, NoteItem, Quicklink};
use crate::launcher::{
    LauncherState, ResultRow, CALCULATOR_PREFIX, CALCULATOR_SECTION, COMMAND_SECTION,
    CUSTOM_COMMAND_SECTION, FILE_PREFIX, FILE_SECTION, RANKING_FILE, SYSTEM_ACTION_SECTION,
    WINDOW_COMMAND_SECTION,
};
use crate::open::Runner;
use crate::palette::{PaletteState, SettingRow};
use crate::screens::{
    calc_history_model, clipboard_model, create_note_model, create_quicklink_model,
    create_snippet_model, emoji_model, fallback_search_rows, files_model, next_clipboard_filter,
    notes_model, quicklink_arg_model, quicklinks_model, run_shell_model, shows_create_note_row,
    shows_create_quicklink_row, shows_create_snippet_row, snippets_model, status_screen_model,
    windows_model, NoteDraft, NoteDraftStep, QuicklinkDraft, QuicklinkDraftStep, SnippetDraft,
    SnippetDraftStep, FALLBACK_URL_PREFIX,
};
use crate::shell::{FILE_SEARCH_ENABLED, FILE_SEARCH_SHOW};

/// Cross-thread command code asking the native palette to open Settings.
pub const COMMAND_OPEN_SETTINGS: u32 = 1;
/// Cross-thread command code asking the palette to re-show with a pending status banner
/// (for example after an off-thread `IFileOpenDialog` backup or Raycast import finishes).
const COMMAND_SHOW_STATUS: u32 = 2;

/// Setting key storing the launcher's global hotkey in `settings.json`.
pub const KEY_LAUNCHER_HOTKEY: &str = "launcher.hotkey";
/// Setting key toggling Window Management commands in the launcher.
const KEY_WINDOW_MANAGEMENT_ENABLED: &str = "windowManagement.enabled";

/// Prefix for an inline snippet keyword match surfaced inside Launcher search.
const INLINE_SNIPPET_PREFIX: &str = "snippet-inline:";
const SNIPPET_SECTION: &str = "Snippets";

/// Built-in hotkey presets cycled in the native Settings pane.
const HOTKEY_PRESETS: [&str; 4] = ["Alt+Space", "Ctrl+Space", "Double Ctrl", "Double Alt"];

/// The section an application belongs to.
const APPLICATIONS: &str = "Applications";

/// Callback invoked when the user changes the launcher hotkey in Settings.
pub type HotkeyChangedCallback = Arc<dyn Fn(HotkeyTrigger) + Send + Sync>;

/// Application context shared with the native Direct2D palette.
pub struct NativeContext {
    pub palette: Arc<PaletteState>,
    pub settings: Arc<WindowsSettings>,
    pub features: Arc<FeatureStores>,
    pub on_hotkey_changed: Option<HotkeyChangedCallback>,
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

#[derive(Default, PartialEq, Eq)]
enum Mode {
    #[default]
    Launcher,
    Clipboard,
    Emoji,
    Snippets,
    CreateSnippet(SnippetDraft),
    Notes,
    CreateNote(NoteDraft),
    Quicklinks,
    QuicklinkArg(Quicklink),
    CreateQuicklink(QuicklinkDraft),
    RunShell,
    Status,
    Files,
    SwitchWindows,
    CalcHistory,
    Settings,
}

struct Shown {
    mode: Mode,
    query: String,
    rows: Vec<ResultRow>,
    clipboard_items: Vec<ClipboardItem>,
    clipboard_filter: ClipboardFilter,
    emoji_cells: Vec<GridCell>,
    snippets: Vec<StoredSnippet>,
    notes: Vec<NoteItem>,
    quicklinks: Vec<Quicklink>,
    last_shell_output: Option<String>,
    pending_status: Option<(String, String, String, bool)>,
    file_rows: Vec<ResultRow>,
    file_filter: FileType,
    windows: Vec<WindowInfo>,
    calc_entries: Vec<CalcHistoryEntry>,
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
            query: String::new(),
            rows: Vec::new(),
            clipboard_items: Vec::new(),
            clipboard_filter: ClipboardFilter::All,
            emoji_cells: Vec::new(),
            snippets: Vec::new(),
            notes: Vec::new(),
            quicklinks: Vec::new(),
            last_shell_output: None,
            pending_status: None,
            file_rows: Vec::new(),
            file_filter: FileType::All,
            windows: Vec::new(),
            calc_entries: Vec::new(),
            settings_rows: Vec::new(),
            icons,
            queued: HashSet::new(),
            icon_tx: Some(icon_tx),
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

    let handler = move |event: PaletteEvent| -> PaletteReply {
        match event {
            PaletteEvent::Shown => {
                if let Some(ctx) = handler_context.as_ref() {
                    ctx.palette.back_to_root();
                }
                let mut shown = handler_shown.lock().expect("the palette's state");
                enter_launcher(
                    &handler_launcher,
                    handler_context.as_deref(),
                    &mut shown,
                    false,
                )
            }
            PaletteEvent::Refresh => {
                let mut shown = handler_shown.lock().expect("the palette's state");
                match shown.mode {
                    Mode::Launcher => {
                        let query = shown.query.clone();
                        let rows = shown.rows.clone();
                        let is_suggestions = query.trim().is_empty();
                        PaletteReply::Update(model_for_with_icons(
                            &query,
                            &rows,
                            is_suggestions,
                            handler_context.as_deref(),
                            &mut shown,
                        ))
                    }
                    Mode::Files => {
                        let query = shown.query.clone();
                        let filter = shown.file_filter;
                        let rows = handler_launcher.file_screen_rows(&query, filter);
                        let indexing = handler_context
                            .as_ref()
                            .is_some_and(|c| c.features.is_file_indexing());
                        shown.file_rows = rows.clone();
                        PaletteReply::Update(files_model(
                            &query,
                            &rows,
                            filter,
                            indexing,
                            |target| shown.lookup_or_queue_icon(target),
                        ))
                    }
                    Mode::RunShell => {
                        let query = shown.query.clone();
                        let last = shown.last_shell_output.clone();
                        PaletteReply::Update(run_shell_model(&query, last.as_deref()))
                    }
                    _ => PaletteReply::Keep,
                }
            }
            PaletteEvent::Command(COMMAND_OPEN_SETTINGS) | PaletteEvent::OpenSettings => {
                let Some(ctx) = handler_context.as_ref() else {
                    return PaletteReply::Keep;
                };
                let mut shown = handler_shown.lock().expect("the palette's state");
                enter_settings(ctx, &mut shown)
            }
            PaletteEvent::Command(COMMAND_SHOW_STATUS) => {
                let mut shown = handler_shown.lock().expect("the palette's state");
                let Some((badge, title, detail, refresh_theme)) = shown.pending_status.take()
                else {
                    return PaletteReply::Keep;
                };
                shown.mode = Mode::Status;
                shown.query.clear();
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
            PaletteEvent::Command(_) => PaletteReply::Keep,
            PaletteEvent::QueryChanged(query) => {
                let mut shown = handler_shown.lock().expect("the palette's state");
                shown.query = query.clone();
                match &shown.mode {
                    Mode::Launcher => {
                        let is_suggestions = query.trim().is_empty();
                        let mut rows = if is_suggestions {
                            suggestions_with_favorites(
                                &handler_launcher,
                                handler_context.as_deref(),
                            )
                        } else {
                            handler_launcher.search(&query)
                        };
                        // Inline snippet keyword expansion (`!sig` / `sig`).
                        if !is_suggestions {
                            if let Some(ctx) = handler_context.as_ref() {
                                if let Some(snippet) = ctx.features.find_snippet_by_keyword(&query)
                                {
                                    rows.insert(0, inline_snippet_row(&snippet));
                                }
                                // Fallback web search when nothing local matched.
                                if rows.is_empty() {
                                    let links = ctx.features.search_quicklinks("");
                                    rows = fallback_search_rows(&query, &links);
                                }
                            }
                        }
                        shown.rows = rows.clone();
                        PaletteReply::Update(model_for_with_icons(
                            &query,
                            &rows,
                            is_suggestions,
                            handler_context.as_deref(),
                            &mut shown,
                        ))
                    }
                    Mode::Clipboard => {
                        let Some(ctx) = handler_context.as_ref() else {
                            return PaletteReply::Keep;
                        };
                        let filter = shown.clipboard_filter;
                        let items = ctx.features.search_clipboard(&query, filter);
                        shown.clipboard_items = items.clone();
                        PaletteReply::Update(clipboard_model(&query, &items, filter, 0))
                    }
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
                        PaletteReply::Update(snippets_model(&query, &list, 0))
                    }
                    Mode::CreateSnippet(draft) => {
                        let draft_clone = draft.clone();
                        PaletteReply::Update(create_snippet_model(&draft_clone, &query))
                    }
                    Mode::Notes => {
                        let Some(ctx) = handler_context.as_ref() else {
                            return PaletteReply::Keep;
                        };
                        let list = ctx.features.search_notes(&query);
                        shown.notes = list.clone();
                        PaletteReply::Update(notes_model(&query, &list, 0))
                    }
                    Mode::CreateNote(draft) => {
                        let draft_clone = draft.clone();
                        PaletteReply::Update(create_note_model(&draft_clone, &query))
                    }
                    Mode::Quicklinks => {
                        let Some(ctx) = handler_context.as_ref() else {
                            return PaletteReply::Keep;
                        };
                        let list = ctx.features.search_quicklinks(&query);
                        shown.quicklinks = list.clone();
                        PaletteReply::Update(quicklinks_model(&query, &list, 0))
                    }
                    Mode::QuicklinkArg(link) => {
                        let link_clone = link.clone();
                        PaletteReply::Update(quicklink_arg_model(&link_clone, &query))
                    }
                    Mode::CreateQuicklink(draft) => {
                        let draft_clone = draft.clone();
                        PaletteReply::Update(create_quicklink_model(&draft_clone, &query))
                    }
                    Mode::RunShell => {
                        let last = shown.last_shell_output.clone();
                        PaletteReply::Update(run_shell_model(&query, last.as_deref()))
                    }
                    Mode::Status => PaletteReply::Keep,
                    Mode::Files => {
                        let filter = shown.file_filter;
                        let rows = handler_launcher.file_screen_rows(&query, filter);
                        let indexing = handler_context
                            .as_ref()
                            .is_some_and(|c| c.features.is_file_indexing());
                        shown.file_rows = rows.clone();
                        PaletteReply::Update(files_model(
                            &query,
                            &rows,
                            filter,
                            indexing,
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
                        let entries = ctx.features.search_calc_history(&query);
                        shown.calc_entries = entries.clone();
                        PaletteReply::Update(calc_history_model(&query, &entries, 0))
                    }
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
            ),
            PaletteEvent::Action(action, selected) => handle_action(
                action,
                selected,
                &handler_shown,
                &handler_launcher,
                handler_context.as_deref(),
            ),
            PaletteEvent::Escape => {
                let mut shown = handler_shown.lock().expect("the palette's state");
                match shown.mode {
                    Mode::Launcher => close_reply(),
                    Mode::CreateSnippet(_) => {
                        if let Some(ctx) = handler_context.as_deref() {
                            enter_snippets(ctx, &mut shown)
                        } else {
                            enter_launcher(
                                &handler_launcher,
                                handler_context.as_deref(),
                                &mut shown,
                                true,
                            )
                        }
                    }
                    Mode::CreateNote(_) => {
                        if let Some(ctx) = handler_context.as_deref() {
                            enter_notes(ctx, &mut shown)
                        } else {
                            enter_launcher(
                                &handler_launcher,
                                handler_context.as_deref(),
                                &mut shown,
                                true,
                            )
                        }
                    }
                    Mode::QuicklinkArg(_) | Mode::CreateQuicklink(_) => {
                        if let Some(ctx) = handler_context.as_deref() {
                            enter_quicklinks(ctx, &mut shown)
                        } else {
                            enter_launcher(
                                &handler_launcher,
                                handler_context.as_deref(),
                                &mut shown,
                                true,
                            )
                        }
                    }
                    _ => {
                        if let Some(ctx) = handler_context.as_ref() {
                            ctx.palette.back_to_root();
                        }
                        enter_launcher(
                            &handler_launcher,
                            handler_context.as_deref(),
                            &mut shown,
                            true,
                        )
                    }
                }
            }
            PaletteEvent::Dismissed => {
                if quit_on_close {
                    PaletteReply::Quit
                } else {
                    PaletteReply::Keep
                }
            }
        }
    };

    let rows = suggestions_with_favorites(&launcher, context.as_deref());
    let initial_model = {
        let mut guard = shown.lock().expect("the palette's state");
        guard.rows = rows.clone();
        model_for_with_icons("", &rows, true, context.as_deref(), &mut guard)
    };

    let palette = Palette::with_handler(theme, initial_model, Box::new(handler))?;
    *handle_slot.lock().expect("icon handle slot") = Some(palette.handle());
    Ok(palette)
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
) -> PaletteReply {
    let ctx = ctx_arc.map(Arc::as_ref);
    let mut shown = shown_lock.lock().expect("the palette's state");
    match &shown.mode {
        Mode::Launcher => {
            let Some(row) = shown.rows.get(index).cloned() else {
                return PaletteReply::Keep;
            };

            // 1. Inline snippet keyword match -> expand & paste (or copy on Ctrl+Enter).
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
                        drop(shown);
                        if secondary {
                            let _ = tc_platform_win::clipboard().write_text(&expanded);
                        } else {
                            let _ = tc_platform_win::copy_and_paste_async(expanded);
                        }
                        return close_reply();
                    }
                }
                return PaletteReply::Keep;
            }

            // 1b. Fallback web search URL -> open in default browser immediately.
            if let Some(url) = row.id.strip_prefix(FALLBACK_URL_PREFIX) {
                let url = url.to_string();
                drop(shown);
                let _ = tc_platform_win::open(&url);
                return close_reply();
            }

            // 2. Built-in screen commands navigate cleanly inside the same palette window!
            if let Some(cmd) = LauncherState::command_of(&row.id) {
                if let Some(ctx) = ctx {
                    match cmd {
                        CommandId::ClipboardHistory => return enter_clipboard(ctx, &mut shown),
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
                        CommandId::ExportSettings => return run_export_backup(ctx, &mut shown),
                        CommandId::ImportSettings => {
                            if let Some(ctx_shared) = ctx_arc {
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
                        CommandId::CalculatorHistory => return enter_calc_history(ctx, &mut shown),
                        CommandId::Settings => return enter_settings(ctx, &mut shown),
                        _ => {}
                    }
                }
            }

            // 3. Calculator answer -> remember in persistent Calculator History!
            if row.id.starts_with(CALCULATOR_PREFIX) {
                if let Some(ctx) = ctx {
                    let badge = row.calc_badge.as_deref().unwrap_or("Result");
                    ctx.features
                        .record_calc(&row.detail, &row.name, &row.target, badge);
                }
            }

            // 4. Secondary action (`Ctrl+Enter`) on a File row -> Reveal in File Explorer.
            if secondary {
                if let Some(path) = row.id.strip_prefix(FILE_PREFIX) {
                    let path = path.to_string();
                    drop(shown);
                    let _ = tc_platform_win::reveal(&path);
                    return close_reply();
                }
            }

            let id = row.id.clone();
            let query = shown.query.clone();
            drop(shown);
            let _ = runner.open(&id, &query, false);
            close_reply()
        }
        Mode::Clipboard => {
            let Some(ctx) = ctx else {
                return PaletteReply::Keep;
            };
            let Some(item) = shown.clipboard_items.get(index).cloned() else {
                return PaletteReply::Keep;
            };
            let Some(text) = item.text.clone() else {
                return PaletteReply::Keep;
            };
            ctx.features.promote_clipboard_item(&item);
            drop(shown);
            if secondary {
                let _ = tc_platform_win::clipboard().write_text(&text);
            } else {
                let _ = tc_platform_win::copy_and_paste_async(text);
            }
            close_reply()
        }
        Mode::Emoji => {
            let Some(ctx) = ctx else {
                return PaletteReply::Keep;
            };
            let Some(cell) = shown.emoji_cells.get(index).cloned() else {
                return PaletteReply::Keep;
            };
            ctx.features.record_emoji_use(&cell.glyph);
            drop(shown);
            if secondary {
                let _ = tc_platform_win::clipboard().write_text(&cell.glyph);
            } else {
                let _ = tc_platform_win::copy_and_paste_async(cell.glyph);
            }
            close_reply()
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
            drop(shown);
            if secondary {
                let _ = tc_platform_win::clipboard().write_text(&expanded);
            } else {
                let _ = tc_platform_win::copy_and_paste_async(expanded);
            }
            close_reply()
        }
        Mode::CreateSnippet(draft) => {
            let Some(ctx) = ctx else {
                return PaletteReply::Keep;
            };
            let mut next_draft = draft.clone();
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
            drop(shown);
            if secondary {
                let _ = tc_platform_win::open(&note.path.to_string_lossy());
            } else {
                let _ = tc_platform_win::clipboard().write_text(&note.body);
            }
            close_reply()
        }
        Mode::CreateNote(draft) => {
            let Some(ctx) = ctx else {
                return PaletteReply::Keep;
            };
            let mut next_draft = draft.clone();
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
                shown.mode = Mode::QuicklinkArg(link.clone());
                shown.query.clear();
                return PaletteReply::Navigate(quicklink_arg_model(&link, ""));
            }
            let url = link.expand_url("");
            drop(shown);
            let _ = tc_platform_win::open(&url);
            close_reply()
        }
        Mode::QuicklinkArg(link) => {
            let url = link.expand_url(&shown.query);
            drop(shown);
            let _ = tc_platform_win::open(&url);
            close_reply()
        }
        Mode::CreateQuicklink(draft) => {
            let Some(ctx) = ctx else {
                return PaletteReply::Keep;
            };
            let mut next_draft = draft.clone();
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
            drop(shown);
            if secondary {
                let _ = tc_platform_win::reveal(&path);
            } else {
                let _ = tc_platform_win::open(&path);
            }
            close_reply()
        }
        Mode::SwitchWindows => {
            let Some(win) = shown.windows.get(index).cloned() else {
                return PaletteReply::Keep;
            };
            drop(shown);
            let _ = tc_platform_win::window_control::focus_window(win.handle);
            close_reply()
        }
        Mode::CalcHistory => {
            let Some(entry) = shown.calc_entries.get(index).cloned() else {
                return PaletteReply::Keep;
            };
            drop(shown);
            if secondary {
                let _ = tc_platform_win::copy_and_paste_async(entry.copy_text);
            } else {
                let _ = tc_platform_win::clipboard().write_text(&entry.copy_text);
            }
            close_reply()
        }
        Mode::Settings => {
            let Some(ctx) = ctx else {
                return PaletteReply::Keep;
            };
            let Some(setting) = shown.settings_rows.get(index) else {
                return PaletteReply::Keep;
            };
            let key = setting.key.clone();
            let query = shown.query.clone();
            advance_native_setting(ctx, launcher, handle_slot, &key);
            let updated = collect_settings_rows(ctx, &query);
            let clamped = index.min(updated.len().saturating_sub(1));
            shown.settings_rows = updated.clone();
            let model = settings_model_for(&query, &updated, clamped);
            if key == tc_palette::KEY_APPEARANCE {
                PaletteReply::UpdateTheme(Box::new(theme_from_settings(&ctx.settings)), model)
            } else {
                PaletteReply::Update(model)
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
) -> PaletteReply {
    let Some(ctx) = ctx else {
        return PaletteReply::Keep;
    };
    let mut shown = shown_lock.lock().expect("the palette's state");
    match (&shown.mode, action) {
        // Tab from Launcher (with empty search) flips straight to Clipboard History;
        // Tab from Clipboard History returns to Launcher.
        (Mode::Launcher, ActionKey::Tab) if shown.query.trim().is_empty() => {
            enter_clipboard(ctx, &mut shown)
        }
        (Mode::Clipboard, ActionKey::Tab) => enter_launcher(launcher, Some(ctx), &mut shown, true),

        // Launcher Ctrl+P: Pin / unpin selected app as a Favorite!
        (Mode::Launcher, ActionKey::CtrlP) => {
            if let Some(row) = shown.rows.get(selected).cloned() {
                if row.kind == APPLICATIONS {
                    ctx.features.toggle_favorite(&row.id);
                    let q = shown.query.clone();
                    let is_suggestions = q.trim().is_empty();
                    let rows = if is_suggestions {
                        suggestions_with_favorites(launcher, Some(ctx))
                    } else {
                        shown.rows.clone()
                    };
                    shown.rows = rows.clone();
                    return PaletteReply::Update(model_for_with_icons(
                        &q,
                        &rows,
                        is_suggestions,
                        Some(ctx),
                        &mut shown,
                    ));
                }
            }
            PaletteReply::Keep
        }

        // Notes actions: Ctrl+K Create New, Ctrl+Delete Remove selected note.
        (Mode::Notes, ActionKey::CtrlK) => enter_create_note(&mut shown),
        (Mode::Notes, ActionKey::CtrlDelete) => {
            if let Some(note) = shown.notes.get(selected).cloned() {
                ctx.features.delete_note(&note.path);
                let q = shown.query.clone();
                let list = ctx.features.search_notes(&q);
                let next_sel = selected.min(list.len().saturating_sub(1));
                shown.notes = list.clone();
                return PaletteReply::Update(notes_model(&q, &list, next_sel));
            }
            PaletteReply::Keep
        }

        // Quicklinks actions: Ctrl+K Create New, Ctrl+Delete Remove selected link.
        (Mode::Quicklinks, ActionKey::CtrlK) => enter_create_quicklink(&mut shown),
        (Mode::Quicklinks, ActionKey::CtrlDelete) => {
            if let Some(link) = shown.quicklinks.get(selected).cloned() {
                ctx.features.delete_quicklink(&link.name);
                let q = shown.query.clone();
                let list = ctx.features.search_quicklinks(&q);
                let next_sel = selected.min(list.len().saturating_sub(1));
                shown.quicklinks = list.clone();
                return PaletteReply::Update(quicklinks_model(&q, &list, next_sel));
            }
            PaletteReply::Keep
        }

        // Clipboard actions: Ctrl+P Pin, Ctrl+K Cycle Filter, Ctrl+Delete Remove.
        (Mode::Clipboard, ActionKey::CtrlP) => {
            if let Some(item) = shown.clipboard_items.get(selected).cloned() {
                ctx.features.toggle_clipboard_pin(&item);
                let q = shown.query.clone();
                let filter = shown.clipboard_filter;
                let items = ctx.features.search_clipboard(&q, filter);
                let next_sel = selected.min(items.len().saturating_sub(1));
                shown.clipboard_items = items.clone();
                return PaletteReply::Update(clipboard_model(&q, &items, filter, next_sel));
            }
            PaletteReply::Keep
        }
        (Mode::Clipboard, ActionKey::CtrlK) => {
            shown.clipboard_filter = next_clipboard_filter(shown.clipboard_filter);
            let q = shown.query.clone();
            let filter = shown.clipboard_filter;
            let items = ctx.features.search_clipboard(&q, filter);
            shown.clipboard_items = items.clone();
            PaletteReply::Update(clipboard_model(&q, &items, filter, 0))
        }
        (Mode::Clipboard, ActionKey::CtrlDelete) => {
            if let Some(item) = shown.clipboard_items.get(selected).cloned() {
                ctx.features.remove_clipboard_item(&item);
                let q = shown.query.clone();
                let filter = shown.clipboard_filter;
                let items = ctx.features.search_clipboard(&q, filter);
                let next_sel = selected.min(items.len().saturating_sub(1));
                shown.clipboard_items = items.clone();
                return PaletteReply::Update(clipboard_model(&q, &items, filter, next_sel));
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
                return PaletteReply::Update(emoji_model(&q, cells, next_sel));
            }
            PaletteReply::Keep
        }

        // Snippets actions: Ctrl+K Create New, Ctrl+Delete Remove selected snippet.
        (Mode::Snippets, ActionKey::CtrlK) => enter_create_snippet(&mut shown),
        (Mode::Snippets, ActionKey::CtrlDelete) => {
            if let Some(rec) = shown.snippets.get(selected).cloned() {
                ctx.features.delete_snippet(&rec.id());
                let q = shown.query.clone();
                let list = ctx.features.search_snippets(&q);
                let next_sel = selected.min(list.len().saturating_sub(1));
                shown.snippets = list.clone();
                return PaletteReply::Update(snippets_model(&q, &list, next_sel));
            }
            PaletteReply::Keep
        }

        // Files actions: Ctrl+P Cycle FileType filter.
        (Mode::Files, ActionKey::CtrlP) => {
            shown.file_filter = shown.file_filter.next();
            let q = shown.query.clone();
            let filter = shown.file_filter;
            let rows = launcher.file_screen_rows(&q, filter);
            let indexing = ctx.features.is_file_indexing();
            shown.file_rows = rows.clone();
            PaletteReply::Update(files_model(&q, &rows, filter, indexing, |target| {
                shown.lookup_or_queue_icon(target)
            }))
        }

        // Calculator History: Ctrl+Delete Remove selected entry.
        (Mode::CalcHistory, ActionKey::CtrlDelete) => {
            if let Some(entry) = shown.calc_entries.get(selected).cloned() {
                ctx.features.remove_calc_entry(&entry);
                let q = shown.query.clone();
                let entries = ctx.features.search_calc_history(&q);
                let next_sel = selected.min(entries.len().saturating_sub(1));
                shown.calc_entries = entries.clone();
                return PaletteReply::Update(calc_history_model(&q, &entries, next_sel));
            }
            PaletteReply::Keep
        }

        _ => PaletteReply::Keep,
    }
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
    shown.rows = rows.clone();
    shown.settings_rows.clear();
    let model = model_for_with_icons("", &rows, true, ctx, shown);
    if clear_edit {
        PaletteReply::Navigate(model)
    } else {
        PaletteReply::Update(model)
    }
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
    let list = ctx.features.search_notes("");
    shown.notes = list.clone();
    PaletteReply::Navigate(notes_model("", &list, 0))
}

fn enter_create_note(shown: &mut Shown) -> PaletteReply {
    let draft = NoteDraft::new();
    shown.mode = Mode::CreateNote(draft.clone());
    shown.query.clear();
    PaletteReply::Navigate(create_note_model(&draft, ""))
}

fn enter_quicklinks(ctx: &NativeContext, shown: &mut Shown) -> PaletteReply {
    shown.mode = Mode::Quicklinks;
    shown.query.clear();
    let list = ctx.features.search_quicklinks("");
    shown.quicklinks = list.clone();
    PaletteReply::Navigate(quicklinks_model("", &list, 0))
}

fn enter_create_quicklink(shown: &mut Shown) -> PaletteReply {
    let draft = QuicklinkDraft::new();
    shown.mode = Mode::CreateQuicklink(draft.clone());
    shown.query.clear();
    PaletteReply::Navigate(create_quicklink_model(&draft, ""))
}

fn enter_run_shell(shown: &mut Shown) -> PaletteReply {
    shown.mode = Mode::RunShell;
    shown.query.clear();
    let last = shown.last_shell_output.clone();
    PaletteReply::Navigate(run_shell_model("", last.as_deref()))
}

fn run_export_backup(ctx: &NativeContext, shown: &mut Shown) -> PaletteReply {
    shown.mode = Mode::Status;
    shown.query.clear();
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
                            == Some("true");
                        launcher.set_show_window_commands(wm_on);
                        let fs_show = ctx.settings.get(FILE_SEARCH_SHOW).as_deref() == Some("true");
                        launcher.set_show_files(fs_show);
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
                    Ok(count) => (
                        "Raycast Import".to_string(),
                        format!("Imported {count} Raycast snippets"),
                        "Open Search Snippets to browse & expand them".to_string(),
                        false,
                    ),
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
    ];
    PaletteReply::Navigate(status_screen_model(
        "About YAL",
        &format!("YAL v{}", env!("CARGO_PKG_VERSION")),
        "Yet Another Launcher — Native Windows Port of Tinycast",
        extra,
    ))
}

fn enter_clipboard(ctx: &NativeContext, shown: &mut Shown) -> PaletteReply {
    ctx.palette.open_screen("Clipboard History");
    shown.mode = Mode::Clipboard;
    shown.query.clear();
    shown.clipboard_filter = ClipboardFilter::All;
    let items = ctx.features.search_clipboard("", ClipboardFilter::All);
    shown.clipboard_items = items.clone();
    PaletteReply::Navigate(clipboard_model("", &items, ClipboardFilter::All, 0))
}

fn enter_emoji(ctx: &NativeContext, shown: &mut Shown) -> PaletteReply {
    ctx.palette.open_screen("Emoji & Symbols");
    shown.mode = Mode::Emoji;
    shown.query.clear();
    let cells = ctx.features.emoji_cells("");
    shown.emoji_cells = cells.clone();
    PaletteReply::Navigate(emoji_model("", cells, 0))
}

fn enter_snippets(ctx: &NativeContext, shown: &mut Shown) -> PaletteReply {
    ctx.palette.open_screen("Snippets");
    shown.mode = Mode::Snippets;
    shown.query.clear();
    let list = ctx.features.search_snippets("");
    shown.snippets = list.clone();
    PaletteReply::Navigate(snippets_model("", &list, 0))
}

fn enter_create_snippet(shown: &mut Shown) -> PaletteReply {
    let draft = SnippetDraft::new();
    shown.mode = Mode::CreateSnippet(draft.clone());
    shown.query.clear();
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
    shown.file_filter = FileType::All;
    if !launcher.has_file_search() {
        let _ = ctx.settings.set(FILE_SEARCH_ENABLED, "true");
        ctx.features
            .trigger_file_index(Arc::clone(launcher), Some(Arc::clone(handle_slot)));
    }
    let rows = launcher.file_screen_rows("", FileType::All);
    let indexing = ctx.features.is_file_indexing();
    shown.file_rows = rows.clone();
    PaletteReply::Navigate(files_model("", &rows, FileType::All, indexing, |target| {
        shown.lookup_or_queue_icon(target)
    }))
}

fn enter_switch_windows(shown: &mut Shown) -> PaletteReply {
    shown.mode = Mode::SwitchWindows;
    shown.query.clear();
    let windows = filter_windows("");
    shown.windows = windows.clone();
    PaletteReply::Navigate(windows_model("", &windows, 0))
}

fn enter_calc_history(ctx: &NativeContext, shown: &mut Shown) -> PaletteReply {
    shown.mode = Mode::CalcHistory;
    shown.query.clear();
    let entries = ctx.features.search_calc_history("");
    shown.calc_entries = entries.clone();
    PaletteReply::Navigate(calc_history_model("", &entries, 0))
}

fn enter_settings(ctx: &NativeContext, shown: &mut Shown) -> PaletteReply {
    ctx.palette.open_screen("Settings");
    let rows = collect_settings_rows(ctx, "");
    shown.mode = Mode::Settings;
    shown.query.clear();
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
    let kw = record
        .snippet
        .keyword
        .as_deref()
        .map(|k| format!("!{k}"))
        .unwrap_or_default();
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
        name: format!("{}  ({kw})", record.snippet.name),
        detail: preview,
        kind: SNIPPET_SECTION.to_string(),
        target: record.id().to_string(),
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
    let mut rows = ctx.palette.settings_rows();
    let current_hotkey = ctx
        .settings
        .get(KEY_LAUNCHER_HOTKEY)
        .unwrap_or_else(|| HotkeyTrigger::default_launcher().format());
    let wm_on = ctx.settings.get(KEY_WINDOW_MANAGEMENT_ENABLED).as_deref() == Some("true");
    let fs_on = ctx.settings.get(FILE_SEARCH_ENABLED).as_deref() == Some("true");
    let fs_show = ctx.settings.get(FILE_SEARCH_SHOW).as_deref() == Some("true");

    let extra = vec![
        SettingRow {
            key: KEY_LAUNCHER_HOTKEY.to_string(),
            label: "Launcher Hotkey".to_string(),
            value: current_hotkey,
            options: HOTKEY_PRESETS.iter().map(|s| (*s).to_string()).collect(),
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
    ];
    for (idx, item) in extra.into_iter().enumerate() {
        rows.insert(idx, item);
    }

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
) {
    if key == KEY_LAUNCHER_HOTKEY {
        let current = ctx
            .settings
            .get(KEY_LAUNCHER_HOTKEY)
            .unwrap_or_else(|| HOTKEY_PRESETS[0].to_string());
        let pos = HOTKEY_PRESETS
            .iter()
            .position(|preset| preset.eq_ignore_ascii_case(&current))
            .map(|i| (i + 1) % HOTKEY_PRESETS.len())
            .unwrap_or(0);
        let next = HOTKEY_PRESETS[pos];
        let _ = ctx.settings.set(KEY_LAUNCHER_HOTKEY, next);
        if let (Some(cb), Some(trigger)) = (&ctx.on_hotkey_changed, HotkeyTrigger::parse(next)) {
            cb(trigger);
        }
        return;
    }

    if key == KEY_WINDOW_MANAGEMENT_ENABLED {
        let now_on = ctx.settings.get(KEY_WINDOW_MANAGEMENT_ENABLED).as_deref() != Some("true");
        let _ = ctx.settings.set(
            KEY_WINDOW_MANAGEMENT_ENABLED,
            if now_on { "true" } else { "false" },
        );
        launcher.set_show_window_commands(now_on);
        return;
    }

    if key == FILE_SEARCH_ENABLED {
        let now_on = ctx.settings.get(FILE_SEARCH_ENABLED).as_deref() != Some("true");
        let _ = ctx
            .settings
            .set(FILE_SEARCH_ENABLED, if now_on { "true" } else { "false" });
        if now_on {
            ctx.features
                .trigger_file_index(Arc::clone(launcher), Some(Arc::clone(handle_slot)));
        }
        return;
    }

    if key == FILE_SEARCH_SHOW {
        let now_on = ctx.settings.get(FILE_SEARCH_SHOW).as_deref() != Some("true");
        let _ = ctx
            .settings
            .set(FILE_SEARCH_SHOW, if now_on { "true" } else { "false" });
        launcher.set_show_files(now_on);
        if now_on && !launcher.has_file_search() {
            let _ = ctx.settings.set(FILE_SEARCH_ENABLED, "true");
            ctx.features
                .trigger_file_index(Arc::clone(launcher), Some(Arc::clone(handle_slot)));
        }
        return;
    }

    if let Some(next) = ctx.palette.advance_setting(key) {
        let _ = ctx.settings.set(key, &next);
    }
}

fn model_for_with_icons(
    query: &str,
    rows: &[ResultRow],
    is_suggestions: bool,
    ctx: Option<&NativeContext>,
    shown: &mut Shown,
) -> Model {
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

    Model {
        query: query.to_string(),
        rows: ui_rows,
        selected: 0,
        hovered: None,
        footer: count_label.clone(),
        placeholder: "Search apps, commands, or calculate…".to_string(),
        header_badge: None,
        result_count: Some(count_label),
        footer_hints: vec![
            ("↵".to_string(), "Open".to_string()),
            ("Tab".to_string(), "Clipboard".to_string()),
            ("Ctrl+,".to_string(), "Settings".to_string()),
            ("Esc".to_string(), "Close".to_string()),
        ],
        scroll_offset: 0.0,
        grid: None,
    }
}

/// Pure helper without live shell icon lookup (used by unit tests).
#[cfg(test)]
fn model_for(query: &str, rows: &[ResultRow]) -> Model {
    let ui_rows: Vec<Row> = rows
        .iter()
        .map(|r| row_of(r, query, query.trim().is_empty()))
        .collect();
    Model {
        query: query.to_string(),
        rows: ui_rows,
        selected: 0,
        hovered: None,
        footer: "Enter to open".to_string(),
        placeholder: "Search apps, commands, or calculate…".to_string(),
        header_badge: None,
        result_count: None,
        footer_hints: vec![
            ("↵".to_string(), "Open".to_string()),
            ("Esc".to_string(), "Close".to_string()),
        ],
        scroll_offset: 0.0,
        grid: None,
    }
}

/// The palette's content for the native Settings screen.
fn settings_model_for(query: &str, rows: &[SettingRow], selected: usize) -> Model {
    let ui_rows: Vec<Row> = rows
        .iter()
        .map(|row| {
            let section = match row.key.as_str() {
                KEY_LAUNCHER_HOTKEY => "Hotkeys",
                KEY_WINDOW_MANAGEMENT_ENABLED | FILE_SEARCH_ENABLED | FILE_SEARCH_SHOW => {
                    "Features"
                }
                _ => "General",
            };
            let glyph = match row.key.as_str() {
                KEY_LAUNCHER_HOTKEY => "⌘",
                KEY_WINDOW_MANAGEMENT_ENABLED => "⊞",
                FILE_SEARCH_ENABLED | FILE_SEARCH_SHOW => "F",
                _ => "⚙",
            };
            let display_value = match row.value.trim().to_ascii_lowercase().as_str() {
                "true" => "On".to_string(),
                "false" => "Off".to_string(),
                _ => row.value.clone(),
            };
            let description = match row.key.as_str() {
                KEY_LAUNCHER_HOTKEY => "Global shortcut to summon YAL",
                KEY_WINDOW_MANAGEMENT_ENABLED => {
                    "Snap windows with Left/Right Half, Maximize, Center"
                }
                FILE_SEARCH_ENABLED => "Index Desktop, Documents, Downloads in background",
                FILE_SEARCH_SHOW => "Include indexed files inside root Launcher results",
                tc_palette::KEY_APPEARANCE => "Color theme",
                tc_palette::KEY_ANIMATIONS => "Motion & transitions",
                tc_palette::KEY_INTERFACE_SIZE => "Palette text scale",
                tc_palette::KEY_WINDOW_MODE => "Compact or expanded list",
                tc_palette::KEY_ESCAPE_BEHAVIOUR => "Action on Esc key",
                tc_palette::KEY_POP_TO_ROOT => "Reset screen after hiding",
                _ => "",
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

    let detail = if result.detail.is_empty() && result.kind != APPLICATIONS {
        result.kind.clone()
    } else {
        result.detail.clone()
    };

    Row {
        name: result.name.clone(),
        detail,
        shortcut: String::new(),
        icon: Some(icon_of(result)),
        section,
        ..Row::default()
    }
}

fn icon_of(result: &ResultRow) -> Icon {
    let glyph = match result.kind.as_str() {
        CALCULATOR_SECTION => "=".to_string(),
        COMMAND_SECTION => "›".to_string(),
        CUSTOM_COMMAND_SECTION => ">_".to_string(),
        WINDOW_COMMAND_SECTION => "⊞".to_string(),
        SYSTEM_ACTION_SECTION => "⚡".to_string(),
        FILE_SECTION => "📄".to_string(),
        SNIPPET_SECTION => "✂".to_string(),
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
        APPLICATIONS => Color::rgb(0x5e, 0x8b, 0xff),
        FILE_SECTION => Color::rgb(0x38, 0xbd, 0xf8),
        COMMAND_SECTION | SNIPPET_SECTION => Color::rgb(0x34, 0xd3, 0x99),
        CUSTOM_COMMAND_SECTION => Color::rgb(0xa7, 0x8b, 0xfa),
        WINDOW_COMMAND_SECTION => Color::rgb(0xfb, 0x92, 0x3c),
        SYSTEM_ACTION_SECTION => Color::rgb(0xf8, 0x71, 0x71),
        CALCULATOR_SECTION => Color::rgb(0x60, 0xa5, 0xfa),
        _ => Color::rgb(0x94, 0xa3, 0xb8),
    }
}

/// Runs the native palette on its own (`--native-palette`), without tray or global hotkey.
pub fn run(arguments: &[String]) -> Result<(), String> {
    let paths = tc_platform_win::windows_paths();
    let data = tc_platform_win::WindowsFileSystem::new();
    let _ = data.ensure_dir(&paths.data_dir);
    let _ = data.ensure_dir(&paths.cache_dir);

    let launcher = Arc::new(LauncherState::new(paths.data_dir.join(RANKING_FILE)));
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
        assert_eq!(color_of("Something Else"), Color::rgb(0x94, 0xa3, 0xb8));
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
}
