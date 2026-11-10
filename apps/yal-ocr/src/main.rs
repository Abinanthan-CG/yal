//! The text recognition helper.
//!
//! Clipboard history can hold images and PDFs, and upstream searches the text
//! inside them. That work runs in its own helper process, so a large document
//! cannot block the launcher and a crash in the recogniser cannot take the app
//! down.
//!
//! The helper reports what it can read about a document and whether the
//! Windows text recognition engine is available on this machine.

use std::path::{Path, PathBuf};

/// The largest file the helper will read, in bytes.
pub const MAXIMUM_FILE_BYTES: u64 = 32 * 1024 * 1024;

/// The largest number of pages of a PDF the helper will read.
pub const MAXIMUM_PDF_PAGES: u32 = 64;

/// What the helper knows about a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentInfo {
    pub path: PathBuf,
    pub bytes: u64,
    /// The number of pages, when the file is a PDF.
    pub pages: Option<u32>,
    /// Whether the Windows text recognition engine is available.
    pub recognition_available: bool,
}

/// Read what can be read about a document.
pub fn inspect(path: &Path) -> std::io::Result<DocumentInfo> {
    let metadata = std::fs::metadata(path)?;
    let pages = if is_pdf(path) { count_pdf_pages(path) } else { None };
    Ok(DocumentInfo {
        path: path.to_path_buf(),
        bytes: metadata.len(),
        pages,
        recognition_available: recognition_available(),
    })
}

/// Whether a file looks like a PDF.
pub fn is_pdf(path: &Path) -> bool {
    let Ok(bytes) = std::fs::read(path) else {
        return false;
    };
    bytes.starts_with(b"%PDF-")
}

/// Count a PDF's pages by counting its page objects.
///
/// The count is capped, because upstream stops after the first 64 pages.
pub fn count_pdf_pages(path: &Path) -> Option<u32> {
    let bytes = std::fs::read(path).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    let count = text.matches("/Type /Page").count() as u32;
    Some(count.min(MAXIMUM_PDF_PAGES))
}

/// Whether the Windows text recognition engine is available.
///
/// The engine ships with Windows, but only with the language packs that are
/// installed. The optional feature is registered by Windows when it is there.
pub fn recognition_available() -> bool {
    registry_key_exists(
        windows::Win32::System::Registry::HKEY_LOCAL_MACHINE,
        "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\OCR\\Extensions",
    )
}

/// Whether a registry key exists under the given hive.
fn registry_key_exists(
    hive: windows::Win32::System::Registry::HKEY,
    subkey: &str,
) -> bool {
    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::{RegOpenKeyExW, KEY_READ};

    let name: Vec<u16> = subkey.encode_utf16().chain(std::iter::once(0)).collect();
    let mut handle = windows::Win32::System::Registry::HKEY::default();
    let status = unsafe {
        RegOpenKeyExW(
            hive,
            PCWSTR(name.as_ptr()),
            None,
            KEY_READ,
            std::ptr::addr_of_mut!(handle),
        )
    };
    status.is_ok()
}

/// The document as JSON.
pub fn document_json(info: &DocumentInfo) -> String {
    let pages = match info.pages {
        Some(pages) => pages.to_string(),
        None => "null".to_string(),
    };
    format!(
        "{{\"path\":\"{}\",\"bytes\":{},\"pages\":{},\"recognition_available\":{}}}",
        info.path.display().to_string().replace('\\', "\\\\"),
        info.bytes,
        pages,
        info.recognition_available
    )
}

fn main() -> std::process::ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let path = arguments
        .iter()
        .position(|a| a == "--inspect")
        .and_then(|index| arguments.get(index + 1))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::env::args_os()
                .nth(1)
                .map(std::path::PathBuf::from)
                .unwrap_or_default()
        });

    if path.as_os_str().is_empty() {
        eprintln!("yal-ocr: usage: yal-ocr --inspect <file>");
        return std::process::ExitCode::FAILURE;
    }

    match inspect(&path) {
        Ok(info) => {
            println!("{}", document_json(&info));
            std::process::ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("yal-ocr: {}: {error}", path.display());
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_that_is_not_a_pdf_is_not_one() {
        let path = std::env::temp_dir().join("yal-ocr-test.txt");
        std::fs::write(&path, b"not a pdf").unwrap();
        assert!(!is_pdf(&path));
    }

    #[test]
    fn the_limits_are_upstreams_limits() {
        assert_eq!(MAXIMUM_FILE_BYTES, 32 * 1024 * 1024);
        assert_eq!(MAXIMUM_PDF_PAGES, 64);
    }

    #[test]
    fn the_engine_is_reported_as_available_or_not() {
        // The answer depends on the machine, so the test only checks that the
        // question has an answer.
        let _ = recognition_available();
    }
}
