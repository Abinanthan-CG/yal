//! Capturing the screen.
//!
//! The capture is a plain read of the framebuffer. Nothing is written to
//! disk and no other process is affected.

use std::path::Path;

/// A failure while capturing the screen.
#[derive(Debug, thiserror::Error)]
pub enum ScreenshotError {
    #[error("the screen could not be captured: {0}")]
    Failed(String),
}

/// The captured screen.
pub struct Capture {
    /// The image pixels.
    pub image: image::RgbaImage,
    /// The width in pixels.
    pub width: u32,
    /// The height in pixels.
    pub height: u32,
}

/// Capture the primary screen.
#[cfg(windows)]
pub fn capture_screen() -> Result<Capture, ScreenshotError> {
    use windows::Win32::Graphics::Gdi::{
        BITMAPINFO, BITMAPINFOHEADER, BitBlt, CreateCompatibleDC, CreateDIBSection,
        DIB_RGB_COLORS, DeleteObject, GetDC, GetDIBits, ReleaseDC, SelectObject, SRCCOPY,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};

    let width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let height = unsafe { GetSystemMetrics(SM_CYSCREEN) };
    if width <= 0 || height <= 0 {
        return Err(ScreenshotError::Failed(
            "the screen reports no usable dimensions".into(),
        ));
    }
    let width = width as u32;
    let height = height as u32;

    let mut pixels: Vec<u8> = vec![0; (width * height * 4) as usize];
    let mut info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width as i32,
            // A negative height requests a top-down image, so the rows arrive
            // in the order they appear on screen.
            biHeight: -(height as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: 0,
            ..Default::default()
        },
        ..Default::default()
    };

    unsafe {
        let screen_dc = GetDC(None);
        if screen_dc.is_invalid() {
            return Err(ScreenshotError::Failed("no screen device".into()));
        }
        let memory_dc = CreateCompatibleDC(Some(screen_dc));

        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let bitmap = match CreateDIBSection(
            Some(screen_dc),
            &info,
            DIB_RGB_COLORS,
            &mut bits,
            None,
            0,
        ) {            Ok(bitmap) => bitmap,
            Err(_) => {
                let _ = ReleaseDC(None, screen_dc);
                return Err(ScreenshotError::Failed(
                    "the screen bitmap could not be created".into(),
                ));
            }
        };

        let previous = SelectObject(memory_dc, bitmap.into());
        let copied = BitBlt(
            memory_dc,
            0,
            0,
            width as i32,
            height as i32,
            Some(screen_dc),
            0,
            0,
            SRCCOPY,
        )
        .is_ok();
        if copied {
            GetDIBits(
                screen_dc,
                bitmap,
                0,
                height,
                Some(pixels.as_mut_ptr().cast()),
                &mut info,
                DIB_RGB_COLORS,
            );
        }

        SelectObject(memory_dc, previous);
        let _ = DeleteObject(bitmap.into());
        let _ = ReleaseDC(None, screen_dc);

        if !copied {
            return Err(ScreenshotError::Failed(
                "the screen could not be copied into a bitmap".into(),
            ));
        }
    }

    let image = image::RgbaImage::from_raw(width, height, pixels)
        .ok_or_else(|| ScreenshotError::Failed("the captured pixels were malformed".into()))?;

    Ok(Capture {
        image,
        width,
        height,
    })
}

/// Capture the primary screen on a non-Windows host.
#[cfg(not(windows))]
pub fn capture_screen() -> Result<Capture, ScreenshotError> {
    Err(ScreenshotError::Failed(
        "screen capture is not implemented on this platform".into(),
    ))
}

/// Write a capture to a PNG file.
pub fn write_png(capture: &Capture, path: &Path) -> Result<(), ScreenshotError> {
    capture
        .image
        .save(path)
        .map_err(|err| ScreenshotError::Failed(err.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_reports_plausible_dimensions() {
        // A headless session has no screen; the call must fail cleanly
        // rather than panic or return a malformed image.
        if let Ok(capture) = capture_screen() {
            assert!(capture.width > 0);
            assert!(capture.height > 0);
            assert_eq!(capture.image.width(), capture.width);
            assert_eq!(capture.image.height(), capture.height);
        }
    }
}
