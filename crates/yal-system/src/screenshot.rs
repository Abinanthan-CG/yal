//! Capturing the screen.
//!
//! The capture is a plain read of the framebuffer. Nothing is written to
//! disk and no other process is affected.

use std::path::{Path, PathBuf};

/// A failure while capturing the screen.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ScreenshotError {
    #[error("the screen could not be captured: {0}")]
    Failed(String),
    #[error("the requested region is not part of the screen")]
    OffScreen,
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

/// The width and height, in pixels, of the primary screen.
#[cfg(windows)]
pub fn screen_extent() -> (u32, u32) {
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
    let width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let height = unsafe { GetSystemMetrics(SM_CYSCREEN) };
    (width.max(0) as u32, height.max(0) as u32)
}

/// The width and height, in pixels, of the primary screen on a non-Windows host.
#[cfg(not(windows))]
pub fn screen_extent() -> (u32, u32) {
    (0, 0)
}

/// Trim a region so it sits entirely within the screen.
///
/// The clamp keeps a wrong or hostile region request from handing the
/// bitmap allocator a size the screen can never have carried. A region
/// with nothing left after the clamping is an off-screen request.
pub fn clamp_region(
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    screen: (u32, u32),
) -> Option<(i32, i32, u32, u32)> {
    let (screen_w, screen_h) = (i64::from(screen.0), i64::from(screen.1));
    let mut x = i64::from(x);
    let mut y = i64::from(y);
    let mut width = i64::from(width);
    let mut height = i64::from(height);

    if width <= 0 || height <= 0 {
        return None;
    }
    // Pull the origin onto the screen, then pull the size back inside it.
    if x < 0 {
        width += x;
        x = 0;
    }
    if y < 0 {
        height += y;
        y = 0;
    }
    width = width.min(screen_w.saturating_sub(x));
    height = height.min(screen_h.saturating_sub(y));
    if width <= 0 || height <= 0 {
        return None;
    }
    Some((x as i32, y as i32, width as u32, height as u32))
}

/// The pixels of a framebuffer DIB, reordered for `image`.
///
/// The screen bitmap arrives as B, G, R, A bytes; the image crate expects
/// R, G, B, A. The channels are swizzled in place and the alpha forced
/// opaque, because a screen read's alpha is whatever the driver happened
/// to leave there — `zoomy` pixels came out honest, transparency would
/// not be.
fn swizzle_buffer(bytes: &mut [u8]) {
    for pixel in bytes.chunks_exact_mut(4) {
        pixel.swap(0, 2);
        pixel[3] = 255;
    }
}

/// Read a region of the primary screen into an RGBA image.
#[cfg(windows)]
fn read_region(x: i32, y: i32, width: u32, height: u32) -> Result<Capture, ScreenshotError> {
    use windows::Win32::Graphics::Gdi::{
        BITMAPINFO, BITMAPINFOHEADER, BitBlt, CreateCompatibleDC, CreateDIBSection,
        DIB_RGB_COLORS, DeleteObject, GetDC, GetDIBits, ReleaseDC, SelectObject, SRCCOPY,
    };

    if width == 0 || height == 0 {
        return Err(ScreenshotError::OffScreen);
    }

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
            x,
            y,
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

    swizzle_buffer(&mut pixels);
    let image = image::RgbaImage::from_raw(width, height, pixels)
        .ok_or_else(|| ScreenshotError::Failed("the captured pixels were malformed".into()))?;

    Ok(Capture {
        image,
        width,
        height,
    })
}

/// Read a region of the screen on a non-Windows host.
#[cfg(not(windows))]
fn read_region(_x: i32, _y: i32, _width: u32, _height: u32) -> Result<Capture, ScreenshotError> {
    Err(ScreenshotError::Failed(
        "screen capture is not implemented on this platform".into(),
    ))
}

/// Capture a region of the primary screen.
///
/// A region poking off the screen is clamped to the screen; a region with
/// nothing left after that is refused.
pub fn capture_region(x: i32, y: i32, width: u32, height: u32) -> Result<Capture, ScreenshotError> {
    let region = clamp_region(x, y, width, height, screen_extent())
        .ok_or(ScreenshotError::OffScreen)?;
    read_region(region.0, region.1, region.2, region.3)
}

/// Capture the primary screen.
pub fn capture_screen() -> Result<Capture, ScreenshotError> {
    let (width, height) = screen_extent();
    read_region(0, 0, width, height)
}

/// Capture one window's own surface, decorations and all.
///
/// `PrintWindow` reads the window's *own* picture, so it works even where
/// the window sits behind others; `PW_RENDERFULLCONTENT` asks the newer
/// compositor content along for DirectComposition windows that plain
/// `PrintWindow` leaves blank.
#[cfg(windows)]
pub fn capture_window(handle: u64) -> Result<Capture, ScreenshotError> {
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::Graphics::Gdi::{
        BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS,
        DeleteObject, GetDIBits, GetWindowDC, ReleaseDC, SelectObject,
    };
    use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;
    // In windows 0.62 `PrintWindow` is filed under Xps, not windowing —
    // the same user32 export either way — and its flag type lives with it.
    // `PW_RENDERFULLCONTENT` (2) is the compositor-content content flag.
    use windows::Win32::Storage::Xps::{PrintWindow as WinPrintWindow, PRINT_WINDOW_FLAGS};
    use windows::Win32::UI::WindowsAndMessaging::PW_RENDERFULLCONTENT;

    let window = HWND(handle as *mut core::ffi::c_void);
    let mut bounds = RECT::default();
    unsafe { GetWindowRect(window, &mut bounds) }
        .map_err(|_| ScreenshotError::Failed("the window is gone".into()))?;
    let width = (bounds.right - bounds.left).max(1) as u32;
    let height = (bounds.bottom - bounds.top).max(1) as u32;

    let mut pixels: Vec<u8> = vec![0; (width * height * 4) as usize];
    let mut info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width as i32,
            biHeight: -(height as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: 0,
            ..Default::default()
        },
        ..Default::default()
    };

    unsafe {
        let window_dc = GetWindowDC(Some(window));
        if window_dc.is_invalid() {
            return Err(ScreenshotError::Failed("no window device".into()));
        }
        let memory_dc = CreateCompatibleDC(Some(window_dc));

        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let bitmap = match CreateDIBSection(
            Some(window_dc),
            &info,
            DIB_RGB_COLORS,
            &mut bits,
            None,
            0,
        ) {
            Ok(bitmap) => bitmap,
            Err(_) => {
                let _ = ReleaseDC(Some(window), window_dc);
                return Err(ScreenshotError::Failed(
                    "the window bitmap could not be created".into(),
                ));
            }
        };

        let previous = SelectObject(memory_dc, bitmap.into());
        let printed = WinPrintWindow(
            window,
            memory_dc,
            PRINT_WINDOW_FLAGS(PW_RENDERFULLCONTENT),
        )
        .as_bool();
        if printed {
            GetDIBits(
                window_dc,
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
        let _ = ReleaseDC(Some(window), window_dc);

        if !printed {
            return Err(ScreenshotError::Failed(
                "the window refused to be printed".into(),
            ));
        }
    }

    swizzle_buffer(&mut pixels);
    let image = image::RgbaImage::from_raw(width, height, pixels)
        .ok_or_else(|| ScreenshotError::Failed("the captured pixels were malformed".into()))?;

    Ok(Capture {
        image,
        width,
        height,
    })
}

/// Capture a window on a non-Windows host.
#[cfg(not(windows))]
pub fn capture_window(_handle: u64) -> Result<Capture, ScreenshotError> {
    Err(ScreenshotError::Failed(
        "window capture is not implemented on this platform".into(),
    ))
}

/// Where a capture is written when nobody said otherwise.
///
/// The user's Pictures folder, under the launcher's own name, with the
/// time in the file name so one drag after another cannot overwrite the
/// last one. A machine without a Pictures folder puts the captures in the
/// working directory's place — worse, but never silent and never failed.
pub fn default_capture_path() -> PathBuf {
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S%3f");
    let folder = dirs::picture_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
        .join("YAL");
    if let Err(error) = std::fs::create_dir_all(&folder) {
        tracing::warn!("the capture folder could not be made: {error}");
        return std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(format!("capture-{stamp}.png"));
    }
    folder.join(format!("capture-{stamp}.png"))
}

/// Parse the `x,y,w,h` text of a region argument.
///
/// Four comma-separated numbers, negative origins allowed: a quick,
/// honest parse that names the part it could not read.
pub fn parse_region(text: &str) -> Result<(i32, i32, u32, u32), ScreenshotError> {
    let numbers: Vec<&str> = text.split(',').map(str::trim).collect();
    if numbers.len() != 4 {
        return Err(ScreenshotError::Failed(format!(
            "a region is four numbers, `x,y,w,h` — got `{text}`"
        )));
    }
    let mut parsed = [0_i32; 4];
    for (slot, part) in parsed.iter_mut().zip(numbers.iter()) {
        *slot = part.parse::<i32>().map_err(|_| {
            ScreenshotError::Failed(format!("`{part}` is not a number in a region"))
        })?;
    }
    if parsed[2] <= 0 || parsed[3] <= 0 {
        return Err(ScreenshotError::Failed(
            "the width and height of a region must be positive".into(),
        ));
    }
    Ok((
        parsed[0],
        parsed[1],
        parsed[2] as u32,
        parsed[3] as u32,
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

    #[test]
    fn regions_are_clamped_to_the_screen() {
        // A region hanging off the bottom-right comes back shortened; a
        // region entirely off it is refused.
        assert_eq!(clamp_region(-10, -10, 50, 50, (1000, 1000)), Some((0, 0, 40, 40)));
        assert_eq!(
            clamp_region(800, 900, 400, 400, (1000, 1000)),
            Some((800, 900, 200, 100))
        );
        assert_eq!(clamp_region(2000, 0, 50, 50, (1000, 1000)), None);
        assert_eq!(clamp_region(0, 0, 0, 50, (1000, 1000)), None);
        // A fit exactly at the screen edge survives untouched.
        assert_eq!(clamp_region(0, 0, 1000, 1000, (1000, 1000)), Some((0, 0, 1000, 1000)));
    }

    #[test]
    fn the_framebuffer_swizzles_to_rgba() {
        // One BGRA pixel: blue hot. After the swizzle the red channel
        // carries it, and the arrived-at alpha is forced opaque.
        let mut frame = vec![255, 0, 0, 0];
        swizzle_buffer(&mut frame);
        assert_eq!(frame, vec![0, 0, 255, 255]);
    }

    #[test]
    fn when_nobody_names_a_capture_it_lands_in_pictures() {
        let path = default_capture_path();
        assert_eq!(path.extension().and_then(|ext| ext.to_str()), Some("png"));
        assert!(
            path.to_string_lossy().contains("YAL"),
            "the captures have their own folder: {path:?}"
        );
        if let Some(pictures) = dirs::picture_dir() {
            assert!(path.starts_with(pictures), "default captures live in Pictures: {path:?}");
        }
        // The name carries the moment, so two captures strung together
        // cannot quietly overwrite one another.
        let again = default_capture_path();
        assert_ne!(path, again, "the stamp distinguishes captures");
    }

    #[test]
    fn region_text_parses_fours_with_room_to_negotiate() {
        assert_eq!(parse_region("0, 0, 800, 600"), Ok((0, 0, 800, 600)));
        assert_eq!(parse_region("-40,-10,120,90"), Ok((-40, -10, 120, 90)));
        assert!(
            parse_region("0,0,800").is_err(),
            "three numbers are not a region"
        );
        assert!(
            parse_region("a,b,c,d").is_err(),
            "words are not numbers in a region"
        );
        assert!(
            parse_region("0,0,0,600").is_err(),
            "an empty region is not a capture"
        );
    }
}
