//! Opening things the way Windows does.
//!
//! Upstream uses `NSWorkspace` to open an application, a file or a URL. The
//! Windows equivalent is the shell itself, which is also what makes a `.lnk`
//! shortcut work without resolving it to an executable first.

use windows::core::PCWSTR;
use windows::Win32::UI::Shell::{
    SHFileOperationW, ShellExecuteW, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT,
    FO_DELETE, SHFILEOPSTRUCTW,
};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

/// Open a program, a file or a folder, the way the shell does.
///
/// The target may be an executable, a Start Menu shortcut, a document or a
/// folder: the shell knows what each of them is.
pub fn open(target: &str) -> Result<(), String> {
    let wide = to_wide(target);
    // SAFETY: both strings are null-terminated, and the shell only reads
    // them for the length of the call.
    let result = unsafe {
        ShellExecuteW(
            None,
            PCWSTR::null(),
            PCWSTR(wide.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    // A result of 32 or more is an error code; below that it is a handle.
    if result.0 as isize <= 32 {
        return Err(format!("Windows could not open `{target}`"));
    }
    Ok(())
}

/// Show a file in the file manager, with it selected.
pub fn reveal(target: &str) -> Result<(), String> {
    let wide = to_wide(target);
    // SAFETY: the string is null-terminated and the shell only reads it.
    let result = unsafe {
        ShellExecuteW(
            None,
            PCWSTR(wide_explorer().as_ptr()),
            PCWSTR(wide.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    if result.0 as isize <= 32 {
        return Err(format!("Windows could not show `{target}`"));
    }
    Ok(())
}

/// The verb that opens the file manager with a file selected.
fn wide_explorer() -> Vec<u16> {
    to_wide("explorer")
}

/// Move a file or a folder to the Recycle Bin.
///
/// Upstream's Trash is the Finder's; Windows' is the Recycle Bin, and a file
/// in it can be put back. The `FOF_ALLOWUNDO` flag is what asks for that:
/// without it the shell would delete the thing outright.
pub fn recycle(target: &str) -> Result<(), String> {
    // The list of paths the shell works on ends with two nulls, so the list
    // itself is closed.
    let mut paths = to_wide(target);
    paths.push(0);
    let mut operation = SHFILEOPSTRUCTW {
        wFunc: FO_DELETE,
        pFrom: PCWSTR(paths.as_ptr()),
        fFlags: (FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_NOERRORUI | FOF_SILENT).0 as u16,
        ..Default::default()
    };
    // SAFETY: the struct is filled in, and the path list is null-terminated
    // twice, which is what the shell expects.
    let code = unsafe { SHFileOperationW(&mut operation) };
    if code != 0 {
        return Err(format!(
            "Windows would not move `{target}` to the Recycle Bin: {code}"
        ));
    }
    if operation.fAnyOperationsAborted.as_bool() {
        return Err(format!(
            "Windows did not move `{target}` to the Recycle Bin"
        ));
    }
    Ok(())
}

/// A string, wide and null-terminated.
fn to_wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_something_that_is_not_there_fails_rather_than_panicking() {
        // The shell answers with an error code for a path that does not exist.
        let result = open(r"C:\yal-does-not-exist\nothing-here.exe");
        assert!(result.is_err());
    }

    #[test]
    fn a_wide_string_is_null_terminated() {
        assert_eq!(to_wide("ab"), vec![0x61, 0x62, 0]);
    }

    #[test]
    fn a_file_goes_to_the_recycle_bin_and_leaves_its_place() {
        // The bin is the bin: whether the machine keeps the file or throws it
        // away, the file must not be where it was.
        let folder = std::env::temp_dir().join(format!("yal-recycle-{}", std::process::id()));
        std::fs::create_dir_all(&folder).expect("a folder to work in");
        let file = folder.join("a-file-to-bin.txt");
        std::fs::write(&file, b"something to throw away").expect("a file to throw away");
        let result = recycle(file.to_string_lossy().as_ref());
        assert!(result.is_ok(), "the file went to the bin: {result:?}");
        assert!(!file.exists(), "the file left its place");
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn a_file_that_is_not_there_cannot_go_anywhere() {
        let result = recycle(r"C:\yal-does-not-exist\nothing-here.txt");
        assert!(result.is_err());
    }
}
