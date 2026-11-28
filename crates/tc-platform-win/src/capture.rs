//! Virtual desktop screen capture and pixel/region extraction via Win32 GDI.
//!
//! Captures the entire virtual screen (spanning all monitors, including negative
//! coordinates in PerMonitorV2 multi-monitor setups) into a top-down 32-bit BGRA
//! buffer with strict RAII cleanup for every GDI device context and bitmap.

use std::ffi::c_void;

use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GdiFlush, GetDC,
    ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, CAPTUREBLT, DIB_RGB_COLORS,
    HBITMAP, HDC, HGDIOBJ, SRCCOPY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
};

/// A captured bitmap of the full Windows virtual desktop in physical pixels.
#[derive(Debug, Clone)]
pub struct ScreenSnapshot {
    pub origin_x: i32,
    pub origin_y: i32,
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

struct ScreenDcGuard(HDC);
impl Drop for ScreenDcGuard {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            // SAFETY: `self.0` was obtained from `GetDC(None)` and is released once on drop.
            unsafe {
                let _ = ReleaseDC(None, self.0);
            }
        }
    }
}

struct MemDcGuard(HDC);
impl Drop for MemDcGuard {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            // SAFETY: `self.0` came from `CreateCompatibleDC` and has its original object restored.
            unsafe {
                let _ = DeleteDC(self.0);
            }
        }
    }
}

struct BitmapGuard(HBITMAP);
impl Drop for BitmapGuard {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            // SAFETY: `self.0` came from `CreateDIBSection` and is deselected before drop.
            unsafe {
                let _ = DeleteObject(HGDIOBJ(self.0 .0));
            }
        }
    }
}

struct SelectedObjectGuard {
    hdc: HDC,
    previous: HGDIOBJ,
}
impl Drop for SelectedObjectGuard {
    fn drop(&mut self) {
        if !self.hdc.is_invalid() && !self.previous.is_invalid() {
            // SAFETY: Restores `self.previous` into `self.hdc` before the bitmap or DC is deleted.
            unsafe {
                let _ = SelectObject(self.hdc, self.previous);
            }
        }
    }
}

impl ScreenSnapshot {
    /// Capture the entire Windows virtual desktop across all connected displays.
    pub fn capture_virtual_desktop() -> Result<Self, String> {
        // SAFETY: `GetSystemMetrics` reads read-only virtual desktop dimensions from the OS.
        let (origin_x, origin_y, raw_width, raw_height) = unsafe {
            (
                GetSystemMetrics(SM_XVIRTUALSCREEN),
                GetSystemMetrics(SM_YVIRTUALSCREEN),
                GetSystemMetrics(SM_CXVIRTUALSCREEN),
                GetSystemMetrics(SM_CYVIRTUALSCREEN),
            )
        };

        if raw_width <= 0 || raw_height <= 0 {
            return Err(format!(
                "virtual desktop reported non-positive dimensions ({raw_width}x{raw_height})"
            ));
        }

        let width = raw_width as u32;
        let height = raw_height as u32;
        let top_down_height = raw_height
            .checked_neg()
            .ok_or_else(|| "virtual desktop height overflowed top-down DIB header".to_string())?;

        let byte_len = (width as usize)
            .checked_mul(height as usize)
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or_else(|| "virtual desktop dimensions overflow byte length".to_string())?;

        // SAFETY: Passing `None` retrieves the entire virtual screen device context.
        let raw_screen_dc = unsafe { GetDC(None) };
        if raw_screen_dc.is_invalid() {
            return Err("GetDC(None) returned an invalid screen device context".to_string());
        }
        let screen_dc = ScreenDcGuard(raw_screen_dc);

        // SAFETY: `screen_dc.0` is a valid screen DC obtained immediately above.
        let raw_mem_dc = unsafe { CreateCompatibleDC(Some(screen_dc.0)) };
        if raw_mem_dc.is_invalid() {
            return Err("CreateCompatibleDC failed for virtual screen capture".to_string());
        }
        let mem_dc = MemDcGuard(raw_mem_dc);

        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: raw_width,
                biHeight: top_down_height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut bits_ptr: *mut c_void = std::ptr::null_mut();
        // SAFETY: `screen_dc.0` is valid, `bmi` describes a top-down 32-bit BI_RGB DIB,
        // and `bits_ptr` receives the GDI-allocated pixel buffer valid while `dib` lives.
        let raw_hbmp = unsafe {
            CreateDIBSection(
                Some(screen_dc.0),
                &bmi,
                DIB_RGB_COLORS,
                &mut bits_ptr,
                None,
                0,
            )
        }
        .map_err(|error| format!("CreateDIBSection failed: {error}"))?;

        if raw_hbmp.is_invalid() || bits_ptr.is_null() {
            if !raw_hbmp.is_invalid() {
                // SAFETY: Clean up the handle if GDI returned a handle with a null buffer pointer.
                unsafe {
                    let _ = DeleteObject(HGDIOBJ(raw_hbmp.0));
                }
            }
            return Err(
                "CreateDIBSection returned an invalid bitmap or null pixel buffer".to_string(),
            );
        }
        let dib = BitmapGuard(raw_hbmp);

        {
            // SAFETY: Both `mem_dc.0` and `dib.0` are valid GDI handles of matching format.
            let prev_obj = unsafe { SelectObject(mem_dc.0, HGDIOBJ(dib.0 .0)) };
            if prev_obj.is_invalid() {
                return Err(
                    "SelectObject failed when selecting capture DIB into memory DC".to_string(),
                );
            }
            let _selected_guard = SelectedObjectGuard {
                hdc: mem_dc.0,
                previous: prev_obj,
            };

            // SAFETY: `mem_dc.0` has a `width x height` 32-bit DIB selected, and `screen_dc.0`
            // spans `[origin_x, origin_y, width, height]` on the virtual desktop.
            unsafe {
                BitBlt(
                    mem_dc.0,
                    0,
                    0,
                    raw_width,
                    raw_height,
                    Some(screen_dc.0),
                    origin_x,
                    origin_y,
                    SRCCOPY | CAPTUREBLT,
                )
            }
            .map_err(|error| format!("BitBlt virtual desktop capture failed: {error}"))?;

            // SAFETY: Flushes pending GDI operations so `bits_ptr` is readable on CPU.
            unsafe {
                let _ = GdiFlush();
            }
        }

        // SAFETY: `dib` is still alive and deselected, and GDI allocated `byte_len` readable
        // bytes at `bits_ptr` for this `width x height` 32-bit DIB section.
        let src_slice = unsafe { std::slice::from_raw_parts(bits_ptr.cast::<u8>(), byte_len) };
        let mut bgra = src_slice.to_vec();
        for chunk in bgra.as_chunks_mut::<4>().0 {
            chunk[3] = 0xFF;
        }

        Ok(Self {
            origin_x,
            origin_y,
            width,
            height,
            bgra,
        })
    }

    /// Read the RGB color `[r, g, b]` of a physical virtual-screen coordinate.
    pub fn pixel_rgb(&self, screen_x: i32, screen_y: i32) -> Option<[u8; 3]> {
        let rel_x = (screen_x as i64).checked_sub(self.origin_x as i64)?;
        let rel_y = (screen_y as i64).checked_sub(self.origin_y as i64)?;
        if rel_x < 0 || rel_y < 0 {
            return None;
        }
        let x = usize::try_from(rel_x).ok()?;
        let y = usize::try_from(rel_y).ok()?;
        let w = self.width as usize;
        let h = self.height as usize;
        if x >= w || y >= h {
            return None;
        }
        let idx = y.checked_mul(w)?.checked_add(x)?.checked_mul(4)?;
        let pixel = self.bgra.get(idx..idx.checked_add(4)?)?;
        Some([pixel[2], pixel[1], pixel[0]])
    }

    /// Crop a virtual-screen rectangle, clamping to the captured desktop bounds and
    /// converting top-down BGRA pixels into RGBA (`[r, g, b, 255]`).
    pub fn crop_rgba(
        &self,
        screen_x: i32,
        screen_y: i32,
        width: u32,
        height: u32,
    ) -> Option<(Vec<u8>, u32, u32)> {
        if width == 0 || height == 0 || self.width == 0 || self.height == 0 {
            return None;
        }

        let snap_left = self.origin_x as i64;
        let snap_top = self.origin_y as i64;
        let snap_right = snap_left + (self.width as i64);
        let snap_bottom = snap_top + (self.height as i64);

        let req_left = screen_x as i64;
        let req_top = screen_y as i64;
        let req_right = req_left + (width as i64);
        let req_bottom = req_top + (height as i64);

        let clamped_left = req_left.max(snap_left);
        let clamped_top = req_top.max(snap_top);
        let clamped_right = req_right.min(snap_right);
        let clamped_bottom = req_bottom.min(snap_bottom);

        if clamped_right <= clamped_left || clamped_bottom <= clamped_top {
            return None;
        }

        let out_w = u32::try_from(clamped_right - clamped_left).ok()?;
        let out_h = u32::try_from(clamped_bottom - clamped_top).ok()?;
        let start_x = usize::try_from(clamped_left - snap_left).ok()?;
        let start_y = usize::try_from(clamped_top - snap_top).ok()?;

        let src_w = self.width as usize;
        let out_w_usize = out_w as usize;
        let out_h_usize = out_h as usize;
        let out_len = out_w_usize.checked_mul(out_h_usize)?.checked_mul(4)?;
        let mut rgba = Vec::with_capacity(out_len);

        for row in 0..out_h_usize {
            let src_row = start_y.checked_add(row)?;
            let row_start = src_row
                .checked_mul(src_w)?
                .checked_add(start_x)?
                .checked_mul(4)?;
            let row_end = row_start.checked_add(out_w_usize.checked_mul(4)?)?;
            let src_bytes = self.bgra.get(row_start..row_end)?;
            for px in src_bytes.as_chunks::<4>().0 {
                rgba.extend_from_slice(&[px[2], px[1], px[0], 0xFF]);
            }
        }

        Some((rgba, out_w, out_h))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_negative_origin_snapshot() -> ScreenSnapshot {
        let pixels_rgb: [[u8; 3]; 6] = [
            [10, 20, 30],
            [40, 50, 60],
            [70, 80, 90],
            [11, 21, 31],
            [41, 51, 61],
            [71, 81, 91],
        ];
        let mut bgra = Vec::with_capacity(6 * 4);
        for [r, g, b] in pixels_rgb {
            bgra.extend_from_slice(&[b, g, r, 0xFF]);
        }
        ScreenSnapshot {
            origin_x: -1920,
            origin_y: -1080,
            width: 3,
            height: 2,
            bgra,
        }
    }

    #[test]
    fn pixel_rgb_supports_negative_multi_monitor_origins() {
        let snap = synthetic_negative_origin_snapshot();
        assert_eq!(snap.pixel_rgb(-1920, -1080), Some([10, 20, 30]));
        assert_eq!(snap.pixel_rgb(-1918, -1079), Some([71, 81, 91]));
        assert_eq!(snap.pixel_rgb(-1921, -1080), None);
        assert_eq!(snap.pixel_rgb(-1917, -1080), None);
        assert_eq!(snap.pixel_rgb(-1920, -1078), None);
        assert_eq!(snap.pixel_rgb(0, 0), None);
    }

    #[test]
    fn crop_rgba_clamps_partial_and_rejects_out_of_bounds_rects() {
        let snap = synthetic_negative_origin_snapshot();

        let (rgba, w, h) = snap
            .crop_rgba(-1922, -1081, 4, 2)
            .expect("overlapping rect should clamp");
        assert_eq!((w, h), (2, 1));
        assert_eq!(rgba, vec![10, 20, 30, 255, 40, 50, 60, 255]);

        let (rgba2, w2, h2) = snap
            .crop_rgba(-1919, -1079, u32::MAX, u32::MAX)
            .expect("clamped rect should succeed without overflow");
        assert_eq!((w2, h2), (2, 1));
        assert_eq!(rgba2, vec![41, 51, 61, 255, 71, 81, 91, 255]);

        assert!(snap.crop_rgba(-2000, -1080, 10, 2).is_none());
        assert!(snap.crop_rgba(-1917, -1080, 5, 2).is_none());
        assert!(snap.crop_rgba(-1920, -1078, 3, 5).is_none());
        assert!(snap.crop_rgba(-1920, -1080, 0, 2).is_none());
    }

    #[test]
    fn capture_virtual_desktop_smoke_test() {
        let snap = ScreenSnapshot::capture_virtual_desktop()
            .expect("virtual desktop capture should succeed on Windows desktop");
        assert!(snap.width > 0);
        assert!(snap.height > 0);
        let expected_bytes = (snap.width as usize) * (snap.height as usize) * 4;
        assert_eq!(snap.bgra.len(), expected_bytes);
        assert!(snap.bgra.as_chunks::<4>().0.iter().all(|px| px[3] == 0xFF));
        assert!(snap.pixel_rgb(snap.origin_x, snap.origin_y).is_some());
    }
}
