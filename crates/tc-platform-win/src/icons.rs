//! Real Windows application and shell item icon extraction.
//!
//! Asks Windows' shell (`IShellItemImageFactory` first, falling back to
//! `SHGetFileInfoW`) for crisp 48x48 icons from `.lnk` shortcuts, `.exe` files,
//! folders, or Store AUMIDs, and converts the pixels to top-down premultiplied
//! RGBA ready for Direct2D `ID2D1Bitmap` upload.

use std::cell::Cell;
use std::ffi::c_void;
use std::path::Path;

use windows::core::{BOOL, PCWSTR};
use windows::Win32::Foundation::SIZE;
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO,
    BITMAPINFOHEADER, DIB_RGB_COLORS, HBITMAP, HGDIOBJ,
};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::Shell::{
    IShellItemImageFactory, SHCreateItemFromParsingName, SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON,
    SHGFI_LARGEICON, SIIGBF_ICONONLY,
};
use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, ICONINFO};

/// Premultiplied top-down RGBA bitmap ready for Direct2D GPU upload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IconImage {
    pub width: u32,
    pub height: u32,
    /// Premultiplied RGBA pixels (`width * height * 4` bytes, top row first).
    pub premultiplied_rgba: Vec<u8>,
}

thread_local! {
    static COM_READY: Cell<bool> = const { Cell::new(false) };
}

fn ensure_com_initialized() {
    COM_READY.with(|ready| {
        if !ready.get() {
            // S_OK, S_FALSE, or RPC_E_CHANGED_MODE all mean COM is usable on this thread.
            let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
            ready.set(true);
        }
    });
}

/// Extract a crisp icon for a `.lnk` shortcut, `.exe` path, folder, or shell
/// parsing name. Returns `None` if Windows cannot supply visible pixels.
pub fn extract_icon(target: &str) -> Option<IconImage> {
    let trimmed = target.trim();
    if trimmed.is_empty() {
        return None;
    }
    ensure_com_initialized();
    if let Some(image) = shell_item_icon(trimmed) {
        return Some(image);
    }
    application_icon(Path::new(trimmed))
}

/// Ask Windows' `IShellItemImageFactory` for a 48x48 icon (works for `.lnk`,
/// `.exe`, folders, and `shell:AppsFolder\<AUMID>`).
pub fn shell_item_icon(parsing_name: &str) -> Option<IconImage> {
    let wide: Vec<u16> = parsing_name
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let factory: IShellItemImageFactory =
            SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None).ok()?;
        let bitmap = factory
            .GetImage(SIZE { cx: 48, cy: 48 }, SIIGBF_ICONONLY)
            .ok()?;
        let image = hbitmap_to_premultiplied_rgba(bitmap, None);
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        image
    }
}

/// Fallback icon lookup via `SHGetFileInfoW` for plain file/executable paths.
pub fn application_icon(path: &Path) -> Option<IconImage> {
    use std::os::windows::ffi::OsStrExt;
    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let mut file_info = SHFILEINFOW::default();
        let ok = SHGetFileInfoW(
            PCWSTR(wide.as_ptr()),
            Default::default(),
            Some(&mut file_info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        );
        if ok == 0 || file_info.hIcon.is_invalid() {
            return None;
        }
        let hicon = file_info.hIcon;
        let mut info = ICONINFO {
            fIcon: BOOL(1),
            ..Default::default()
        };
        if GetIconInfo(hicon, &mut info).is_err() {
            let _ = DestroyIcon(hicon);
            return None;
        }
        let mask = if !info.hbmMask.is_invalid() {
            Some(info.hbmMask)
        } else {
            None
        };
        let image = hbitmap_to_premultiplied_rgba(info.hbmColor, mask);
        if !info.hbmColor.is_invalid() {
            let _ = DeleteObject(HGDIOBJ(info.hbmColor.0));
        }
        if !info.hbmMask.is_invalid() {
            let _ = DeleteObject(HGDIOBJ(info.hbmMask.0));
        }
        let _ = DestroyIcon(hicon);
        image
    }
}

unsafe fn hbitmap_to_premultiplied_rgba(
    color_bitmap: HBITMAP,
    mask_bitmap: Option<HBITMAP>,
) -> Option<IconImage> {
    if color_bitmap.is_invalid() {
        return None;
    }
    let mut bm = BITMAP::default();
    if GetObjectW(
        HGDIOBJ(color_bitmap.0),
        std::mem::size_of::<BITMAP>() as i32,
        Some(&mut bm as *mut BITMAP as *mut c_void),
    ) == 0
    {
        return None;
    }
    let width = bm.bmWidth.max(0) as u32;
    let height = bm.bmHeight.unsigned_abs();
    if width == 0 || height == 0 || width > 512 || height > 512 {
        return None;
    }

    let dc = CreateCompatibleDC(None);
    if dc.is_invalid() {
        return None;
    }
    let mut info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width as i32,
            biHeight: -(height as i32), // Top-down rows.
            biPlanes: 1,
            biBitCount: 32,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    let rows = GetDIBits(
        dc,
        color_bitmap,
        0,
        height,
        Some(pixels.as_mut_ptr() as *mut c_void),
        &mut info,
        DIB_RGB_COLORS,
    );
    if rows != height as i32 {
        let _ = DeleteDC(dc);
        return None;
    }

    let has_alpha = pixels.as_chunks::<4>().0.iter().any(|px| px[3] != 0);
    if !has_alpha {
        // Legacy 24-bit icon without an alpha channel: derive alpha from the
        // 1-bit AND mask if provided, otherwise treat non-black pixels or full
        // bitmap as opaque.
        if let Some(mask) = mask_bitmap {
            let mut mask_pixels = vec![0u8; (width * height * 4) as usize];
            let mask_rows = GetDIBits(
                dc,
                mask,
                0,
                height,
                Some(mask_pixels.as_mut_ptr() as *mut c_void),
                &mut info,
                DIB_RGB_COLORS,
            );
            if mask_rows == height as i32 {
                for (px, m) in pixels
                    .as_chunks_mut::<4>()
                    .0
                    .iter_mut()
                    .zip(mask_pixels.as_chunks::<4>().0.iter())
                {
                    // In Windows icon AND masks, white (0xFF) = transparent, black (0x00) = opaque.
                    px[3] = if m[0] == 0 { 255 } else { 0 };
                }
            }
        }
    }
    let _ = DeleteDC(dc);

    if pixels.as_chunks::<4>().0.iter().all(|px| px[3] == 0) {
        return None;
    }

    bgra_to_premultiplied_rgba(&mut pixels);
    Some(IconImage {
        width,
        height,
        premultiplied_rgba: pixels,
    })
}

fn bgra_to_premultiplied_rgba(bytes: &mut [u8]) {
    for px in bytes.as_chunks_mut::<4>().0 {
        let b = px[0] as u16;
        let g = px[1] as u16;
        let r = px[2] as u16;
        let a = px[3] as u16;
        // Direct2D expects premultiplied RGBA (`r <= a`, `g <= a`, `b <= a`).
        px[0] = ((r * a + 127) / 255) as u8;
        px[1] = ((g * a + 127) / 255) as u8;
        px[2] = ((b * a + 127) / 255) as u8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bgra_converts_to_premultiplied_rgba() {
        let mut bytes = vec![100, 200, 50, 255, 255, 128, 0, 128];
        bgra_to_premultiplied_rgba(&mut bytes);
        assert_eq!(&bytes[0..4], &[50, 200, 100, 255]);
        assert_eq!(&bytes[4..8], &[0, 64, 128, 128]);
    }

    #[test]
    fn extracts_real_icon_for_windows_notepad() {
        let icon = extract_icon(r"C:\Windows\System32\notepad.exe")
            .expect("notepad always ships with a shell icon");
        assert!(icon.width >= 32 && icon.width == icon.height);
        assert_eq!(
            icon.premultiplied_rgba.len(),
            (icon.width * icon.height * 4) as usize
        );
        assert!(icon
            .premultiplied_rgba
            .as_chunks::<4>()
            .0
            .iter()
            .any(|px| px[3] > 0));
    }
}
