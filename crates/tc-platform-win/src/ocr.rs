//! Native Windows WinRT OCR (`Windows.Media.Ocr.OcrEngine`) wrapper.
//!
//! All WinRT calls execute inside a freshly spawned worker thread initialized with
//! `RoInitialize(RO_INIT_MULTITHREADED)` and an RAII drop guard that calls
//! `RoUninitialize()` strictly after every WinRT reference has been released, so
//! callers on STA UI threads never fail with `RPC_E_CHANGED_MODE`.

use windows::core::HSTRING;
use windows::Globalization::Language;
use windows::Graphics::Imaging::{BitmapAlphaMode, BitmapPixelFormat, SoftwareBitmap};
use windows::Media::Ocr::OcrEngine;
use windows::Storage::Streams::DataWriter;
use windows::Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED};

/// An installed Windows OCR recognizer language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OcrLanguage {
    pub tag: String,
    pub display_name: String,
}

/// Recognized text lines and the resolved BCP-47 language tag used by Windows OCR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OcrRecognition {
    pub lines: Vec<String>,
    pub language_tag: String,
}

/// Errors returned by native Windows OCR operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OcrError {
    #[error("no Windows OCR language packs are installed")]
    NoLanguagePacks,
    #[error("Windows OCR language `{0}` is not installed")]
    LanguageNotAvailable(String),
    #[error("cannot run OCR on an empty or invalid image buffer")]
    EmptyImage,
    #[error("failed to initialize Windows OCR engine: {0}")]
    EngineInit(String),
    #[error("Windows OCR recognition failed: {0}")]
    RecognitionFailed(String),
}

fn ensure_process_mta() {
    use std::sync::Once;
    use windows::Win32::System::Com::CoIncrementMTAUsage;

    static MTA_KEEPALIVE: Once = Once::new();
    MTA_KEEPALIVE.call_once(|| {
        // SAFETY: Keeps the process-wide multithreaded apartment alive so `windows` crate's
        // static `FactoryCache` pointers remain valid across short-lived MTA worker threads.
        let _ = unsafe { CoIncrementMTAUsage() };
    });
}

struct RoInitGuard {
    initialized: bool,
}

impl RoInitGuard {
    fn new() -> Self {
        ensure_process_mta();
        // SAFETY: Initializes the Windows Runtime multithreaded apartment on a freshly
        // spawned worker thread before any WinRT objects are constructed.
        let initialized = unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.is_ok();
        Self { initialized }
    }
}

impl Drop for RoInitGuard {
    fn drop(&mut self) {
        if self.initialized {
            // SAFETY: Paired with `RoInitialize` in `RoInitGuard::new`, and declared before
            // the inner scope holding all WinRT objects so uninitialization happens last.
            unsafe { RoUninitialize() };
        }
    }
}

/// Enumerate all OCR recognizer languages currently installed on Windows.
pub fn available_ocr_languages() -> Vec<OcrLanguage> {
    let Ok(handle) = std::thread::Builder::new()
        .name("yal-winrt-ocr-langs".to_string())
        .spawn(|| {
            let _ro_guard = RoInitGuard::new();
            {
                let Ok(view) = OcrEngine::AvailableRecognizerLanguages() else {
                    return Vec::new();
                };
                let mut out = Vec::new();
                for lang in view {
                    let Ok(tag) = lang.LanguageTag() else {
                        continue;
                    };
                    let display_name = lang
                        .DisplayName()
                        .map(|name| name.to_string())
                        .unwrap_or_else(|_| tag.to_string());
                    out.push(OcrLanguage {
                        tag: tag.to_string(),
                        display_name,
                    });
                }
                out
            }
        })
    else {
        return Vec::new();
    };

    handle.join().unwrap_or_default()
}

/// Recognize text inside a top-down 32-bit RGBA (`[R, G, B, A]`) pixel buffer.
pub fn recognize_rgba(
    rgba: &[u8],
    width: u32,
    height: u32,
    language_tag: Option<&str>,
) -> Result<OcrRecognition, OcrError> {
    if width == 0 || height == 0 {
        return Err(OcrError::EmptyImage);
    }
    let expected_len = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or(OcrError::EmptyImage)?;
    if rgba.len() != expected_len {
        return Err(OcrError::EmptyImage);
    }
    let bmp_width = i32::try_from(width).map_err(|_| {
        OcrError::RecognitionFailed(format!("image width {width} exceeds i32::MAX"))
    })?;
    let bmp_height = i32::try_from(height).map_err(|_| {
        OcrError::RecognitionFailed(format!("image height {height} exceeds i32::MAX"))
    })?;

    let rgba_owned = rgba.to_vec();
    let lang_owned = language_tag.map(str::to_owned);

    let handle = std::thread::Builder::new()
        .name("yal-winrt-ocr".to_string())
        .spawn(move || -> Result<OcrRecognition, OcrError> {
            let _ro_guard = RoInitGuard::new();
            recognize_in_mta(
                &rgba_owned,
                width,
                height,
                bmp_width,
                bmp_height,
                lang_owned.as_deref(),
            )
        })
        .map_err(|e| OcrError::EngineInit(format!("failed to spawn OCR worker thread: {e}")))?;

    handle.join().map_err(|_| {
        OcrError::RecognitionFailed("WinRT OCR worker thread panicked unexpectedly".to_string())
    })?
}

/// Open Windows Settings -> Time & language -> Language & region.
pub fn open_windows_language_settings() -> Result<(), String> {
    crate::launch::open("ms-settings:regionlanguage")
}

fn recognize_in_mta(
    rgba: &[u8],
    width: u32,
    height: u32,
    bmp_width: i32,
    bmp_height: i32,
    language_tag: Option<&str>,
) -> Result<OcrRecognition, OcrError> {
    let max_dim = OcrEngine::MaxImageDimension()
        .map_err(|e| OcrError::EngineInit(format!("MaxImageDimension failed: {e}")))?;
    if width > max_dim || height > max_dim {
        return Err(OcrError::RecognitionFailed(format!(
            "image dimensions ({width}x{height}) exceed Windows OCR maximum ({max_dim}px)"
        )));
    }

    let engine = create_ocr_engine(language_tag)?;
    let resolved_tag = engine
        .RecognizerLanguage()
        .and_then(|lang| lang.LanguageTag())
        .map(|tag| tag.to_string())
        .unwrap_or_else(|_| {
            language_tag
                .map(str::trim)
                .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("auto"))
                .unwrap_or("und")
                .to_string()
        });

    let writer = DataWriter::new()
        .map_err(|e| OcrError::RecognitionFailed(format!("DataWriter::new failed: {e}")))?;
    writer
        .WriteBytes(rgba)
        .map_err(|e| OcrError::RecognitionFailed(format!("DataWriter::WriteBytes failed: {e}")))?;
    let buffer = writer.DetachBuffer().map_err(|e| {
        OcrError::RecognitionFailed(format!("DataWriter::DetachBuffer failed: {e}"))
    })?;

    let bitmap = SoftwareBitmap::CreateCopyWithAlphaFromBuffer(
        &buffer,
        BitmapPixelFormat::Rgba8,
        bmp_width,
        bmp_height,
        BitmapAlphaMode::Premultiplied,
    )
    .map_err(|e| {
        OcrError::RecognitionFailed(format!(
            "SoftwareBitmap::CreateCopyWithAlphaFromBuffer failed: {e}"
        ))
    })?;

    let async_op = engine
        .RecognizeAsync(&bitmap)
        .map_err(|e| OcrError::RecognitionFailed(format!("RecognizeAsync failed: {e}")))?;
    let result = async_op
        .join()
        .map_err(|e| OcrError::RecognitionFailed(format!("RecognizeAsync::join failed: {e}")))?;

    let ocr_lines = result
        .Lines()
        .map_err(|e| OcrError::RecognitionFailed(format!("OcrResult::Lines failed: {e}")))?;

    let mut lines = Vec::new();
    for line in ocr_lines {
        let Ok(words) = line.Words() else {
            continue;
        };
        let mut word_strings = Vec::new();
        for word in words {
            if let Ok(text) = word.Text() {
                let s = text.to_string();
                if !s.is_empty() {
                    word_strings.push(s);
                }
            }
        }
        let joined = join_ocr_words(&word_strings);
        if !joined.is_empty() {
            lines.push(joined);
        }
    }

    Ok(OcrRecognition {
        lines,
        language_tag: resolved_tag,
    })
}

fn create_ocr_engine(language_tag: Option<&str>) -> Result<OcrEngine, OcrError> {
    if let Some(trimmed) = language_tag
        .map(str::trim)
        .filter(|tag| !tag.is_empty() && !tag.eq_ignore_ascii_case("auto"))
    {
        let lang = Language::CreateLanguage(&HSTRING::from(trimmed))
            .map_err(|_| OcrError::LanguageNotAvailable(trimmed.to_string()))?;
        let supported = OcrEngine::IsLanguageSupported(&lang).unwrap_or(false);
        if !supported {
            return Err(OcrError::LanguageNotAvailable(trimmed.to_string()));
        }
        return OcrEngine::TryCreateFromLanguage(&lang)
            .map_err(|e| OcrError::EngineInit(e.to_string()));
    }

    if let Ok(engine) = OcrEngine::TryCreateFromUserProfileLanguages() {
        return Ok(engine);
    }

    let languages = OcrEngine::AvailableRecognizerLanguages()
        .map_err(|e| OcrError::EngineInit(e.to_string()))?;
    let Some(first_lang) = languages.into_iter().next() else {
        return Err(OcrError::NoLanguagePacks);
    };
    OcrEngine::TryCreateFromLanguage(&first_lang).map_err(|e| OcrError::EngineInit(e.to_string()))
}

fn join_ocr_words(words: &[String]) -> String {
    let mut out = String::new();
    for word in words {
        if word.is_empty() {
            continue;
        }
        if let (Some(prev_last), Some(next_first)) = (out.chars().last(), word.chars().next()) {
            if !(is_cjk_scalar(prev_last) && is_cjk_scalar(next_first)) {
                out.push(' ');
            }
        }
        out.push_str(word);
    }
    out
}

fn is_cjk_scalar(ch: char) -> bool {
    matches!(
        ch as u32,
        0x1100..=0x11FF
            | 0x2E80..=0x2EFF
            | 0x2F00..=0x2FDF
            | 0x3000..=0x303F
            | 0x3040..=0x309F
            | 0x30A0..=0x30FF
            | 0x3100..=0x312F
            | 0x3130..=0x318F
            | 0x3190..=0x319F
            | 0x31A0..=0x31BF
            | 0x31C0..=0x31EF
            | 0x31F0..=0x31FF
            | 0x3200..=0x32FF
            | 0x3300..=0x33FF
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xA960..=0xA97F
            | 0xAC00..=0xD7AF
            | 0xD7B0..=0xD7FF
            | 0xF900..=0xFAFF
            | 0xFE30..=0xFE4F
            | 0xFF00..=0xFFEF
            | 0x20000..=0x2A6DF
            | 0x2A700..=0x2B73F
            | 0x2B740..=0x2B81F
            | 0x2B820..=0x2CEAF
            | 0x2CEB0..=0x2EBEF
            | 0x30000..=0x3134F
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::c_void;
    use windows::core::w;
    use windows::Win32::Foundation::{COLORREF, RECT};
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleDC, CreateDIBSection, CreateFontW, DeleteDC, DeleteObject, DrawTextW,
        GdiFlush, SelectObject, SetBkColor, SetBkMode, SetTextColor, ANTIALIASED_QUALITY,
        BITMAPINFO, BITMAPINFOHEADER, BI_RGB, CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET, DIB_RGB_COLORS,
        DT_CENTER, DT_SINGLELINE, DT_VCENTER, HBITMAP, HDC, HFONT, HGDIOBJ, OPAQUE, OUT_TT_PRECIS,
    };
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};

    struct TestDcGuard(HDC);
    impl Drop for TestDcGuard {
        fn drop(&mut self) {
            if !self.0.is_invalid() {
                // SAFETY: Created by `CreateCompatibleDC` in this test helper.
                unsafe {
                    let _ = DeleteDC(self.0);
                }
            }
        }
    }

    struct TestBitmapGuard(HBITMAP);
    impl Drop for TestBitmapGuard {
        fn drop(&mut self) {
            if !self.0.is_invalid() {
                // SAFETY: Created by `CreateDIBSection` and deselected before drop.
                unsafe {
                    let _ = DeleteObject(HGDIOBJ(self.0 .0));
                }
            }
        }
    }

    struct TestFontGuard(HFONT);
    impl Drop for TestFontGuard {
        fn drop(&mut self) {
            if !self.0.is_invalid() {
                // SAFETY: Created by `CreateFontW` and deselected before drop.
                unsafe {
                    let _ = DeleteObject(HGDIOBJ(self.0 .0));
                }
            }
        }
    }

    struct TestSelectGuard {
        hdc: HDC,
        previous: HGDIOBJ,
    }
    impl Drop for TestSelectGuard {
        fn drop(&mut self) {
            if !self.hdc.is_invalid() && !self.previous.is_invalid() {
                // SAFETY: Restores previous GDI object into `self.hdc` before deletion.
                unsafe {
                    let _ = SelectObject(self.hdc, self.previous);
                }
            }
        }
    }

    struct StaComGuard(bool);
    impl Drop for StaComGuard {
        fn drop(&mut self) {
            if self.0 {
                // SAFETY: Balances `CoInitializeEx(COINIT_APARTMENTTHREADED)` on this test thread.
                unsafe { CoUninitialize() };
            }
        }
    }

    fn render_text_rgba(text: &str, width: u32, height: u32) -> Vec<u8> {
        // SAFETY: Passing `None` creates a memory DC compatible with the current screen.
        let raw_dc = unsafe { CreateCompatibleDC(None) };
        assert!(!raw_dc.is_invalid(), "CreateCompatibleDC failed");
        let mem_dc = TestDcGuard(raw_dc);

        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                biHeight: -(height as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut bits_ptr: *mut c_void = std::ptr::null_mut();
        // SAFETY: `mem_dc.0` and `bmi` are valid; `bits_ptr` receives the DIB pixel buffer.
        let raw_hbmp = unsafe {
            CreateDIBSection(Some(mem_dc.0), &bmi, DIB_RGB_COLORS, &mut bits_ptr, None, 0)
        }
        .expect("CreateDIBSection should succeed");
        assert!(!raw_hbmp.is_invalid() && !bits_ptr.is_null());
        let dib = TestBitmapGuard(raw_hbmp);

        let byte_len = (width as usize) * (height as usize) * 4;
        // SAFETY: Fill top-down 32-bit DIB with opaque white (`0xFF` in all channels).
        unsafe {
            std::slice::from_raw_parts_mut(bits_ptr.cast::<u8>(), byte_len).fill(0xFF);
        }

        // SAFETY: `w!("Segoe UI")` is a static null-terminated UTF-16 face name.
        let raw_font = unsafe {
            CreateFontW(
                -56,
                0,
                0,
                0,
                700,
                0,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_TT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                ANTIALIASED_QUALITY,
                0,
                w!("Segoe UI"),
            )
        };
        assert!(!raw_font.is_invalid(), "CreateFontW failed");
        let font = TestFontGuard(raw_font);

        {
            // SAFETY: Both `mem_dc.0` and `dib.0` are valid handles.
            let prev_bmp = unsafe { SelectObject(mem_dc.0, HGDIOBJ(dib.0 .0)) };
            assert!(!prev_bmp.is_invalid());
            let _bmp_sel = TestSelectGuard {
                hdc: mem_dc.0,
                previous: prev_bmp,
            };

            // SAFETY: Both `mem_dc.0` and `font.0` are valid handles.
            let prev_font = unsafe { SelectObject(mem_dc.0, HGDIOBJ(font.0 .0)) };
            assert!(!prev_font.is_invalid());
            let _font_sel = TestSelectGuard {
                hdc: mem_dc.0,
                previous: prev_font,
            };

            // SAFETY: `mem_dc.0` is a valid memory DC with the DIB and font selected.
            unsafe {
                SetBkMode(mem_dc.0, OPAQUE);
                SetBkColor(mem_dc.0, COLORREF(0x00FF_FFFF));
                SetTextColor(mem_dc.0, COLORREF(0x0000_0000));
            }

            let mut wide: Vec<u16> = text.encode_utf16().collect();
            let mut rect = RECT {
                left: 0,
                top: 0,
                right: width as i32,
                bottom: height as i32,
            };
            // SAFETY: `wide` and `rect` are valid mutable buffers for the selected DIB DC.
            let drawn = unsafe {
                DrawTextW(
                    mem_dc.0,
                    &mut wide,
                    &mut rect,
                    DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                )
            };
            assert!(drawn > 0, "DrawTextW should draw text");
            // SAFETY: Flush GDI drawing commands before reading DIB pixels on CPU.
            unsafe {
                let _ = GdiFlush();
            }
        }

        // SAFETY: `dib` is still alive and deselected, containing `byte_len` BGRA bytes.
        let bgra = unsafe { std::slice::from_raw_parts(bits_ptr.cast::<u8>(), byte_len) };
        let mut rgba = Vec::with_capacity(byte_len);
        for px in bgra.as_chunks::<4>().0 {
            rgba.extend_from_slice(&[px[2], px[1], px[0], 0xFF]);
        }
        rgba
    }

    #[test]
    fn available_ocr_languages_runs_without_panicking() {
        let _langs = available_ocr_languages();
    }

    #[test]
    fn recognize_rgba_rejects_empty_or_mismatched_buffers() {
        assert_eq!(recognize_rgba(&[], 0, 10, None), Err(OcrError::EmptyImage));
        assert_eq!(recognize_rgba(&[], 10, 0, None), Err(OcrError::EmptyImage));
        assert_eq!(
            recognize_rgba(&[255u8; 12], 2, 2, None),
            Err(OcrError::EmptyImage)
        );
    }

    #[test]
    fn join_ocr_words_handles_latin_and_cjk_spacing() {
        assert_eq!(
            join_ocr_words(&["YAL".to_string(), "OCR".to_string(), "2026".to_string()]),
            "YAL OCR 2026"
        );
        assert_eq!(
            join_ocr_words(&["東京".to_string(), "タワー".to_string()]),
            "東京タワー"
        );
        assert_eq!(
            join_ocr_words(&["YAL".to_string(), "日本語".to_string(), "2026".to_string()]),
            "YAL 日本語 2026"
        );
    }

    #[test]
    fn live_winrt_ocr_recognizes_rendered_text_even_from_sta_thread() {
        let langs = available_ocr_languages();
        if langs.is_empty() {
            return;
        }

        let (w, h) = (640u32, 160u32);
        let rgba = render_text_rgba("YAL OCR 2026", w, h);

        let rec = recognize_rgba(&rgba, w, h, None)
            .expect("WinRT OCR should recognize crisp GDI-rendered text");
        let joined = rec.lines.join(" ");
        eprintln!(
            "WinRT OCR recognized ({}): {:?}",
            rec.language_tag, rec.lines
        );
        assert!(
            joined.contains("YAL") && joined.contains("OCR") && joined.contains("2026"),
            "unexpected OCR output ({}): `{joined}`",
            rec.language_tag
        );

        // Verify STA thread callers never hit RPC_E_CHANGED_MODE.
        let sta_handle = std::thread::spawn(move || {
            // SAFETY: Initializes COM as STA on a dedicated test thread.
            let sta_ok = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok();
            let _sta_guard = StaComGuard(sta_ok);
            recognize_rgba(&rgba, w, h, None)
        });
        let sta_rec = sta_handle
            .join()
            .expect("STA test thread should not panic")
            .expect("WinRT OCR must succeed when invoked from an STA thread");
        let sta_joined = sta_rec.lines.join(" ");
        assert!(
            sta_joined.contains("YAL") && sta_joined.contains("OCR") && sta_joined.contains("2026"),
            "unexpected STA OCR output ({}): `{sta_joined}`",
            sta_rec.language_tag
        );
    }
}
