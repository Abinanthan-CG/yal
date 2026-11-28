//! Background orchestration for Screen OCR, Color Picker, and Clipboard Image OCR.
//!
//! All capture, overlay, and WinRT OCR work runs on dedicated worker threads so
//! the Direct2D palette message loop never blocks. Pending handoffs back to the
//! UI thread pass through [`ToolsBridge`] and custom palette command messages.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use tc_clipboard::{
    color_from_rgb, decode_png_rgba, decode_qr_codes_rgba, format_ocr_lines, format_qr_hud_preview,
    prepare_ocr_rgba, OcrFormatMode, PickerColorFormat,
};
use tc_platform::{Clipboard, Settings};
use tc_platform_win::{
    available_ocr_languages, open_windows_language_settings, recognize_rgba, OcrError, OcrLanguage,
    ScreenSnapshot, WindowsClipboard, WindowsSettings,
};
use tc_ui::{
    CaptureMode, CaptureResult, OverlayColorFormat, OverlayOcrMode, OverlaySnapshot, PaletteHandle,
};

use crate::features::FeatureStores;

/// Settings key for the configured Windows OCR language tag (`"auto"` or BCP-47 tag).
pub const KEY_OCR_LANGUAGE: &str = "ocr.language";
/// Settings key for the default OCR formatting mode (`"smart"` or `"raw"`).
pub const KEY_OCR_MODE: &str = "ocr.mode";
/// Settings key for the default Color Picker copy format (`"hex"`, `"rgb"`, `"hsl"`, `"oklch"`).
pub const KEY_COLOR_PICKER_FORMAT: &str = "colorPicker.defaultFormat";

/// Palette command posted when a background Screen OCR capture opens the OCR Preview screen.
pub const COMMAND_OPEN_OCR_PREVIEW: u32 = 5;
/// Palette command posted when a background Color Picker pick opens the Color Inspector screen.
pub const COMMAND_OPEN_COLOR_INSPECTOR: u32 = 6;

static CAPTURE_OVERLAY_ACTIVE: AtomicBool = AtomicBool::new(false);

/// RAII guard ensuring only one full-desktop capture overlay runs at a time.
struct CaptureOverlayGuard;

impl CaptureOverlayGuard {
    fn try_acquire() -> Option<Self> {
        CAPTURE_OVERLAY_ACTIVE
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| Self)
    }
}

impl Drop for CaptureOverlayGuard {
    fn drop(&mut self) {
        CAPTURE_OVERLAY_ACTIVE.store(false, Ordering::Release);
    }
}

fn lock_or_recover<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn count_lines(text: &str) -> usize {
    if text.is_empty() {
        0
    } else {
        text.lines().count().max(1)
    }
}

/// Active OCR preview payload displayed in the OCR Preview screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OcrPreviewState {
    pub text: String,
    pub raw_text: String,
    pub smart_text: String,
    pub mode: OcrFormatMode,
    pub language_tag: String,
    pub line_count: usize,
    pub char_count: usize,
}

impl OcrPreviewState {
    /// Construct an [`OcrPreviewState`] from pre-formatted Smart and Raw OCR strings.
    pub fn from_texts(
        smart_text: String,
        raw_text: String,
        mode: OcrFormatMode,
        language_tag: String,
    ) -> Self {
        let text = match mode {
            OcrFormatMode::Smart => smart_text.clone(),
            OcrFormatMode::Raw => raw_text.clone(),
        };
        let line_count = count_lines(&text);
        let char_count = text.chars().count();
        Self {
            text,
            raw_text,
            smart_text,
            mode,
            language_tag,
            line_count,
            char_count,
        }
    }

    /// Flip between Smart and Raw OCR formatting modes and refresh derived counts.
    pub fn toggle_mode(&mut self) {
        self.mode = self.mode.toggle();
        self.text = match self.mode {
            OcrFormatMode::Smart => self.smart_text.clone(),
            OcrFormatMode::Raw => self.raw_text.clone(),
        };
        self.line_count = count_lines(&self.text);
        self.char_count = self.text.chars().count();
    }
}

/// Thread-safe handoff bridge carrying pending OCR Preview / Color Inspector payloads
/// from background capture worker threads onto the Direct2D palette message loop.
#[derive(Debug, Default)]
pub struct ToolsBridge {
    pending_ocr_preview: Mutex<Option<OcrPreviewState>>,
    pending_color_hex: Mutex<Option<String>>,
}

impl ToolsBridge {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_pending_ocr_preview(&self, state: OcrPreviewState) {
        *lock_or_recover(&self.pending_ocr_preview) = Some(state);
    }

    pub fn take_pending_ocr_preview(&self) -> Option<OcrPreviewState> {
        lock_or_recover(&self.pending_ocr_preview).take()
    }

    pub fn set_pending_color_hex(&self, hex: String) {
        *lock_or_recover(&self.pending_color_hex) = Some(hex);
    }

    pub fn take_pending_color_hex(&self) -> Option<String> {
        lock_or_recover(&self.pending_color_hex).take()
    }
}

/// Read the configured OCR formatting mode, defaulting to [`OcrFormatMode::Smart`].
pub fn configured_ocr_mode(settings: &impl Settings) -> OcrFormatMode {
    settings
        .get(KEY_OCR_MODE)
        .and_then(|raw| OcrFormatMode::parse(&raw))
        .unwrap_or(OcrFormatMode::Smart)
}

/// Read the configured OCR language tag, defaulting to `"auto"`.
pub fn configured_ocr_language(settings: &impl Settings) -> String {
    settings
        .get(KEY_OCR_LANGUAGE)
        .map(|raw| raw.trim().to_string())
        .filter(|tag| !tag.is_empty())
        .unwrap_or_else(|| "auto".to_string())
}

/// Resolve a human-readable display label for an OCR language setting value.
pub fn ocr_language_display(tag: &str, langs: &[OcrLanguage]) -> String {
    let trimmed = tag.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("auto") {
        return "Auto (System)".to_string();
    }
    langs
        .iter()
        .find(|lang| lang.tag.eq_ignore_ascii_case(trimmed))
        .map(|lang| lang.display_name.clone())
        .unwrap_or_else(|| trimmed.to_string())
}

/// Read the configured default Color Picker copy format, defaulting to [`PickerColorFormat::Hex`].
pub fn configured_color_format(settings: &impl Settings) -> PickerColorFormat {
    settings
        .get(KEY_COLOR_PICKER_FORMAT)
        .and_then(|raw| PickerColorFormat::parse(&raw))
        .unwrap_or(PickerColorFormat::Hex)
}

/// Resolve whether HUD toasts should render in dark mode based on YAL's appearance setting.
pub fn settings_prefers_dark(settings: &impl Settings) -> bool {
    match settings
        .get(tc_palette::KEY_APPEARANCE)
        .as_deref()
        .map(str::trim)
    {
        Some(v) if v.eq_ignore_ascii_case("light") => false,
        Some(v) if v.eq_ignore_ascii_case("dark") => true,
        _ => tc_platform_win::apps_use_dark_theme(),
    }
}

/// Cycle the configured Windows OCR language across `"auto"` and all installed OCR packs.
pub fn cycle_ocr_language(settings: &impl Settings) -> Result<String, &'static str> {
    let langs = available_ocr_languages();
    if langs.is_empty() {
        let _ = open_windows_language_settings();
        return Err("No Windows OCR language packs installed — opening Language Settings");
    }

    let mut cycle: Vec<&str> = Vec::with_capacity(langs.len() + 1);
    cycle.push("auto");
    for lang in &langs {
        cycle.push(lang.tag.as_str());
    }

    let current = configured_ocr_language(settings);
    let next_idx = cycle
        .iter()
        .position(|tag| tag.eq_ignore_ascii_case(&current))
        .map(|idx| (idx + 1) % cycle.len())
        .unwrap_or(0);
    let next_tag = cycle.get(next_idx).copied().unwrap_or("auto");
    let _ = settings.set(KEY_OCR_LANGUAGE, next_tag);
    Ok(ocr_language_display(next_tag, &langs))
}

/// Toggle the configured OCR formatting mode between Smart and Raw and persist it.
pub fn toggle_ocr_mode(settings: &impl Settings) -> OcrFormatMode {
    let next = configured_ocr_mode(settings).toggle();
    let _ = settings.set(KEY_OCR_MODE, next.key());
    next
}

/// Advance the configured Color Picker copy format to the next notation and persist it.
pub fn cycle_color_format(settings: &impl Settings) -> PickerColorFormat {
    let next = configured_color_format(settings).next();
    let _ = settings.set(KEY_COLOR_PICKER_FORMAT, next.key());
    next
}

fn ocr_mode_to_overlay(mode: OcrFormatMode) -> OverlayOcrMode {
    match mode {
        OcrFormatMode::Smart => OverlayOcrMode::Smart,
        OcrFormatMode::Raw => OverlayOcrMode::Raw,
    }
}

fn overlay_to_ocr_mode(mode: OverlayOcrMode) -> OcrFormatMode {
    match mode {
        OverlayOcrMode::Smart => OcrFormatMode::Smart,
        OverlayOcrMode::Raw => OcrFormatMode::Raw,
    }
}

fn color_format_to_overlay(format: PickerColorFormat) -> OverlayColorFormat {
    match format {
        PickerColorFormat::Hex => OverlayColorFormat::Hex,
        PickerColorFormat::Rgb => OverlayColorFormat::Rgb,
        PickerColorFormat::Hsl => OverlayColorFormat::Hsl,
        PickerColorFormat::Oklch => OverlayColorFormat::Oklch,
    }
}

fn overlay_to_color_format(format: OverlayColorFormat) -> PickerColorFormat {
    match format {
        OverlayColorFormat::Hex => PickerColorFormat::Hex,
        OverlayColorFormat::Rgb => PickerColorFormat::Rgb,
        OverlayColorFormat::Hsl => PickerColorFormat::Hsl,
        OverlayColorFormat::Oklch => PickerColorFormat::Oklch,
    }
}

fn snapshot_to_overlay(snapshot: ScreenSnapshot) -> OverlaySnapshot {
    OverlaySnapshot {
        origin_x: snapshot.origin_x,
        origin_y: snapshot.origin_y,
        width: snapshot.width,
        height: snapshot.height,
        bgra: snapshot.bgra,
    }
}

fn overlay_to_snapshot(overlay: OverlaySnapshot) -> ScreenSnapshot {
    ScreenSnapshot {
        origin_x: overlay.origin_x,
        origin_y: overlay.origin_y,
        width: overlay.width,
        height: overlay.height,
        bgra: overlay.bgra,
    }
}

/// Spawn the Screen OCR capture overlay and recognition pipeline off the UI thread.
pub fn launch_screen_ocr(
    handle: PaletteHandle,
    settings: Arc<WindowsSettings>,
    bridge: Arc<ToolsBridge>,
    hide_delay_ms: u64,
) {
    let Some(guard) = CaptureOverlayGuard::try_acquire() else {
        return;
    };
    let _ = std::thread::Builder::new()
        .name("yal-screen-ocr".to_string())
        .spawn(move || {
            let _guard = guard;
            let dark = settings_prefers_dark(settings.as_ref());
            if hide_delay_ms > 0 {
                std::thread::sleep(Duration::from_millis(hide_delay_ms));
            }
            let prev_hwnd = tc_platform_win::window_control::foreground_window();
            let snapshot = match ScreenSnapshot::capture_virtual_desktop() {
                Ok(snap) => snap,
                Err(err) => {
                    tc_ui::show_hud(format!("Screen capture failed: {err}"), dark);
                    return;
                }
            };
            let initial_mode = ocr_mode_to_overlay(configured_ocr_mode(settings.as_ref()));
            let lang_tag = configured_ocr_language(settings.as_ref());
            let language_label = ocr_language_display(&lang_tag, &[]);
            let overlay_snap = snapshot_to_overlay(snapshot);

            match tc_ui::run_capture_overlay(
                &overlay_snap,
                CaptureMode::Ocr {
                    initial_mode,
                    language_label,
                },
                prev_hwnd,
            ) {
                Ok(CaptureResult::OcrRegion {
                    x,
                    y,
                    width,
                    height,
                    mode,
                    open_preview,
                }) => {
                    let snapshot = overlay_to_snapshot(overlay_snap);
                    let Some((rgba, cw, ch)) = snapshot.crop_rgba(x, y, width, height) else {
                        tc_ui::show_hud("Selected region was empty", dark);
                        return;
                    };

                    // Fast pure-Rust QR pass first (works offline with zero OCR language packs).
                    let qr_codes = decode_qr_codes_rgba(&rgba, cw, ch);
                    if !qr_codes.is_empty() {
                        let joined = qr_codes.join("\n");
                        if open_preview {
                            let state = OcrPreviewState::from_texts(
                                joined.clone(),
                                joined,
                                OcrFormatMode::Raw,
                                "QR Code".to_string(),
                            );
                            bridge.set_pending_ocr_preview(state);
                            tc_ui::post_command(handle, COMMAND_OPEN_OCR_PREVIEW);
                        } else {
                            match WindowsClipboard::new().write_text(&joined) {
                                Ok(()) => {
                                    tc_ui::show_hud(format_qr_hud_preview(&qr_codes), dark);
                                }
                                Err(err) => {
                                    tc_ui::show_hud(format!("Clipboard copy failed: {err}"), dark);
                                }
                            }
                        }
                        return;
                    }

                    let Some((prep_rgba, pw, ph)) = prepare_ocr_rgba(&rgba, cw, ch) else {
                        tc_ui::show_hud("Unable to prepare region for OCR", dark);
                        return;
                    };
                    let lang_opt = if lang_tag.eq_ignore_ascii_case("auto") {
                        None
                    } else {
                        Some(lang_tag.as_str())
                    };
                    match recognize_rgba(&prep_rgba, pw, ph, lang_opt) {
                        Err(OcrError::NoLanguagePacks) => {
                            tc_ui::show_hud(
                                "No Windows OCR language pack — opening Language Settings",
                                dark,
                            );
                            let _ = open_windows_language_settings();
                        }
                        Err(err) => {
                            tc_ui::show_hud(format!("OCR failed: {err}"), dark);
                        }
                        Ok(rec) => {
                            let smart_text = format_ocr_lines(&rec.lines, OcrFormatMode::Smart);
                            let raw_text = format_ocr_lines(&rec.lines, OcrFormatMode::Raw);
                            let active_mode = overlay_to_ocr_mode(mode);
                            let state = OcrPreviewState::from_texts(
                                smart_text,
                                raw_text,
                                active_mode,
                                rec.language_tag,
                            );
                            if state.text.trim().is_empty() {
                                tc_ui::show_hud("No text or QR code detected", dark);
                                return;
                            }
                            if open_preview {
                                bridge.set_pending_ocr_preview(state);
                                tc_ui::post_command(handle, COMMAND_OPEN_OCR_PREVIEW);
                            } else {
                                match WindowsClipboard::new().write_text(&state.text) {
                                    Ok(()) => {
                                        let msg = if state.line_count > 1 {
                                            format!(
                                                "Copied {} lines ({})",
                                                state.line_count,
                                                state.mode.label()
                                            )
                                        } else {
                                            format!(
                                                "Copied {} chars ({})",
                                                state.char_count,
                                                state.mode.label()
                                            )
                                        };
                                        tc_ui::show_hud(msg, dark);
                                    }
                                    Err(err) => {
                                        tc_ui::show_hud(
                                            format!("Clipboard copy failed: {err}"),
                                            dark,
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(CaptureResult::Cancelled | CaptureResult::ColorPicked { .. }) => {}
                Err(err) => {
                    tc_ui::show_hud(format!("OCR overlay failed: {err}"), dark);
                }
            }
        });
}

/// Spawn the full-desktop Color Picker loupe overlay off the UI thread.
pub fn launch_color_picker(
    handle: PaletteHandle,
    features: Arc<FeatureStores>,
    settings: Arc<WindowsSettings>,
    bridge: Arc<ToolsBridge>,
    hide_delay_ms: u64,
) {
    let Some(guard) = CaptureOverlayGuard::try_acquire() else {
        return;
    };
    let _ = std::thread::Builder::new()
        .name("yal-color-picker".to_string())
        .spawn(move || {
            let _guard = guard;
            let dark = settings_prefers_dark(settings.as_ref());
            if hide_delay_ms > 0 {
                std::thread::sleep(Duration::from_millis(hide_delay_ms));
            }
            let prev_hwnd = tc_platform_win::window_control::foreground_window();
            let snapshot = match ScreenSnapshot::capture_virtual_desktop() {
                Ok(snap) => snap,
                Err(err) => {
                    tc_ui::show_hud(format!("Screen capture failed: {err}"), dark);
                    return;
                }
            };
            let initial_format =
                color_format_to_overlay(configured_color_format(settings.as_ref()));
            let overlay_snap = snapshot_to_overlay(snapshot);

            match tc_ui::run_capture_overlay(
                &overlay_snap,
                CaptureMode::ColorPicker { initial_format },
                prev_hwnd,
            ) {
                Ok(CaptureResult::ColorPicked {
                    rgb: [r, g, b],
                    format,
                    open_inspector,
                }) => {
                    let entry = features.record_picked_color(r, g, b);
                    let picker_format = overlay_to_color_format(format);
                    let formatted = picker_format.format(color_from_rgb(r, g, b));
                    let copy_res = WindowsClipboard::new().write_text(&formatted);
                    if open_inspector {
                        bridge.set_pending_color_hex(entry.hex);
                        tc_ui::post_command(handle, COMMAND_OPEN_COLOR_INSPECTOR);
                    } else if let Err(err) = copy_res {
                        tc_ui::show_hud(format!("Clipboard copy failed: {err}"), dark);
                    } else {
                        tc_ui::show_hud(format!("Copied {formatted}"), dark);
                    }
                }
                Ok(CaptureResult::Cancelled | CaptureResult::OcrRegion { .. }) => {}
                Err(err) => {
                    tc_ui::show_hud(format!("Color picker failed: {err}"), dark);
                }
            }
        });
}

/// Run Windows OCR on a clipboard image PNG blob on a background worker thread.
pub fn run_clipboard_image_ocr(png_bytes: Vec<u8>, settings: Arc<WindowsSettings>) {
    let _ = std::thread::Builder::new()
        .name("yal-clip-image-ocr".to_string())
        .spawn(move || {
            let dark = settings_prefers_dark(settings.as_ref());
            let Some((width, height, rgba)) = decode_png_rgba(&png_bytes) else {
                tc_ui::show_hud("Unable to decode clipboard image", dark);
                return;
            };
            let qr_codes = decode_qr_codes_rgba(&rgba, width, height);
            if !qr_codes.is_empty() {
                let joined = qr_codes.join("\n");
                match WindowsClipboard::new().write_text(&joined) {
                    Ok(()) => {
                        tc_ui::show_hud(format_qr_hud_preview(&qr_codes), dark);
                    }
                    Err(err) => {
                        tc_ui::show_hud(format!("Clipboard copy failed: {err}"), dark);
                    }
                }
                return;
            }
            let Some((prep_rgba, pw, ph)) = prepare_ocr_rgba(&rgba, width, height) else {
                tc_ui::show_hud("Unable to prepare clipboard image for OCR", dark);
                return;
            };
            let lang_tag = configured_ocr_language(settings.as_ref());
            let lang_opt = if lang_tag.eq_ignore_ascii_case("auto") {
                None
            } else {
                Some(lang_tag.as_str())
            };
            match recognize_rgba(&prep_rgba, pw, ph, lang_opt) {
                Err(OcrError::NoLanguagePacks) => {
                    tc_ui::show_hud(
                        "No Windows OCR language pack — opening Language Settings",
                        dark,
                    );
                    let _ = open_windows_language_settings();
                }
                Err(err) => {
                    tc_ui::show_hud(format!("OCR failed: {err}"), dark);
                }
                Ok(rec) => {
                    let mode = configured_ocr_mode(settings.as_ref());
                    let text = format_ocr_lines(&rec.lines, mode);
                    if text.trim().is_empty() {
                        tc_ui::show_hud("No text or QR code detected in image", dark);
                        return;
                    }
                    match WindowsClipboard::new().write_text(&text) {
                        Ok(()) => {
                            tc_ui::show_hud(
                                format!("Copied {} chars from image", text.chars().count()),
                                dark,
                            );
                        }
                        Err(err) => {
                            tc_ui::show_hud(format!("Clipboard copy failed: {err}"), dark);
                        }
                    }
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use tc_platform::fakes::FakeSettings;

    #[test]
    fn ocr_preview_state_toggle_mode_updates_text_and_counts() {
        let mut state = OcrPreviewState::from_texts(
            "Hello world from OCR.".to_string(),
            "Hello world\nfrom OCR.".to_string(),
            OcrFormatMode::Smart,
            "en-US".to_string(),
        );
        assert_eq!(state.mode, OcrFormatMode::Smart);
        assert_eq!(state.text, "Hello world from OCR.");
        assert_eq!(state.line_count, 1);
        assert_eq!(state.char_count, 21);

        state.toggle_mode();
        assert_eq!(state.mode, OcrFormatMode::Raw);
        assert_eq!(state.text, "Hello world\nfrom OCR.");
        assert_eq!(state.line_count, 2);
        assert_eq!(state.char_count, 21);

        state.toggle_mode();
        assert_eq!(state.mode, OcrFormatMode::Smart);
        assert_eq!(state.text, "Hello world from OCR.");
        assert_eq!(state.line_count, 1);
    }

    #[test]
    fn tools_bridge_sets_and_takes_pending_payloads_once() {
        let bridge = ToolsBridge::new();
        assert!(bridge.take_pending_ocr_preview().is_none());
        assert!(bridge.take_pending_color_hex().is_none());

        let preview = OcrPreviewState::from_texts(
            "Smart".to_string(),
            "Raw".to_string(),
            OcrFormatMode::Smart,
            "en-US".to_string(),
        );
        bridge.set_pending_ocr_preview(preview.clone());
        assert_eq!(bridge.take_pending_ocr_preview(), Some(preview));
        assert!(bridge.take_pending_ocr_preview().is_none());

        bridge.set_pending_color_hex("#3B82F6".to_string());
        assert_eq!(bridge.take_pending_color_hex().as_deref(), Some("#3B82F6"));
        assert!(bridge.take_pending_color_hex().is_none());
    }

    #[test]
    fn configured_and_toggled_ocr_mode_persists_in_settings() {
        let settings = FakeSettings::new();
        assert_eq!(configured_ocr_mode(&settings), OcrFormatMode::Smart);

        let next = toggle_ocr_mode(&settings);
        assert_eq!(next, OcrFormatMode::Raw);
        assert_eq!(configured_ocr_mode(&settings), OcrFormatMode::Raw);
        assert_eq!(settings.get(KEY_OCR_MODE).as_deref(), Some("raw"));

        let back = toggle_ocr_mode(&settings);
        assert_eq!(back, OcrFormatMode::Smart);
        assert_eq!(configured_ocr_mode(&settings), OcrFormatMode::Smart);
    }

    #[test]
    fn configured_and_cycled_color_format_persists_in_settings() {
        let settings = FakeSettings::new();
        assert_eq!(configured_color_format(&settings), PickerColorFormat::Hex);

        assert_eq!(cycle_color_format(&settings), PickerColorFormat::Rgb);
        assert_eq!(
            settings.get(KEY_COLOR_PICKER_FORMAT).as_deref(),
            Some("rgb")
        );
        assert_eq!(cycle_color_format(&settings), PickerColorFormat::Hsl);
        assert_eq!(cycle_color_format(&settings), PickerColorFormat::Oklch);
        assert_eq!(cycle_color_format(&settings), PickerColorFormat::Hex);
    }

    #[test]
    fn ocr_language_display_resolves_auto_known_and_fallback_tags() {
        let langs = vec![
            OcrLanguage {
                tag: "en-US".to_string(),
                display_name: "English (United States)".to_string(),
            },
            OcrLanguage {
                tag: "ja".to_string(),
                display_name: "Japanese".to_string(),
            },
        ];
        assert_eq!(ocr_language_display("auto", &langs), "Auto (System)");
        assert_eq!(ocr_language_display("", &langs), "Auto (System)");
        assert_eq!(ocr_language_display("AUTO", &langs), "Auto (System)");
        assert_eq!(
            ocr_language_display("en-us", &langs),
            "English (United States)"
        );
        assert_eq!(ocr_language_display("ja", &langs), "Japanese");
        assert_eq!(ocr_language_display("fr-FR", &langs), "fr-FR");
    }
}
