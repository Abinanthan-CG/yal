//! Application icons for the palette's rows.
//!
//! The observed launcher lists programs under the icon their Start Menu
//! entry carries, and Windows keeps those pixels inside the file itself.
//! The extraction walks the shell: it is asked for the icon it owns for
//! a path, the pixels are picked out through GDI, and the result is flat
//! straight-alpha RGBA the UI can put straight into a texture. A missing
//! file, an icon the shell refuses to speak about, anything short of
//! pixels — met with `None`, and the row draws its drawn tile instead.

/// The flat pixels of one icon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IconImage {
    /// The icon's width, in pixels.
    pub width: u32,
    /// The icon's height, in pixels.
    pub height: u32,
    /// Straight (not premultiplied) alpha, row-major, top row first.
    pub rgba: Vec<u8>,
}

/// The icon the shell owns for a file — for an executable, the one its
/// Start Menu listing shows.
///
/// This is a per-render-time ask, worth once per program: reading an
/// icon walks through GDI object creation and a couple of kernel calls,
/// so callers keep their own cache and this function is never called
/// twice for the same path while the process lives.
#[cfg(windows)]
pub fn application_icon(executable: &std::path::Path) -> Option<IconImage> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::{BOOL, PCWSTR};
    use windows::Win32::Graphics::Gdi::DeleteObject;
    use windows::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};
    use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, ICONINFO};

    let wide: Vec<u16> = executable
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let mut file_info = SHFILEINFOW::default();
        let asked = SHGetFileInfoW(
            PCWSTR(wide.as_ptr()),
            Default::default(),
            Some(&mut file_info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        );
        if asked == 0 {
            return None;
        }
        let shell_icon = file_info.hIcon;
        if shell_icon.is_invalid() {
            return None;
        }

        let mut info = ICONINFO {
            fIcon: BOOL(1),
            ..Default::default()
        };
        if GetIconInfo(shell_icon, &mut info).is_err() {
            let _ = DestroyIcon(shell_icon);
            return None;
        }
        let colour_bitmap = info.hbmColor;
        let mask_bitmap = info.hbmMask;
        let pixels = icon_pixels(&colour_bitmap);
        let _ = DeleteObject(colour_bitmap.into());
        let _ = DeleteObject(mask_bitmap.into());
        let _ = DestroyIcon(shell_icon);
        pixels
    }
}

/// The pixels inside one icon's colour bitmap, through GDI.
///
/// The colour bitmap arrives bottom-up in memory, so the negative
/// height asks `GetDIBits` to hand the rows over top-down — the texture
/// order. The shell's memory byte order is BGRA; the UI's is RGBA.
#[cfg(windows)]
unsafe fn icon_pixels(colour_bitmap: &windows::Win32::Graphics::Gdi::HBITMAP) -> Option<IconImage> {
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleDC, DeleteDC, GetDIBits, GetObjectW, BITMAP, BITMAPINFO, BITMAPINFOHEADER,
        DIB_RGB_COLORS, HGDIOBJ,
    };

    if colour_bitmap.is_invalid() {
        return None;
    }

    unsafe {
        let mut bitmap = BITMAP::default();
        if GetObjectW(
            HGDIOBJ(colour_bitmap.0),
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut bitmap as *mut BITMAP as *mut core::ffi::c_void),
        ) == 0
        {
            return None;
        }
        let width = bitmap.bmWidth.max(0) as u32;
        let height = bitmap.bmHeight.unsigned_abs();
        if width == 0 || height == 0 {
            return None;
        }

        let screen = CreateCompatibleDC(None);
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                // Negative: the rows arrive top-down, as the texture
                // wants them.
                biHeight: -(height as i32),
                biPlanes: 1,
                biBitCount: 32,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        let rows = GetDIBits(
            screen,
            *colour_bitmap,
            0,
            height,
            Some(pixels.as_mut_ptr() as *mut core::ffi::c_void),
            &mut info,
            DIB_RGB_COLORS,
        );
        let _ = DeleteDC(screen);
        if rows != height as i32 {
            return None;
        }

        bgra_to_rgba(&mut pixels);
        // An icon whose bits carry no alpha anywhere would draw as one
        // invisible square: the drawn fallback tile is the answer to a
        // shell that refuses pixel-level honesty.
        if pixels.iter().skip(3).step_by(4).all(|alpha| *alpha == 0) {
            return None;
        }
        Some(IconImage {
            width,
            height,
            rgba: pixels,
        })
    }
}

/// Turn the shell's byte order (BGRA per pixel) into the texture one.
fn bgra_to_rgba(bytes: &mut [u8]) {
    for pixel in bytes.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
}

/// There is nothing outside Windows to mine icons from.
#[cfg(not(windows))]
pub fn application_icon(_executable: &std::path::Path) -> Option<IconImage> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_order_turns_bgra_into_rgba() {
        let mut bytes = vec![1, 2, 3, 4, 5, 6, 7, 8];
        bgra_to_rgba(&mut bytes);
        assert_eq!(bytes, vec![3, 2, 1, 4, 7, 6, 5, 8]);
    }

    #[cfg(windows)]
    #[test]
    fn the_shell_speaks_pixels_for_a_known_executable() {
        let notepad = std::path::Path::new(
            r"C:\Windows\System32\notepad.exe",
        );
        let image = application_icon(notepad).expect("notepad always carries an icon");
        assert_eq!(image.width % 2, 0, "icons are squares");
        assert_eq!(image.width, image.height);
        assert_eq!(image.rgba.len(), (image.width * image.height * 4) as usize);
        // Some pixel somewhere is not fully transparent.
        assert!(image.rgba.chunks_exact(4).any(|pixel| pixel[3] > 0));
    }
}
