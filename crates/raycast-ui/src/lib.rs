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
use yal_engine::Engine;
use yal_search::Scored;
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
}

/// The palette application.
pub struct Palette {
    store: Store,
    engine: Engine,
    query: String,
    results: Vec<Scored>,
    selected: usize,
    applications: Vec<yal_index::Discovered>,
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
    /// The image bytes of the result whose texture is cached.
    cached_image: Option<Vec<u8>>,
    /// The cached texture.
    cached_texture: Option<egui::TextureHandle>,
    /// The last error shown to the user.
    status: Option<String>,
    /// The last timer check, so timers are only read once a second.
    last_timer_check: Option<Instant>,
    /// A timer that rang while the palette was away.
    ring: Option<String>,
}

/// The identifier the launcher hotkey is registered under.
const HOTKEY_ID: i32 = 0x5241;

impl Palette {
    /// Build a palette over a store.
    pub fn new(store: Store) -> Self {
        let (sender, receiver) = channel();

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
            cached_image: None,
            cached_texture: None,
            status: None,
            last_timer_check: None,
            ring: None,
        };
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
    fn selected_image(&self) -> Option<Vec<u8>> {
        let result = self.results.get(self.selected)?;
        let payload = result.item.payload.as_ref()?;
        let encoded = payload.get("png_base64")?.as_str()?;
        decode_base64(encoded)
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
    fn show(&mut self, ctx: &egui::Context) {
        self.visible = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        self.query.clear();
        self.ring = None;
        self.run_search();
        self.needs_focus = true;
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

        if let Err(error) = self.perform(&action) {
            self.status = Some(error.to_string());
            return;
        }

        // Remember the choice so frecency can prefer it next time.
        if let Some(result) = self.results.get(self.selected) {
            let key = result.item.id.0.clone();
            let _ = self.store.record_use(&key, chrono::Utc::now());
        }

        // The palette gets out of the way once a result is chosen.
        self.visible = false;
        self.hide_requested = true;
    }
}

impl Palette {
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
            (ActionKind::CopyToClipboard, OpenTarget::Text(text))
            | (ActionKind::PasteToClipboard, OpenTarget::Text(text)) => {
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
                // Detached: the palette does not wait and does not keep a
                // console for the child.
                std::process::Command::new(program)
                    .args(args)
                    .spawn()
                    .map(|_| ())
                    .map_err(|err| format!("{program:?} could not be started: {err}"))
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

    /// Check the stored timers once a second.
    ///
    /// A timer that has run out is announced with a toast when the system
    /// allows it, and always removed. When the toast is refused — an
    /// unpackaged program may be without a registered user model — the
    /// palette raises itself with a banner instead, which cannot fail.
    fn check_timers(&mut self, ctx: &egui::Context) {
        const CHECK_INTERVAL: std::time::Duration = std::time::Duration::from_secs(1);
        if let Some(last) = self.last_timer_check {
            if last.elapsed() < CHECK_INTERVAL {
                return;
            }
        }
        self.last_timer_check = Some(Instant::now());

        let Ok(settings) = self.store.settings() else {
            return;
        };
        let now = chrono::Utc::now();
        for (key, value) in settings {
            if !key.starts_with("timer:") || value.is_empty() {
                continue;
            }
            let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&value) else {
                continue;
            };
            let Some(label) = parsed.get("label").and_then(|label| label.as_str()) else {
                continue;
            };
            let Some(ends_at) = parsed
                .get("ends_at")
                .and_then(|ends| ends.as_i64())
                .and_then(|timestamp| chrono::DateTime::<chrono::Utc>::from_timestamp(timestamp, 0))
            else {
                continue;
            };
            if ends_at > now {
                continue;
            }

            let _ = self.store.clear_setting(&key);
            let announcement = format!("{label}: the timer has finished");
            match yal_system::notify::toast(label, "The timer has finished.") {
                Ok(()) => {}
                Err(_) => {
                    self.ring = Some(announcement);
                    self.visible = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.request_repaint();
                }
            }
        }
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
        // The global hotkey toggles the palette. It is polled before
        // anything else so that the toggle is never lost between frames.
        if self.hotkey.poll() {
            self.visible = !self.visible;
            if self.visible {
                self.show(ctx);
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            }
        }

        // Timers are checked every second, even while the palette is away,
        // because a finished one raises a banner here.
        self.check_timers(ctx);

        // Deliver messages sent from other threads.
        while let Ok(message) = self.receiver.try_recv() {
            match message {
                PaletteMessage::Close => self.close_requested = true,
                PaletteMessage::Show => {
                    self.show(ctx);
                }
            }
        }

        let palette_style = PaletteStyle::new(ctx.style().visuals.dark_mode);

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
        });

        if dismiss {
            // Escape hides the palette rather than closing it, so that the
            // hotkey can bring it back.
            self.visible = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }

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

        let frame = egui::Frame::new()
            .fill(palette_style.background)
            .inner_margin(egui::Margin::same(10))
            .stroke(Stroke::new(1.0_f32, palette_style.border))
            .corner_radius(8.0);

        // A result that carries an image gets a panel of its own. Drawing it
        // below the list would put it off the screen, because the list takes
        // all the space that is left.
        if let Some(bytes) = self.selected_image() {
            if let Some(texture) = self.image_texture(ctx, &bytes) {
                let size = texture.size_vec2();
                let scale = 170.0 / size.x.max(1.0);
                egui::TopBottomPanel::bottom("detail").show(ctx, |ui| {
                    ui.with_layout(Layout::top_down(Align::Center), |ui| {
                        ui.add(egui::Image::new(&texture).fit_to_exact_size(size * scale));
                    });
                });
            }
        }

        egui::CentralPanel::default()
            .frame(frame)
            .show(ctx, |ui| {
                // The query field.
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .hint_text("Search applications, snippets, and more…")
                        .font(FontId::proportional(22.0))
                        .desired_width(f32::INFINITY),
                );
                if response.changed() {
                    self.pending = true;
                    self.last_search = Some(Instant::now());
                    self.selected = 0;
                }
                response.request_focus();

                ui.add_space(6.0);
                ui.separator();
                ui.add_space(4.0);

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
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }

        self.take_focus(ctx);

        // A hidden window does not produce events, so the event loop would
        // sleep and never notice the hotkey. Asking for a repaint on an
        // interval keeps the loop ticking while the palette is away.
        if !self.visible {
            ctx.request_repaint_after(std::time::Duration::from_millis(30));
        }

        if self.close_requested {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
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
        "launcher",
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
}
