//! The application identity Windows uses to trust notifications.
//!
//! An unpackaged program's toasts are refused a banner unless the system can
//! tie the user model identifier to a Start Menu shortcut. Writing that
//! shortcut — a link to the launcher with the identifier set as a property —
//! is what installs normally do; it is done here so the launcher can do it
//! for itself, once, on request.

/// A failure while establishing the identity.
#[derive(Debug, thiserror::Error)]
pub enum IdentityError {
    #[error("the identity could not be established: {0}")]
    Failed(String),
}

/// A Start Menu shortcut for the launcher, carrying its user model
/// identifier. Call once per installation; it is safe to call again.
///
/// The link lands in the user's Start Menu Programs folder so no installer
/// rights are needed, and the launcher is not copied or modified. The
/// shortcut points at `target` — the running binary, wherever the user
/// has put it.
#[cfg(windows)]
pub fn register_shortcut(
    application_id: &str,
    display_name: &str,
    target: &std::path::Path,
) -> Result<(), IdentityError> {
    use windows::core::{Interface, PWSTR};
    use windows::Win32::Storage::EnhancedStorage::PKEY_AppUserModel_ID;
    use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemAlloc, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED, IPersistFile,
    };
    use windows::Win32::System::Variant::VT_LPWSTR;
    use windows::Win32::UI::Shell::IShellLinkW;

    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok().map_err(|err| {
            IdentityError::Failed(format!("the COM apartment could not be started: {err}"))
        })?;

        // The shell link coclass is one of the few identifiers the windows
        // crate does not ship, so it is spelled out here. It is fixed by
        // the shell and cannot change.
        let clsid = windows::core::GUID::from_u128(0x00021401_0000_0000_c000_000000000046);
        let link: IShellLinkW = CoCreateInstance(&clsid, None, CLSCTX_INPROC_SERVER)
            .map_err(|err| IdentityError::Failed(format!("the shortcut could not be created: {err}")))?;

        let path = windows::core::HSTRING::from(target.as_os_str());
        link.SetPath(windows::core::PCWSTR::from_raw(path.as_ptr()))
            .map_err(|err| {
                IdentityError::Failed(format!(
                    "the shortcut target could not be set: {err}"
                ))
            })?;
        // No description: the shell's SetDescription is the one call among
        // these that corrupted the process heap every time it ran, and a
        // description carries no weight for the identity.

        // The user model identifier is a property of the shortcut, not
        // of the program it points at: this is what makes the shell
        // treat the identifier as known.
        let store: windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore = link
            .cast()
            .map_err(|err| {
                IdentityError::Failed(format!(
                    "the shortcut properties could not be opened: {err}"
                ))
            })?;

        // The shell's property store releases the value it was given
        // with the COM allocator, so the string has to come from that
        // same allocator: a plain Rust buffer freed by CoTaskMemFree
        // corrupts the process heap.
        let wide: Vec<u16> =
            application_id.encode_utf16().chain(core::iter::once(0)).collect();
        let len = wide.len();
        let buffer = CoTaskMemAlloc(len * 2);
        if buffer.is_null() {
            return Err(IdentityError::Failed(
                "the COM allocator refused the string allocation".into(),
            ));
        }
        core::ptr::copy_nonoverlapping(wide.as_ptr(), buffer as *mut u16, len);
        // The value is built as a plain struct first, without union
        // writes, and wrapped last.
        let mut inner = windows::Win32::System::Com::StructuredStorage::PROPVARIANT_0_0 {
            vt: VT_LPWSTR,
            wReserved1: 0,
            wReserved2: 0,
            wReserved3: 0,
            Anonymous: windows::Win32::System::Com::StructuredStorage::PROPVARIANT_0_0_0::default(),
        };
        inner.Anonymous.pwszVal = PWSTR::from_raw(buffer as *mut u16);
        let mut value = PROPVARIANT::default();
        value.Anonymous =
            windows::Win32::System::Com::StructuredStorage::PROPVARIANT_0 {
                Anonymous: core::mem::ManuallyDrop::new(inner),
            };
        store
            .SetValue(&PKEY_AppUserModel_ID, &value)
            .map_err(|err| {
                IdentityError::Failed(format!(
                    "the identifier could not be set: {err}"
                ))
            })?;
        store.Commit().map_err(|err| {
            IdentityError::Failed(format!(
                "the identifier could not be saved: {err}"
            ))
        })?;

        let file: IPersistFile = link
            .cast()
            .map_err(|err| {
                IdentityError::Failed(format!(
                    "the shortcut could not be saved: {err}"
                ))
            })?;
        let path = shortcut_path(display_name)?;
        file.Save(&windows::core::HSTRING::from(path.as_os_str()), false)
            .map_err(|err| {
                IdentityError::Failed(format!(
                    "the shortcut could not be written: {err}"
                ))
            })?;
    }

    Ok(())
}

/// The Start Menu path the shortcut is written to.
///
/// `%APPDATA%\Microsoft\Windows\Start Menu\Programs\<name>.lnk` — the user's
/// own Start Menu, which needs no administrator rights.
#[cfg(windows)]
fn shortcut_path(display_name: &str) -> Result<std::path::PathBuf, IdentityError> {
    let Some(roaming) = dirs::data_dir() else {
        return Err(IdentityError::Failed(
            "the roaming application data folder could not be found".into(),
        ));
    };
    let mut path = roaming;
    for part in ["Microsoft", "Windows", "Start Menu", "Programs"] {
        path.push(part);
    }
    path.push(format!("{display_name}.lnk"));
    Ok(path)
}

/// Register the shortcut on a host without shortcuts.
#[cfg(not(windows))]
pub fn register_shortcut(
    _application_id: &str,
    _display_name: &str,
    _target: &std::path::Path,
) -> Result<(), IdentityError> {
    Err(IdentityError::Failed(
        "shortcut registration is not implemented on this platform".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shortcut_lands_in_the_start_menu() {
        // Without writing anything: the path is derived, not used.
        let path = shortcut_path("launcher (clean-room)").unwrap();
        assert!(path.to_string_lossy().to_lowercase().contains("start menu"));
        assert!(path.to_string_lossy().ends_with("launcher (clean-room).lnk"));
    }
}
