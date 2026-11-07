//! Resolution of Windows shell shortcuts (`.lnk`).
//!
//! A shortcut stores its target behind the `IShellLink` COM interface, and
//! the link data itself behind `IPersistFile`. BOTH halves are read: the
//! link is opened through `IPersistFile::Load` — never through
//! `IShellLink::Resolve`, which can reach out to a dead network share —
//! the target, arguments and working directory are read from the loaded
//! link, and the link is released. Nothing is executed and the shell is
//! never launched.
//!
//! An empty `IShellLink` with a `SetPath` call answers `GetPath` with
//! whatever was set — the first version of this module read the
//! shortcut's own file path that way, which made every application
//! "resolve" to its `.lnk`. The load is what makes the read honest.

use std::path::PathBuf;

/// The target of a shortcut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutTarget {
    /// The target the shortcut points at — usually an executable.
    pub target: PathBuf,
    /// The working directory, if the shortcut recorded one.
    pub working_directory: Option<PathBuf>,
    /// The arguments, if the shortcut recorded any.
    pub arguments: Option<String>,
}

/// Resolve a `.lnk` file to its target.
///
/// Returns `None` when the file is not a shortcut, is unreadable, or
/// carries no filesystem target at all.
pub fn resolve(path: &std::path::Path) -> Option<ShortcutTarget> {
    resolve_inner(path)
}

#[cfg(windows)]
fn resolve_inner(path: &std::path::Path) -> Option<ShortcutTarget> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::{Interface, PCWSTR};
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, IPersistFile,
        STGM_READ,
    };
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink, SLGP_RAWPATH};

    // COM must be initialised on this thread before any shell object is
    // created. When it cannot be initialised the caller simply gets no
    // shortcut, which is not worth failing the whole index over.
    let com_initialised = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok();
    let _guard = ComGuard {
        uninitialise: com_initialised,
    };

    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;

        // The link data: without this load the object is empty and
        // every read below would answer with nothing.
        let persist: IPersistFile = link.cast().ok()?;
        persist.Load(PCWSTR(wide.as_ptr()), STGM_READ).ok()?;

        // A generous buffer avoids truncating long target paths.
        let mut target = vec![0u16; 32768];
        link.GetPath(&mut target, std::ptr::null_mut(), SLGP_RAWPATH.0 as u32)
            .ok()?;
        let target_path = PathBuf::from(wide_to_string(&target));
        if target_path.as_os_str().is_empty() {
            return None;
        }

        let mut working_directory = vec![0u16; 32768];
        let working = link
            .GetWorkingDirectory(&mut working_directory)
            .ok()
            .map(|_| PathBuf::from(wide_to_string(&working_directory)))
            .filter(|path| !path.as_os_str().is_empty());

        let mut arguments = vec![0u16; 32768];
        let args = link
            .GetArguments(&mut arguments)
            .ok()
            .map(|_| wide_to_string(&arguments))
            .filter(|value| !value.is_empty());

        Some(ShortcutTarget {
            target: target_path,
            working_directory: working,
            arguments: args,
        })
    }
}

/// The URL a `.url` Internet shortcut hands out.
///
/// The file is an INI document; Windows reads the `InternetShortcut`
/// section's `URL` value, and so does this, through the same API —
/// file-based, nothing launched.
#[cfg(windows)]
pub fn url_target(path: &std::path::Path) -> Option<String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::System::WindowsProgramming::GetPrivateProfileStringW;
    use windows::core::PCWSTR;

    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let utf16 = |text: &str| -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    };
    let section = utf16("InternetShortcut");
    let key = utf16("URL");
    let default = utf16("");

    let mut out = vec![0u16; 2048];
    let count = unsafe {
        GetPrivateProfileStringW(
            PCWSTR(section.as_ptr()),
            PCWSTR(key.as_ptr()),
            PCWSTR(default.as_ptr()),
            Some(&mut out),
            PCWSTR(wide.as_ptr()),
        )
    };
    // The count excludes the terminator, and the buffer keeps its size
    // on truncation, so the slice stops at whichever came first.
    let read = &out[..count.min(out.len() as u32 - 1) as usize];
    let url = String::from_utf16_lossy(read);
    if url.is_empty() {
        None
    } else {
        Some(url)
    }
}

/// The URL a `.url` file carries; nothing outside Windows to read.
#[cfg(not(windows))]
pub fn url_target(_path: &std::path::Path) -> Option<String> {
    None
}

/// Convert a slice of wide buffer into a string, stopping at the terminator.
#[cfg(windows)]
fn wide_to_string(buffer: &[u16]) -> String {
    let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..end])
}

/// Releases COM on drop when this thread initialised it.
#[cfg(windows)]
struct ComGuard {
    uninitialise: bool,
}

#[cfg(windows)]
impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.uninitialise {
            unsafe {
                windows::Win32::System::Com::CoUninitialize();
            }
        }
    }
}

#[cfg(not(windows))]
fn resolve_inner(_path: &std::path::Path) -> Option<ShortcutTarget> {
    None
}
