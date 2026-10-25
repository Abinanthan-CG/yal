//! Resolution of Windows shell shortcuts (`.lnk`).
//!
//! A shortcut stores its target behind the `IShellLink` COM interface. The
//! resolution below is read-only: the link is opened, the target path is read,
//! and the link is released. Nothing is executed and the shell is never
//! launched.

use std::path::PathBuf;

/// The target of a shortcut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutTarget {
    /// The executable the shortcut points at.
    pub target: PathBuf,
    /// The working directory, if the shortcut recorded one.
    pub working_directory: Option<PathBuf>,
    /// The arguments, if the shortcut recorded any.
    pub arguments: Option<String>,
}

/// Resolve a `.lnk` file to its target.
///
/// Returns `None` when the file is not a shortcut, is unreadable, or points at
/// something that no longer exists.
pub fn resolve(path: &std::path::Path) -> Option<ShortcutTarget> {
    resolve_inner(path)
}

#[cfg(windows)]
fn resolve_inner(path: &std::path::Path) -> Option<ShortcutTarget> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
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
        link.SetPath(PCWSTR(wide.as_ptr())).ok()?;

        // A generous buffer avoids truncating long target paths.
        let mut target = vec![0u16; 32768];
        link.GetPath(&mut target, std::ptr::null_mut(), SLGP_RAWPATH.0 as u32)
            .ok()?;
        let target_path = PathBuf::from(wide_to_string(&target));

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

/// Convert a null-terminated wide buffer into a string.
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
