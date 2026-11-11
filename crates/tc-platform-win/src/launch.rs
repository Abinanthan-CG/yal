//! Opening things the way Windows does.
//!
//! Upstream uses `NSWorkspace` to open an application, a file or a URL. The
//! Windows equivalent is the shell itself, which is also what makes a `.lnk`
//! shortcut work without resolving it to an executable first.

use windows::core::PCWSTR;
use windows::Win32::UI::Shell::ShellExecuteW;
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
}
