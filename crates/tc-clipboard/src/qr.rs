//! Pure-Rust QR code decoding from raw RGBA buffers.
//!
//! Designed to run inside Screen OCR (`Ctrl+Alt+T`) and Clipboard Image OCR
//! before falling back to WinRT text recognition. Handles tightly cropped
//! screen regions (by adding a white quiet zone), dark-mode inverted QR codes
//! (light modules on dark background), and tiny low-resolution QR captures.

use std::collections::HashSet;
use std::panic::{catch_unwind, AssertUnwindSafe};

/// Maximum dimension (width or height) fed into `rqrr`; larger crops are
/// proportionally downscaled first to keep decode latency fast and bounded.
const MAX_QR_SCAN_DIM: u32 = 2400;

/// Quiet-zone margin (in pixels) added around each scan pass so tightly cropped
/// QR codes without surrounding whitespace still decode cleanly.
const QUIET_ZONE_PX: usize = 16;

/// Crops below this width or height get an extra 2× upscaled fallback pass if
/// the native-resolution passes find no QR codes.
const SMALL_QR_THRESHOLD_PX: u32 = 260;

/// Decode every valid QR code present in an 8-bit RGBA image buffer.
///
/// Returns deduplicated, non-empty decoded UTF-8 payloads in detection order.
/// Returns an empty `Vec` on invalid dimensions, non-QR images, or decoder
/// errors without ever panicking.
pub fn decode_qr_codes_rgba(rgba: &[u8], width: u32, height: u32) -> Vec<String> {
    if width == 0 || height == 0 {
        return Vec::new();
    }
    let expected = (width as usize)
        .checked_mul(height as usize)
        .and_then(|px| px.checked_mul(4));
    if expected != Some(rgba.len()) {
        return Vec::new();
    }

    let (luma, lw, lh) = rgba_to_bounded_luma(rgba, width, height);

    // Pass 1: standard dark-on-light polarity with white quiet-zone padding.
    let found = scan_luma_pass(&luma, lw, lh, QUIET_ZONE_PX, false);
    if !found.is_empty() {
        return found;
    }

    // Pass 2: dark-mode inverted QR codes (light modules on dark surface).
    let found = scan_luma_pass(&luma, lw, lh, QUIET_ZONE_PX, true);
    if !found.is_empty() {
        return found;
    }

    // Pass 3: tiny QR code fallback — upscale 2× and retry both polarities.
    if lw < SMALL_QR_THRESHOLD_PX as usize || lh < SMALL_QR_THRESHOLD_PX as usize {
        let (up, uw, uh) = upscale_luma_2x(&luma, lw, lh);
        let found = scan_luma_pass(&up, uw, uh, QUIET_ZONE_PX * 2, false);
        if !found.is_empty() {
            return found;
        }
        let found = scan_luma_pass(&up, uw, uh, QUIET_ZONE_PX * 2, true);
        if !found.is_empty() {
            return found;
        }
    }

    Vec::new()
}

/// Format a crisp single-line HUD notice summarizing decoded QR code(s).
pub fn format_qr_hud_preview(codes: &[String]) -> String {
    match codes.len() {
        0 => "No QR code detected".to_string(),
        1 => {
            let single_line: String = codes[0].split_whitespace().collect::<Vec<_>>().join(" ");
            let mut preview = String::new();
            let mut chars = single_line.chars();
            for ch in chars.by_ref().take(54) {
                preview.push(ch);
            }
            if chars.next().is_some() {
                preview.push('…');
            }
            format!("QR copied: {preview}")
        }
        n => format!("Copied {n} QR codes"),
    }
}

fn rgba_to_bounded_luma(rgba: &[u8], width: u32, height: u32) -> (Vec<u8>, usize, usize) {
    let max_dim = width.max(height);
    if max_dim <= MAX_QR_SCAN_DIM {
        let w = width as usize;
        let h = height as usize;
        let mut luma = Vec::with_capacity(w * h);
        for px in rgba.as_chunks::<4>().0 {
            let y =
                ((77u32 * u32::from(px[0]) + 150u32 * u32::from(px[1]) + 29u32 * u32::from(px[2]))
                    >> 8) as u8;
            luma.push(y);
        }
        return (luma, w, h);
    }

    let scale = f64::from(MAX_QR_SCAN_DIM) / f64::from(max_dim);
    let dst_w = ((f64::from(width) * scale).round() as u32).clamp(1, MAX_QR_SCAN_DIM) as usize;
    let dst_h = ((f64::from(height) * scale).round() as u32).clamp(1, MAX_QR_SCAN_DIM) as usize;
    let src_w = width as usize;
    let src_h = height as usize;
    let mut luma = Vec::with_capacity(dst_w * dst_h);

    for dy in 0..dst_h {
        let sy = ((dy * src_h) / dst_h).min(src_h - 1);
        for dx in 0..dst_w {
            let sx = ((dx * src_w) / dst_w).min(src_w - 1);
            let idx = (sy * src_w + sx) * 4;
            let y = ((77u32 * u32::from(rgba[idx])
                + 150u32 * u32::from(rgba[idx + 1])
                + 29u32 * u32::from(rgba[idx + 2]))
                >> 8) as u8;
            luma.push(y);
        }
    }
    (luma, dst_w, dst_h)
}

fn upscale_luma_2x(luma: &[u8], w: usize, h: usize) -> (Vec<u8>, usize, usize) {
    let out_w = w * 2;
    let out_h = h * 2;
    let mut out = Vec::with_capacity(out_w * out_h);
    for y in 0..out_h {
        let sy = y / 2;
        let row_start = sy * w;
        for x in 0..out_w {
            out.push(luma[row_start + (x / 2)]);
        }
    }
    (out, out_w, out_h)
}

fn scan_luma_pass(luma: &[u8], w: usize, h: usize, pad: usize, invert: bool) -> Vec<String> {
    let total_w = match w.checked_add(pad.saturating_mul(2)) {
        Some(v) if v > 0 => v,
        _ => return Vec::new(),
    };
    let total_h = match h.checked_add(pad.saturating_mul(2)) {
        Some(v) if v > 0 => v,
        _ => return Vec::new(),
    };
    if total_w.checked_mul(total_h).is_none() {
        return Vec::new();
    }

    let result = catch_unwind(AssertUnwindSafe(|| {
        let mut prepared = rqrr::PreparedImage::prepare_from_greyscale(total_w, total_h, |x, y| {
            if x < pad || y < pad || x >= pad + w || y >= pad + h {
                // Always pad with a bright white quiet zone around the QR modules.
                255u8
            } else {
                let raw = luma[(y - pad) * w + (x - pad)];
                if invert {
                    255u8.saturating_sub(raw)
                } else {
                    raw
                }
            }
        });

        let grids = prepared.detect_grids();
        let mut seen = HashSet::new();
        let mut decoded = Vec::new();
        for grid in grids {
            if let Ok((_meta, content)) = grid.decode() {
                let trimmed = content.trim();
                if !trimmed.is_empty() && seen.insert(trimmed.to_string()) {
                    decoded.push(trimmed.to_string());
                }
            }
        }
        decoded
    }));

    result.unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use qrcode::{Color as QrColor, QrCode};

    fn render_qr_rgba(
        payload: &str,
        scale: usize,
        margin: usize,
        fg: [u8; 4],
        bg: [u8; 4],
    ) -> (Vec<u8>, u32, u32) {
        let code = QrCode::new(payload.as_bytes()).expect("valid qr payload");
        let modules = code.width();
        let size = (modules + margin * 2) * scale;
        let mut rgba = vec![0u8; size * size * 4];
        for y in 0..size {
            for x in 0..size {
                let mx = x / scale;
                let my = y / scale;
                let is_dark = if mx >= margin
                    && my >= margin
                    && mx < margin + modules
                    && my < margin + modules
                {
                    code[(mx - margin, my - margin)] == QrColor::Dark
                } else {
                    false
                };
                let color = if is_dark { fg } else { bg };
                let idx = (y * size + x) * 4;
                rgba[idx..idx + 4].copy_from_slice(&color);
            }
        }
        (rgba, size as u32, size as u32)
    }

    #[test]
    fn decodes_standard_qr_url() {
        let (rgba, w, h) = render_qr_rgba(
            "https://github.com/Abinanthan-CG/yal",
            4,
            4,
            [0, 0, 0, 255],
            [255, 255, 255, 255],
        );
        let decoded = decode_qr_codes_rgba(&rgba, w, h);
        assert_eq!(decoded, vec!["https://github.com/Abinanthan-CG/yal"]);
    }

    #[test]
    fn decodes_tight_zero_margin_crop_via_quiet_zone_padding() {
        let (rgba, w, h) = render_qr_rgba(
            "YAL-TIGHT-CROP-2026",
            4,
            0,
            [18, 18, 18, 255],
            [248, 248, 248, 255],
        );
        let decoded = decode_qr_codes_rgba(&rgba, w, h);
        assert_eq!(decoded, vec!["YAL-TIGHT-CROP-2026"]);
    }

    #[test]
    fn decodes_dark_mode_inverted_qr_code() {
        let (rgba, w, h) = render_qr_rgba(
            "wifi:T:WPA;S:StudioNet;P:secret123;;",
            4,
            2,
            [245, 245, 245, 255],
            [20, 22, 26, 255],
        );
        let decoded = decode_qr_codes_rgba(&rgba, w, h);
        assert_eq!(decoded, vec!["wifi:T:WPA;S:StudioNet;P:secret123;;"]);
    }

    #[test]
    fn decodes_tiny_low_resolution_qr_via_upscale_pass() {
        let (rgba, w, h) = render_qr_rgba(
            "https://yal.dev/tiny",
            2,
            0,
            [0, 0, 0, 255],
            [255, 255, 255, 255],
        );
        assert!(w < 100 && h < 100);
        let decoded = decode_qr_codes_rgba(&rgba, w, h);
        assert_eq!(decoded, vec!["https://yal.dev/tiny"]);
    }

    #[test]
    fn decodes_multiple_distinct_qr_codes_in_one_capture() {
        let (q1, w1, h1) = render_qr_rgba(
            "https://example.com/first",
            4,
            4,
            [0, 0, 0, 255],
            [255, 255, 255, 255],
        );
        let (q2, w2, h2) = render_qr_rgba(
            "https://example.com/second",
            4,
            4,
            [0, 0, 0, 255],
            [255, 255, 255, 255],
        );
        let gap = 32usize;
        let canvas_w = w1 as usize + gap + w2 as usize;
        let canvas_h = (h1.max(h2)) as usize;
        let mut canvas = vec![255u8; canvas_w * canvas_h * 4];

        for y in 0..h1 as usize {
            for x in 0..w1 as usize {
                let src = (y * w1 as usize + x) * 4;
                let dst = (y * canvas_w + x) * 4;
                canvas[dst..dst + 4].copy_from_slice(&q1[src..src + 4]);
            }
        }
        let x_off = w1 as usize + gap;
        for y in 0..h2 as usize {
            for x in 0..w2 as usize {
                let src = (y * w2 as usize + x) * 4;
                let dst = (y * canvas_w + x_off + x) * 4;
                canvas[dst..dst + 4].copy_from_slice(&q2[src..src + 4]);
            }
        }

        let mut decoded = decode_qr_codes_rgba(&canvas, canvas_w as u32, canvas_h as u32);
        decoded.sort();
        assert_eq!(
            decoded,
            vec!["https://example.com/first", "https://example.com/second"]
        );
    }

    #[test]
    fn returns_empty_for_invalid_or_blank_buffers() {
        assert!(decode_qr_codes_rgba(&[], 0, 0).is_empty());
        assert!(decode_qr_codes_rgba(&[0u8; 12], 2, 2).is_empty());
        assert!(decode_qr_codes_rgba(&vec![255u8; 64 * 64 * 4], 64, 64).is_empty());
    }

    #[test]
    fn formats_qr_hud_preview_for_single_long_and_multiple_codes() {
        assert_eq!(
            format_qr_hud_preview(&["https://example.com".to_string()]),
            "QR copied: https://example.com"
        );
        let long = "https://example.com/".to_string() + &"a".repeat(80);
        let preview = format_qr_hud_preview(&[long]);
        assert!(preview.starts_with("QR copied: https://example.com/"));
        assert!(preview.ends_with('…'));
        assert_eq!(
            format_qr_hud_preview(&["one".to_string(), "two".to_string()]),
            "Copied 2 QR codes"
        );
    }
}
