//! The palette window.
//!
//! The palette is a single always-on-top window: a query field at the top and
//! a flat list of ranked results below it. There are no modes and no pages —
//! everything the launcher can do is a result in the same list.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Instant;

use eframe::egui::{self, Align, Color32, FontId, Key, Layout, RichText, Stroke, TextFormat};
use egui::text::LayoutJob;
use yal_core::action::{Action, ActionKind, OpenTarget};
use yal_core::ranking::Match;
use yal_engine::Engine;
use yal_search::{Frecency, Scored};
use yal_store::Store;

/// How long the palette waits before re-running a search.
///
/// Searching is fast, but re-running it on every keystroke still costs more
/// than the frame it would save. The delay is short enough to feel
/// immediate and long enough to collapse a burst of keystrokes into one
/// search.
const SEARCH_DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(35);

/// The height of one result row.
const ROW_HEIGHT: f32 = 38.0;

/// A background message delivered to the palette.
pub enum PaletteMessage {
    /// The user asked for the palette to close.
    Close,
    /// The user asked for the palette to show again.
    Show,
    /// Something due could not be toasted; the palette is asked to raise
    /// its own banner, which cannot fail.
    Ring(String),
    /// A script in `full` mode finished; its output is delivered to the
    /// palette instead of only living in the clipboard.
    Script(ScriptAnnouncement),
    /// The whole-drive file index's walker published a fresh snapshot of
    /// every indexed path; the next search answers from it.
    Files(std::sync::Arc<Vec<std::path::PathBuf>>),
}

/// A script's full output, shown inside the palette.
#[derive(Debug, Clone)]
pub struct ScriptAnnouncement {
    /// The script's title.
    pub title: String,
    /// The whole output, or a note that there was none.
    pub text: String,
    /// Whether the script exited non-zero.
    pub failed: bool,
}

/// The palette application.
pub struct Palette {
    store: Store,
    engine: Engine,
    query: String,
    results: Vec<Scored>,
    selected: usize,
    applications: Vec<yal_index::Discovered>,
    /// The whole-drive file index's paths, as the walker last published
    /// them. Empty until the index's first snapshot lands; the searches
    /// stay fresh through the live walk meanwhile.
    indexed: std::sync::Arc<Vec<std::path::PathBuf>>,
    last_search: Option<Instant>,
    pending: bool,
    receiver: Receiver<PaletteMessage>,
    sender: Sender<PaletteMessage>,
    /// The global hotkey that opens the palette.
    hotkey: yal_system::hotkey::HotkeyListener,
    /// Whether the palette window is currently visible.
    visible: bool,
    /// Set when the palette should close after the next frame.
    close_requested: bool,
    /// Set when the palette should hide after the next frame.
    hide_requested: bool,
    /// Set when the palette still needs the keyboard focus.
    needs_focus: bool,
    /// The window that held the keyboard focus when the palette opened,
    /// so snippets can expand back into it.
    previous_window: Option<u64>,
    /// An expansion waiting to be typed once the palette is gone.
    pending_paste: Option<String>,
    /// Where the palette sits when it is shown, so the park-and-return
    /// cycle can put it back exactly where it was.
    shown_location: Option<(i32, i32)>,
    /// The image bytes of the result whose texture is cached.
    cached_image: Option<Vec<u8>>,
    /// The cached texture.
    cached_texture: Option<egui::TextureHandle>,
    /// The last error shown to the user.
    status: Option<String>,
    /// A timer that rang while the palette was away.
    ring: Option<String>,
    /// Whether the palette is searching or showing its preferences.
    mode: Mode,
    /// The appearance the palette is drawn with.
    theme: Theme,
    /// Whether the next key press becomes the new palette hotkey.
    recording_hotkey: bool,
    /// The scripts folder path being edited in the preferences pane.
    scripts_draft: String,
    /// A script's full output, shown on top of the search while present.
    script_outcome: Option<ScriptAnnouncement>,
    /// The frame context, remembered so background threads can wake the
    /// palette when they have something to show.
    frame_ctx: Option<egui::Context>,
    /// The secondary acts of the selected result, when the panel is open.
    ///
    /// Keyed to the list as it stood when the panel was opened: the panel
    /// is a sheet the user acts from, not a live view.
    action_panel: Option<Vec<Action>>,
    /// The highlighted act inside the panel.
    panel_selected: usize,
    /// Where the selected result row sat when it was last drawn, so the
    /// panel can open beside its row.
    selected_rect: Option<egui::Rect>,
    /// A screen-capture act in flight; it owns the window while present.
    capture: Option<CaptureState>,
}

/// What the palette is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// The search results.
    Search,
    /// The settings pane.
    Preferences,
}

/// The capture act underway.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaptureKind {
    /// The whole screen, delivered to a file and to the clipboard.
    WholeScreen,
    /// A band dragged over the frozen screen.
    Area,
    /// A pixel taken from the frozen screen under a loupe.
    Pick,
}

/// A capture act in flight.
///
/// The little machine, in order: the palette leaves the screen, a beat
/// later the frame is read (so the launcher is not in its own picture),
/// and for the area act the window grows over the screen showing the
/// frozen frame while the user drags a band; for the pick act the same
/// stopping of the world serves a loupe that follows the pointer.
struct CaptureState {
    /// Which act opened the overlay.
    kind: CaptureKind,
    /// The palette window's shape before the takeover, as it was measured.
    restore: (i32, i32, i32, i32),
    /// When the leaving began; the frame is read only after the beat.
    left_at: Instant,
    /// The frozen frame in its native pixels, once read.
    frame: Option<image::RgbaImage>,
    /// The frozen frame as the palette draws it.
    texture: Option<egui::TextureHandle>,
    /// The band's first corner, in overlay points.
    band_start: Option<egui::Pos2>,
}

/// What one frame of the overlay decides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaptureOutcome {
    /// The band is still moving; more frames.
    Continue,
    /// The band closed; the crop is in screen pixels.
    Banded { region: (i32, i32, u32, u32) },
    /// A pixel was taken; the point is a pixel of the frozen frame.
    Picked { x: i32, y: i32 },
    /// Escape or a click without a band: the act ends without a picture.
    Abandoned,
}

/// The palette's appearance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Theme {
    /// Follow the system theme.
    System,
    /// Draw dark, whatever the system says.
    Dark,
    /// Draw light, whatever the system says.
    Light,
}

impl Theme {
    /// The theme named in the settings, when it is one of the three.
    fn parse(text: &str) -> Option<Self> {
        match text {
            "system" => Some(Theme::System),
            "dark" => Some(Theme::Dark),
            "light" => Some(Theme::Light),
            _ => None,
        }
    }

    fn slug(self) -> &'static str {
        match self {
            Theme::System => "system",
            Theme::Dark => "dark",
            Theme::Light => "light",
        }
    }
}

/// The identifier the launcher hotkey is registered under.
const HOTKEY_ID: i32 = 0x5241;

impl Palette {
    /// Build a palette over a store.
    pub fn new(store: Store) -> Self {
        let (sender, receiver) = channel();

        // The due watcher runs against its own connection to the same
        // store, because the palette's frame loop is not a clock; it only
        // starts for a store on disk, whose due times matter beyond this
        // process.
        if let Some(path) = store.path().map(|path| path.to_path_buf()) {
            let watcher_sender = sender.clone();
            spawn_due_watcher(path, watcher_sender);
        }

        // The whole-drive index's walker runs the same way: its own
        // store connection, its own thread, snapshot published to the
        // palette by message. An in-memory store has nothing to index
        // into; a turned-off index is not walked at all.
        {
            let path = store.path().map(|path| path.to_path_buf());
            if let Some(path) = path {
                if yal_engine::bridge::file_index_enabled(&store) {
                    let walker_sender = sender.clone();
                    spawn_index_walker(path, walker_sender);
                }
            }
        }

        // The hotkey is a setting, so the user can change it without
        // recompiling.
        let spec = store
            .setting("hotkey")
            .ok()
            .flatten()
            .and_then(|text| yal_system::hotkey::HotkeySpec::parse(&text))
            .unwrap_or_else(yal_system::hotkey::HotkeySpec::default_spec);

        let mut palette = Self {
            store,
            engine: Engine::new(),
            query: String::new(),
            results: Vec::new(),
            selected: 0,
            applications: yal_index::discover(),
            indexed: std::sync::Arc::new(Vec::new()),
            last_search: None,
            pending: false,
            receiver,
            sender,
            hotkey: yal_system::hotkey::HotkeyListener::spawn(
                spec.virtual_key,
                spec.modifiers,
                HOTKEY_ID,
            ),
            visible: true,
            close_requested: false,
            hide_requested: false,
            needs_focus: true,
            previous_window: None,
            pending_paste: None,
            shown_location: None,
            cached_image: None,
            cached_texture: None,
            status: None,
            ring: None,
            mode: Mode::Search,
            theme: Theme::System,
            recording_hotkey: false,
            scripts_draft: String::new(),
            script_outcome: None,
            frame_ctx: None,
            action_panel: None,
            panel_selected: 0,
            selected_rect: None,
            capture: None,
        };
        palette.theme = palette
            .store
            .setting("theme")
            .ok()
            .flatten()
            .and_then(|text| Theme::parse(&text))
            .unwrap_or(Theme::System);
        palette.scripts_draft = palette
            .store
            .setting("scripts_folder")
            .ok()
            .flatten()
            .unwrap_or_default();
        palette.register_hotkey();
        // The built-in search templates are seeded once, on install.
        if let Err(error) = palette.store.seed_default_quicklinks() {
            palette.status = Some(format!("the bundled links could not be seeded: {error}"));
        }
        palette.run_search();
        palette
    }

    /// Register the global hotkey that opens the palette.
    ///
    /// The listener runs on its own thread with its own message queue; see
    /// `yal-system`'s hotkey module for why.
    fn register_hotkey(&mut self) {
        if self.hotkey.is_taken() {
            // Another application already owns the combination. The palette
            // still works; it just has to be focused by hand.
            self.status =
                Some("the launcher hotkey is taken by another application".into());
        }
    }

    /// The channel used to talk to the palette from another thread.
    pub fn channel(&self) -> Sender<PaletteMessage> {
        self.sender.clone()
    }

    /// The store the palette reads from.
    pub fn store(&self) -> &Store {
        &self.store
    }

    /// The image carried by the selected result, if it has one.
    /// The image bytes of the selected result, when one is shown.
    ///
    /// Two sources carry an image: a payload the candidate built (the QR
    /// code, for one), and a file result whose name ends in an image
    /// extension. A file is read wholly into memory, because the palette
    /// draws it scaled down; files beyond ten megabytes are left alone, so
    /// a stray giant image cannot stall the window.
    fn selected_image(&self) -> Option<Vec<u8>> {
        let result = self.results.get(self.selected)?;

        if let Some(payload) = result.item.payload.as_ref() {
            if let Some(encoded) = payload.get("png_base64").and_then(|value| value.as_str()) {
                return decode_base64(encoded);
            }
        }

        if result.kind != yal_core::CommandKind::Files {
            return None;
        }
        let path = result.item.id.0.strip_prefix("file:")?;
        if !is_image_file_name(path) {
            return None;
        }
        let path = std::path::PathBuf::from(path);
        let size = std::fs::metadata(&path).ok()?.len();
        if size > 10_000_000 {
            return None;
        }
        std::fs::read(&path).ok()
    }

    /// The selected result's photograph metadata, when it carries any.
    fn selected_photo_metadata(&self) -> Option<yal_system::exif::PhotoMetadata> {
        let result = self.results.get(self.selected)?;
        if result.kind != yal_core::CommandKind::Files {
            return None;
        }
        let path = result.item.id.0.strip_prefix("file:")?;
        yal_system::exif::read(std::path::Path::new(path))
    }

    /// Turn the selected result's image into a texture.
    ///
    /// The texture is cached against the image bytes, so a result that stays
    /// selected is only decoded once.
    fn image_texture(
        &mut self,
        ctx: &egui::Context,
        bytes: &[u8],
    ) -> Option<egui::TextureHandle> {
        if self.cached_image.as_deref() == Some(bytes) {
            return self.cached_texture.clone();
        }

        let decoded = image::load_from_memory(bytes).ok()?.to_rgba8();
        let size = [decoded.width() as usize, decoded.height() as usize];
        let color = egui::ColorImage::from_rgba_unmultiplied(size, decoded.as_raw());
        let texture = ctx.load_texture(
            "result-image",
            color,
            egui::TextureOptions::LINEAR,
        );

        self.cached_image = Some(bytes.to_vec());
        self.cached_texture = Some(texture.clone());
        Some(texture)
    }

    /// Show the palette and give it the keyboard focus.
    fn show(&mut self, _ctx: &egui::Context) {
        // Remember which window the user came from: a snippet expands
        // there, not here.
        if let Some(own) = yal_system::windows::own_window_handle() {
            self.previous_window = yal_system::windows::foreground_window()
                .filter(|handle| *handle != own);
        }
        self.visible = true;
        // The parked window comes back to exactly where it was, and its
        // rank above every other window is claimed again — a window that
        // parked once can rise behind a maximised application, exactly
        // where a launcher is useless.
        if let Some(handle) = yal_system::windows::own_window_handle() {
            if let Some((x, y)) = self.shown_location {
                let _ = yal_system::window_controls::place(handle, Some(x), Some(y), None, None);
            }
            yal_system::window_controls::pin_above_everything(handle);
        }
        self.query.clear();
        self.ring = None;
        self.action_panel = None;
        self.panel_selected = 0;
        self.selected_rect = None;
        self.run_search();
        self.needs_focus = true;
    }

    /// Park the palette off-screen instead of hiding it.
    ///
    /// A hidden Windows window stops painting, and with it the whole
    /// event loop goes quiet: the hotkey that would bring the palette
    /// back is never polled again. That was verified live — a hidden
    /// palette never returned. Parked off-screen the window keeps
    /// repainting, so the hotkey polling, the timers and the staged
    /// expansions stay alive whatever the palette's visibility.
    ///
    /// The keyboard focus moves to the window the user came from before
    /// the palette parked, because a window that sits off-screen and
    /// still owns the keyboard would silently eat every keystroke.
    fn park(&mut self, ctx: &egui::Context) {
        self.visible = false;
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
            -16000.0,
            -16000.0,
        )));
        if let Some(handle) = self.previous_window {
            yal_system::windows::focus_window_with_permission(handle);
        }
    }

    /// Take the keyboard focus if the palette does not have it.
    ///
    /// A launcher that is visible but not focused is useless: the keystrokes
    /// go to whatever else is on screen. The request is repeated until it
    /// lands, because the window manager may refuse the first attempt.
    fn take_focus(&mut self, ctx: &egui::Context) {
        if !self.needs_focus {
            return;
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        if let Some(handle) = yal_system::windows::own_window_handle() {
            if yal_system::windows::focus_window_with_permission(handle) {
                self.needs_focus = false;
                return;
            }
            yal_system::windows::focus_window(handle);
        }
    }

    /// Re-run the search for the current query.
    fn run_search(&mut self) {
        self.last_search = Some(Instant::now());
        self.pending = false;
        match self.engine.search(
            &self.store,
            &self.applications,
            &self.indexed,
            &self.query,
            chrono::Utc::now(),
        ) {
            Ok(results) => {
                self.results = results;
                self.selected = self.selected.min(self.results.len().saturating_sub(1));
                self.status = None;
            }
            Err(error) => {
                self.results.clear();
                self.status = Some(error.to_string());
            }
        }
    }

    /// Carry out the selected result's action.
    fn perform_selected(&mut self) {
        let Some(action) = self
            .results
            .get(self.selected)
            .and_then(|result| result.item.action.clone())
        else {
            return;
        };
        self.carry_out(&action);
    }

    /// Carry out one action and put the palette away for it.
    ///
    /// Both the primary act (Enter on a result) and a secondary act chosen
    /// from the panel travel through here, so they behave identically: the
    /// snippet expansion is staged, the frecency record is credited to the
    /// selected result either way, and the palette gets out of the way.
    fn carry_out(&mut self, action: &Action) {
        // A snippet expansion types into the application the user came
        // from, so it cannot run while the palette is here; it is staged
        // for after the hide.
        if action.kind == ActionKind::PasteToClipboard {
            if let OpenTarget::Text(text) = &action.target {
                self.perform_paste(text.clone());
                return;
            }
        }

        // The capture acts take the window over themselves: they leave
        // the screen, read the frame, grow it back, and report by toast.
        // They are not carried out the general way and the palette is
        // not put away by the tail below.
        if action.kind == ActionKind::LauncherTask {
            if let OpenTarget::Task { name, .. } = &action.target {
                match name.as_str() {
                    "screenshot-screen" => {
                        self.credit_selected();
                        self.begin_capture(CaptureKind::WholeScreen);
                        return;
                    }
                    "capture-area" => {
                        self.credit_selected();
                        self.begin_capture(CaptureKind::Area);
                        return;
                    }
                    "pick-colour" => {
                        self.credit_selected();
                        self.begin_capture(CaptureKind::Pick);
                        return;
                    }
                    _ => {}
                }
            }
        }

        if let Err(error) = self.perform(action) {
            self.status = Some(error.to_string());
            return;
        }

        // Remember the choice so frecency can prefer it next time.
        self.credit_selected();

        // The palette gets out of the way once a result is chosen.
        self.visible = false;
        self.hide_requested = true;
    }

    /// Credit the selected result in the usage history.
    fn credit_selected(&mut self) {
        if let Some(result) = self.results.get(self.selected) {
            let key = result.item.id.0.clone();
            let _ = self.store.record_use(&key, chrono::Utc::now());
        }
    }

    /// Carry out the act highlighted in the secondary panel.
    ///
    /// The panel acts on the selected result — the same result the
    /// frecency record credits — and closes itself; the palette then
    /// leaves the screen as it does for the primary act.
    fn perform_panel_action(&mut self) {
        let chosen = self
            .action_panel
            .as_ref()
            .and_then(|panel| panel.get(self.panel_selected).cloned());
        match chosen {
            Some(action) => {
                self.action_panel = None;
                self.carry_out(&action);
            }
            None => self.action_panel = None,
        }
    }

    /// Stage a snippet expansion: remember it and get out of the way.
    fn perform_paste(&mut self, text: String) {
        // Remember the choice so frecency can prefer it next time.
        if let Some(result) = self.results.get(self.selected) {
            let key = result.item.id.0.clone();
            let _ = self.store.record_use(&key, chrono::Utc::now());
        }
        self.pending_paste = Some(text);
        self.visible = false;
        self.hide_requested = true;
    }

    /// Expand a staged snippet in the application the user came from.
    ///
    /// The palette is hidden by the time this runs. The previous window is
    /// raised again and the text typed into it — but only after the
    /// system confirms that window really holds the focus: blind typing
    /// can land in whatever happens to be in front, which is worse than
    /// not expanding. When the focus cannot be verified or the system
    /// refuses the injection, the expansion lands on the clipboard and a
    /// toast says so: the honest fallback, not a wrong-window typo.
    fn expand_in_the_background(&self, text: String) {
        let previous = self.previous_window;
        let own = yal_system::windows::own_window_handle();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(250));

            // The window that must hold the focus before a character is
            // sent: the one the user came from. Anything else — our own
            // window, an unspecified one, a window the system would not
            // raise — is not trusted with the injection.
            let verified = match previous {
                Some(handle) if Some(handle) != own => {
                    if yal_system::windows::foreground_window() == Some(handle) {
                        true
                    } else if yal_system::windows::focus_window_with_permission(handle) {
                        std::thread::sleep(std::time::Duration::from_millis(80));
                        yal_system::windows::foreground_window() == Some(handle)
                    } else {
                        false
                    }
                }
                _ => false,
            };
            if !verified {
                if let Ok(mut clipboard) = yal_system::clipboard::Clipboard::new() {
                    let _ = clipboard.write_text(&text);
                }
                let _ = yal_system::notify::toast(
                    "Snippet",
                    "Could not return to the previous window; the text is on the clipboard instead.",
                );
                return;
            }

            if yal_system::keyboard::type_text(&text).is_err() {
                if let Ok(mut clipboard) = yal_system::clipboard::Clipboard::new() {
                    let _ = clipboard.write_text(&text);
                }
                let _ = yal_system::notify::toast(
                    "Snippet",
                    "Typing in place was refused; the text is on the clipboard instead.",
                );
            }
        });
    }
}

impl Palette {
    // ---- the screen-capture acts -----------------------------------------

    /// Open a capture act: the palette leaves the screen now, and the
    /// frame is read a beat later by the pump.
    ///
    /// The leaving is done with `place`, not with egui's viewport
    /// command, so it needs no context and lands inside this frame —
    /// whichever frame the keystroke came from.
    fn begin_capture(&mut self, kind: CaptureKind) {
        // Close the panel with the secondary acts, if one was open: the
        // window is about to be something else entirely.
        self.action_panel = None;
        if self.capture.is_some() {
            // A capture already under way; entering twice would lose the
            // first band and steal the window shape, so the second ask
            // wins nothing: it is turned around.
            return;
        }
        let Some(handle) = yal_system::windows::own_window_handle() else {
            self.status = Some("the palette window could not be found".into());
            return;
        };
        let restore = match yal_system::window_controls::rect(handle) {
            Ok(rect) => rect,
            Err(error) => {
                self.status = Some(error.to_string());
                return;
            }
        };
        if let Err(error) =
            yal_system::window_controls::place(handle, Some(-16000), Some(-16000), None, None)
        {
            self.status = Some(error.to_string());
            return;
        }
        self.capture = Some(CaptureState {
            kind,
            restore,
            left_at: std::time::Instant::now(),
            frame: None,
            texture: None,
            band_start: None,
        });
        // The palette stays "visible" during the act — an off-screen
        // window that stops painting takes the frame loop with it, which
        // is just what park() works around. The pump keeps the repaints
        // up instead.
        self.visible = true;
    }

    /// Drive an in-flight capture act through its states.
    fn pump_capture(&mut self, ctx: &egui::Context) {
        // The beat the window needs to be off the screen before the
        // frame is read, so the launcher is not inside its own picture.
        const LEAVE_BEAT: std::time::Duration = std::time::Duration::from_millis(180);

        let Some(state) = self.capture.take() else {
            return;
        };

        // Nothing read yet: wait out the beat, then read once.
        if state.frame.is_none() {
            let elapsed = state.left_at.elapsed();
            if elapsed < LEAVE_BEAT {
                self.capture = Some(state);
                ctx.request_repaint_after(LEAVE_BEAT - elapsed);
                return;
            }

            // The frame is read once, after the beat. A failure ends
            // the act cleanly: word by toast, since the palette is
            // parked away from its own status line.
            let capture = match yal_system::screenshot::capture_screen() {
                Ok(capture) => capture,
                Err(error) => {
                    self.capture = None;
                    let _ = yal_system::notify::toast("Capture", &error.to_string());
                    self.park(ctx);
                    return;
                }
            };

            match state.kind {
                CaptureKind::WholeScreen => {
                    let outcome = self.store_with_clipboard_image(&capture);
                    self.capture = None;
                    let word = match &outcome {
                        Ok(path) => format!(
                            "saved to {}\nthe pixels are on the clipboard",
                            path.display()
                        ),
                        Err(error) => error.clone(),
                    };
                    let _ = yal_system::notify::toast("Capture", &word);
                    self.park(ctx);
                    return;
                }
                CaptureKind::Area => {}
                CaptureKind::Pick => {}
            }

            // The overlay acts (the band, the loupe): the frame freezes
            // into a texture, the window grows over the screen, and the
            // pointer does the rest.
            let (screen_width, screen_height) = yal_system::screenshot::screen_extent();
            if screen_width == 0 || screen_height == 0 {
                self.capture = None;
                let _ = yal_system::notify::toast(
                    "Capture",
                    "the screen reports no usable dimensions",
                );
                self.park(ctx);
                return;
            }
            // The band wants smoothing over the whole screen at once;
            // the loupe magnifies a fistful of pixels and wants them
            // crisp, so it reads with the nearest texel.
            let options = match state.kind {
                CaptureKind::Pick => egui::TextureOptions::NEAREST,
                _ => egui::TextureOptions::LINEAR,
            };
            let image = egui::ColorImage::from_rgba_unmultiplied(
                [capture.width as usize, capture.height as usize],
                capture.image.as_raw(),
            );
            let texture = ctx.load_texture("yal-frozen-screen", image, options);
            if let Some(handle) = yal_system::windows::own_window_handle() {
                let _ = yal_system::window_controls::place(
                    handle,
                    Some(0),
                    Some(0),
                    Some(screen_width as i32),
                    Some(screen_height as i32),
                );
                yal_system::window_controls::pin_above_everything(handle);
            }
            self.capture = Some(CaptureState {
                restore: state.restore,
                left_at: state.left_at,
                frame: Some(capture.image),
                texture: Some(texture),
                band_start: None,
                kind: state.kind,
            });
            self.needs_focus = true;
            ctx.request_repaint();
            return;
        }

        // Frame in hand: the overlay runs until the band closes, a pixel
        // is taken, or the act is abandoned.
        let (outcome, state) = match state.kind {
            CaptureKind::Pick => draw_pick(ctx, state),
            _ => draw_capture(ctx, state),
        };
        match outcome {
            CaptureOutcome::Continue => {
                self.capture = Some(state);
                ctx.request_repaint_after(std::time::Duration::from_millis(15));
            }
            CaptureOutcome::Abandoned => {
                self.capture = None;
                dismiss_capture_window(state.restore);
                self.park(ctx);
            }
            CaptureOutcome::Banded { region } => {
                self.finish_area_capture(ctx, state, region);
            }
            CaptureOutcome::Picked { x, y } => {
                self.finish_pick(ctx, state, x, y);
            }
        }
    }

    /// Save a captured frame: a PNG in Pictures, then the pixels
    /// themselves on the clipboard, so the picture lands wherever it is
    /// going without a round trip through the file.
    fn store_with_clipboard_image(
        &self,
        capture: &yal_system::screenshot::Capture,
    ) -> Result<std::path::PathBuf, String> {
        let path = yal_system::screenshot::default_capture_path();
        yal_system::screenshot::write_png(capture, &path).map_err(|err| err.to_string())?;
        let mut clipboard =
            yal_system::clipboard::Clipboard::new().map_err(|err| err.to_string())?;
        clipboard
            .write_image(
                capture.image.as_raw(),
                capture.width as usize,
                capture.height as usize,
            )
            .map_err(|err| err.to_string())?;
        Ok(path)
    }

    /// Close an area capture: crop the band out of the frozen frame,
    /// save it, put the pixels on the clipboard, and leave a receipt row
    /// in front of the results for when the palette comes back.
    fn finish_area_capture(
        &mut self,
        ctx: &egui::Context,
        state: CaptureState,
        region: (i32, i32, u32, u32),
    ) {
        let Some(frame) = state.frame else {
            return;
        };
        // The band is clamped to what the frame holds, in case a beat of
        // window settling left a pointer corner outside of it.
        let Some(region) =
            yal_system::screenshot::clamp_region(region.0, region.1, region.2, region.3, (frame.width(), frame.height()))
        else {
            self.capture = None;
            dismiss_capture_window(state.restore);
            self.park(ctx);
            return;
        };
        let (x, y, width, height) = region;

        // The frozen frame was swizzled on read; the band keeps its
        // pixels as they are.
        let band = image::imageops::crop_imm(&frame, x as u32, y as u32, width, height).to_image();
        let capture = yal_system::screenshot::Capture {
            image: band,
            width,
            height,
        };
        self.capture = None;
        dismiss_capture_window(state.restore);

        match self.store_with_clipboard_image(&capture) {
            Ok(path) => {
                let display = path.display().to_string();
                self.insert_capture_row(path);
                let _ = yal_system::notify::toast(
                    "Capture",
                    &format!("saved to {display}\nthe picture is on the clipboard"),
                );
            }
            Err(error) => {
                let _ = yal_system::notify::toast("Capture", &error);
            }
        }
        self.park(ctx);
    }

    /// Put the fresh capture in front of the results.
    ///
    /// The row is the act's receipt, not machine state: it lives until
    /// the next keystroke, when the search rebuilds the list.
    fn insert_capture_row(&mut self, path: std::path::PathBuf) {
        let display = path.display().to_string();
        self.results.insert(
            0,
            Scored {
                item: yal_core::item::Item::new("The new capture")
                    .with_id(format!("capture:{display}"))
                    .with_subtitle(display.clone())
                    .with_action(Action::reveal(path))
                    .with_secondary_action(Action::copy(display))
                    .with_secondary_action(Action::task("capture-area", Vec::new())),
                kind: yal_core::CommandKind::System,
                matched: Match::none(),
                score: 0.0,
                frecency: Frecency::never(),
            },
        );
        self.selected = 0;
    }

    /// Close a colour pick: read the pixel out of the frozen frame, say
    /// the colour in its three ways, put the hex on the clipboard, and
    /// leave a receipt row in front of the results.
    fn finish_pick(&mut self, ctx: &egui::Context, state: CaptureState, x: i32, y: i32) {
        let Some(frame) = &state.frame else {
            return;
        };
        // The frame was swizzled on read, so the bytes here are RGBA as
        // they showed on the screen.
        let px = (x as u32).min(frame.width().saturating_sub(1));
        let py = (y as u32).min(frame.height().saturating_sub(1));
        let pixel = frame.get_pixel(px, py);
        let colour = yal_data::colors::Color::new(pixel[0], pixel[1], pixel[2]);
        let hex = format!("#{}", colour.to_hex());
        let hsl = colour.to_hsl();
        let hsl_text = format!(
            "hsl({:.0}, {:.0}%, {:.0}%)",
            hsl.hue,
            hsl.saturation * 100.0,
            hsl.lightness * 100.0
        );

        self.capture = None;
        dismiss_capture_window(state.restore);

        let word = match yal_system::clipboard::Clipboard::new()
            .and_then(|mut board| board.write_text(&hex))
        {
            Ok(()) => {
                self.insert_colour_row(&hex, &colour.to_rgb_string(), &hsl_text);
                format!("picked {hex}\nit is on the clipboard")
            }
            Err(error) => error.to_string(),
        };
        let _ = yal_system::notify::toast("Colour", &word);
        self.park(ctx);
    }

    /// Put the picked colour in front of the results.
    ///
    /// The row is the act's receipt, not machine state: it lives until
    /// the next keystroke, when the search rebuilds the list. Its acts
    /// say the colour again: hex, then rgb and hsl beside it.
    fn insert_colour_row(&mut self, hex: &str, rgb: &str, hsl: &str) {
        self.results.insert(
            0,
            Scored {
                item: colour_row(hex, rgb, hsl),
                kind: yal_core::CommandKind::System,
                matched: Match::none(),
                score: 0.0,
                frecency: Frecency::never(),
            },
        );
        self.selected = 0;
    }

    /// Carry out an action.
    ///
    /// The palette owns the tasks that are not intents about other
    /// programs: it has the applications list to resolve a launch, and the
    /// store to keep a timer in.
    fn perform(&self, action: &Action) -> Result<(), String> {
        match (&action.kind, &action.target) {
            (ActionKind::OpenPath, OpenTarget::Path(path)) => {
                yal_system::launch::launch(path).map_err(|err| err.to_string())
            }
            (ActionKind::OpenUrl, OpenTarget::Url(url)) => {
                yal_system::launch::open_url(url).map_err(|err| err.to_string())
            }
            (ActionKind::RevealInFileManager, OpenTarget::Path(path)) => {
                yal_system::launch::reveal_in_file_manager(path).map_err(|err| err.to_string())
            }
            (ActionKind::CopyToClipboard, OpenTarget::Text(text)) => {
                let mut clipboard = yal_system::clipboard::Clipboard::new()
                    .map_err(|err| err.to_string())?;
                clipboard.write_text(text).map_err(|err| err.to_string())
            }
            (ActionKind::OpenApplication, OpenTarget::Application(id)) => {
                // The identifier is resolved against the palette's own
                // index, because only it knows where the application lives.
                let resolved = self
                    .applications
                    .iter()
                    .find(|application| application.id == *id)
                    .ok_or_else(|| format!("\"{id}\" is not an application this launcher knows"))?;
                yal_system::launch::launch(&resolved.path).map_err(|err| err.to_string())
            }
            (ActionKind::RunCommand, OpenTarget::Command { program, args }) => {
                // Detached, with no host console: the palette does not
                // wait and does not flash a window the user did not ask
                // for.
                yal_system::launch::launch_program(
                    &program.to_string_lossy(),
                    args,
                )
                    .map_err(|err| err.to_string())
            }
            (ActionKind::LauncherTask, OpenTarget::Task { name, arguments }) => {
                self.perform_task(name, arguments)
            }
            // An action with no matching implementation is not an error; it
            // is simply not carried out.
            _ => Ok(()),
        }
    }

    /// Carry out a task the launcher itself owns.
    fn perform_task(&self, name: &str, arguments: &[String]) -> Result<(), String> {
        let head: Option<String> = arguments.first().cloned();
        let head = head.as_deref();

        match name {
            "system" => {
                let command = head
                    .ok_or_else(|| "the system command is missing".to_string())?;
                perform_system(command)
            }
            "kill" => {
                let pid = head
                    .ok_or_else(|| "the process identifier is missing".to_string())?
                    .parse::<u32>()
                    .map_err(|err| format!("\"{}\" is not a process id: {err}", arguments[0]))?;
                yal_system::processes::terminate(pid).map_err(|err| err.to_string())
            }
            "clipboard-remove" => {
                let id = arguments
                    .first()
                    .ok_or_else(|| "the clipboard entry is missing".to_string())?
                    .parse::<i64>()
                    .map_err(|err| format!("the clipboard entry is not an identifier: {err}"))?;
                self.store
                    .delete_clipboard(id)
                    .map_err(|err| err.to_string())
            }
            "screenshot-window" => {
                // PrintWindow reads the named window's own surface, so
                // the palette need not leave the screen for this one.
                let handle = arguments
                    .first()
                    .ok_or_else(|| "the window handle is missing".to_string())?
                    .parse::<u64>()
                    .map_err(|err| format!("the window handle is not a number: {err}"))?;
                let capture = yal_system::screenshot::capture_window(handle)
                    .map_err(|err| err.to_string())?;
                let path = self.store_with_clipboard_image(&capture)?;
                let _ = yal_system::notify::toast(
                    "Capture",
                    &format!(
                        "saved to {}\nthe window's picture is on the clipboard",
                        path.display()
                    ),
                );
                Ok(())
            }
            "window" => {
                let handle = arguments
                    .first()
                    .ok_or_else(|| "the window handle is missing".to_string())?
                    .parse::<u64>()
                    .map_err(|err| format!("the window handle is not a number: {err}"))?;
                let action_slug = arguments
                    .get(1)
                    .map(String::as_str)
                    .unwrap_or("focus");
                let action = match action_slug {
                    "close" => yal_system::window_controls::WindowAction::Close,
                    "minimize" => yal_system::window_controls::WindowAction::Minimize,
                    "maximize" => yal_system::window_controls::WindowAction::Maximize,
                    "restore" => yal_system::window_controls::WindowAction::Restore,
                    "left-half" => yal_system::window_controls::WindowAction::LeftHalf,
                    "right-half" => yal_system::window_controls::WindowAction::RightHalf,
                    "top-half" => yal_system::window_controls::WindowAction::TopHalf,
                    "bottom-half" => yal_system::window_controls::WindowAction::BottomHalf,
                    "top-left-quarter" => yal_system::window_controls::WindowAction::TopLeftQuarter,
                    "top-right-quarter" => yal_system::window_controls::WindowAction::TopRightQuarter,
                    "bottom-left-quarter" => {
                        yal_system::window_controls::WindowAction::BottomLeftQuarter
                    }
                    "bottom-right-quarter" => {
                        yal_system::window_controls::WindowAction::BottomRightQuarter
                    }
                    "left-third" => yal_system::window_controls::WindowAction::LeftThird,
                    "middle-third" => yal_system::window_controls::WindowAction::CenterThird,
                    "right-third" => yal_system::window_controls::WindowAction::RightThird,
                    "center" => yal_system::window_controls::WindowAction::Center,
                    "fullscreen" => yal_system::window_controls::WindowAction::Fullscreen,
                    _ => yal_system::window_controls::WindowAction::Focus,
                };
                yal_system::window_controls::perform(handle, action)
                    .map_err(|err| err.to_string())
            }
            "text" => {
                let slug = head.ok_or_else(|| "the transform is missing".to_string())?;
                let input = arguments.get(1).map(String::as_str).unwrap_or("");
                let result = yal_data::text::apply(slug, input)
                    .map_err(|err| err.to_string())?;
                let mut clipboard = yal_system::clipboard::Clipboard::new()
                    .map_err(|err| err.to_string())?;
                clipboard.write_text(&result).map_err(|err| err.to_string())
            }
            "timer" => {
                let seconds = head
                    .ok_or_else(|| "the timer length is missing".to_string())?
                    .parse::<u64>()
                    .map_err(|err| format!("the timer length is not seconds: {err}"))?;
                let label = arguments.get(1).cloned().unwrap_or_else(|| "Timer".into());
                self.create_timer(seconds, &label).map_err(|err| err.to_string())
            }
            "timer-cancel" => {
                let id = head
                    .ok_or_else(|| "the timer identifier is missing".to_string())?;
                self.store
                    .clear_setting(&format!("timer:{id}"))
                    .map_err(|err| err.to_string())
            }
            "reminder" => {
                // Arguments: the due time as a Unix timestamp in seconds,
                // then the reminder's text.
                let due_at = head
                    .ok_or_else(|| "the reminder's time is missing".to_string())?
                    .parse::<i64>()
                    .map_err(|err| format!("the reminder's time is not a timestamp: {err}"))?;
                let text = arguments.get(1).cloned().unwrap_or_default();
                if text.is_empty() {
                    return Err("the reminder's text is missing".to_string());
                }
                self.create_reminder(due_at, &text)
                    .map_err(|err| err.to_string())
            }
            "reminder-done" => {
                let id = head
                    .ok_or_else(|| "the reminder identifier is missing".to_string())?;
                self.store
                    .clear_setting(&format!("reminder:{id}"))
                    .map_err(|err| err.to_string())
            }
            "note-add" => {
                let text = arguments
                    .first()
                    .ok_or_else(|| "the note text is missing".to_string())?;
                let id = chrono::Utc::now().timestamp_millis().to_string();
                let record = serde_json::json!({
                    "text": text,
                    "created_at": chrono::Utc::now().timestamp(),
                });
                self.store
                    .set_setting(&format!("note:{id}"), &record.to_string())
                    .map_err(|err| err.to_string())
            }
            "note-remove" => {
                let id = head.ok_or_else(|| "the note identifier is missing".to_string())?;
                self.store
                    .clear_setting(&format!("note:{id}"))
                    .map_err(|err| err.to_string())
            }
            "script" => {
                // Arguments: the script's path, then its mode slug. The run
                // happens in a thread the palette does not wait on, so a
                // slow script never holds the window; the outcome arrives
                // as a toast when it is known.
                let path = arguments
                    .first()
                    .ok_or_else(|| "the script path is missing".to_string())?
                    .clone();
                let mode = arguments
                    .get(1)
                    .map(String::as_str)
                    .unwrap_or("compact")
                    .to_string();
                let sender = self.sender.clone();
                let frame_ctx = self.frame_ctx.clone();
                std::thread::spawn(move || {
                    finish_script(&path, &mode, sender, frame_ctx)
                });
                Ok(())
            }
            "rates-update" => {
                // The launcher's one network act, at the user's ask. The
                // fetch runs in its own thread with its own store
                // connection, so a slow reply never holds the window;
                // the outcome arrives as a toast, and the calculator
                // picks the fresh table up on the next search.
                let Some(store_path) = self.store.path().map(|path| path.to_path_buf()) else {
                    return Err(
                        "the palette runs without a store on disk, so there is nowhere to put a fresh table"
                            .to_string(),
                    );
                };
                std::thread::Builder::new()
                    .name("rates-update".into())
                    .spawn(move || {
                        let outcome = match yal_store::Store::open(&store_path) {
                            Ok(store) => yal_engine::bridge::update_rates(&store),
                            Err(error) => Err(error.to_string()),
                        };
                        match outcome {
                            Ok((count, vintage)) => {
                                let _ = yal_system::notify::toast(
                                    "Exchange rates",
                                    &format!("{count} currencies as of {vintage}"),
                                );
                            }
                            Err(error) => {
                                let _ = yal_system::notify::toast("Exchange rates", &error);
                            }
                        }
                    })
                    .map_err(|error| format!("the update thread could not start: {error}"))?;
                Ok(())
            }
            unknown => {
                let _ = unknown;
                Err(format!("the task \"{name}\" is not one this launcher carries out"))
            }
        }
    }

    /// Store a timer in the settings, where the tick can find it again.
    fn create_timer(
        &self,
        seconds: u64,
        label: &str,
    ) -> Result<(), yal_store::StoreError> {
        let id = chrono::Utc::now().timestamp_millis().to_string();
        let ends_at = chrono::Utc::now() + chrono::Duration::seconds(seconds as i64);
        let record = serde_json::json!({
            "label": label,
            "ends_at": ends_at.timestamp(),
            "seconds": seconds,
        });
        self.store.set_setting(&format!("timer:{id}"), &record.to_string())
    }

    /// Store a reminder, to be raised when its time comes.
    ///
    /// The store keeps it beside the timers: a settings key
    /// `reminder:<id>` carrying the text and the due timestamp.
    fn create_reminder(
        &self,
        due_at: i64,
        text: &str,
    ) -> Result<(), yal_store::StoreError> {
        let id = chrono::Utc::now().timestamp_millis().to_string();
        let record = serde_json::json!({
            "text": text,
            "due_at": due_at,
        });
        self.store
            .set_setting(&format!("reminder:{id}"), &record.to_string())
    }
}

/// A system command by its slug, carried out on demand.
fn perform_system(command: &str) -> Result<(), String> {
    use yal_system::audio as audio_system;
    use yal_system::display as display_system;
    use yal_system::system as power_system;

    match command {
        "lock" => power_system::lock().map_err(|err| err.to_string()),
        "sleep" => power_system::sleep().map_err(|err| err.to_string()),
        "shutdown" => power_system::shutdown().map_err(|err| err.to_string()),
        "restart" => power_system::restart().map_err(|err| err.to_string()),
        "logout" => power_system::log_out().map_err(|err| err.to_string()),
        "empty-bin" => power_system::empty_recycle_bin().map_err(|err| err.to_string()),
        "mute" => audio_system::set_muted(true).map_err(|err| err.to_string()),
        "unmute" => audio_system::set_muted(false).map_err(|err| err.to_string()),
        "volume-up" => audio_system::adjust_volume(5.0).map(|_| ()).map_err(|err| err.to_string()),
        "volume-down" => audio_system::adjust_volume(-5.0).map(|_| ()).map_err(|err| err.to_string()),
        "brighten" => display_system::set_brightness(current_brightness()
            .map(|value| (value + 10.0).min(100.0))
            .unwrap_or(60.0))
        .map_err(|err| err.to_string()),
        "dim" => display_system::set_brightness(current_brightness()
            .map(|value| (value - 10.0).max(0.0))
            .unwrap_or(40.0))
        .map_err(|err| err.to_string()),
        "dark-mode" | "light-mode" => {
            let dark = display_system::is_dark_mode().unwrap_or(false);
            display_system::set_dark_mode(!dark)
                .map(|_| ())
                .map_err(|err| err.to_string())
        }
        other => Err(format!("the system command \"{other}\" is not one this launcher carries out")),
    }
}

/// The palette's due watcher: timers and reminders raised from off-thread.
///
/// The palette's frame loop is not a clock. An idle palette only runs an
/// update when something wakes its window — a key, a mouse, a message —
/// so a due time could sit unnoticed through a whole quiet afternoon,
/// which was verified live: an overdue reminder was still offering
/// itself half a minute past its time. The schedule belongs to this
/// thread instead. It keeps its own connection to the same store,
/// completes whatever has come due, and toasts; a toast the system
/// refuses is sent back as a `Ring`, for the banner the palette raises
/// the moment it next runs an update.
fn spawn_due_watcher(store_path: std::path::PathBuf, sender: Sender<PaletteMessage>) {
    std::thread::Builder::new()
        .name("due-watcher".into())
        .spawn(move || {
            let Ok(store) = yal_store::Store::open(&store_path) else {
                // No store to watch; the palette keeps running without a
                // schedule rather than spinning on a lost cause.
                return;
            };
            loop {
                std::thread::sleep(std::time::Duration::from_millis(500));
                for announcement in sweep_due(&store, chrono::Utc::now(), &toast_or_err) {
                    sender.send(PaletteMessage::Ring(announcement)).ok();
                }
            }
        })
        .ok();
}

/// The interval the whole-drive index walker keeps between passes.
///
/// Half an hour: the pass itself costs a burst of stat calls and one
/// listing per changed directory, and it runs on its own thread, so even
/// a busy pass costs the disk, not the interface.
const INDEX_REFRESH_PERIOD: std::time::Duration =
    std::time::Duration::from_secs(30 * 60);

/// The whole-drive index's walker thread.
///
/// It opens its own store connection — the palette's is not shareable —
/// publishes the snapshot that is already in the database before the
/// first pass (so a restart answers whole-drive queries within a moment,
/// not within a walk), runs a refresh pass over the fixed drives, and
/// republishes only when the pass changed something. The setting is read
/// every cycle, so turning the index off stops the passes and the next
/// start of the palette takes the walker down with it.
fn spawn_index_walker(store_path: std::path::PathBuf, sender: Sender<PaletteMessage>) {
    std::thread::Builder::new()
        .name("file-index".into())
        .spawn(move || {
            let Ok(store) = yal_store::Store::open(&store_path) else {
                return;
            };
            loop {
                if !yal_engine::bridge::file_index_enabled(&store) {
                    std::thread::sleep(INDEX_REFRESH_PERIOD);
                    continue;
                }

                // Publish what is already known, before any work.
                sender
                    .send(PaletteMessage::Files(std::sync::Arc::new(
                        yal_engine::bridge::snapshot(&store),
                    )))
                    .ok();

                let report = yal_engine::bridge::refresh_once(&store);
                if report.changed {
                    sender
                        .send(PaletteMessage::Files(std::sync::Arc::new(
                            yal_engine::bridge::snapshot(&store),
                        )))
                        .ok();
                }
                tracing::debug!(
                    "the index pass listed {} directories and forgot {}",
                    report.listed,
                    report.removed
                );
                std::thread::sleep(INDEX_REFRESH_PERIOD);
            }
        })
        .ok();
}

/// Complete every timer and reminder whose time has come.
///
/// Returns the announcements whose toast was refused, so the caller can
/// have the palette banner them. The raising itself is injected, so
/// tests observe the sweep without involving the platform's
/// notification centre.
fn sweep_due(
    store: &yal_store::Store,
    now: chrono::DateTime<chrono::Utc>,
    notify: &dyn Fn(&str, &str) -> Result<(), String>,
) -> Vec<String> {
    let mut refused = Vec::new();
    let Ok(timers) = yal_engine::sources::saved_timers(store) else {
        return refused;
    };
    for timer in timers {
        if timer.ends_at > now {
            continue;
        }
        let _ = store.clear_setting(&format!("timer:{}", timer.id));
        if notify(&timer.label, "The timer has finished.").is_err() {
            refused.push(format!("{}: the timer has finished", timer.label));
        }
    }
    let Ok(reminders) = yal_engine::sources::saved_reminders(store) else {
        return refused;
    };
    for reminder in reminders {
        if reminder.due_at > now {
            continue;
        }
        let _ = store.clear_setting(&format!("reminder:{}", reminder.id));
        if notify(&reminder.text, "The reminder is due.").is_err() {
            refused.push(format!("{}: the reminder is due", reminder.text));
        }
    }
    refused
}

/// The platform toast, shaped for the sweep's injected raising.
fn toast_or_err(title: &str, body: &str) -> Result<(), String> {
    yal_system::notify::toast(title, body).map_err(|err| err.to_string())
}

/// Run a script command in the background and announce the outcome.
///
/// Called from a thread the palette does not wait on, so the window
/// stays responsive while the script runs. `compact` mode toasts what
/// happened; `full` mode additionally delivers the whole output to the
/// palette — where it is shown on top of the search — with the clipboard
/// as the copy the user can paste elsewhere. When both transports share
/// the same content, the palette is the readable one and the clipboard
/// keeps its paste-elsewhere duty.
/// The toast declaring an unreadable runner honestly reports the refusal
/// the machine gives.
fn finish_script(
    path: &str,
    mode: &str,
    sender: Sender<PaletteMessage>,
    frame_ctx: Option<egui::Context>,
) {
    let full_path = std::path::PathBuf::from(path);
    let named = yal_system::scripts::read(&full_path);
    let title = named
        .as_ref()
        .map(|script| script.title.clone())
        .unwrap_or_else(|| full_path.to_string_lossy().into_owned());

    // An absent runner — python without python, say — surfaces here as a
    // run error rather than of silence.
    let outcome = match yal_system::scripts::read(&full_path) {
        None => Err(format!("\"{path}\" is no longer a script this launcher can read")),
        Some(script) => yal_system::scripts::run(&script).map_err(|err| err.to_string()),
    };

    match outcome {
        Err(reason) => {
            let _ = yal_system::notify::toast(&title, &reason);
            if mode == "full" {
                deliver_script(
                    sender,
                    frame_ctx,
                    ScriptAnnouncement {
                        title: title.clone(),
                        text: reason,
                        failed: true,
                    },
                );
            }
        }
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let failure = !output.status.success();

            if mode == "full" {
                let combined: String = if stdout.trim().is_empty() {
                    stderr.trim().to_string()
                } else {
                    stdout.trim().to_string()
                };
                if !combined.is_empty() {
                    // Full output mode: the launcher keeps the whole thing in the
                    // clipboard; the palette shows it.
                    if let Ok(mut clipboard) =
                        yal_system::clipboard::Clipboard::new()
                    {
                        let _ = clipboard.write_text(&combined);
                    }
                }
                deliver_script(
                    sender,
                    frame_ctx,
                    ScriptAnnouncement {
                        title: title.clone(),
                        text: if combined.is_empty() {
                            "(the script wrote nothing)".to_string()
                        } else {
                            combined
                        },
                        failed: failure,
                    },
                );
            }

            // The toast's second line shows one line of what the script
            // said: the first line of the output, or of the output on
            // stderr when that is all there is.
            let first_stdout = stdout.lines().next().unwrap_or("").trim();
            let words: String = if first_stdout.is_empty() {
                stderr
                    .lines()
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string()
            } else {
                first_stdout.to_string()
            };
            let state = if failure { "failed" } else { "finished" };
            let _ = yal_system::notify::toast(&format!("{title} {state}"), &words);
        }
    }
}

/// Hand a script's output to the palette and wake its frame loop.
fn deliver_script(
    sender: Sender<PaletteMessage>,
    frame_ctx: Option<egui::Context>,
    announcement: ScriptAnnouncement,
) {
    let _ = sender.send(PaletteMessage::Script(announcement));
    if let Some(ctx) = frame_ctx {
        ctx.request_repaint();
    }
}

impl Palette {
    /// The dark/light decision behind the palette's colours.
    fn theme_dark(&self, ctx: &egui::Context) -> bool {
        match self.theme {
            Theme::Dark => true,
            Theme::Light => false,
            Theme::System => ctx.style().visuals.dark_mode,
        }
    }

    /// Persist the palette hotkey and bind the listener to it again.
    fn save_hotkey(&mut self, spec: yal_system::hotkey::HotkeySpec, text: String) {
        if let Err(error) = self.store.set_setting("hotkey", &text) {
            self.status = Some(format!("the hotkey could not be saved: {error}"));
            return;
        }
        self.hotkey.stop();
        self.hotkey = yal_system::hotkey::HotkeyListener::spawn(
            spec.virtual_key,
            spec.modifiers,
            HOTKEY_ID,
        );
        self.register_hotkey();
    }

    /// The hotkey currently configured, or the default.
    fn hotkey_spec(&self) -> yal_system::hotkey::HotkeySpec {
        self.store
            .setting("hotkey")
            .ok()
            .flatten()
            .and_then(|text| yal_system::hotkey::HotkeySpec::parse(&text))
            .unwrap_or_else(yal_system::hotkey::HotkeySpec::default_spec)
    }

    /// The preferences pane, drawn instead of the search results.
    fn draw_preferences(&mut self, ui: &mut egui::Ui, style: &PaletteStyle) {
        ui.label(RichText::new("Preferences").color(style.text).size(20.0));
        ui.add_space(6.0);

        // Appearance.
        ui.label(
            RichText::new("Appearance")
                .color(style.dim_text)
                .size(13.0),
        );
        ui.horizontal(|ui| {
            for (theme, label) in [
                (Theme::System, "System"),
                (Theme::Dark, "Dark"),
                (Theme::Light, "Light"),
            ] {
                let chosen = self.theme == theme;
                if ui.selectable_label(chosen, label).clicked() {
                    self.theme = theme;
                    if let Err(error) = self.store.set_setting("theme", theme.slug()) {
                        self.status =
                            Some(format!("the theme could not be saved: {error}"));
                    }
                }
            }
        });
        ui.add_space(8.0);

        // The palette hotkey.
        ui.label(
            RichText::new("Palette hotkey")
                .color(style.dim_text)
                .size(13.0),
        );
        if self.recording_hotkey {
            ui.label(
                RichText::new("press a key — with ctrl, alt or shift — Esc cancels")
                    .color(style.accent)
                    .size(14.0),
            );
        } else {
            let spec = self.hotkey_spec();
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(spec.to_text())
                        .color(style.text)
                        .size(14.0),
                );
                if ui.button("Record…").clicked() {
                    self.recording_hotkey = true;
                }
            });
        }
        ui.add_space(8.0);

        // The script commands folder.
        ui.label(
            RichText::new("Script commands folder")
                .color(style.dim_text)
                .size(13.0),
        );
        ui.horizontal(|ui| {
            let draft_width = ui.available_width() - 90.0;
            ui.add(
                egui::TextEdit::singleline(&mut self.scripts_draft)
                    .hint_text(r"%USERPROFILE%\.yal\scripts")
                    .font(FontId::proportional(14.0))
                    .desired_width(draft_width),
            );
            if ui.button("Save").clicked() {
                let outcome = if self.scripts_draft.trim().is_empty() {
                    self.store
                        .clear_setting("scripts_folder")
                        .map(|_| "restored the default scripts folder".to_string())
                } else {
                    self.store
                        .set_setting("scripts_folder", &self.scripts_draft)
                        .map(|_| "script commands are read from the new folder".to_string())
                };
                match outcome {
                    Ok(message) => {
                        self.status = Some(message);
                        self.pending = true;
                        self.last_search = Some(Instant::now());
                        self.mode = Mode::Search;
                    }
                    Err(error) => {
                        self.status =
                            Some(format!("the folder could not be saved: {error}"));
                    }
                }
            }
        });
        ui.add_space(8.0);

        if let Some(status) = &self.status {
            ui.label(RichText::new(status).color(style.dim_text).size(13.0));
        }
        ui.add_space(4.0);
        ui.label(
            RichText::new("Esc goes back to the search")
                .color(style.dim_text)
                .size(12.0),
        );
    }
}

/// The first controllable monitor's brightness, for nudging it.
fn current_brightness() -> Option<f64> {
    yal_system::display::read_brightness()
        .ok()?
        .first()
        .map(|(_monitor, level)| *level)
}

/// The colours the palette draws with.
struct PaletteStyle {
    background: Color32,
    text: Color32,
    dim_text: Color32,
    selection: Color32,
    accent: Color32,
    border: Color32,
}

impl PaletteStyle {
    /// The palette's colours.
    ///
    /// Two palettes are defined because the palette is drawn on top of an
    /// unknown desktop background, and a single set of colours cannot be
    /// legible on both a light and a dark one.
    fn new(dark: bool) -> Self {
        if dark {
            Self {
                background: Color32::from_rgb(28, 28, 30),
                text: Color32::from_rgb(236, 236, 238),
                dim_text: Color32::from_rgb(150, 150, 158),
                selection: Color32::from_rgb(58, 58, 62),
                accent: Color32::from_rgb(120, 170, 255),
                border: Color32::from_rgb(64, 64, 68),
            }
        } else {
            Self {
                background: Color32::from_rgb(250, 250, 252),
                text: Color32::from_rgb(28, 28, 30),
                dim_text: Color32::from_rgb(110, 110, 118),
                selection: Color32::from_rgb(226, 230, 240),
                accent: Color32::from_rgb(40, 110, 220),
                border: Color32::from_rgb(210, 210, 216),
            }
        }
    }
}

impl eframe::App for Palette {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // The show location is remembered while the palette sits on
        // screen, so a park-and-return cycle can put it back exactly
        // where it was.
        if self.shown_location.is_none() {
            if let Some(handle) = yal_system::windows::own_window_handle() {
                if let Ok((x, y, _, _)) = yal_system::window_controls::rect(handle) {
                    if x >= 0 && y >= 0 {
                        self.shown_location = Some((x, y));
                    }
                }
            }
        }

        // The global hotkey toggles the palette. It is polled before
        // anything else so that the toggle is never lost between frames.
        if self.hotkey.poll() {
            if let Some(state) = self.capture.take() {
                // While the band is open the window is summoned at the
                // screen's top; the press abandons the capture outright
                // — going into a frozen picture's drag, the launcher
                // itself steps aside.
                self.capture = None;
                dismiss_capture_window(state.restore);
                self.park(ctx);
            } else if self.visible {
                self.park(ctx);
            } else {
                self.show(ctx);
            }
        }

        // Timers and reminders are watched from their own thread, because
        // the frame loop is not a clock.

        // Deliver messages sent from other threads.
        while let Ok(message) = self.receiver.try_recv() {
            match message {
                PaletteMessage::Close => self.close_requested = true,
                PaletteMessage::Show => {
                    self.show(ctx);
                }
                PaletteMessage::Ring(announcement) => {
                    self.ring = Some(announcement);
                    if !self.visible {
                        self.show(ctx);
                    }
                }
                PaletteMessage::Script(announcement) => {
                    // A full-mode script finished with output: show it on
                    // top of the search, bringing the palette back if it
                    // had parked while the script ran.
                    self.script_outcome = Some(announcement);
                    if !self.visible {
                        self.show(ctx);
                    }
                }
                PaletteMessage::Files(paths) => {
                    // A fresh snapshot of the whole-drive index. The
                    // search the palette sits in is not re-run: the next
                    // keystroke's search answers from it, which keeps a
                    // big refresh from fighting the user's typing.
                    self.indexed = paths;
                }
            }
        }

        // A capture act in flight owns the window and the frame: the
        // overlay is what this frame shows, and the normal palette
        // waits until it is over.
        if self.capture.is_some() {
            self.pump_capture(ctx);
            self.take_focus(ctx);
            ctx.request_repaint_after(std::time::Duration::from_millis(15));
            return;
        }

        let palette_style = PaletteStyle::new(self.theme_dark(ctx));

        // Remembered for the script threads: they cannot wake the frame
        // loop without it, and without the wake the output would arrive
        // only after the next unrelated repaint.
        self.frame_ctx = Some(ctx.clone());

        ctx.style_mut(|visuals| {
            visuals.visuals.window_fill = palette_style.background;
            visuals.visuals.panel_fill = palette_style.background;
            visuals.visuals.extreme_bg_color = palette_style.background;
            visuals.visuals.widgets.active.bg_fill = palette_style.selection;
            visuals.visuals.widgets.hovered.bg_fill = palette_style.selection;
            visuals.visuals.widgets.inactive.bg_fill = palette_style.background;
            visuals.visuals.widgets.noninteractive.bg_fill = palette_style.background;
            visuals.visuals.window_stroke = Stroke::new(1.0_f32, palette_style.border);
        });

        // Keyboard handling runs before the field is drawn so that the
        // arrow keys never move a caret inside the query text.
        let mut move_down = false;
        let mut move_up = false;
        let mut activate = false;
        let mut dismiss = false;
        let mut copy_outcome = false;
        // The secondary panel opens on ctrl+K when a result with further
        // acts is selected. Typed text closes it — read here, before the
        // field consumes the events — because the list under it is about
        // to change and the acts were keyed to what was selected when it
        // opened.
        let mut wants_panel = false;
        let mut typed_text = false;

        ctx.input_mut(|input| {
            if input.key_pressed(Key::ArrowDown) {
                move_down = true;
            }
            if input.key_pressed(Key::ArrowUp) {
                move_up = true;
            }
            if input.key_pressed(Key::Enter) {
                activate = true;
            }
            if input.key_pressed(Key::Escape) {
                dismiss = true;
            }
            if self.script_outcome.is_some() && input.key_pressed(Key::C) {
                copy_outcome = true;
            }
            if !self.recording_hotkey
                && self.script_outcome.is_none()
                && self.mode == Mode::Search
                && input.key_pressed(Key::K)
                && input.modifiers.ctrl
            {
                wants_panel = true;
            }
            if input
                .events
                .iter()
                .any(|event| matches!(event, egui::Event::Text(_)))
            {
                typed_text = true;
            }
            // While the preferences pane waits for a hotkey, the first
            // key pressed together with a modifier becomes it. Escape
            // does not count: it means "never mind".
            if self.recording_hotkey && !input.key_pressed(Key::Escape) {
                let pressed = [
                    Key::A, Key::B, Key::C, Key::D, Key::E, Key::F, Key::G, Key::H,
                    Key::I, Key::J, Key::K, Key::L, Key::M, Key::N, Key::O, Key::P,
                    Key::Q, Key::R, Key::S, Key::T, Key::U, Key::V, Key::W, Key::X,
                    Key::Y, Key::Z,
                ]
                .into_iter()
                .find(|key| input.key_pressed(*key));
                if let Some(key) = pressed {
                    if let Some(spec) = hotkey_spec_from(key, input.modifiers) {
                        // The spec is small and copied, so deriving its
                        // text before persisting is the plain order.
                        let text = yal_system::hotkey::HotkeySpec::to_text(spec);
                        self.status = Some(format!("hotkey saved: {text}"));
                        self.save_hotkey(spec, text);
                        self.recording_hotkey = false;
                    } else {
                        self.status = Some(
                            "a modifier is needed: hold ctrl, alt or shift with the key".into(),
                        );
                    }
                    move_down = false;
                    move_up = false;
                    activate = false;
                }
            }
        });

        // While a script's full output is on screen the keys belong to it:
        // C copies, Enter and Escape close the sheet back to the search the
        // user left behind. The results underneath are not driven.
        if self.script_outcome.is_some() {
            if copy_outcome {
                if let Some(outcome) = &self.script_outcome {
                    if let Ok(mut clipboard) = yal_system::clipboard::Clipboard::new() {
                        let _ = clipboard.write_text(&outcome.text);
                    }
                }
            }
            if activate || dismiss {
                self.script_outcome = None;
            }
            activate = false;
            move_down = false;
            move_up = false;
            dismiss = false;
        }

        // The secondary panel, while open, owns the keys: arrows move its
        // highlight, enter acts in it, escape returns to the search, typed
        // text closes it ahead of a re-search. It opened on ctrl+K only
        // when the selected result carried further acts; a result with
        // none says so.
        let mut panel_opening = false;
        if wants_panel {
            let acts = self
                .results
                .get(self.selected)
                .map(|result| result.item.secondary_actions.clone())
                .unwrap_or_default();
            if acts.is_empty() {
                self.status =
                    Some("the chosen result carries nothing beyond its first act".into());
            } else {
                self.action_panel = Some(acts);
                self.panel_selected = 0;
                panel_opening = true;
            }
        }
        if self.action_panel.is_some() {
            if panel_opening {
                move_down = false;
                move_up = false;
                activate = false;
                dismiss = false;
            } else {
                let panel_len = self.action_panel.as_ref().map(Vec::len);
                if let Some(len) = panel_len {
                    if move_down {
                        self.panel_selected = (self.panel_selected + 1) % len;
                    }
                    if move_up {
                        self.panel_selected = if self.panel_selected == 0 {
                            len - 1
                        } else {
                            self.panel_selected - 1
                        };
                    }
                }
                if activate {
                    self.perform_panel_action();
                }
                if dismiss || typed_text {
                    self.action_panel = None;
                }
                move_down = false;
                move_up = false;
                activate = false;
                dismiss = false;
            }
        }

        if dismiss && self.mode == Mode::Preferences {
            // Escape in the preferences pane goes back to the search; the
            // palette itself stays open.
            self.mode = Mode::Search;
            self.recording_hotkey = false;
            dismiss = false;
        }
        if dismiss {
            // Escape parks the palette rather than closing it, so that the
            // hotkey can bring it back.
            self.park(ctx);
        }

        if self.mode == Mode::Search {
            if move_down && !self.results.is_empty() {
                self.selected = (self.selected + 1) % self.results.len();
            }
            if move_up && !self.results.is_empty() {
                self.selected = if self.selected == 0 {
                    self.results.len() - 1
                } else {
                    self.selected - 1
                };
            }
            if activate {
                self.perform_selected();
            }
        }

        let frame = egui::Frame::new()
            .fill(palette_style.background)
            .inner_margin(egui::Margin::same(10))
            .stroke(Stroke::new(1.0_f32, palette_style.border))
            .corner_radius(8.0);

        // A result that carries an image or photograph metadata gets a
        // panel of its own. Drawing it below the list would put it off the
        // screen, because the list takes all the space that is left, so the
        // results list scrolls in what remains: the window keeps its size.
        let photo_metadata = self.selected_photo_metadata();
        let detail_panel = self.selected_image().is_some() || photo_metadata.is_some();
        if detail_panel {
            let image = self
                .selected_image()
                .and_then(|bytes| self.image_texture(ctx, &bytes))
                .filter(|texture| texture.size()[0] > 0);
            egui::TopBottomPanel::bottom("detail").show(ctx, |ui| {
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    if let Some(texture) = &image {
                        let size = texture.size_vec2();
                        let height = 150.0_f32.min(size.y);
                        let scale = height / size.y.max(1.0);
                        ui.add(
                            egui::Image::new(texture)
                                .fit_to_exact_size(
                                    egui::vec2(size.x * scale, size.y * scale),
                                )
                                .corner_radius(4.0),
                        );
                        ui.add_space(10.0);
                    }
                    if let Some(metadata) = &photo_metadata {
                        ui.vertical(|ui| {
                            for (label, value) in metadata.lines() {
                                ui.label(
                                    RichText::new(format!("{label}: {value}"))
                                        .color(palette_style.dim_text)
                                        .size(13.0),
                                );
                            }
                        });
                    }
                });
            });
        }

        egui::CentralPanel::default()
            .frame(frame)
            .show(ctx, |ui| {
                // The query field, plus the preferences button beside it.
                ui.horizontal(|ui| {
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.query)
                            .hint_text("Search applications, snippets, and more…")
                            .font(FontId::proportional(22.0))
                            .desired_width(ui.available_width() - 46.0),
                    );
                    if response.changed() {
                        self.pending = true;
                        self.last_search = Some(Instant::now());
                        self.selected = 0;
                    }
                    response.request_focus();

                    let gear = if self.mode == Mode::Preferences {
                        "⬅"
                    } else {
                        "⚙"
                    };
                    let gear_button = egui::Button::new(RichText::new(gear).size(18.0))
                        .frame(false)
                        .small();
                    if ui
                        .add(gear_button)
                        .on_hover_text(if self.mode == Mode::Preferences {
                            "Back to the search"
                        } else {
                            "Preferences"
                        })
                        .clicked()
                    {
                        self.mode = match self.mode {
                            Mode::Search => {
                                self.scripts_draft = self
                                    .store
                                    .setting("scripts_folder")
                                    .ok()
                                    .flatten()
                                    .unwrap_or_default();
                                self.recording_hotkey = false;
                                Mode::Preferences
                            }
                            Mode::Preferences => Mode::Search,
                        };
                    }
                });

                ui.add_space(6.0);
                ui.separator();
                ui.add_space(4.0);

                if self.mode == Mode::Preferences {
                    self.draw_preferences(ui, &palette_style);
                    return;
                }

                // The results.
                egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
                    if self.results.is_empty() {
                        ui.add_space(24.0);
                        ui.with_layout(Layout::top_down(Align::Center), |ui| {
                            ui.label(
                                RichText::new("No results")
                                    .color(palette_style.dim_text)
                                    .size(15.0),
                            );
                        });
                        return;
                    }

                    for index in 0..self.results.len() {
                        let result = &self.results[index];
                        let selected = index == self.selected;
                        let (rect, response) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width(), ROW_HEIGHT),
                            egui::Sense::click(),
                        );
                        draw_result(ui, rect, result, selected, &palette_style);
                        // The panel positions itself beside the selected
                        // row; the rect is from the frame that last drew
                        // it, which is one frame of lag at most.
                        if selected {
                            self.selected_rect = Some(rect);
                        }
                        if response.clicked() {
                            self.selected = index;
                            self.perform_selected();
                        }
                    }
                });

                if let Some(ring) = &self.ring {
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(ring.clone())
                            .color(Color32::from_rgb(230, 120, 120))
                            .size(15.0),
                    );
                }
                if let Some(status) = &self.status {
                    ui.add_space(6.0);
                    ui.label(RichText::new(status).color(Color32::from_rgb(230, 120, 120)));
                }
            });

        // A script's full output, drawn on top of the palette. The search
        // beneath it is the list the user left behind; Enter or Escape
        // returns to it.
        if let Some(outcome) = &self.script_outcome {
            let failed = outcome.failed;
            egui::Window::new(RichText::new(outcome.title.clone()).strong())
                .anchor(egui::Align2::CENTER_CENTER, [0.0, -20.0])
                .collapsible(false)
                .resizable(true)
                .default_width(540.0)
                .default_height(260.0)
                .max_width(660.0)
                .max_height(380.0)
                .show(ctx, |ui| {
                    ui.set_min_height(200.0);
                    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                        ui.label(
                            RichText::new(&outcome.text)
                                .monospace()
                                .color(palette_style.text),
                        );
                    });
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(if failed {
                            format!("{} · Esc closes · C copies", "the script exited with an error")
                        } else {
                            "Esc closes · C copies the output".to_string()
                        })
                        .small()
                        .color(palette_style.dim_text),
                    );
                });
        }

        // The secondary acts of the selected result, opened with ctrl+K.
        // The panel sits beside the row it acts on — below it, or above
        // when the row is near the screen floor.
        if self.action_panel.is_some() {
            let acts: Vec<Action> =
                self.action_panel.clone().unwrap_or_default();
            let header = self
                .results
                .get(self.selected)
                .map(|result| result.item.title.clone())
                .unwrap_or_default();
            let row_height = 30.0;
            let estimate = acts.len() as f32 * row_height + 46.0;

            let position = match self.selected_rect {
                Some(rect) => {
                    let floor = ctx.screen_rect().bottom();
                    let mut y = rect.bottom() + 4.0;
                    if y + estimate > floor {
                        y = (rect.top() - estimate - 4.0).max(ctx.screen_rect().top() + 8.0);
                    }
                    let x = rect.left().max(ctx.screen_rect().left() + 8.0);
                    egui::pos2(x, y)
                }
                None => ctx.screen_rect().center() + egui::vec2(140.0, -estimate / 2.0),
            };

            egui::Window::new("the acts of the chosen result")
                .title_bar(false)
                .collapsible(false)
                .resizable(false)
                .current_pos(position)
                .default_width(300.0)
                .max_width(300.0)
                .min_width(300.0)
                .show(ctx, |ui| {
                    ui.label(
                        RichText::new(header)
                            .small()
                            .strong()
                            .color(palette_style.dim_text),
                    );
                    ui.separator();
                    for (index, action) in acts.iter().enumerate() {
                        let selected = index == self.panel_selected;
                        let (row_rect, response) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width(), row_height),
                            egui::Sense::click(),
                        );
                        if selected {
                            ui.painter()
                                .rect_filled(row_rect, 4.0, palette_style.selection);
                        }
                        ui.painter().text(
                            row_rect.left_center() + egui::vec2(8.0, 0.0),
                            egui::Align2::LEFT_CENTER,
                            action_label(action),
                            FontId::proportional(14.0),
                            palette_style.text,
                        );
                        if response.clicked() {
                            self.panel_selected = index;
                            self.perform_panel_action();
                        }
                    }
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new("↑ ↓ choose · ↵ do it · esc back")
                            .small()
                            .color(palette_style.dim_text),
                    );
                });
        }

        // Run the search once the debounce window has passed.
        if self.pending {
            if let Some(last) = self.last_search {
                if last.elapsed() >= SEARCH_DEBOUNCE {
                    self.run_search();
                } else {
                    ctx.request_repaint_after(SEARCH_DEBOUNCE);
                }
            }
        }

        if self.hide_requested {
            self.hide_requested = false;
            if let Some(text) = self.pending_paste.take() {
                self.expand_in_the_background(text);
            }
            self.park(ctx);
        }

        self.take_focus(ctx);

        // A parked window is visible to the window manager, so the frames
        // keep coming; the paint request keeps the loop ticking in any
        // case.
        if !self.visible {
            ctx.request_repaint_after(std::time::Duration::from_millis(30));
        }

        if self.close_requested {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

/// A screen-capture act's last frame: put the window away with its old
/// shape back, tucked at the park corner.
///
/// The size is the important part — the area act grew the window over
/// the screen, and a show() that restores only the position would leave
/// a window the size of the screen parked off it.
fn dismiss_capture_window(restore: (i32, i32, i32, i32)) {
    if let Some(handle) = yal_system::windows::own_window_handle() {
        let _ = yal_system::window_controls::place(
            handle,
            Some(-16000),
            Some(-16000),
            Some(restore.2),
            Some(restore.3),
        );
    }
}

/// The receipt row for a picked colour.
///
/// The primary act puts the hex on the clipboard again; the panel offers
/// the rgb and hsl sayings beside it. Built here, away from the palette,
/// so the row's promises are testable in isolation.
fn colour_row(hex: &str, rgb: &str, hsl: &str) -> yal_core::item::Item {
    yal_core::item::Item::new(format!("The colour {hex}"))
        .with_id(format!("colour:{hex}"))
        .with_subtitle(format!("{rgb} · {hsl}"))
        .with_action(Action::copy(hex.to_string()))
        .with_secondary_action(Action::copy(rgb.to_string()))
        .with_secondary_action(Action::copy(hsl.to_string()))
}

/// Draw one frame of the frozen-screen overlay and read the band.
///
/// The frozen frame is drawn over the whole window; wherever a band is
/// being dragged, the rest of the screen is dimmed around it and the
/// band's size counted in pixels. A band closed too small counts as a
/// click, and a click abandons the act.
fn draw_capture(ctx: &egui::Context, mut state: CaptureState) -> (CaptureOutcome, CaptureState) {
    let (Some(texture), Some(frame)) = (&state.texture, &state.frame) else {
        return (CaptureOutcome::Abandoned, state);
    };

    // Escape ends the act before anything is drawn.
    if ctx.input(|input| input.key_pressed(Key::Escape)) {
        return (CaptureOutcome::Abandoned, state);
    }

    let scale = ctx.pixels_per_point();
    let mut band: Option<egui::Pos2> = state.band_start;
    let mut closed: Option<egui::Pos2> = None;

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(ctx, |ui| {
            let available = ui.available_rect_before_wrap();
            let (_, response) =
                ui.allocate_exact_size(available.size(), egui::Sense::click_and_drag());
            let response = response.on_hover_cursor(egui::CursorIcon::Crosshair);

            // The frozen frame over every point of the window.
            ui.painter().image(
                texture.id(),
                available,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );

            let pointer_now = response.interact_pointer_pos();
            if response.drag_started() {
                band = pointer_now;
            }

            let band_color = Color32::from_rgb(140, 190, 255);
            let dim = Color32::from_rgba_unmultiplied(0, 0, 0, 90);

            if let Some(start) = band {
                let end = pointer_now.unwrap_or(start);
                let bx = egui::Rect::from_two_pos(start, end);
                // Four dim sheets around the band, so the picture's own
                // brightness says what is in and what is out.
                let zero = egui::CornerRadius::ZERO;
                ui.painter().rect_filled(egui::Rect::from_min_max(available.min, egui::pos2(available.right(), bx.top())), zero, dim);
                ui.painter().rect_filled(egui::Rect::from_min_max(egui::pos2(available.left(), bx.bottom()), available.max), zero, dim);
                ui.painter().rect_filled(egui::Rect::from_min_max(egui::pos2(available.left(), bx.top()), egui::pos2(bx.left(), bx.bottom())), zero, dim);
                ui.painter().rect_filled(egui::Rect::from_min_max(egui::pos2(bx.right(), bx.top()), egui::pos2(available.right(), bx.bottom())), zero, dim);
                ui.painter().rect_stroke(
                    bx,
                    zero,
                    Stroke::new(1.4_f32, band_color),
                    egui::StrokeKind::Middle,
                );
                // The band's size, in the pixels that will be cropped.
                let width = ((bx.width() * scale).round() as i64).max(0);
                let height = ((bx.height() * scale).round() as i64).max(0);
                ui.painter().text(
                    bx.left_top() + egui::vec2(4.0, -6.0),
                    egui::Align2::LEFT_BOTTOM,
                    format!("{width} × {height} px — esc leaves, let go keeps"),
                    FontId::proportional(13.0),
                    Color32::from_rgb(225, 232, 245),
                );
            } else {
                // Waiting for the first press: a light dim and a word.
                let zero = egui::CornerRadius::ZERO;
                ui.painter().rect_filled(available, zero, Color32::from_rgba_unmultiplied(0, 0, 0, 40));
                ui.painter().text(
                    available.center(),
                    egui::Align2::CENTER_CENTER,
                    "drag the band around what you want · esc leaves",
                    FontId::proportional(15.0),
                    Color32::from_rgb(225, 232, 245),
                );
            }

            if response.drag_stopped() {
                closed = pointer_now;
            }
        });

    state.band_start = band;

    if let Some((start, end)) = band.zip(closed) {
        // The band's pixels, clamped inside the frozen frame.
        let x1 = (start.x.min(end.x) * scale).floor();
        let y1 = (start.y.min(end.y) * scale).floor();
        let x2 = (start.x.max(end.x) * scale).ceil();
        let y2 = (start.y.max(end.y) * scale).ceil();
        let x1 = x1.max(0.0).min(frame.width() as f32) as i32;
        let y1 = y1.max(0.0).min(frame.height() as f32) as i32;
        let x2 = x2.max(0.0).min(frame.width() as f32) as i32;
        let y2 = y2.max(0.0).min(frame.height() as f32) as i32;
        let width = x2.saturating_sub(x1) as u32;
        let height = y2.saturating_sub(y1) as u32;
        if width < 4 || height < 4 {
            (CaptureOutcome::Abandoned, state)
        } else {
            (
                CaptureOutcome::Banded {
                    region: (x1, y1, width, height),
                },
                state,
            )
        }
    } else {
        (CaptureOutcome::Continue, state)
    }
}

/// Draw one frame of the colour-pick overlay and read the taken pixel.
///
/// A loupe blown up out of the frozen texture follows the pointer — the
/// nearest texel option keeps its magnified pixels crisp and unblended —
/// with the pixel that would be taken marked, and its colour named. A
/// click or Enter takes it, escape leaves.
fn draw_pick(ctx: &egui::Context, state: CaptureState) -> (CaptureOutcome, CaptureState) {
    let (Some(texture), Some(frame)) = (&state.texture, &state.frame) else {
        return (CaptureOutcome::Abandoned, state);
    };

    // Escape ends the act before anything is drawn.
    if ctx.input(|input| input.key_pressed(Key::Escape)) {
        return (CaptureOutcome::Abandoned, state);
    }

    let scale = ctx.pixels_per_point();
    let mut pointer_seen: Option<egui::Pos2> = None;
    let mut taken: Option<egui::Pos2> = None;

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(ctx, |ui| {
            let available = ui.available_rect_before_wrap();
            let (_, response) =
                ui.allocate_exact_size(available.size(), egui::Sense::click());
            let response = response.on_hover_cursor(egui::CursorIcon::Crosshair);

            // The frozen frame over every point of the window. Nothing
            // is dimmed here: the loupe is drawn from the frame's own
            // pixels, and a tint under the pointer would lie about the
            // colour it is pointing at.
            ui.painter().image(
                texture.id(),
                available,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );

            pointer_seen = response.hover_pos();
            let Some(pointer) = pointer_seen else {
                // No pointer over the window yet: a word only.
                ui.painter().text(
                    available.center(),
                    egui::Align2::CENTER_CENTER,
                    "move over what you want · click or enter takes the pixel · esc leaves",
                    FontId::proportional(15.0),
                    Color32::from_rgb(225, 232, 245),
                );
                return;
            };

            // The pixel under the pointer, in the frame's own pixels.
            let cx = ((pointer.x * scale).floor() as i32).clamp(0, frame.width() as i32 - 1);
            let cy = ((pointer.y * scale).floor() as i32).clamp(0, frame.height() as i32 - 1);
            let pixel = frame.get_pixel(cx as u32, cy as u32);
            let hex = format!("#{:02x}{:02x}{:02x}", pixel[0], pixel[1], pixel[2]);

            // The loupe: the block of pixels around the pointer, blown
            // up out of the texture. The block shifts rather than
            // showing edges of nothing when the pointer is at a rim.
            const HALF: i32 = 8; // a 17 × 17 block of frame pixels
            const CELL: f32 = 6.0; // display points per frame pixel
            let left = (cx - HALF).max(0);
            let top = (cy - HALF).max(0);
            let right = (cx + HALF + 1).min(frame.width() as i32);
            let bottom = (cy + HALF + 1).min(frame.height() as i32);
            let span = egui::vec2(
                (right - left) as f32 * CELL,
                (bottom - top) as f32 * CELL,
            );
            // Near the pointer, a little below and to the side, clamped
            // into the window with room underneath for the colour's
            // name.
            let mut loupe_min =
                pointer + egui::vec2(18.0, 18.0);
            loupe_min.x = loupe_min
                .x
                .clamp(available.left() + 4.0, (available.right() - span.x - 4.0).max(available.left() + 4.0));
            loupe_min.y = loupe_min
                .y
                .clamp(available.top() + 4.0, (available.bottom() - span.y - 30.0).max(available.top() + 4.0));
            let loupe = egui::Rect::from_min_size(loupe_min, span);
            let zero = egui::CornerRadius::ZERO;
            let frame_width = frame.width() as f32;
            let frame_height = frame.height() as f32;
            ui.painter().rect_stroke(
                loupe.expand(2.0),
                zero,
                Stroke::new(2.0_f32, Color32::from_rgb(18, 22, 30)),
                egui::StrokeKind::Middle,
            );
            ui.painter().image(
                texture.id(),
                loupe,
                egui::Rect::from_min_max(
                    egui::pos2(left as f32 / frame_width, top as f32 / frame_height),
                    egui::pos2(right as f32 / frame_width, bottom as f32 / frame_height),
                ),
                Color32::WHITE,
            );
            ui.painter().rect_stroke(
                loupe,
                zero,
                Stroke::new(2.0_f32, Color32::from_rgb(233, 238, 248)),
                egui::StrokeKind::Middle,
            );

            // The one pixel that would be taken, marked inside the
            // loupe.
            let at = loupe.min + egui::vec2((cx - left) as f32, (cy - top) as f32) * CELL;
            let mark = egui::Rect::from_min_size(at, egui::vec2(CELL, CELL));
            ui.painter().rect_stroke(
                mark,
                zero,
                Stroke::new(1.4_f32, Color32::from_rgb(10, 12, 16)),
                egui::StrokeKind::Inside,
            );
            ui.painter().rect_stroke(
                mark,
                zero,
                Stroke::new(0.6_f32, Color32::WHITE),
                egui::StrokeKind::Inside,
            );

            // The colour's name, beside its loupe.
            let label = format!(
                "{hex} · rgb({}, {}, {})",
                pixel[0], pixel[1], pixel[2]
            );
            let place = egui::pos2(loupe.left() + 2.0, loupe.bottom() + 12.0);
            ui.painter().text(
                place + egui::vec2(1.0, 1.0),
                egui::Align2::LEFT_TOP,
                &label,
                FontId::proportional(14.0),
                Color32::from_rgb(10, 12, 16),
            );
            ui.painter().text(
                place,
                egui::Align2::LEFT_TOP,
                &label,
                FontId::proportional(14.0),
                Color32::WHITE,
            );

            if response.clicked() {
                taken = pointer_seen;
            }
        });

    // Enter takes the pixel under the pointer too, for a hand that
    // would rather not click.
    if ctx.input(|input| input.key_pressed(Key::Enter)) {
        taken = pointer_seen;
    }

    if let Some(pos) = taken {
        // The pixel, clamped inside the frozen frame.
        let x = ((pos.x * scale).floor() as i32).clamp(0, frame.width() as i32 - 1);
        let y = ((pos.y * scale).floor() as i32).clamp(0, frame.height() as i32 - 1);
        (CaptureOutcome::Picked { x, y }, state)
    } else {
        (CaptureOutcome::Continue, state)
    }
}

/// The hotkey a recorded key press names, when the launcher can own it.
///
/// A hotkey needs at least one modifier — without one, every keystroke
/// would summon the palette, so a plain key is refused here. The Windows
/// key is not recorded, because the system reserves it for the Start menu
/// on this platform.
fn hotkey_spec_from(
    key: egui::Key,
    modifiers: egui::Modifiers,
) -> Option<yal_system::hotkey::HotkeySpec> {
    use egui::Key;

    // Letters and digits are at their Windows virtual-key codes; the
    // function keys start at 0x70 with F1.
    let virtual_key = match key {
        Key::A => 0x41,
        Key::B => 0x42,
        Key::C => 0x43,
        Key::D => 0x44,
        Key::E => 0x45,
        Key::F => 0x46,
        Key::G => 0x47,
        Key::H => 0x48,
        Key::I => 0x49,
        Key::J => 0x4A,
        Key::K => 0x4B,
        Key::L => 0x4C,
        Key::M => 0x4D,
        Key::N => 0x4E,
        Key::O => 0x4F,
        Key::P => 0x50,
        Key::Q => 0x51,
        Key::R => 0x52,
        Key::S => 0x53,
        Key::T => 0x54,
        Key::U => 0x55,
        Key::V => 0x56,
        Key::W => 0x57,
        Key::X => 0x58,
        Key::Y => 0x59,
        Key::Z => 0x5A,
        Key::Num0 => 0x30,
        Key::Num1 => 0x31,
        Key::Num2 => 0x32,
        Key::Num3 => 0x33,
        Key::Num4 => 0x34,
        Key::Num5 => 0x35,
        Key::Num6 => 0x36,
        Key::Num7 => 0x37,
        Key::Num8 => 0x38,
        Key::Num9 => 0x39,
        Key::Space => 0x20,
        _ => return None,
    };

    if !(modifiers.ctrl || modifiers.alt || modifiers.shift) {
        return None;
    }

    Some(yal_system::hotkey::HotkeySpec {
        virtual_key,
        modifiers: yal_system::hotkey::HotkeyModifiers {
            control: modifiers.ctrl,
            alt: modifiers.alt,
            shift: modifiers.shift,
            super_key: false,
        },
    })
}

/// Whether a file name is one the preview panel draws.
fn is_image_file_name(name: &str) -> bool {
    std::path::Path::new(name)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp"
            )
        })
}

/// A short readable sentence for an act, for the secondary panel.
///
/// The panel does not carry the raw action kinds the CLI and the engine
/// speak; a user acts on words.
fn action_label(action: &Action) -> String {
    match (&action.kind, &action.target) {
        (ActionKind::RevealInFileManager, _) => "show it in the file manager".to_string(),
        (ActionKind::CopyToClipboard, OpenTarget::Path(_)) => "copy the path".to_string(),
        (ActionKind::CopyToClipboard, _) => "copy it to the clipboard".to_string(),
        (ActionKind::OpenUrl, OpenTarget::Url(url)) => format!("open {url} in the browser"),
        (ActionKind::PasteToClipboard, _) => "type it where I came from".to_string(),
        (ActionKind::OpenPath, OpenTarget::Path(path)) => format!(
            "open {}",
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.to_string_lossy().into_owned())
        ),
        (ActionKind::OpenApplication, _) => "open the application".to_string(),
        (ActionKind::RunCommand, OpenTarget::Command { program, .. }) => format!(
            "run {}",
            program
                .file_stem()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| program.to_string_lossy().into_owned())
        ),
        (ActionKind::OpenInApplication, OpenTarget::ApplicationAndText { application, .. }) => {
            format!("open it with {application}")
        }
        (ActionKind::LauncherTask, OpenTarget::Task { name, .. }) => match name.as_str() {
            "clipboard-remove" => "remove it from the history".to_string(),
            _ => format!("carry out {name}"),
        },
        (ActionKind::WriteFile, _) => "write it to a file".to_string(),
        // Every other combination of kind and target is not produced by
        // the sources; it would be a lie to describe it in detail.
        _ => "carry it out".to_string(),
    }
}

/// Draw one result row.
///
/// The row has three columns: the title on the left, the subtitle in the
/// middle, and the command section on the right. Each column is given a
/// measured width budget so that a long path can never run over the section
/// label.
fn draw_result(
    ui: &egui::Ui,
    rect: egui::Rect,
    result: &Scored,
    selected: bool,
    style: &PaletteStyle,
) {
    let painter = ui.painter();

    if selected {
        painter.rect_filled(rect, 6.0, style.selection);
    }

    let section_font = FontId::proportional(12.0);
    let section_galley = painter.layout(
        result.kind.section_title().to_string(),
        section_font,
        style.dim_text,
        f32::INFINITY,
    );
    let section_width = section_galley.size().x + 12.0;

    let text_left = rect.left() + 10.0;
    let text_right = rect.right() - 10.0 - section_width;
    let text_width = (text_right - text_left).max(40.0);

    // The title takes at most a little over half the text area; the rest
    // belongs to the subtitle.
    let title_font = FontId::proportional(15.0);
    let title_max = text_width * 0.55;
    let title_galley = build_title_galley(
        painter,
        &result.item.title,
        &result.matched.ranges,
        title_font,
        style.text,
        style.accent,
        title_max,
    );
    let title_width = title_galley.size().x;

    painter.galley(
        egui::pos2(
            text_left,
            rect.center().y - title_galley.size().y / 2.0,
        ),
        title_galley,
        style.text,
    );

    if let Some(subtitle) = &result.item.subtitle {
        let subtitle_font = FontId::proportional(13.0);
        let subtitle_max = (text_width - title_width - 10.0).max(40.0);
        let subtitle_galley = truncate_to_width(
            painter,
            subtitle,
            subtitle_font,
            style.dim_text,
            subtitle_max,
        );
        painter.galley(
            egui::pos2(
                text_left + title_width + 10.0,
                rect.center().y - subtitle_galley.size().y / 2.0,
            ),
            subtitle_galley,
            style.dim_text,
        );
    }

    painter.galley(
        egui::pos2(
            rect.right() - 10.0 - section_width + 6.0,
            rect.center().y - section_galley.size().y / 2.0,
        ),
        section_galley,
        style.dim_text,
    );
}

/// Build the title layout, with the matched characters highlighted.
///
/// The title is truncated to fit its column. When it is truncated the match
/// ranges are clamped to the text that remains, so the highlighting can
/// never point past the end of the drawn text.
fn build_title_galley(
    painter: &egui::Painter,
    title: &str,
    ranges: &[std::ops::Range<usize>],
    font: FontId,
    normal: Color32,
    accent: Color32,
    max_width: f32,
) -> std::sync::Arc<egui::text::Galley> {
    let characters: Vec<char> = title.chars().collect();
    let full = job_for(painter, &characters, characters.len(), ranges, font.clone(), normal, accent);
    if full.size().x <= max_width {
        return full;
    }

    // Find the longest prefix that fits the column.
    let mut low = 0usize;
    let mut high = characters.len();
    let mut best = job_for(painter, &characters, 0, ranges, font.clone(), normal, accent);
    while low <= high {
        let middle = (low + high) / 2;
        let candidate = job_for(painter, &characters, middle, ranges, font.clone(), normal, accent);
        if candidate.size().x <= max_width {
            best = candidate;
            low = middle + 1;
        } else {
            if middle == 0 {
                break;
            }
            high = middle - 1;
        }
    }
    best
}

/// Build a layout job for the first `budget` characters of a title.
fn job_for(
    painter: &egui::Painter,
    characters: &[char],
    budget: usize,
    ranges: &[std::ops::Range<usize>],
    font: FontId,
    normal: Color32,
    accent: Color32,
) -> std::sync::Arc<egui::text::Galley> {
    let visible = budget.min(characters.len());
    let clipped: Vec<char> = characters[..visible].to_vec();
    let text: String = clipped.iter().collect();
    let truncated = visible < characters.len();
    let suffix = if truncated { "…" } else { "" };

    let mut job = LayoutJob::default();
    for (segment, matched) in split_by_ranges(&text, ranges) {
        job.append(
            &segment,
            0.0,
            TextFormat {
                font_id: font.clone(),
                color: if matched { accent } else { normal },
                ..Default::default()
            },
        );
    }
    if truncated {
        job.append(
            suffix,
            0.0,
            TextFormat {
                font_id: font.clone(),
                color: normal,
                ..Default::default()
            },
        );
    }
    painter.layout_job(job)
}

/// Lay out a string, truncating it with an ellipsis when it does not fit.
fn truncate_to_width(
    painter: &egui::Painter,
    text: &str,
    font: FontId,
    color: Color32,
    max_width: f32,
) -> std::sync::Arc<egui::text::Galley> {
    let full = painter.layout(text.to_string(), font.clone(), color, f32::INFINITY);
    if full.size().x <= max_width {
        return full;
    }

    let characters: Vec<char> = text.chars().collect();
    let mut low = 0usize;
    let mut high = characters.len();
    let mut best = painter.layout(String::new(), font.clone(), color, f32::INFINITY);
    while low <= high {
        let middle = (low + high) / 2;
        let candidate: String = characters[..middle].iter().collect::<String>() + "…";
        let galley = painter.layout(candidate, font.clone(), color, f32::INFINITY);
        if galley.size().x <= max_width {
            best = galley;
            low = middle + 1;
        } else {
            if middle == 0 {
                break;
            }
            high = middle - 1;
        }
    }
    best
}

/// Decode standard base64 into bytes.
fn decode_base64(text: &str) -> Option<Vec<u8>> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut buffer = 0u32;
    let mut bits = 0u32;
    let mut bytes = Vec::new();
    for character in text.chars() {
        if character == '=' || character.is_whitespace() {
            continue;
        }
        let index = ALPHABET.iter().position(|&c| c == character as u8)? as u32;
        buffer = (buffer << 6) | index;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            bytes.push(((buffer >> bits) & 0xff) as u8);
        }
    }
    Some(bytes)
}

/// Split a string into matched and unmatched segments.
///
/// The ranges are character indices, which is what the matcher produces;
/// the split is performed over characters so that a multi-byte character is
/// never divided.
fn split_by_ranges(
    text: &str,
    ranges: &[std::ops::Range<usize>],
) -> Vec<(String, bool)> {
    if ranges.is_empty() {
        return vec![(text.to_string(), false)];
    }

    let mut segments: Vec<(String, bool)> = Vec::new();
    let mut current = String::new();
    let mut current_matched = false;

    for (index, ch) in text.chars().enumerate() {
        let matched = ranges.iter().any(|range| range.contains(&index));
        if matched != current_matched && !current.is_empty() {
            segments.push((std::mem::take(&mut current), current_matched));
        }
        current_matched = matched;
        current.push(ch);
    }
    if !current.is_empty() {
        segments.push((current, current_matched));
    }
    segments
}

/// Run the palette.
///
/// This blocks until the window is closed.
pub fn run(store: Store) -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([760.0, 480.0])
            .with_min_inner_size([420.0, 240.0])
            .with_decorations(false)
            .with_always_on_top()
            .with_resizable(true),
        ..Default::default()
    };

    eframe::run_native(
        "YAL",
        options,
        Box::new(move |_cc| Ok(Box::new(Palette::new(store)))),
    )
}

/// The location of the default store.
pub fn default_store_path() -> Option<PathBuf> {
    yal_store::default_path()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sweep_clears_what_is_due_and_reports_refused_toasts() {
        let store = yal_store::Store::open_in_memory().unwrap();
        let now = chrono::Utc::now();
        let past = (now - chrono::Duration::minutes(5)).timestamp();
        let future = (now + chrono::Duration::minutes(5)).timestamp();
        for (key, record) in [
            (
                "timer:due",
                serde_json::json!({"label": "old timer", "ends_at": past, "seconds": 10}),
            ),
            (
                "reminder:due",
                serde_json::json!({"text": "late reminder", "due_at": past}),
            ),
            (
                "timer:waiting",
                serde_json::json!({"label": "running timer", "ends_at": future, "seconds": 500}),
            ),
            (
                "reminder:waiting",
                serde_json::json!({"text": "later reminder", "due_at": future}),
            ),
            ("snippet:keep", serde_json::json!({"text": "untouched"})),
        ] {
            store.set_setting(key, &record.to_string()).unwrap();
        }

        // A notify that refuses everything: every due item must come
        // back as an announcement for the banner.
        let refused = |_: &str, _: &str| Err("refused".to_string());
        let announcements = sweep_due(&store, now, &refused);
        assert_eq!(
            announcements,
            vec![
                "old timer: the timer has finished".to_string(),
                "late reminder: the reminder is due".to_string(),
            ]
        );

        let keys: Vec<String> = store
            .settings()
            .unwrap()
            .into_iter()
            .map(|(key, _)| key)
            .collect();
        assert!(!keys.iter().any(|key| key == "timer:due"));
        assert!(!keys.iter().any(|key| key == "reminder:due"));
        assert!(keys.iter().any(|key| key == "timer:waiting"));
        assert!(keys.iter().any(|key| key == "reminder:waiting"));
        assert!(keys.iter().any(|key| key == "snippet:keep"));

        // A notify that succeeds announces nothing.
        let accepted = |_: &str, _: &str| Ok(());
        let announcements = sweep_due(&store, now + chrono::Duration::minutes(10), &accepted);
        assert!(announcements.is_empty());
        let keys: Vec<String> = store
            .settings()
            .unwrap()
            .into_iter()
            .map(|(key, _)| key)
            .collect();
        assert!(!keys.iter().any(|key| key.contains("waiting")));
    }

    #[test]
    fn splits_a_title_into_matched_and_unmatched_segments() {
        let segments = split_by_ranges("Visual Studio Code", &[0..1, 7..8, 14..15]);
        assert_eq!(segments[0], ("V".to_string(), true));
        assert_eq!(segments[1], ("isual ".to_string(), false));
        assert_eq!(segments[2], ("S".to_string(), true));
    }

    #[test]
    fn an_empty_range_list_leaves_the_title_untouched() {
        let segments = split_by_ranges("Code", &[]);
        assert_eq!(segments, vec![("Code".to_string(), false)]);
    }

    #[test]
    fn splits_multibyte_characters_without_breaking_them() {
        let segments = split_by_ranges("日本語", std::slice::from_ref(&(1..2)));
        assert_eq!(segments.len(), 3);
        assert_eq!(segments[1], ("本".to_string(), true));
    }

    #[test]
    fn a_recorded_key_becomes_a_spec_with_the_its_modifiers() {
        use egui::Key;

        let spec = hotkey_spec_from(
            Key::L,
            egui::Modifiers {
                ctrl: true,
                ..Default::default()
            },
        )
        .expect("ctrl+letter is a legal palette hotkey");
        assert_eq!(spec.virtual_key, 0x4C);
        assert!(spec.modifiers.control);
        assert!(!spec.modifiers.alt);

        // The function key and digit mappings are at their Windows codes.
        let spec = hotkey_spec_from(
            Key::Num1,
            egui::Modifiers {
                shift: true,
                ..Default::default()
            },
        )
        .expect("shift+digit is legal");
        assert_eq!(spec.virtual_key, 0x31);
        assert!(spec.modifiers.shift);
        assert!(!spec.modifiers.control);
    }

    #[test]
    fn a_key_without_a_modifier_is_refused() {
        use egui::Key;

        assert!(hotkey_spec_from(Key::A, egui::Modifiers::default()).is_none());
    }

    #[test]
    fn the_colour_row_says_the_colour_in_its_three_ways() {
        let row = colour_row("#1b2cfd", "rgb(27, 44, 253)", "hsl(231, 98%, 55%)");

        assert_eq!(row.title, "The colour #1b2cfd");
        assert_eq!(row.id.0, "colour:#1b2cfd");
        assert!(row.subtitle.as_deref().unwrap().contains("rgb(27, 44, 253)"));
        assert!(row.subtitle.as_deref().unwrap().contains("hsl(231, 98%, 55%)"));

        let action = row.action.as_ref().expect("the row's act is copying the hex");
        assert_eq!(action.kind, ActionKind::CopyToClipboard);
        let yal_core::action::OpenTarget::Text(text) = &action.target else {
            panic!("the act should carry the hex itself");
        };
        assert_eq!(text, "#1b2cfd");

        assert_eq!(row.secondary_actions.len(), 2);
        assert_eq!(row.secondary_actions[0].kind, ActionKind::CopyToClipboard);
        let yal_core::action::OpenTarget::Text(rgb) = &row.secondary_actions[0].target else {
            panic!("the first secondary should be the rgb saying");
        };
        assert_eq!(rgb, "rgb(27, 44, 253)");
        let yal_core::action::OpenTarget::Text(hsl) = &row.secondary_actions[1].target else {
            panic!("the second secondary should be the hsl saying");
        };
        assert_eq!(hsl, "hsl(231, 98%, 55%)");
    }

    #[test]
    fn theme_preferences_survive_their_setting_names() {
        assert_eq!(Theme::parse("dark"), Some(Theme::Dark));
        assert_eq!(Theme::parse("light"), Some(Theme::Light));
        assert_eq!(Theme::parse("system"), Some(Theme::System));
        assert_eq!(Theme::parse("neon"), None);
        assert_eq!(Theme::Dark.slug(), "dark");
        assert_eq!(Theme::Light.slug(), "light");
        assert_eq!(Theme::System.slug(), "system");
    }

    #[test]
    fn a_digit_hotkey_round_trips_through_its_setting_text() {
        // The hotkey setting is written as text, so a digit combination
        // has to read back out of it.
        let spec = yal_system::hotkey::HotkeySpec::parse("ctrl+5")
            .expect("digits parse as keys");
        assert_eq!(spec.virtual_key, 0x35);
        assert!(spec.modifiers.control);
        assert_eq!(
            yal_system::hotkey::HotkeySpec::parse(&spec.to_text()),
            Some(spec),
            "the text form reads back to the same combination"
        );
    }

    #[test]
    fn image_names_decide_the_preview_panel() {
        assert!(is_image_file_name("C:/pics/photo.jpg"));
        assert!(is_image_file_name("C:/pics/photo.PNG"));
        assert!(is_image_file_name("C:/pics/photo.webp"));
        assert!(!is_image_file_name("C:/pics/notes.txt"));
        assert!(!is_image_file_name("C:/pics/photo"));
        assert!(!is_image_file_name("C:/pics/archive.jpg.bak"));
    }
}
