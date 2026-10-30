//! Installed fonts: what the system offers for text, by family.
//!
//! Enumeration goes through GDI's font-family walk, which reports exactly
//! the families a program choosing a font would see — not the raw font
//! file names on disk. A family is kept once, no matter how many faces
//! (regular, bold, italic…) it contains.

/// One installed font family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontFamily {
    /// The name a font-choosing program displays, e.g. "Segoe UI".
    pub family: String,
}

#[cfg(windows)]
mod windows_impl {
    use super::FontFamily;
    use windows::Win32::Foundation::LPARAM;
    use windows::Win32::Graphics::Gdi::{
        EnumFontFamiliesExW, GetDC, HDC, LOGFONTW, TEXTMETRICW,
    };

    /// The families installed system-wide, sorted by name.
    ///
    /// `EnumFontFamiliesExW` calls back once per face in up to four sets
    /// (bitmap, vector, TrueType, device); the same family name is
    /// therefore seen several times and is folded to one entry here.
    pub fn families() -> Vec<FontFamily> {
        unsafe {
            let hdc: HDC = GetDC(None);
            let logfont = LOGFONTW {
                lfCharSet: windows::Win32::Graphics::Gdi::DEFAULT_CHARSET,
                ..Default::default()
            };
            let mut seen: Vec<String> = Vec::new();

            unsafe extern "system" fn callback(
                logfont: *const LOGFONTW,
                _textmetric: *const TEXTMETRICW,
                _font_type: u32,
                lparam: LPARAM,
            ) -> i32 {
                unsafe {
                    let families = &mut *(lparam.0 as *mut Vec<String>);
                    let face = (*logfont).lfFaceName;
                    let name = String::from_utf16_lossy(
                        &face.iter().take_while(|c| **c != 0).copied().collect::<Vec<_>>(),
                    );
                    let name = name.trim().to_string();
                    if !name.is_empty() && !families.contains(&name) {
                        families.push(name);
                    }
                }
                1 // continue enumeration
            }

            EnumFontFamiliesExW(hdc, &logfont, Some(callback), LPARAM(&mut seen as *mut _ as isize), 0);
            seen.sort_by_key(|name| name.to_lowercase());
            seen.into_iter().map(|family| FontFamily { family }).collect()
        }
    }
}

/// The font families installed on this machine, sorted by name.
pub fn families() -> Vec<FontFamily> {
    #[cfg(windows)]
    return windows_impl::families();
    #[cfg(not(windows))]
    return Vec::new();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_system_carries_the_standard_families() {
        let families = families();
        assert!(!families.is_empty(), "a Windows machine always has font families");
        // Segoe UI ships with every supported Windows build and is in fact
        // the default system font.
        assert!(
            families.iter().any(|font| font.family.eq_ignore_ascii_case("Segoe UI")),
            "Segoe UI should be among {:#?}",
            families.iter().take(10).collect::<Vec<_>>()
        );
    }

    #[test]
    fn families_are_sorted_and_unique() {
        let families = families();
        let names: Vec<String> = families.iter().map(|font| font.family.to_lowercase()).collect();
        let mut sorted: Vec<String> = names.clone();
        sorted.sort();
        assert_eq!(names, sorted, "families should come back sorted by name");
        let before = names.len();
        let mut deduped = names;
        deduped.dedup();
        assert_eq!(before, deduped.len(), "no family should appear twice");
    }
}
