//! Opening things the way Windows does.
//!
//! Upstream uses `NSWorkspace` to open an application, a file or a URL. The
//! Windows equivalent is the shell itself, which is also what makes a `.lnk`
//! shortcut work without resolving it to an executable first.

use windows::core::{w, PCWSTR};
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::Shell::{
    SHFileOperationW, ShellExecuteExW, ShellExecuteW, FOF_ALLOWUNDO, FOF_NOCONFIRMATION,
    FOF_NOERRORUI, FOF_SILENT, FO_DELETE, SEE_MASK_INVOKEIDLIST, SHELLEXECUTEINFOW,
    SHFILEOPSTRUCTW,
};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

/// Open a program, Store app (`shell:AppsFolder\<AUMID>`), file, URL, or folder.
pub fn open(target: &str) -> Result<(), String> {
    let wide = to_wide(target);
    let com_ok = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok();
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(wide.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    if com_ok {
        unsafe { CoUninitialize() };
    }
    if result.0 as isize <= 32 {
        return Err(format!("Windows could not open `{target}`"));
    }
    Ok(())
}

/// Show a file in File Explorer, with the file selected.
pub fn reveal(target: &str) -> Result<(), String> {
    let args = to_wide(&format!("/select,\"{target}\""));
    let com_ok = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok();
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            w!("explorer.exe"),
            PCWSTR(args.as_ptr()),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    if com_ok {
        unsafe { CoUninitialize() };
    }
    if result.0 as isize <= 32 {
        return Err(format!("Windows could not show `{target}`"));
    }
    Ok(())
}

/// Launch an application or executable elevated (`runas` UAC prompt).
pub fn run_as_admin(target: &str) -> Result<(), String> {
    let wide = to_wide(target);
    let com_ok = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok();
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("runas"),
            PCWSTR(wide.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    if com_ok {
        unsafe { CoUninitialize() };
    }
    if result.0 as isize <= 32 {
        return Err(format!("Windows could not run `{target}` as administrator"));
    }
    Ok(())
}

/// Show the native Windows File/Folder Properties dialog (`properties` shell verb).
pub fn show_properties(target: &str) -> Result<(), String> {
    let wide = to_wide(target);
    let com_ok = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok();
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_INVOKEIDLIST,
        lpVerb: w!("properties"),
        lpFile: PCWSTR(wide.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    let ok = unsafe { ShellExecuteExW(&mut info) }.is_ok();
    if com_ok {
        unsafe { CoUninitialize() };
    }
    if !ok {
        return Err(format!("Windows could not show properties for `{target}`"));
    }
    Ok(())
}

/// Open Windows Terminal (falling back to PowerShell) inside `dir_or_file`.
pub fn open_in_terminal(dir_or_file: &str) -> Result<(), String> {
    let p = std::path::Path::new(dir_or_file);
    let dir = if p.is_dir() {
        p.to_path_buf()
    } else {
        p.parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(std::path::Path::to_path_buf)
            .unwrap_or_else(|| p.to_path_buf())
    };
    let dir_str = dir.to_string_lossy();
    let wt_args = to_wide(&format!("-d \"{dir_str}\""));
    let com_ok = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok();
    let res = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            w!("wt.exe"),
            PCWSTR(wt_args.as_ptr()),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    if res.0 as isize <= 32 {
        let dir_wide = to_wide(&dir_str);
        let fallback = unsafe {
            ShellExecuteW(
                None,
                w!("open"),
                w!("powershell.exe"),
                w!("-NoExit"),
                PCWSTR(dir_wide.as_ptr()),
                SW_SHOWNORMAL,
            )
        };
        if com_ok {
            unsafe { CoUninitialize() };
        }
        if fallback.0 as isize <= 32 {
            return Err(format!("could not open terminal in `{dir_str}`"));
        }
        return Ok(());
    }
    if com_ok {
        unsafe { CoUninitialize() };
    }
    Ok(())
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
