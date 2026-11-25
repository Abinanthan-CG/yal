use std::ptr::NonNull;
use std::sync::{Mutex, MutexGuard, OnceLock};

use tc_platform::Clipboard;
use windows::core::w;
use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL, POINT};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::Memory::{
    GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE, GMEM_ZEROINIT,
};
use windows::Win32::System::Ole::{CF_DIB, CF_HDROP, CF_UNICODETEXT};
use windows::Win32::UI::Shell::{DragQueryFileW, DROPFILES, HDROP};

const MAX_DIB_DIMENSION: u32 = 4096;
const BITMAPINFOHEADER_SIZE: usize = 40;
const BI_RGB: u32 = 0;
const BI_BITFIELDS: u32 = 3;

/// Process-wide lock serializing Win32 `OpenClipboard(None)` calls across threads
/// of this process (Win32 `OpenClipboard(NULL)` tracks ownership per-process,
/// so concurrent threads could otherwise `EmptyClipboard` while another thread
/// reads a `GetClipboardData` handle).
static CLIPBOARD_PROCESS_LOCK: Mutex<()> = Mutex::new(());

/// RAII guard for an open Win32 clipboard session (`OpenClipboard` / `CloseClipboard`).
struct ClipboardGuard {
    _process_lock: MutexGuard<'static, ()>,
}

impl ClipboardGuard {
    fn open() -> Result<Self, String> {
        let process_lock = CLIPBOARD_PROCESS_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // SAFETY: Passing `None` associates the open clipboard with the current task,
        // serialized within this process by `CLIPBOARD_PROCESS_LOCK`.
        // `CloseClipboard` is guaranteed to run on `Drop` if this succeeds.
        unsafe { OpenClipboard(None) }
            .map(|()| Self {
                _process_lock: process_lock,
            })
            .map_err(|_| "the clipboard is in use by another program".to_string())
    }
}

impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        // SAFETY: `ClipboardGuard` is only constructed after a successful `OpenClipboard`,
        // and `_process_lock` is dropped after this `Drop` body finishes.
        unsafe {
            let _ = CloseClipboard();
        }
    }
}

/// RAII guard for a locked Win32 global memory handle (`GlobalLock` / `GlobalUnlock`).
struct GlobalLockGuard {
    handle: HGLOBAL,
    ptr: NonNull<u8>,
}

impl GlobalLockGuard {
    fn lock(handle: HGLOBAL) -> Option<Self> {
        // SAFETY: Caller supplies a valid `HGLOBAL` from `GlobalAlloc` or `GetClipboardData`.
        // `GlobalUnlock` is guaranteed to run on `Drop` if the pointer is non-null.
        let raw = unsafe { GlobalLock(handle) } as *mut u8;
        let ptr = NonNull::new(raw)?;
        Some(Self { handle, ptr })
    }

    fn as_ptr(&self) -> *const u8 {
        self.ptr.as_ptr()
    }

    fn as_mut_ptr(&self) -> *mut u8 {
        self.ptr.as_ptr()
    }

    /// Return a byte slice view of `len` bytes from the locked memory block.
    ///
    /// # Safety
    /// `len` must not exceed the initialized size of the underlying `HGLOBAL` allocation.
    unsafe fn as_slice(&self, len: usize) -> &[u8] {
        // SAFETY: `self.ptr` is non-null and remains locked for the lifetime of `&self`,
        // and the caller guarantees `len` bytes are valid and initialized.
        unsafe { std::slice::from_raw_parts(self.ptr.as_ptr(), len) }
    }
}

impl Drop for GlobalLockGuard {
    fn drop(&mut self) {
        // SAFETY: `self.handle` was successfully locked in `GlobalLockGuard::lock`.
        unsafe {
            let _ = GlobalUnlock(self.handle);
        }
    }
}

/// RAII owner for a freshly allocated `HGLOBAL` prior to transferring ownership
/// to the clipboard via `SetClipboardData`.
struct OwnedGlobal(Option<HGLOBAL>);

impl OwnedGlobal {
    fn new(handle: HGLOBAL) -> Self {
        Self(Some(handle))
    }

    /// Transfer ownership to the Windows clipboard so `Drop` will not free it.
    fn into_clipboard(mut self) {
        self.0 = None;
    }
}

impl Drop for OwnedGlobal {
    fn drop(&mut self) {
        if let Some(handle) = self.0.take() {
            // SAFETY: Ownership was not transferred to the clipboard, and any
            // `GlobalLockGuard` on this handle was dropped before `OwnedGlobal`.
            unsafe {
                let _ = GlobalFree(Some(handle));
            }
        }
    }
}

/// The Windows clipboard, supporting Unicode text, `CF_HDROP` file lists, and `CF_DIB` images.
#[derive(Debug, Default)]
pub struct WindowsClipboard;

impl WindowsClipboard {
    pub fn new() -> WindowsClipboard {
        WindowsClipboard
    }

    /// Read copied file paths (`CF_HDROP`) from the clipboard, if any.
    pub fn read_files(&self) -> Option<Vec<String>> {
        let _clip = ClipboardGuard::open().ok()?;
        read_clipboard_files()
    }

    /// Place one or more file/folder paths onto the clipboard as `CF_HDROP` so
    /// the user can paste them directly into Explorer, browsers, or chat apps.
    pub fn write_files(&self, paths: &[String]) -> Result<(), String> {
        if paths.is_empty() {
            return Err("no files to copy".to_string());
        }
        let _clip = ClipboardGuard::open()?;
        write_clipboard_files(paths)
    }

    /// Read a `CF_DIB` bitmap from the clipboard as straight (unassociated) top-down RGBA.
    pub fn read_dib_rgba(&self) -> Option<(u32, u32, Vec<u8>)> {
        let _clip = ClipboardGuard::open().ok()?;
        // SAFETY: Clipboard is open via `_clip`; `GetClipboardData` returns a
        // clipboard-owned handle valid until `CloseClipboard`.
        let handle = unsafe { GetClipboardData(CF_DIB.0 as u32) }.ok()?;
        let hglobal = HGLOBAL(handle.0);
        // SAFETY: `hglobal` is a valid global memory handle returned by `GetClipboardData`.
        let size = unsafe { GlobalSize(hglobal) };
        if size < BITMAPINFOHEADER_SIZE {
            return None;
        }
        let lock = GlobalLockGuard::lock(hglobal)?;
        // SAFETY: `GlobalSize` reported `size` bytes available for the locked `CF_DIB` block.
        let bytes = unsafe { lock.as_slice(size) };
        parse_dib_rgba(bytes)
    }

    /// Write straight (unassociated) top-down RGBA pixels to the clipboard as a 32-bit `CF_DIB`.
    pub fn write_dib_rgba(&self, width: u32, height: u32, rgba: &[u8]) -> Result<(), String> {
        let dib = encode_dib_rgba(width, height, rgba)?;
        let _clip = ClipboardGuard::open()?;
        // SAFETY: Clipboard is open via `_clip`; emptying stale formats before setting data.
        unsafe {
            let _ = EmptyClipboard();
        }
        // SAFETY: `dib.len()` is non-zero (at least 44 bytes) and `GMEM_MOVEABLE` is
        // required by `SetClipboardData`.
        let raw_handle =
            unsafe { GlobalAlloc(GMEM_MOVEABLE, dib.len()) }.map_err(|error| error.to_string())?;
        let owned = OwnedGlobal::new(raw_handle);
        {
            let lock = GlobalLockGuard::lock(raw_handle)
                .ok_or_else(|| "the clipboard could not be locked".to_string())?;
            // SAFETY: `raw_handle` was allocated with `dib.len()` bytes, `lock` holds an
            // exclusive valid pointer to it, and `dib` does not overlap with global memory.
            unsafe {
                std::ptr::copy_nonoverlapping(dib.as_ptr(), lock.as_mut_ptr(), dib.len());
            }
        }
        // Lock is dropped before `SetClipboardData`.
        // SAFETY: Clipboard is open, `raw_handle` is unlocked `GMEM_MOVEABLE` memory
        // containing a valid `CF_DIB` payload.
        if let Err(error) = unsafe { SetClipboardData(CF_DIB.0 as u32, Some(HANDLE(raw_handle.0))) }
        {
            return Err(format!("the clipboard image could not be written: {error}"));
        }
        owned.into_clipboard();
        Ok(())
    }
}

impl Clipboard for WindowsClipboard {
    fn read_text(&self) -> Option<String> {
        let _clip = ClipboardGuard::open().ok()?;
        read_clipboard_text()
    }

    fn write_text(&self, text: &str) -> Result<(), String> {
        let _clip = ClipboardGuard::open()?;
        write_clipboard_text(text)
    }

    fn available_formats(&self) -> Vec<String> {
        Vec::new()
    }
}

/// Returns `true` when the current clipboard payload carries Windows privacy
/// hints from a password manager or security tool indicating it must not be
/// recorded in clipboard history.
pub fn is_sensitive_clipboard() -> bool {
    static EXCLUDE_FORMAT: OnceLock<u32> = OnceLock::new();
    static HISTORY_FORMAT: OnceLock<u32> = OnceLock::new();

    let exclude_fmt = *EXCLUDE_FORMAT.get_or_init(|| {
        // SAFETY: `w!(...)` produces a valid null-terminated UTF-16 string literal.
        unsafe { RegisterClipboardFormatW(w!("ExcludeClipboardContentFromMonitorProcessing")) }
    });
    if exclude_fmt != 0 {
        // SAFETY: `exclude_fmt` is a valid registered clipboard format identifier.
        if unsafe { IsClipboardFormatAvailable(exclude_fmt) }.is_ok() {
            return true;
        }
    }

    let history_fmt = *HISTORY_FORMAT.get_or_init(|| {
        // SAFETY: `w!(...)` produces a valid null-terminated UTF-16 string literal.
        unsafe { RegisterClipboardFormatW(w!("CanIncludeInClipboardHistory")) }
    });
    if history_fmt != 0 {
        // SAFETY: `history_fmt` is a valid registered clipboard format identifier.
        if unsafe { IsClipboardFormatAvailable(history_fmt) }.is_ok() {
            if let Ok(_clip) = ClipboardGuard::open() {
                // SAFETY: Clipboard is open via `_clip` and `history_fmt` is valid.
                if let Ok(handle) = unsafe { GetClipboardData(history_fmt) } {
                    let hglobal = HGLOBAL(handle.0);
                    // SAFETY: `hglobal` is a valid handle returned by `GetClipboardData`.
                    if unsafe { GlobalSize(hglobal) } >= 4 {
                        if let Some(lock) = GlobalLockGuard::lock(hglobal) {
                            // SAFETY: `GlobalSize` confirmed at least 4 bytes are available.
                            let bytes = unsafe { lock.as_slice(4) };
                            let flag = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                            if flag == 0 {
                                return true;
                            }
                        }
                    }
                }
            }
        }
    }

    false
}

/// Parse a Windows `CF_DIB` byte buffer into straight (unassociated) top-down RGBA pixels.
pub fn parse_dib_rgba(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    if bytes.len() < BITMAPINFOHEADER_SIZE {
        return None;
    }

    let bi_size = read_u32_le(bytes, 0)?;
    let bi_width = read_i32_le(bytes, 4)?;
    let bi_height = read_i32_le(bytes, 8)?;
    let bi_planes = read_u16_le(bytes, 12)?;
    let bi_bit_count = read_u16_le(bytes, 14)?;
    let bi_compression = read_u32_le(bytes, 16)?;
    let bi_clr_used = read_u32_le(bytes, 32)?;

    let header_len = usize::try_from(bi_size).ok()?;
    if header_len < BITMAPINFOHEADER_SIZE || header_len > bytes.len() {
        return None;
    }
    if bi_planes != 1 || bi_width <= 0 || bi_height == 0 || bi_height == i32::MIN {
        return None;
    }

    let width = u32::try_from(bi_width).ok()?;
    let height = bi_height.unsigned_abs();
    let top_down = bi_height < 0;
    if width > MAX_DIB_DIMENSION || height > MAX_DIB_DIMENSION {
        return None;
    }

    let bpp: usize = match (bi_compression, bi_bit_count) {
        (BI_RGB, 24) => 3,
        (BI_RGB, 32) | (BI_BITFIELDS, 32) => 4,
        _ => return None,
    };

    let (pixel_offset, bitfields) = if bi_compression == BI_RGB {
        let palette_bytes = usize::try_from(bi_clr_used).ok()?.checked_mul(4)?;
        let offset = header_len.checked_add(palette_bytes)?;
        (offset, None)
    } else {
        let (masks_offset, base_offset, alpha_mask) = if header_len == BITMAPINFOHEADER_SIZE {
            let after_masks = BITMAPINFOHEADER_SIZE.checked_add(12)?;
            (BITMAPINFOHEADER_SIZE, after_masks, 0u32)
        } else if header_len >= 52 {
            let a_mask = if header_len >= 56 {
                read_u32_le(bytes, 52)?
            } else {
                0u32
            };
            let palette_bytes = usize::try_from(bi_clr_used).ok()?.checked_mul(4)?;
            let offset = header_len.checked_add(palette_bytes)?;
            (BITMAPINFOHEADER_SIZE, offset, a_mask)
        } else {
            return None;
        };

        let r_mask = read_u32_le(bytes, masks_offset)?;
        let g_mask = read_u32_le(bytes, masks_offset.checked_add(4)?)?;
        let b_mask = read_u32_le(bytes, masks_offset.checked_add(8)?)?;
        if r_mask == 0
            || g_mask == 0
            || b_mask == 0
            || (r_mask & g_mask) != 0
            || (r_mask & b_mask) != 0
            || (g_mask & b_mask) != 0
        {
            return None;
        }
        (base_offset, Some((r_mask, g_mask, b_mask, alpha_mask)))
    };

    let w = width as usize;
    let h = height as usize;
    let row_bytes = w.checked_mul(bpp)?;
    let stride = row_bytes.checked_add(3)? & !3usize;
    let pixel_bytes = stride.checked_mul(h)?;
    let needed = pixel_offset.checked_add(pixel_bytes)?;
    if bytes.len() < needed {
        return None;
    }

    let out_len = w.checked_mul(h)?.checked_mul(4)?;
    let mut out = vec![0u8; out_len];

    match (bi_compression, bi_bit_count) {
        (BI_RGB, 24) => {
            for y in 0..h {
                let src_y = if top_down { y } else { h - 1 - y };
                let src_row = pixel_offset + src_y * stride;
                let dst_row = y * w * 4;
                for x in 0..w {
                    let s = src_row + x * 3;
                    let d = dst_row + x * 4;
                    out[d] = bytes[s + 2];
                    out[d + 1] = bytes[s + 1];
                    out[d + 2] = bytes[s];
                    out[d + 3] = 255;
                }
            }
        }
        (BI_RGB, 32) => {
            let mut all_alpha_zero = true;
            'scan: for y in 0..h {
                let src_row = pixel_offset + y * stride;
                for x in 0..w {
                    if bytes[src_row + x * 4 + 3] != 0 {
                        all_alpha_zero = false;
                        break 'scan;
                    }
                }
            }
            for y in 0..h {
                let src_y = if top_down { y } else { h - 1 - y };
                let src_row = pixel_offset + src_y * stride;
                let dst_row = y * w * 4;
                for x in 0..w {
                    let s = src_row + x * 4;
                    let d = dst_row + x * 4;
                    let b = bytes[s];
                    let g = bytes[s + 1];
                    let r = bytes[s + 2];
                    if all_alpha_zero {
                        out[d] = r;
                        out[d + 1] = g;
                        out[d + 2] = b;
                        out[d + 3] = 255;
                    } else {
                        out[d] = r;
                        out[d + 1] = g;
                        out[d + 2] = b;
                        out[d + 3] = bytes[s + 3];
                    }
                }
            }
        }
        (BI_BITFIELDS, 32) => {
            let (r_mask, g_mask, b_mask, a_mask) = bitfields?;
            let all_alpha_zero = if a_mask == 0 {
                true
            } else {
                let mut zero = true;
                'scan_bf: for y in 0..h {
                    let src_row = pixel_offset + y * stride;
                    for x in 0..w {
                        let s = src_row + x * 4;
                        let pixel = u32::from_le_bytes([
                            bytes[s],
                            bytes[s + 1],
                            bytes[s + 2],
                            bytes[s + 3],
                        ]);
                        if (pixel & a_mask) != 0 {
                            zero = false;
                            break 'scan_bf;
                        }
                    }
                }
                zero
            };
            for y in 0..h {
                let src_y = if top_down { y } else { h - 1 - y };
                let src_row = pixel_offset + src_y * stride;
                let dst_row = y * w * 4;
                for x in 0..w {
                    let s = src_row + x * 4;
                    let d = dst_row + x * 4;
                    let pixel =
                        u32::from_le_bytes([bytes[s], bytes[s + 1], bytes[s + 2], bytes[s + 3]]);
                    let r = extract_masked_channel(pixel, r_mask);
                    let g = extract_masked_channel(pixel, g_mask);
                    let b = extract_masked_channel(pixel, b_mask);
                    if all_alpha_zero {
                        out[d] = r;
                        out[d + 1] = g;
                        out[d + 2] = b;
                        out[d + 3] = 255;
                    } else {
                        out[d] = r;
                        out[d + 1] = g;
                        out[d + 2] = b;
                        out[d + 3] = extract_masked_channel(pixel, a_mask);
                    }
                }
            }
        }
        _ => return None,
    }

    Some((width, height, out))
}

/// Encode straight (unassociated) top-down RGBA pixels into a 32-bit top-down `CF_DIB` payload.
pub fn encode_dib_rgba(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    if width == 0 || height == 0 || width > MAX_DIB_DIMENSION || height > MAX_DIB_DIMENSION {
        return Err(format!("invalid DIB dimensions: {width}x{height}"));
    }
    let pixel_count = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| "DIB pixel count overflow".to_string())?;
    let expected_bytes = pixel_count
        .checked_mul(4)
        .ok_or_else(|| "DIB byte size overflow".to_string())?;
    if rgba.len() != expected_bytes {
        return Err(format!(
            "DIB RGBA length mismatch: expected {expected_bytes}, got {}",
            rgba.len()
        ));
    }
    let image_size =
        u32::try_from(expected_bytes).map_err(|_| "DIB image size exceeds u32".to_string())?;
    let neg_height = i32::try_from(height)
        .ok()
        .and_then(i32::checked_neg)
        .ok_or_else(|| "DIB height overflow".to_string())?;

    let total_len = BITMAPINFOHEADER_SIZE
        .checked_add(expected_bytes)
        .ok_or_else(|| "DIB total length overflow".to_string())?;
    let mut out = Vec::with_capacity(total_len);

    // 40-byte BITMAPINFOHEADER (little-endian)
    out.extend_from_slice(&(BITMAPINFOHEADER_SIZE as u32).to_le_bytes()); // biSize
    out.extend_from_slice(&(width as i32).to_le_bytes()); // biWidth
    out.extend_from_slice(&neg_height.to_le_bytes()); // biHeight (negative = top-down)
    out.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
    out.extend_from_slice(&32u16.to_le_bytes()); // biBitCount
    out.extend_from_slice(&BI_RGB.to_le_bytes()); // biCompression
    out.extend_from_slice(&image_size.to_le_bytes()); // biSizeImage
    out.extend_from_slice(&0i32.to_le_bytes()); // biXPelsPerMeter
    out.extend_from_slice(&0i32.to_le_bytes()); // biYPelsPerMeter
    out.extend_from_slice(&0u32.to_le_bytes()); // biClrUsed
    out.extend_from_slice(&0u32.to_le_bytes()); // biClrImportant

    for px in rgba.as_chunks::<4>().0 {
        let r = px[0];
        let g = px[1];
        let b = px[2];
        let a = px[3];
        out.extend_from_slice(&[b, g, r, a]);
    }

    Ok(out)
}

#[inline]
fn read_u16_le(bytes: &[u8], offset: usize) -> Option<u16> {
    let end = offset.checked_add(2)?;
    let slice = bytes.get(offset..end)?;
    Some(u16::from_le_bytes([slice[0], slice[1]]))
}

#[inline]
fn read_u32_le(bytes: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    let slice = bytes.get(offset..end)?;
    Some(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

#[inline]
fn read_i32_le(bytes: &[u8], offset: usize) -> Option<i32> {
    let end = offset.checked_add(4)?;
    let slice = bytes.get(offset..end)?;
    Some(i32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

#[inline]
fn extract_masked_channel(pixel: u32, mask: u32) -> u8 {
    if mask == 0 {
        return 0;
    }
    let shift = mask.trailing_zeros();
    let max = mask >> shift;
    if max == 0 {
        return 0;
    }
    let val = (pixel & mask) >> shift;
    if max == 255 {
        val as u8
    } else {
        ((u64::from(val) * 255 + u64::from(max) / 2) / u64::from(max)).min(255) as u8
    }
}

/// Read the clipboard's Unicode text from an open clipboard.
fn read_clipboard_text() -> Option<String> {
    // SAFETY: Caller holds an open `ClipboardGuard`; handle stays valid until close.
    let handle = unsafe { GetClipboardData(CF_UNICODETEXT.0 as u32) }.ok()?;
    let hglobal = HGLOBAL(handle.0);
    // SAFETY: `hglobal` is a valid clipboard global memory handle while the clipboard is open.
    let byte_size = unsafe { GlobalSize(hglobal) };
    if byte_size < std::mem::size_of::<u16>() {
        return None;
    }
    let lock = GlobalLockGuard::lock(hglobal)?;
    // SAFETY: `lock` keeps `hglobal` locked and valid for `byte_size` bytes.
    let raw_bytes = unsafe { std::slice::from_raw_parts(lock.as_ptr(), byte_size) };
    let mut wide = Vec::with_capacity(byte_size / 2);
    for pair in raw_bytes.as_chunks::<2>().0 {
        let unit = u16::from_le_bytes(*pair);
        if unit == 0 {
            break;
        }
        wide.push(unit);
    }
    Some(String::from_utf16_lossy(&wide))
}

/// Put Unicode text on an open clipboard after emptying stale formats.
fn write_clipboard_text(text: &str) -> Result<(), String> {
    // SAFETY: Caller holds an open `ClipboardGuard`.
    unsafe {
        let _ = EmptyClipboard();
    }
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let bytes = wide
        .len()
        .checked_mul(std::mem::size_of::<u16>())
        .ok_or_else(|| "clipboard text size overflow".to_string())?;
    // SAFETY: `bytes` is non-zero (at least one null terminator) and `GMEM_MOVEABLE` is
    // required by `SetClipboardData`.
    let raw_handle =
        unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes) }.map_err(|error| error.to_string())?;
    let owned = OwnedGlobal::new(raw_handle);
    {
        let lock = GlobalLockGuard::lock(raw_handle)
            .ok_or_else(|| "the clipboard could not be locked".to_string())?;
        // SAFETY: `raw_handle` has `wide.len()` UTF-16 code units allocated and `lock`
        // provides an exclusive non-overlapping destination pointer.
        unsafe {
            std::ptr::copy_nonoverlapping(wide.as_ptr(), lock.as_mut_ptr() as *mut u16, wide.len());
        }
    }
    // SAFETY: Clipboard is open and `raw_handle` is unlocked `GMEM_MOVEABLE` memory.
    if let Err(error) =
        unsafe { SetClipboardData(CF_UNICODETEXT.0 as u32, Some(HANDLE(raw_handle.0))) }
    {
        return Err(format!("the clipboard could not be written: {error}"));
    }
    owned.into_clipboard();
    Ok(())
}

/// Read copied file paths (`CF_HDROP`) from an open clipboard.
fn read_clipboard_files() -> Option<Vec<String>> {
    // SAFETY: Caller holds an open `ClipboardGuard`.
    let handle = unsafe { GetClipboardData(CF_HDROP.0 as u32) }.ok()?;
    let hdrop = HDROP(handle.0);
    // SAFETY: `hdrop` is a valid `CF_HDROP` handle from `GetClipboardData`.
    let count = unsafe { DragQueryFileW(hdrop, u32::MAX, None) };
    if count == 0 {
        return None;
    }
    let mut paths = Vec::with_capacity(count as usize);
    for idx in 0..count {
        // SAFETY: `idx < count` and `hdrop` remains valid while the clipboard is open.
        let len = unsafe { DragQueryFileW(hdrop, idx, None) } as usize;
        if len == 0 {
            continue;
        }
        let mut buf = vec![0u16; len + 1];
        // SAFETY: `buf` has space for `len` code units plus trailing null terminator.
        let written = unsafe { DragQueryFileW(hdrop, idx, Some(&mut buf)) } as usize;
        if written > 0 {
            paths.push(String::from_utf16_lossy(&buf[..written]));
        }
    }
    if paths.is_empty() {
        None
    } else {
        Some(paths)
    }
}

/// Write a double-null-terminated Unicode `DROPFILES` structure (`CF_HDROP`).
fn write_clipboard_files(paths: &[String]) -> Result<(), String> {
    // SAFETY: Caller holds an open `ClipboardGuard`.
    unsafe {
        let _ = EmptyClipboard();
    }
    let mut wide_list: Vec<u16> = Vec::new();
    for path in paths {
        wide_list.extend(path.encode_utf16());
        wide_list.push(0);
    }
    wide_list.push(0);

    let header_size = std::mem::size_of::<DROPFILES>();
    let list_bytes = wide_list
        .len()
        .checked_mul(std::mem::size_of::<u16>())
        .ok_or_else(|| "file list size overflow".to_string())?;
    let total_bytes = header_size
        .checked_add(list_bytes)
        .ok_or_else(|| "DROPFILES allocation overflow".to_string())?;
    // SAFETY: `total_bytes` is non-zero and `GMEM_MOVEABLE | GMEM_ZEROINIT` is valid.
    let raw_handle = unsafe { GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, total_bytes) }
        .map_err(|error| error.to_string())?;
    let owned = OwnedGlobal::new(raw_handle);
    {
        let lock = GlobalLockGuard::lock(raw_handle)
            .ok_or_else(|| "the clipboard could not be locked".to_string())?;
        let pointer = lock.as_mut_ptr();
        let header = DROPFILES {
            pFiles: header_size as u32,
            pt: POINT { x: 0, y: 0 },
            fNC: false.into(),
            fWide: true.into(),
        };
        // SAFETY: `pointer` is aligned for `DROPFILES` (Win32 `GlobalAlloc` returns at
        // least 8-byte aligned memory) and `total_bytes >= header_size + list_bytes`.
        unsafe {
            std::ptr::write(pointer as *mut DROPFILES, header);
            std::ptr::copy_nonoverlapping(
                wide_list.as_ptr(),
                pointer.add(header_size) as *mut u16,
                wide_list.len(),
            );
        }
    }
    // SAFETY: Clipboard is open and `raw_handle` is unlocked `GMEM_MOVEABLE` memory.
    if let Err(error) = unsafe { SetClipboardData(CF_HDROP.0 as u32, Some(HANDLE(raw_handle.0))) } {
        return Err(format!("files could not be copied to clipboard: {error}"));
    }
    owned.into_clipboard();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_header(
        header_size: u32,
        width: i32,
        height: i32,
        planes: u16,
        bit_count: u16,
        compression: u32,
        clr_used: u32,
    ) -> Vec<u8> {
        let mut buf = Vec::with_capacity(header_size as usize);
        buf.extend_from_slice(&header_size.to_le_bytes());
        buf.extend_from_slice(&width.to_le_bytes());
        buf.extend_from_slice(&height.to_le_bytes());
        buf.extend_from_slice(&planes.to_le_bytes());
        buf.extend_from_slice(&bit_count.to_le_bytes());
        buf.extend_from_slice(&compression.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes()); // biSizeImage
        buf.extend_from_slice(&0i32.to_le_bytes()); // biXPelsPerMeter
        buf.extend_from_slice(&0i32.to_le_bytes()); // biYPelsPerMeter
        buf.extend_from_slice(&clr_used.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes()); // biClrImportant
        buf
    }

    #[test]
    fn parse_24bit_bottom_up_with_row_padding() {
        // 3x2 image: each row is 9 bytes + 3 padding bytes = 12 bytes stride.
        let mut dib = make_header(40, 3, 2, 1, 24, BI_RGB, 0);
        // Bottom row (y = 1 in output): R=(10,20,30), G=(40,50,60), B=(70,80,90) stored as BGR.
        dib.extend_from_slice(&[30, 20, 10, 60, 50, 40, 90, 80, 70, 0xAA, 0xBB, 0xCC]);
        // Top row (y = 0 in output): (255,0,0), (0,255,0), (0,0,255) stored as BGR.
        dib.extend_from_slice(&[0, 0, 255, 0, 255, 0, 255, 0, 0, 0x11, 0x22, 0x33]);

        let (w, h, rgba) = parse_dib_rgba(&dib).expect("valid 24-bit bottom-up DIB");
        assert_eq!((w, h), (3, 2));
        assert_eq!(
            rgba,
            vec![
                255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 10, 20, 30, 255, 40, 50, 60, 255,
                70, 80, 90, 255,
            ]
        );
    }

    #[test]
    fn parse_32bit_top_down_preserves_straight_alpha() {
        // 2x1 top-down (height = -1), BI_RGB 32-bit with real alpha.
        let mut dib = make_header(40, 2, -1, 1, 32, BI_RGB, 0);
        // Pixel 0: straight RGBA (200, 100, 50, 128) -> BGRA [50, 100, 200, 128]
        dib.extend_from_slice(&[50, 100, 200, 128]);
        // Pixel 1: fully transparent (255, 255, 255, 0) -> BGRA [255, 255, 255, 0]
        dib.extend_from_slice(&[255, 255, 255, 0]);

        let (w, h, rgba) = parse_dib_rgba(&dib).expect("valid 32-bit top-down DIB");
        assert_eq!((w, h), (2, 1));
        assert_eq!(rgba, vec![200, 100, 50, 128, 255, 255, 255, 0]);
    }

    #[test]
    fn parse_32bit_bi_rgb_all_zero_alpha_treated_as_opaque() {
        let mut dib = make_header(40, 2, -1, 1, 32, BI_RGB, 0);
        // Windows GDI often writes A=0 across every pixel in 32-bit BI_RGB.
        dib.extend_from_slice(&[10, 20, 30, 0, 40, 50, 60, 0]);

        let (w, h, rgba) = parse_dib_rgba(&dib).expect("valid 32-bit opaque DIB");
        assert_eq!((w, h), (2, 1));
        assert_eq!(rgba, vec![30, 20, 10, 255, 60, 50, 40, 255]);
    }

    #[test]
    fn parse_32bit_bi_bitfields_40byte_and_v4_inline() {
        // Case 1: 40-byte header + 12-byte RGB masks (RGBX layout: R=0x000000FF, G=0x0000FF00, B=0x00FF0000)
        let mut dib = make_header(40, 1, -1, 1, 32, BI_BITFIELDS, 0);
        dib.extend_from_slice(&0x0000_00FFu32.to_le_bytes());
        dib.extend_from_slice(&0x0000_FF00u32.to_le_bytes());
        dib.extend_from_slice(&0x00FF_0000u32.to_le_bytes());
        let pixel: u32 = 0x00_33_22_11; // R=0x11, G=0x22, B=0x33
        dib.extend_from_slice(&pixel.to_le_bytes());

        let (w, h, rgba) = parse_dib_rgba(&dib).expect("valid 40-byte BI_BITFIELDS DIB");
        assert_eq!((w, h), (1, 1));
        assert_eq!(rgba, vec![0x11, 0x22, 0x33, 255]);

        // Case 2: 56-byte V4-style header with inline R/G/B/A masks
        let mut v4 = make_header(56, 1, -1, 1, 32, BI_BITFIELDS, 0);
        v4.extend_from_slice(&0x00FF_0000u32.to_le_bytes()); // R
        v4.extend_from_slice(&0x0000_FF00u32.to_le_bytes()); // G
        v4.extend_from_slice(&0x0000_00FFu32.to_le_bytes()); // B
        v4.extend_from_slice(&0xFF00_0000u32.to_le_bytes()); // A
        let px: u32 = 0x80C8_6432; // A=128, R=200, G=100, B=50
        v4.extend_from_slice(&px.to_le_bytes());

        let (w2, h2, rgba2) = parse_dib_rgba(&v4).expect("valid 56-byte BI_BITFIELDS DIB");
        assert_eq!((w2, h2), (1, 1));
        assert_eq!(rgba2, vec![200, 100, 50, 128]);
    }

    #[test]
    fn parse_dib_rejects_oversized_truncated_and_invalid() {
        // Too short
        assert!(parse_dib_rgba(&[0u8; 39]).is_none());

        // Oversized width (> 4096)
        let oversized = make_header(40, 4097, -1, 1, 32, BI_RGB, 0);
        assert!(parse_dib_rgba(&oversized).is_none());

        // i32::MIN height
        let min_h = make_header(40, 1, i32::MIN, 1, 32, BI_RGB, 0);
        assert!(parse_dib_rgba(&min_h).is_none());

        // Truncated pixel data
        let mut truncated = make_header(40, 2, -2, 1, 32, BI_RGB, 0);
        truncated.extend_from_slice(&[0u8; 15]); // needs 16 bytes
        assert!(parse_dib_rgba(&truncated).is_none());

        // Invalid planes
        let mut bad_planes = make_header(40, 1, -1, 2, 32, BI_RGB, 0);
        bad_planes.extend_from_slice(&[0u8; 4]);
        assert!(parse_dib_rgba(&bad_planes).is_none());
    }

    #[test]
    fn encode_and_parse_dib_round_trip() {
        // Straight RGBA: opaque red, 50% green, transparent, opaque blue.
        let original = vec![255, 0, 0, 255, 0, 200, 0, 128, 0, 0, 0, 0, 0, 0, 255, 255];
        let encoded = encode_dib_rgba(2, 2, &original).expect("encode should succeed");
        assert_eq!(encoded.len(), 40 + 16);

        let (w, h, decoded) = parse_dib_rgba(&encoded).expect("parse encoded DIB");
        assert_eq!((w, h), (2, 2));
        assert_eq!(decoded, original);

        // Invalid inputs rejected.
        assert!(encode_dib_rgba(0, 1, &[]).is_err());
        assert!(encode_dib_rgba(4097, 1, &[]).is_err());
        assert!(encode_dib_rgba(2, 2, &[0u8; 12]).is_err());
    }
}
