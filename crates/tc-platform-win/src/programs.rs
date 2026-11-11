//! The installed programs the launcher can open.
//!
//! Windows has no application bundle, so the same three questions upstream
//! asks of LaunchServices are asked of three places here:
//!
//! - the **Start Menu**, whose shortcuts carry the names Windows shows;
//! - the registry's **uninstall** key, which names every installed program;
//! - the registry's **App Paths**, which says how Windows starts one.
//!
//! A shortcut is launched as it is: Windows opens a `.lnk` file perfectly
//! well, so nothing has to resolve it to an executable first.

use std::path::{Path, PathBuf};

use std::ffi::c_void;

use windows::core::PCWSTR;
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{
    RegCloseKey, RegEnumKeyW, RegGetValueW, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER,
    HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY, RRF_RT_REG_SZ,
};

use tc_apps::{deduplicate, Program, Source};

/// The registry keys that list installed programs.
const UNINSTALL_KEYS: [&str; 3] = [
    r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
    r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
    r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
];

/// The registry key that says how Windows starts a program.
const APP_PATHS_KEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths";

/// Every program the machine can open.
pub fn programs() -> Vec<Program> {
    let mut found = Vec::new();
    found.extend(start_menu_programs());
    found.extend(registry_programs());
    found.extend(app_path_programs());
    deduplicate(found)
}

/// The programs in the Start Menu, per user and per machine.
fn start_menu_programs() -> Vec<Program> {
    let mut found = Vec::new();
    for folder in start_menu_folders() {
        collect_shortcuts(&folder, &mut found, 0);
    }
    found
}

/// The two Start Menu folders Windows keeps.
fn start_menu_folders() -> Vec<PathBuf> {
    let mut folders = Vec::new();
    if let Ok(program_data) = std::env::var("ProgramData") {
        folders.push(
            PathBuf::from(program_data)
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs"),
        );
    }
    if let Ok(app_data) = std::env::var("APPDATA") {
        folders.push(
            PathBuf::from(app_data)
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs"),
        );
    }
    folders
}

/// Walk a Start Menu folder, and the folders one level under it.
fn collect_shortcuts(folder: &Path, found: &mut Vec<Program>, depth: usize) {
    if depth > 2 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_shortcuts(&path, found, depth + 1);
            continue;
        }
        let extension = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        if extension != "lnk" && extension != "url" {
            continue;
        }
        let Some(stem) = path.file_stem().map(|s| s.to_string_lossy().to_string()) else {
            continue;
        };
        if stem.trim().is_empty() {
            continue;
        }
        // The folder a shortcut sits in names its publisher often enough:
        // "Microsoft Office" holds Word, Excel and PowerPoint.
        let publisher = path
            .parent()
            .and_then(|p| p.file_name())
            .map(|f| f.to_string_lossy().to_string())
            .filter(|folder| {
                !folder.eq_ignore_ascii_case("Programs")
                    && !folder.eq_ignore_ascii_case("Start Menu")
            });
        found.push(
            Program::new(stem, path.to_string_lossy().to_string(), Source::StartMenu)
                .with_publisher(publisher),
        );
    }
}

/// The programs the registry's uninstall keys list.
fn registry_programs() -> Vec<Program> {
    let mut found = Vec::new();
    for key in UNINSTALL_KEYS {
        // The 64-bit view of the machine's programs, then the 32-bit one.
        for (root, view) in [
            (HKEY_LOCAL_MACHINE, KEY_WOW64_64KEY),
            (HKEY_LOCAL_MACHINE, KEY_WOW64_32KEY),
            (HKEY_CURRENT_USER, KEY_WOW64_64KEY),
        ] {
            collect_uninstall_key(root, key, view, &mut found);
        }
    }
    found
}

/// Read one uninstall key, and every subkey under it.
fn collect_uninstall_key(
    root: HKEY,
    path: &str,
    view: windows::Win32::System::Registry::REG_SAM_FLAGS,
    found: &mut Vec<Program>,
) {
    let Ok(key) = open_key(root, path, view) else {
        return;
    };
    for name in subkey_names(key) {
        let Ok(subkey) = open_key(key, &name, view) else {
            continue;
        };
        // A Windows update or a driver is not something the user opens.
        if string_value(subkey, "SystemComponent").as_deref() == Some("1") {
            close_key(subkey);
            continue;
        }
        if string_value(subkey, "ParentKeyName").is_some() {
            close_key(subkey);
            continue;
        }
        let Some(display_name) = string_value(subkey, "DisplayName") else {
            close_key(subkey);
            continue;
        };
        if display_name.trim().is_empty() {
            close_key(subkey);
            continue;
        }

        let publisher = string_value(subkey, "Publisher");
        let version = string_value(subkey, "DisplayVersion").filter(|v| !v.trim().is_empty());
        let icon = string_value(subkey, "DisplayIcon");
        let install = string_value(subkey, "InstallLocation").filter(|v| !v.trim().is_empty());
        let uninstall = string_value(subkey, "UninstallString").filter(|v| !v.trim().is_empty());
        close_key(subkey);

        // The icon's path is the executable, with a ",0" index behind it.
        let target = icon
            .as_deref()
            .map(|icon| icon.split(',').next().unwrap_or(icon).trim().to_string())
            .filter(|icon| !icon.is_empty())
            .or_else(|| install.clone())
            .or(uninstall)
            .unwrap_or_default();
        if target.is_empty() {
            continue;
        }

        // The version tells two copies of one program apart, which is what
        // upstream does with the version and the containing folder.
        let alternates = version.into_iter().collect::<Vec<_>>();

        found.push(
            Program::new(display_name, target, Source::Registry)
                .with_alternates(alternates)
                .with_publisher(publisher),
        );
    }
    close_key(key);
}

/// The programs the registry's App Paths keys name.
fn app_path_programs() -> Vec<Program> {
    let mut found = Vec::new();
    for (root, view) in [
        (HKEY_LOCAL_MACHINE, KEY_WOW64_64KEY),
        (HKEY_LOCAL_MACHINE, KEY_WOW64_32KEY),
        (HKEY_CURRENT_USER, KEY_WOW64_64KEY),
    ] {
        let Ok(key) = open_key(root, APP_PATHS_KEY, view) else {
            continue;
        };
        for name in subkey_names(key) {
            let Ok(subkey) = open_key(key, &name, view) else {
                continue;
            };
            let target = string_value(subkey, "").or_else(|| string_value(subkey, "Path"));
            close_key(subkey);
            let Some(target) = target else { continue };
            if target.trim().is_empty() {
                continue;
            }
            // The key is the executable's name, without its extension.
            let display = name.trim_end_matches(".exe").trim().to_string();
            if display.is_empty() {
                continue;
            }
            found.push(Program::new(display, target, Source::AppPath));
        }
        close_key(key);
    }
    found
}

/// Close a registry key.
fn close_key(key: HKEY) {
    // SAFETY: the handle came from open_key and is closed exactly once.
    let _ = unsafe { RegCloseKey(key) };
}

/// Open a registry key for reading.
fn open_key(
    root: HKEY,
    path: &str,
    view: windows::Win32::System::Registry::REG_SAM_FLAGS,
) -> Result<HKEY, ()> {
    let wide = to_wide(path);
    let mut key = HKEY::default();
    // SAFETY: the path is a null-terminated wide string, and the out-parameter
    // is a valid handle slot.
    let status =
        unsafe { RegOpenKeyExW(root, PCWSTR(wide.as_ptr()), None, KEY_READ | view, &mut key) };
    if status == ERROR_SUCCESS {
        Ok(key)
    } else {
        Err(())
    }
}

/// The names of a key's subkeys.
fn subkey_names(key: HKEY) -> Vec<String> {
    let mut names = Vec::new();
    let mut index = 0u32;
    loop {
        // A key's name is at most 255 characters, so the buffer is never too
        // small and the call never has to be repeated.
        let mut buffer = [0u16; 256];
        // SAFETY: the buffer holds 256 wide characters, and the name is
        // written into it and null-terminated.
        let status = unsafe { RegEnumKeyW(key, index, Some(&mut buffer)) };
        if status != ERROR_SUCCESS {
            break;
        }
        let length = buffer.iter().position(|c| *c == 0).unwrap_or(buffer.len());
        names.push(String::from_utf16_lossy(&buffer[..length]));
        index += 1;
    }
    names
}

/// A string value of a key.
fn string_value(key: HKEY, name: &str) -> Option<String> {
    let wide_name = to_wide(name);
    let mut size: u32 = 0;
    // SAFETY: the handle is open, and the first call asks for the size only.
    let status = unsafe {
        RegGetValueW(
            key,
            PCWSTR::null(),
            PCWSTR(wide_name.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            None,
            Some(&mut size),
        )
    };
    if status != ERROR_SUCCESS || size == 0 {
        return None;
    }
    let mut buffer = vec![0u16; size as usize / 2 + 1];
    // SAFETY: the buffer holds `size` bytes of wide characters, and the value
    // is written into it and null-terminated.
    let status = unsafe {
        RegGetValueW(
            key,
            PCWSTR::null(),
            PCWSTR(wide_name.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buffer.as_mut_ptr() as *mut c_void),
            Some(&mut size),
        )
    };
    if status != ERROR_SUCCESS {
        return None;
    }
    let length = buffer.iter().position(|c| *c == 0).unwrap_or(buffer.len());
    let text = String::from_utf16_lossy(&buffer[..length]);
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

/// A string, wide and null-terminated.
fn to_wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_machine_has_programs_to_open() {
        let found = programs();
        assert!(!found.is_empty(), "no programs were found at all");
    }

    #[test]
    fn every_program_has_a_name_and_something_to_open() {
        for program in programs() {
            assert!(!program.name.trim().is_empty(), "{program:?}");
            assert!(!program.target.trim().is_empty(), "{program:?}");
        }
    }

    #[test]
    fn the_start_menu_holds_the_names_windows_shows() {
        let found = start_menu_programs();
        assert!(!found.is_empty(), "the Start Menu was empty");
        assert!(found
            .iter()
            .all(|program| program.source == Source::StartMenu));
    }

    #[test]
    fn the_registry_names_the_installed_programs() {
        let found = registry_programs();
        assert!(!found.is_empty(), "no installed programs were found");
        assert!(found
            .iter()
            .all(|program| program.source == Source::Registry));
    }

    #[test]
    fn a_program_every_windows_has_is_found() {
        // Notepad is on every Windows 10 and 11, in the Start Menu.
        let found = programs();
        assert!(
            found
                .iter()
                .any(|program| program.name.eq_ignore_ascii_case("notepad")),
            "notepad was not found among {} programs",
            found.len()
        );
    }

    #[test]
    fn a_wide_string_is_null_terminated() {
        assert_eq!(to_wide("ab"), vec![0x61, 0x62, 0]);
    }
}
