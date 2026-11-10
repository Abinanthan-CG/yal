use tc_platform::Clipboard;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::DataExchange::{
    CloseClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
};
use windows::Win32::Foundation::HGLOBAL;
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::System::Ole::CF_UNICODETEXT;

/// The Windows clipboard, as text.
#[derive(Debug, Default)]
pub struct WindowsClipboard;

impl WindowsClipboard {
    pub fn new() -> WindowsClipboard {
        WindowsClipboard
    }
}

impl Clipboard for WindowsClipboard {
    fn read_text(&self) -> Option<String> {
        unsafe {
            if OpenClipboard(None).is_err() {
                return None;
            }
            let result = read_clipboard_text();
            let _ = CloseClipboard();
            result
        }
    }

    fn write_text(&self, text: &str) -> Result<(), String> {
        unsafe {
            if OpenClipboard(None).is_err() {
                return Err("the clipboard is in use by another program".to_string());
            }
            let result = write_clipboard_text(text);
            let _ = CloseClipboard();
            result
        }
    }

    fn available_formats(&self) -> Vec<String> {
        Vec::new()
    }
}

/// Read the clipboard's text.
unsafe fn read_clipboard_text() -> Option<String> {
    let handle = GetClipboardData(CF_UNICODETEXT.0 as u32).ok()?;
    let pointer = GlobalLock(HGLOBAL(handle.0));
    if pointer.is_null() {
        return None;
    }
    let text = read_wide_string(pointer as *const u16);
    let _ = GlobalUnlock(HGLOBAL(handle.0));
    Some(text)
}

/// Put text on the clipboard.
unsafe fn write_clipboard_text(text: &str) -> Result<(), String> {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let bytes = wide.len() * std::mem::size_of::<u16>();
    let handle = GlobalAlloc(GMEM_MOVEABLE, bytes).map_err(|error| error.to_string())?;
    let pointer = GlobalLock(HGLOBAL(handle.0));
    if pointer.is_null() {
        return Err("the clipboard could not be locked".to_string());
    }
    std::ptr::copy_nonoverlapping(wide.as_ptr(), pointer as *mut u16, wide.len());
    let _ = GlobalUnlock(HGLOBAL(handle.0));
    SetClipboardData(CF_UNICODETEXT.0 as u32, Some(HANDLE(handle.0)))
        .map_err(|error| format!("the clipboard could not be written: {error}"))?;
    Ok(())
}

/// Read a null-terminated wide string.
unsafe fn read_wide_string(pointer: *const u16) -> String {
    let mut length = 0usize;
    while *pointer.add(length) != 0 {
        length += 1;
    }
    let slice = std::slice::from_raw_parts(pointer, length);
    String::from_utf16_lossy(slice)
}


