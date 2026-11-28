//! Pure-Rust OCR text post-processing (NormCap-inspired) and image preprocessing.

/// Formatting strategy applied to recognized OCR lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum OcrFormatMode {
    #[default]
    Smart,
    Raw,
}

impl OcrFormatMode {
    /// Switch between [`OcrFormatMode::Smart`] and [`OcrFormatMode::Raw`].
    pub fn toggle(self) -> Self {
        match self {
            Self::Smart => Self::Raw,
            Self::Raw => Self::Smart,
        }
    }

    /// Stable lowercase key for settings and IPC.
    pub fn key(self) -> &'static str {
        match self {
            Self::Smart => "smart",
            Self::Raw => "raw",
        }
    }

    /// Human-readable mode label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Smart => "Smart",
            Self::Raw => "Raw",
        }
    }

    /// Parse a mode key (case-insensitive, trimmed).
    pub fn parse(raw: &str) -> Option<Self> {
        let trimmed = raw.trim();
        if trimmed.eq_ignore_ascii_case("smart") {
            Some(Self::Smart)
        } else if trimmed.eq_ignore_ascii_case("raw") {
            Some(Self::Raw)
        } else {
            None
        }
    }
}

/// Join OCR word tokens with CJK-aware spacing, skipping empty/whitespace-only items.
pub fn join_ocr_words(words: &[String]) -> String {
    let mut out = String::new();
    let mut prev_last: Option<char> = None;

    for word in words {
        let trimmed = word.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let (Some(prev), Some(next)) = (prev_last, trimmed.chars().next()) {
            if !(is_cjk_like(prev) && is_cjk_like(next)) {
                out.push(' ');
            }
        }
        out.push_str(trimmed);
        prev_last = trimmed.chars().last();
    }

    out
}

/// Format OCR lines according to [`OcrFormatMode`].
pub fn format_ocr_lines(lines: &[String], mode: OcrFormatMode) -> String {
    let trimmed: Vec<&str> = lines.iter().map(|line| line.trim_end()).collect();
    let start = trimmed
        .iter()
        .position(|line| !line.trim().is_empty())
        .unwrap_or(trimmed.len());
    let end = trimmed
        .iter()
        .rposition(|line| !line.trim().is_empty())
        .map_or(0, |idx| idx + 1);

    if start >= end {
        return String::new();
    }

    let slice = &trimmed[start..end];
    match mode {
        OcrFormatMode::Raw => slice.join("\n"),
        OcrFormatMode::Smart => format_smart_lines(slice),
    }
}

fn format_smart_lines(lines: &[&str]) -> String {
    let non_empty: Vec<&str> = lines
        .iter()
        .copied()
        .filter(|line| !line.trim().is_empty())
        .collect();

    if non_empty.is_empty() {
        return String::new();
    }

    let url_or_email_count = non_empty
        .iter()
        .filter(|line| looks_like_url_or_email(line))
        .count();
    if url_or_email_count > 0 && url_or_email_count * 2 >= non_empty.len() {
        return non_empty
            .iter()
            .map(|line| {
                if looks_like_url_or_email(line) {
                    clean_url_or_email(line)
                } else {
                    line.trim().to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
    }

    if looks_like_structured_or_code(&non_empty) {
        return lines.join("\n");
    }

    format_wrapped_prose(lines)
}

fn looks_like_url_or_email(line: &str) -> bool {
    let cleaned = clean_url_or_email(line);
    if cleaned.is_empty() || cleaned.contains(char::is_whitespace) {
        return false;
    }

    let lower = cleaned.to_ascii_lowercase();
    if let Some(rest) = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))
        .or_else(|| lower.strip_prefix("www."))
    {
        return !rest.is_empty() && rest.contains('.');
    }

    if let Some((local, domain)) = cleaned.split_once('@') {
        if domain.contains('@') || local.is_empty() || domain.is_empty() {
            return false;
        }
        if !domain.contains('.') || domain.starts_with('.') || domain.ends_with('.') {
            return false;
        }
        let valid_local = local
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '%' | '+' | '-'));
        let valid_domain = domain
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'));
        return valid_local && valid_domain;
    }

    false
}

fn clean_url_or_email(line: &str) -> String {
    let collapsed: String = line.split_whitespace().collect::<Vec<_>>().join(" ");
    let chars: Vec<char> = collapsed.chars().collect();
    let mut out = String::with_capacity(collapsed.len());

    for (idx, &ch) in chars.iter().enumerate() {
        if ch == ' ' {
            let prev = idx.checked_sub(1).and_then(|i| chars.get(i)).copied();
            let next = chars.get(idx + 1).copied();
            if prev.is_some_and(is_url_join_punct) || next.is_some_and(is_url_join_punct) {
                continue;
            }
        }
        out.push(ch);
    }

    out
}

fn is_url_join_punct(ch: char) -> bool {
    matches!(ch, '.' | '/' | ':' | '@' | '-' | '?' | '&' | '=' | '#')
}

fn looks_like_structured_or_code(non_empty: &[&str]) -> bool {
    if non_empty.len() <= 1 {
        return true;
    }

    let mut code_or_path_hits = 0usize;
    let mut list_hits = 0usize;
    let mut kv_hits = 0usize;
    let mut tabular_hits = 0usize;
    let mut token_or_serial_hits = 0usize;

    for &line in non_empty {
        let trimmed = line.trim();
        if is_code_or_path_line(trimmed) {
            code_or_path_hits += 1;
        }
        if is_list_item_line(trimmed) {
            list_hits += 1;
        }
        if is_key_value_line(trimmed) {
            kv_hits += 1;
        }
        if is_tabular_line(trimmed) {
            tabular_hits += 1;
        }
        if is_short_serial_or_token(trimmed) {
            token_or_serial_hits += 1;
        }
    }

    if code_or_path_hits >= 1 {
        return true;
    }
    if list_hits >= 2 || (list_hits >= 1 && list_hits * 2 >= non_empty.len()) {
        return true;
    }
    if kv_hits >= 2
        || (kv_hits >= 1 && non_empty.len() == 2 && non_empty.iter().all(|l| l.trim().len() <= 48))
    {
        return true;
    }
    if tabular_hits >= 2 || (tabular_hits >= 1 && tabular_hits * 2 >= non_empty.len()) {
        return true;
    }
    if token_or_serial_hits >= 2 && token_or_serial_hits * 2 >= non_empty.len() {
        return true;
    }

    false
}

fn is_code_or_path_line(trimmed: &str) -> bool {
    if trimmed.contains('{')
        || trimmed.contains('}')
        || trimmed.ends_with(';')
        || trimmed.contains("=>")
        || trimmed.contains("::")
        || trimmed.starts_with("//")
        || trimmed.starts_with("#!")
        || trimmed.starts_with("#include")
    {
        return true;
    }

    const UNAMBIGUOUS_CODE_PREFIXES: &[&str] = &[
        "fn ", "pub ", "const ", "struct ", "enum ", "impl ", "trait ", "mod ", "def ", "async ",
        "await ", "SELECT ", "INSERT ", "UPDATE ", "DELETE ",
    ];
    if UNAMBIGUOUS_CODE_PREFIXES
        .iter()
        .any(|prefix| trimmed.starts_with(prefix))
    {
        return true;
    }

    const AMBIGUOUS_CODE_PREFIXES: &[&str] = &[
        "for ", "from ", "if ", "else ", "let ", "use ", "while ", "return ", "class ", "import ",
    ];
    if AMBIGUOUS_CODE_PREFIXES
        .iter()
        .any(|prefix| trimmed.starts_with(prefix))
        && has_ambiguous_code_syntax(trimmed)
    {
        return true;
    }

    // Windows drive path (e.g. `C:\foo` or `D:/bar`) or Unix root path (`/usr/bin`).
    let bytes = trimmed.as_bytes();
    if bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
    {
        return true;
    }
    if trimmed.starts_with('/')
        && trimmed.len() >= 3
        && !trimmed.contains(' ')
        && trimmed[1..].contains('/')
    {
        return true;
    }

    false
}

fn has_ambiguous_code_syntax(trimmed: &str) -> bool {
    if trimmed.contains("->") || trimmed.contains("&&") || trimmed.contains("||") {
        return true;
    }
    if trimmed
        .chars()
        .any(|c| matches!(c, '(' | ')' | '=' | ':' | ';' | '[' | ']' | '<' | '>'))
    {
        return true;
    }
    if has_dotted_identifier(trimmed) {
        return true;
    }
    if trimmed.starts_with("from ") && trimmed.contains(" import ") {
        return true;
    }
    if let Some(rest) = trimmed.strip_prefix("import ") {
        let tokens: Vec<&str> = rest.split_whitespace().collect();
        let is_module_ident = |tok: &str| {
            !tok.is_empty()
                && !tok.starts_with('.')
                && !tok.ends_with('.')
                && tok
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.'))
        };
        match tokens.as_slice() {
            [module] if is_module_ident(module) => return true,
            [module, "as", alias] if is_module_ident(module) && is_module_ident(alias) => {
                return true;
            }
            _ => {}
        }
    }
    false
}

fn has_dotted_identifier(text: &str) -> bool {
    let bytes = text.as_bytes();
    for idx in 1..bytes.len().saturating_sub(1) {
        if bytes[idx] == b'.' {
            let prev = bytes[idx - 1];
            let next = bytes[idx + 1];
            if (prev.is_ascii_alphanumeric() || prev == b'_')
                && (next.is_ascii_alphanumeric() || next == b'_')
            {
                return true;
            }
        }
    }
    false
}

fn is_list_item_line(trimmed: &str) -> bool {
    if trimmed.starts_with("- ")
        || trimmed.starts_with("* ")
        || trimmed.starts_with("• ")
        || trimmed.starts_with("– ")
        || trimmed.starts_with("— ")
    {
        return true;
    }

    let digit_count = trimmed.chars().take_while(|c| c.is_ascii_digit()).count();
    if digit_count > 0 {
        let rest = &trimmed[digit_count..];
        if rest.starts_with(". ") || rest.starts_with(") ") {
            return true;
        }
    }

    false
}

fn is_key_value_line(trimmed: &str) -> bool {
    let Some((key, val)) = trimmed.split_once(':') else {
        return false;
    };
    let key = key.trim();
    let val = val.trim();
    if key.is_empty() || val.is_empty() || key.len() > 32 {
        return false;
    }
    if key.eq_ignore_ascii_case("http") || key.eq_ignore_ascii_case("https") {
        return false;
    }
    if key.contains('.') || key.contains('?') || key.contains('!') || key.contains('/') {
        return false;
    }
    key.chars().any(|c| c.is_alphanumeric())
}

fn is_tabular_line(trimmed: &str) -> bool {
    if trimmed.contains('\t') {
        return true;
    }
    if trimmed.starts_with('|') && trimmed.ends_with('|') && trimmed.len() >= 3 {
        return true;
    }
    // Two or more consecutive spaces separating columns.
    trimmed.contains("  ")
}

fn is_short_serial_or_token(trimmed: &str) -> bool {
    if trimmed.is_empty() || trimmed.len() > 36 || trimmed.contains(' ') {
        return false;
    }
    let has_digit = trimmed.chars().any(|c| c.is_ascii_digit());
    let has_alpha = trimmed.chars().any(|c| c.is_alphabetic());
    (has_digit && has_alpha) || trimmed.contains('-') || trimmed.contains('_')
}

fn format_wrapped_prose(lines: &[&str]) -> String {
    let mut paragraphs: Vec<String> = Vec::new();
    let mut current = String::new();

    for &raw_line in lines {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            if !current.is_empty() {
                paragraphs.push(std::mem::take(&mut current));
            }
            continue;
        }

        if current.is_empty() {
            current.push_str(trimmed);
            continue;
        }

        if should_dehyphenate(&current, trimmed) {
            current.pop();
            current.push_str(trimmed);
        } else {
            let prev_char = current.chars().last();
            let next_char = trimmed.chars().next();
            if let (Some(prev), Some(next)) = (prev_char, next_char) {
                if !(is_cjk_like(prev) && is_cjk_like(next)) {
                    current.push(' ');
                }
            }
            current.push_str(trimmed);
        }
    }

    if !current.is_empty() {
        paragraphs.push(current);
    }

    paragraphs.join("\n\n")
}

fn should_dehyphenate(prev: &str, next: &str) -> bool {
    let Some(without_hyphen) = prev.strip_suffix('-') else {
        return false;
    };
    let Some(before_hyphen) = without_hyphen.chars().last() else {
        return false;
    };
    let Some(next_first) = next.chars().next() else {
        return false;
    };
    before_hyphen.is_alphabetic() && next_first.is_alphabetic()
}

fn is_cjk_like(ch: char) -> bool {
    matches!(
        ch as u32,
        0x1100..=0x11FF
            | 0x2E80..=0x2FDF
            | 0x3000..=0x30FF
            | 0x3100..=0x31FF
            | 0x3200..=0x33FF
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xA960..=0xA97F
            | 0xAC00..=0xD7FF
            | 0xF900..=0xFAFF
            | 0xFE30..=0xFE4F
            | 0xFF00..=0xFFEF
            | 0x20000..=0x2FA1F
    )
}

/// Extract sub-rectangle `(x, y, w, h)` from a `full_w x full_h` BGRA8 buffer and
/// convert it to tightly-packed RGBA8 with opaque alpha `255`.
pub fn crop_bgra_to_rgba(
    bgra: &[u8],
    full_w: u32,
    full_h: u32,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
) -> Option<Vec<u8>> {
    if full_w == 0 || full_h == 0 || w == 0 || h == 0 {
        return None;
    }
    let end_x = x.checked_add(w)?;
    let end_y = y.checked_add(h)?;
    if end_x > full_w || end_y > full_h {
        return None;
    }

    let full_pixels = (full_w as usize).checked_mul(full_h as usize)?;
    let expected_len = full_pixels.checked_mul(4)?;
    if bgra.len() != expected_len {
        return None;
    }

    let out_pixels = (w as usize).checked_mul(h as usize)?;
    let out_len = out_pixels.checked_mul(4)?;
    let mut out = Vec::with_capacity(out_len);

    let full_stride = (full_w as usize).checked_mul(4)?;
    let x_offset = (x as usize).checked_mul(4)?;
    let row_bytes = (w as usize).checked_mul(4)?;

    for row in y..end_y {
        let row_start = (row as usize)
            .checked_mul(full_stride)?
            .checked_add(x_offset)?;
        let row_end = row_start.checked_add(row_bytes)?;
        let src_row = bgra.get(row_start..row_end)?;
        for px in src_row.as_chunks::<4>().0 {
            out.extend_from_slice(&[px[2], px[1], px[0], 255]);
        }
    }

    Some(out)
}

const OCR_BORDER_PAD: u32 = 24;
const OCR_MAX_SCALED_DIM: u32 = 2400;

/// Upscale small captures with bilinear interpolation and surround the image with a
/// `24 px` solid border derived from the median RGB of the input perimeter.
pub fn prepare_ocr_rgba(rgba: &[u8], width: u32, height: u32) -> Option<(Vec<u8>, u32, u32)> {
    if width == 0 || height == 0 {
        return None;
    }

    let in_pixels = (width as usize).checked_mul(height as usize)?;
    let expected_len = in_pixels.checked_mul(4)?;
    if rgba.len() != expected_len {
        return None;
    }

    let border_rgb = median_border_rgb(rgba, width, height);

    let pad_total = OCR_BORDER_PAD.checked_mul(2)?;
    let max_inner = OCR_MAX_SCALED_DIM.checked_sub(pad_total)?;

    let (scaled_w, scaled_h) = if width > max_inner || height > max_inner {
        let ratio_w = f64::from(max_inner) / f64::from(width);
        let ratio_h = f64::from(max_inner) / f64::from(height);
        let downscale = ratio_w.min(ratio_h);
        let target_w = ((f64::from(width) * downscale).floor() as u32).clamp(1, max_inner);
        let target_h = ((f64::from(height) * downscale).floor() as u32).clamp(1, max_inner);
        (target_w, target_h)
    } else {
        let desired_scale: u32 = if height < 48 {
            3
        } else if width < 320 || height < 140 {
            2
        } else {
            1
        };

        let mut scale = desired_scale;
        while scale > 1 {
            let candidate_w = width.saturating_mul(scale).saturating_add(pad_total);
            let candidate_h = height.saturating_mul(scale).saturating_add(pad_total);
            if candidate_w <= OCR_MAX_SCALED_DIM && candidate_h <= OCR_MAX_SCALED_DIM {
                break;
            }
            scale -= 1;
        }
        (width.checked_mul(scale)?, height.checked_mul(scale)?)
    };

    let out_w = scaled_w.checked_add(pad_total)?;
    let out_h = scaled_h.checked_add(pad_total)?;

    let out_pixels = (out_w as usize).checked_mul(out_h as usize)?;
    let out_len = out_pixels.checked_mul(4)?;
    let mut out = vec![0u8; out_len];

    let bg = [border_rgb[0], border_rgb[1], border_rgb[2], 255];
    for px in out.as_chunks_mut::<4>().0 {
        *px = bg;
    }

    let out_stride = (out_w as usize).checked_mul(4)?;
    let pad_offset = (OCR_BORDER_PAD as usize).checked_mul(4)?;

    if scaled_w == width && scaled_h == height {
        let in_stride = (width as usize).checked_mul(4)?;
        for row in 0..(height as usize) {
            let src_start = row.checked_mul(in_stride)?;
            let src_end = src_start.checked_add(in_stride)?;
            let src_row = rgba.get(src_start..src_end)?;

            let dst_row_idx = row.checked_add(OCR_BORDER_PAD as usize)?;
            let dst_start = dst_row_idx
                .checked_mul(out_stride)?
                .checked_add(pad_offset)?;
            let dst_end = dst_start.checked_add(in_stride)?;
            let dst_row = out.get_mut(dst_start..dst_end)?;

            for (dst_px, src_px) in dst_row
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .zip(src_row.as_chunks::<4>().0)
            {
                *dst_px = [src_px[0], src_px[1], src_px[2], 255];
            }
        }
    } else {
        let scale_x = f64::from(width) / f64::from(scaled_w);
        let scale_y = f64::from(height) / f64::from(scaled_h);
        let max_x = f64::from(width - 1);
        let max_y = f64::from(height - 1);
        let w_usize = width as usize;
        let scaled_row_bytes = (scaled_w as usize).checked_mul(4)?;

        for dst_y in 0..(scaled_h as usize) {
            let src_y = (((dst_y as f64) + 0.5) * scale_y - 0.5).clamp(0.0, max_y);
            let y0 = src_y.floor() as usize;
            let y1 = (y0 + 1).min((height - 1) as usize);
            let fy = src_y - (y0 as f64);

            let dst_row_idx = dst_y.checked_add(OCR_BORDER_PAD as usize)?;
            let dst_start = dst_row_idx
                .checked_mul(out_stride)?
                .checked_add(pad_offset)?;
            let dst_end = dst_start.checked_add(scaled_row_bytes)?;
            let dst_row = out.get_mut(dst_start..dst_end)?;

            for (dst_x, dst_px) in dst_row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                let src_x = (((dst_x as f64) + 0.5) * scale_x - 0.5).clamp(0.0, max_x);
                let x0 = src_x.floor() as usize;
                let x1 = (x0 + 1).min((width - 1) as usize);
                let fx = src_x - (x0 as f64);

                let p00 = sample_rgb(rgba, w_usize, x0, y0);
                let p10 = sample_rgb(rgba, w_usize, x1, y0);
                let p01 = sample_rgb(rgba, w_usize, x0, y1);
                let p11 = sample_rgb(rgba, w_usize, x1, y1);

                let r = bilerp(p00[0], p10[0], p01[0], p11[0], fx, fy);
                let g = bilerp(p00[1], p10[1], p01[1], p11[1], fx, fy);
                let b = bilerp(p00[2], p10[2], p01[2], p11[2], fx, fy);
                *dst_px = [r, g, b, 255];
            }
        }
    }

    Some((out, out_w, out_h))
}

fn sample_rgb(rgba: &[u8], width: usize, x: usize, y: usize) -> [u8; 3] {
    let Some(idx) = y
        .checked_mul(width)
        .and_then(|row| row.checked_add(x))
        .and_then(|px| px.checked_mul(4))
    else {
        return [0, 0, 0];
    };
    match rgba.get(idx..idx + 3) {
        Some(slice) => [slice[0], slice[1], slice[2]],
        None => [0, 0, 0],
    }
}

fn bilerp(c00: u8, c10: u8, c01: u8, c11: u8, fx: f64, fy: f64) -> u8 {
    let top = f64::from(c00) * (1.0 - fx) + f64::from(c10) * fx;
    let bottom = f64::from(c01) * (1.0 - fx) + f64::from(c11) * fx;
    let value = top * (1.0 - fy) + bottom * fy;
    value.round().clamp(0.0, 255.0) as u8
}

fn median_border_rgb(rgba: &[u8], width: u32, height: u32) -> [u8; 3] {
    let w = width as usize;
    let h = height as usize;
    let perimeter = w.saturating_mul(2).saturating_add(h.saturating_mul(2));
    let mut rs = Vec::with_capacity(perimeter);
    let mut gs = Vec::with_capacity(perimeter);
    let mut bs = Vec::with_capacity(perimeter);

    let mut push_px = |x: usize, y: usize| {
        let rgb = sample_rgb(rgba, w, x, y);
        rs.push(rgb[0]);
        gs.push(rgb[1]);
        bs.push(rgb[2]);
    };

    for x in 0..w {
        push_px(x, 0);
    }
    if h > 1 {
        for x in 0..w {
            push_px(x, h - 1);
        }
    }
    if h > 2 {
        for y in 1..(h - 1) {
            push_px(0, y);
            if w > 1 {
                push_px(w - 1, y);
            }
        }
    }

    [median_u8(&mut rs), median_u8(&mut gs), median_u8(&mut bs)]
}

fn median_u8(values: &mut [u8]) -> u8 {
    if values.is_empty() {
        return 255;
    }
    values.sort_unstable();
    let mid = values.len() / 2;
    if values.len() % 2 == 1 {
        values[mid]
    } else {
        ((u16::from(values[mid - 1]) + u16::from(values[mid])) / 2) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn ocr_format_mode_helpers_and_serde() {
        let mode = OcrFormatMode::default();
        assert_eq!(mode, OcrFormatMode::Smart);
        assert_eq!(mode.key(), "smart");
        assert_eq!(mode.label(), "Smart");
        assert_eq!(mode.toggle(), OcrFormatMode::Raw);
        assert_eq!(mode.toggle().toggle(), OcrFormatMode::Smart);
        assert_eq!(OcrFormatMode::Raw.key(), "raw");
        assert_eq!(OcrFormatMode::Raw.label(), "Raw");
        assert_eq!(OcrFormatMode::parse("  SMART "), Some(OcrFormatMode::Smart));
        assert_eq!(OcrFormatMode::parse("Raw"), Some(OcrFormatMode::Raw));
        assert_eq!(OcrFormatMode::parse("other"), None);

        for m in [OcrFormatMode::Smart, OcrFormatMode::Raw] {
            let json = serde_json::to_string(&m).unwrap();
            let decoded: OcrFormatMode = serde_json::from_str(&json).unwrap();
            assert_eq!(decoded, m);
        }
    }

    #[test]
    fn join_ocr_words_handles_latin_cjk_and_blanks() {
        let latin = lines(&["  Hello", "", "   ", "world!  "]);
        assert_eq!(join_ocr_words(&latin), "Hello world!");

        let cjk = lines(&["東京", "タワー", "へ", "行く"]);
        assert_eq!(join_ocr_words(&cjk), "東京タワーへ行く");

        let mixed = lines(&["Hello", "世界", "です", "2026"]);
        assert_eq!(join_ocr_words(&mixed), "Hello 世界です 2026");
    }

    #[test]
    fn format_raw_preserves_lines_and_trims_outer_blanks() {
        let input = lines(&["", "  ", "First line   ", "", "  Indented  ", "   "]);
        assert_eq!(
            format_ocr_lines(&input, OcrFormatMode::Raw),
            "First line\n\n  Indented"
        );
    }

    #[test]
    fn smart_mode_cleans_urls_and_emails() {
        let urls = lines(&[
            "https : // example . com / docs / v1",
            "www . rust - lang . org",
        ]);
        assert_eq!(
            format_ocr_lines(&urls, OcrFormatMode::Smart),
            "https://example.com/docs/v1\nwww.rust-lang.org"
        );

        let emails = lines(&["alice . smith @ example . co . uk"]);
        assert_eq!(
            format_ocr_lines(&emails, OcrFormatMode::Smart),
            "alice.smith@example.co.uk"
        );
    }

    #[test]
    fn smart_mode_joins_prose_dehyphenates_and_keeps_paragraphs() {
        let prose = lines(&[
            "This is a wrapped exam-",
            "ple of normal prose that",
            "spans three lines.",
            "",
            "Second paragraph starts",
            "right here.",
        ]);
        assert_eq!(
            format_ocr_lines(&prose, OcrFormatMode::Smart),
            "This is a wrapped example of normal prose that spans three lines.\n\nSecond paragraph starts right here."
        );

        let cjk_prose = lines(&["吾輩は猫である。", "名前はまだ無い。"]);
        assert_eq!(
            format_ocr_lines(&cjk_prose, OcrFormatMode::Smart),
            "吾輩は猫である。名前はまだ無い。"
        );
    }

    #[test]
    fn smart_mode_preserves_code_paths_lists_and_structured_lines() {
        let code = lines(&["pub fn add(a: i32, b: i32) -> i32 {", "    a + b", "}"]);
        assert_eq!(
            format_ocr_lines(&code, OcrFormatMode::Smart),
            "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}"
        );

        let list = lines(&["- First item", "- Second item", "- Third item"]);
        assert_eq!(
            format_ocr_lines(&list, OcrFormatMode::Smart),
            "- First item\n- Second item\n- Third item"
        );

        let paths = lines(&["C:\\Users\\dev\\project", "C:\\Windows\\System32"]);
        assert_eq!(
            format_ocr_lines(&paths, OcrFormatMode::Smart),
            "C:\\Users\\dev\\project\nC:\\Windows\\System32"
        );

        let kv = lines(&["Host: localhost", "Port: 8080"]);
        assert_eq!(
            format_ocr_lines(&kv, OcrFormatMode::Smart),
            "Host: localhost\nPort: 8080"
        );
    }

    #[test]
    fn smart_mode_distinguishes_prose_keywords_from_actual_code() {
        let prose = lines(&[
            "We went to the store",
            "for some groceries.",
            "This works well",
            "if you enable it.",
        ]);
        assert_eq!(
            format_ocr_lines(&prose, OcrFormatMode::Smart),
            "We went to the store for some groceries. This works well if you enable it."
        );

        let c_code = lines(&["if (x == 1)", "return y;"]);
        assert_eq!(
            format_ocr_lines(&c_code, OcrFormatMode::Smart),
            "if (x == 1)\nreturn y;"
        );

        let py_imports = lines(&["import os", "from pathlib import Path"]);
        assert_eq!(
            format_ocr_lines(&py_imports, OcrFormatMode::Smart),
            "import os\nfrom pathlib import Path"
        );
    }

    #[test]
    fn crop_bgra_to_rgba_converts_and_validates_bounds() {
        // 2x2 BGRA image: (0,0)=[10,20,30,40], (1,0)=[50,60,70,80], (0,1)=[1,2,3,4], (1,1)=[9,8,7,6]
        let bgra = vec![
            10, 20, 30, 40, 50, 60, 70, 80, // row 0
            1, 2, 3, 4, 9, 8, 7, 6, // row 1
        ];
        let cropped = crop_bgra_to_rgba(&bgra, 2, 2, 1, 0, 1, 2).unwrap();
        assert_eq!(cropped, vec![70, 60, 50, 255, 7, 8, 9, 255]);

        // Out-of-bounds, overflow, zero-sized, and mismatched length return None.
        assert!(crop_bgra_to_rgba(&bgra, 2, 2, 2, 0, 1, 1).is_none());
        assert!(crop_bgra_to_rgba(&bgra, 2, 2, u32::MAX, 0, 1, 1).is_none());
        assert!(crop_bgra_to_rgba(&bgra, 2, 2, 0, 0, 0, 1).is_none());
        assert!(crop_bgra_to_rgba(&bgra[..12], 2, 2, 0, 0, 1, 1).is_none());
    }

    #[test]
    fn prepare_ocr_rgba_upscales_pads_and_uses_median_border() {
        // 20x10 (height < 48 => 3x scale => 60x30 + 48 => 108x78)
        let w = 20u32;
        let h = 10u32;
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let idx = ((y * w + x) * 4) as usize;
                // Border mostly (240, 230, 220), interior dark (10, 20, 30).
                if x == 0 || y == 0 || x == w - 1 || y == h - 1 {
                    rgba[idx..idx + 4].copy_from_slice(&[240, 230, 220, 255]);
                } else {
                    rgba[idx..idx + 4].copy_from_slice(&[10, 20, 30, 255]);
                }
            }
        }
        // Inject one outlier corner pixel to verify median ignores it.
        rgba[0..4].copy_from_slice(&[0, 0, 0, 255]);

        let (out, out_w, out_h) = prepare_ocr_rgba(&rgba, w, h).unwrap();
        assert_eq!(out_w, 20 * 3 + 48);
        assert_eq!(out_h, 10 * 3 + 48);
        assert_eq!(out.len(), (out_w * out_h * 4) as usize);
        // Top-left padding pixel must match the median border RGB with alpha 255.
        assert_eq!(&out[0..4], &[240, 230, 220, 255]);

        // Medium capture (width < 320, height >= 48 => 2x scale)
        let med = vec![128u8; 100 * 60 * 4];
        let (_, mw, mh) = prepare_ocr_rgba(&med, 100, 60).unwrap();
        assert_eq!(mw, 100 * 2 + 48);
        assert_eq!(mh, 60 * 2 + 48);

        // Large capture (>= 320x140 => 1x scale)
        let big = vec![200u8; 320 * 140 * 4];
        let (_, bw, bh) = prepare_ocr_rgba(&big, 320, 140).unwrap();
        assert_eq!(bw, 320 + 48);
        assert_eq!(bh, 140 + 48);

        // Wide short capture (1000x20): 3x would exceed 2400px width, 2x fits (2048 <= 2400).
        let wide_short = vec![255u8; 1000 * 20 * 4];
        let (_, wsw, wsh) = prepare_ocr_rgba(&wide_short, 1000, 20).unwrap();
        assert_eq!(wsw, 1000 * 2 + 48);
        assert_eq!(wsh, 20 * 2 + 48);

        // Invalid inputs
        assert!(prepare_ocr_rgba(&[], 0, 10).is_none());
        assert!(prepare_ocr_rgba(&[0u8; 3], 1, 1).is_none());
    }

    #[test]
    fn prepare_ocr_rgba_downscales_oversized_wide_captures_with_border_intact() {
        let w = 2560u32;
        let h = 100u32;
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let idx = ((y * w + x) * 4) as usize;
                if x == 0 || y == 0 || x == w - 1 || y == h - 1 {
                    rgba[idx..idx + 4].copy_from_slice(&[245, 240, 235, 255]);
                } else {
                    rgba[idx..idx + 4].copy_from_slice(&[20, 30, 40, 255]);
                }
            }
        }

        let (out, out_w, out_h) = prepare_ocr_rgba(&rgba, w, h).unwrap();
        assert!(out_w <= 2400);
        assert!(out_h <= 2400);
        assert_eq!(out_w, 2400);
        assert!(out_h > 48);
        assert_eq!(out.len(), (out_w * out_h * 4) as usize);

        // Verify 24px border padding on all sides uses median perimeter color.
        assert_eq!(&out[0..4], &[245, 240, 235, 255]);
        let last_px = out.len() - 4;
        assert_eq!(&out[last_px..], &[245, 240, 235, 255]);
        let pad_edge_idx = ((23 * out_w + 23) * 4) as usize;
        assert_eq!(&out[pad_edge_idx..pad_edge_idx + 4], &[245, 240, 235, 255]);

        // Center of the downscaled image preserves interior color.
        let center_idx = (((out_h / 2) * out_w + (out_w / 2)) * 4) as usize;
        assert_eq!(&out[center_idx..center_idx + 4], &[20, 30, 40, 255]);
    }
}
