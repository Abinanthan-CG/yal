//! Windows application discovery.
//!
//! Applications are found in four places, and each contributes a different
//! kind of entry:
//!
//! | Source | What it yields |
//! | --- | --- |
//! | Start Menu shortcuts | User-facing apps, with an icon and a launch target |
//! | Uninstall registry keys | Apps installed by a package manager, including ones with no shortcut |
//! | MSIX packages | Store apps, which have no shortcut in the Start Menu folder |
//! | `PATH` directories | Command line programs that never get installed properly |
//!
//! Every source is best-effort. A source that cannot be read is skipped, not
//! fatal, so a partially readable system still yields results.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Where an indexed application was discovered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Source {
    /// A Start Menu shortcut.
    StartMenu,
    /// A Windows Store / MSIX package.
    Msix,
    /// An uninstall registry key.
    Registry,
    /// An executable found on `PATH`.
    Path,
}

impl Source {
    /// A stable name for the source.
    pub fn as_str(self) -> &'static str {
        match self {
            Source::StartMenu => "start_menu",
            Source::Msix => "msix",
            Source::Registry => "registry",
            Source::Path => "path",
        }
    }
}

/// One discovered application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Discovered {
    /// A stable identifier derived from the launch target.
    pub id: String,
    /// The display name.
    pub name: String,
    /// The executable to launch.
    pub path: PathBuf,
    /// Where the entry came from.
    pub source: Source,
    /// Extra search terms.
    pub keywords: Vec<String>,
}

/// Discover the applications installed on this host.
pub fn discover() -> Vec<Discovered> {
    let mut found: BTreeSet<(String, PathBuf)> = BTreeSet::new();
    let mut results: Vec<Discovered> = Vec::new();

    for shortcut in start_menu_shortcuts() {
        let Some(resolved) = crate::lnk::resolve(&shortcut.path) else {
            continue;
        };
        let name = shortcut.name;
        let key = (name.to_lowercase(), resolved.target.clone());
        if !found.insert(key) {
            continue;
        }
        results.push(Discovered {
            id: identifier(&name, &resolved.target),
            name,
            path: resolved.target,
            source: Source::StartMenu,
            keywords: Vec::new(),
        });
    }

    for (name, target) in registry_applications() {
        let key = (name.to_lowercase(), target.clone());
        if !found.insert(key) {
            continue;
        }
        results.push(Discovered {
            id: identifier(&name, &target),
            name,
            path: target,
            source: Source::Registry,
            keywords: Vec::new(),
        });
    }

    for (name, target) in msix_applications() {
        let key = (name.to_lowercase(), target.clone());
        if !found.insert(key) {
            continue;
        }
        results.push(Discovered {
            id: identifier(&name, &target),
            name,
            path: target,
            source: Source::Msix,
            keywords: Vec::new(),
        });
    }

    for (name, target) in path_applications() {
        let key = (name.to_lowercase(), target.clone());
        if !found.insert(key) {
            continue;
        }
        results.push(Discovered {
            id: identifier(&name, &target),
            name,
            path: target,
            source: Source::Path,
            keywords: Vec::new(),
        });
    }

    results.sort_by_key(|application| application.name.to_lowercase());
    results
}

/// A stable identifier for an application.
///
/// The path participates in the identity because two applications may share a
/// display name while living at different locations; they are different
/// applications and must not collide.
fn identifier(name: &str, path: &Path) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in name.to_lowercase().bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    for byte in path.to_string_lossy().to_lowercase().bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

// ---- Start Menu -----------------------------------------------------------

/// A shortcut discovered in a Start Menu folder.
struct Shortcut {
    name: String,
    path: PathBuf,
}

/// Find shortcuts in the per-user and machine-wide Start Menu folders.
fn start_menu_shortcuts() -> Vec<Shortcut> {
    let mut roots: Vec<PathBuf> = Vec::new();

    if let Some(app_data) = std::env::var_os("APPDATA") {
        roots.push(
            PathBuf::from(app_data)
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs"),
        );
    }
    if let Some(program_data) = std::env::var_os("ProgramData") {
        roots.push(
            PathBuf::from(program_data)
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs"),
        );
    }

    let mut shortcuts = Vec::new();
    for root in roots {
        collect_shortcuts(&root, &mut shortcuts, 0);
    }
    shortcuts
}

/// Walk a Start Menu folder collecting shortcuts, up to a depth limit.
fn collect_shortcuts(root: &Path, out: &mut Vec<Shortcut>, depth: usize) {
    const MAX_DEPTH: usize = 4;
    if depth > MAX_DEPTH {
        return;
    }
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_shortcuts(&path, out, depth + 1);
        } else if path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("lnk")) {
            let name = path
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.to_string_lossy().into_owned());
            out.push(Shortcut { name, path });
        }
    }
}

// ---- PATH -----------------------------------------------------------------

/// Find executables on `PATH`.
fn path_applications() -> Vec<(String, PathBuf)> {
    let Some(path_value) = std::env::var_os("PATH") else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for directory in std::env::split_paths(&path_value) {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("exe")) {
                if let Some(stem) = path.file_stem() {
                    found.push((stem.to_string_lossy().into_owned(), path));
                }
            }
        }
    }
    found
}

// ---- MSIX -----------------------------------------------------------------

/// Find Store applications.
///
/// Store apps are enumerated through the Windows Runtime package manager
/// rather than by listing `Program Files\WindowsApps`. The folder itself is
/// not readable by an ordinary user, but the package manager is, so this
/// route works without elevation. When the package manager is unavailable
/// the source yields nothing rather than failing.
fn msix_applications() -> Vec<(String, PathBuf)> {
    let packages = match enumerate_packages() {
        Some(packages) => packages,
        None => return Vec::new(),
    };

    let mut found = Vec::new();
    for (display_name, install_location) in packages {
        let Some(executable) = find_package_executable(&install_location) else {
            continue;
        };
        found.push((display_name, executable));
    }
    found
}

/// Enumerate the installed packages, returning their display name and
/// installation folder.
#[cfg(windows)]
fn enumerate_packages() -> Option<Vec<(String, PathBuf)>> {
    use windows::Management::Deployment::PackageManager;
    use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};

    // The Windows Runtime must have an apartment on this thread before any
    // WinRT object is activated. Initialising it more than once is
    // harmless: the second call reports the existing apartment.
    unsafe {
        let _ = RoInitialize(RO_INIT_MULTITHREADED);
    }

    let manager = PackageManager::new().ok()?;

    // `FindPackages` enumerates packages for every user on the machine and
    // therefore needs elevation. `FindPackagesByUserSecurityId` with an empty
    // identifier means "the current user" and works from an ordinary process,
    // which is what a launcher needs. The elevated call is only used as a
    // fallback when the current-user call is unavailable.
    let packages = manager
        .FindPackagesByUserSecurityId(&windows::core::HSTRING::from(""))
        .or_else(|_| manager.FindPackages())
        .ok()?;

    let mut found = Vec::new();
    for package in packages {
        let Ok(id) = package.Id() else { continue };
        let Ok(name) = id.Name() else { continue };
        let Ok(location) = package.InstalledLocation() else {
            continue;
        };
        let Ok(path) = location.Path() else { continue };

        // The package's display name is the user-facing name; the package
        // identifier is the fallback when a package declares no display name.
        let display = package.DisplayName().ok().map(|h| h.to_string()).unwrap_or_else(|| {
            let raw = name.to_string();
            match raw.split_once('_') {
                Some((head, _)) => head.to_string(),
                None => raw,
            }
        });

        found.push((display, PathBuf::from(path.to_string())));
    }
    Some(found)
}

#[cfg(not(windows))]
fn enumerate_packages() -> Option<Vec<(String, PathBuf)>> {
    None
}

/// Find the entry executable inside a package folder.
fn find_package_executable(package_root: &Path) -> Option<PathBuf> {
    let manifest = package_root.join("AppxManifest.xml");
    let contents = std::fs::read_to_string(&manifest).ok()?;
    let executable = extract_executable(&contents)?;
    let candidate = package_root.join(&executable);
    if candidate.exists() { Some(candidate) } else { None }
}

/// Extract the application executable named by an Appx manifest.
///
/// The manifest names the executable as `Executable="path\to\app.exe"`. The
/// attribute may appear several times — once for the application and once per
/// extension — and the first occurrence is the application itself.
fn extract_executable(manifest: &str) -> Option<String> {
    let chunk = manifest.split("Executable=").nth(1)?.trim_start();
    let value = if let Some(rest) = chunk.strip_prefix('"') {
        rest.split('"').next()?.to_string()
    } else {
        chunk.split_whitespace().next()?.to_string()
    };
    if value.is_empty() { None } else { Some(value) }
}

// ---- registry -------------------------------------------------------------

/// Find applications through the uninstall registry keys.
#[cfg(windows)]
fn registry_applications() -> Vec<(String, PathBuf)> {
    use windows::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY,
        KEY_WOW64_64KEY, REG_SAM_FLAGS,
    };

    const UNINSTALL: &str = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall";

    let read = KEY_READ.0;
    let roots: [(HKEY, &str, REG_SAM_FLAGS); 4] = [
        (
            HKEY_LOCAL_MACHINE,
            UNINSTALL,
            REG_SAM_FLAGS(read | KEY_WOW64_64KEY.0),
        ),
        (
            HKEY_LOCAL_MACHINE,
            UNINSTALL,
            REG_SAM_FLAGS(read | KEY_WOW64_32KEY.0),
        ),
        (HKEY_CURRENT_USER, UNINSTALL, REG_SAM_FLAGS(read)),
        (
            HKEY_CURRENT_USER,
            UNINSTALL,
            REG_SAM_FLAGS(read | KEY_WOW64_32KEY.0),
        ),
    ];

    let mut found = Vec::new();
    for (root, subkey, access) in roots {
        let Some(handle) = open_key(root, subkey, access) else {
            continue;
        };
        let subkeys = enum_subkeys(handle);

        for subkey in subkeys {
            let Some(child) = open_key(handle, &subkey, KEY_READ) else {
                continue;
            };
            let name = read_string_value(child, "DisplayName");
            let icon = read_string_value(child, "DisplayIcon");
            let location = read_string_value(child, "InstallLocation");
            let _ = unsafe { windows::Win32::System::Registry::RegCloseKey(child) };

            let Some(name) = name else { continue };
            if name.is_empty() {
                continue;
            }
            let Some(target) = icon
                .or(location)
                .map(|value| strip_icon_suffix(&value))
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            found.push((name, PathBuf::from(target)));
        }

        let _ = unsafe { windows::Win32::System::Registry::RegCloseKey(handle) };
    }
    found
}

#[cfg(windows)]
fn open_key(
    root: windows::Win32::System::Registry::HKEY,
    subkey: &str,
    access: windows::Win32::System::Registry::REG_SAM_FLAGS,
) -> Option<windows::Win32::System::Registry::HKEY> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::ERROR_SUCCESS;
    use windows::Win32::System::Registry::{HKEY, RegOpenKeyExW};

    let wide: Vec<u16> = subkey.encode_utf16().chain(std::iter::once(0)).collect();
    let mut handle = HKEY::default();
    let status =
        unsafe { RegOpenKeyExW(root, PCWSTR(wide.as_ptr()), None, access, &mut handle) };
    if status != ERROR_SUCCESS {
        return None;
    }
    Some(handle)
}

#[cfg(windows)]
fn enum_subkeys(handle: windows::Win32::System::Registry::HKEY) -> Vec<String> {
    use windows::core::PWSTR;
    use windows::Win32::Foundation::ERROR_SUCCESS;
    use windows::Win32::System::Registry::RegEnumKeyExW;

    let mut names = Vec::new();
    let mut index = 0u32;
    loop {
        let mut buffer = [0u16; 256];
        let mut length = buffer.len() as u32;
        let status = unsafe {
            RegEnumKeyExW(
                handle,
                index,
                Some(PWSTR(buffer.as_mut_ptr())),
                &mut length,
                None,
                None,
                None,
                None,
            )
        };
        if status != ERROR_SUCCESS {
            break;
        }
        let end = buffer
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(length as usize);
        names.push(String::from_utf16_lossy(&buffer[..end]));
        index += 1;
        if index > 4096 {
            break;
        }
    }
    names
}

#[cfg(windows)]
fn read_string_value(
    handle: windows::Win32::System::Registry::HKEY,
    name: &str,
) -> Option<String> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::ERROR_SUCCESS;
    use windows::Win32::System::Registry::{REG_SZ, RegQueryValueExW};

    let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();

    // First call asks for the size of the value.
    let mut size = 0u32;
    let status = unsafe {
        RegQueryValueExW(
            handle,
            PCWSTR(wide.as_ptr()),
            None,
            None,
            None,
            Some(&mut size),
        )
    };
    if status != ERROR_SUCCESS || size == 0 {
        return None;
    }

    let mut buffer = vec![0u16; (size as usize / 2) + 1];
    let mut size = buffer.len() as u32;
    let mut kind = REG_SZ;
    let status = unsafe {
        RegQueryValueExW(
            handle,
            PCWSTR(wide.as_ptr()),
            None,
            Some(&mut kind),
            Some(buffer.as_mut_ptr().cast::<u8>()),
            Some(&mut size),
        )
    };
    if status != ERROR_SUCCESS {
        return None;
    }
    if kind != REG_SZ {
        return None;
    }
    let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    let value = String::from_utf16_lossy(&buffer[..end]);
    if value.is_empty() { None } else { Some(value) }
}

#[cfg(not(windows))]
fn registry_applications() -> Vec<(String, PathBuf)> {
    Vec::new()
}

/// Remove the `,0` suffix that registry icon paths carry.
///
/// The `DisplayIcon` value has the form `"C:\path\app.exe",0`: a quoted path
/// followed by an icon index. An unquoted path with a trailing index is also
/// accepted.
fn strip_icon_suffix(value: &str) -> String {
    let trimmed = value.trim();

    if let Some(rest) = trimmed.strip_prefix('"') {
        if let Some(end) = rest.find('"') {
            let path = &rest[..end];
            let after = rest[end + 1..].trim();
            let valid_suffix = after.is_empty()
                || after
                    .strip_prefix(',')
                    .is_some_and(|digits| {
                        !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit())
                    });
            if valid_suffix {
                return path.to_string();
            }
        }
    }

    let bare = trimmed.trim_matches('"');
    match bare.rsplit_once(',') {
        Some((head, tail)) if !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit()) => {
            head.to_string()
        }
        _ => bare.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_icon_suffixes() {
        assert_eq!(strip_icon_suffix(r#""C:\app.exe",0"#), r"C:\app.exe");
        assert_eq!(strip_icon_suffix(r"C:\app.exe,2"), r"C:\app.exe");
        assert_eq!(strip_icon_suffix(r"C:\app.exe"), r"C:\app.exe");
        assert_eq!(strip_icon_suffix(r"C:\a,b.exe"), r"C:\a,b.exe");
    }

    #[test]
    fn identifiers_are_stable_and_distinct() {
        let a = identifier("Visual Studio Code", Path::new(r"C:\vs\code.exe"));
        let b = identifier("Visual Studio Code", Path::new(r"C:\vs\code.exe"));
        let c = identifier("Notepad", Path::new(r"C:\np.exe"));
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn identifiers_distinguish_same_name_different_path() {
        let a = identifier("Editor", Path::new(r"C:\one\editor.exe"));
        let b = identifier("Editor", Path::new(r"C:\two\editor.exe"));
        assert_ne!(a, b);
    }

    #[test]
    fn extracts_the_executable_from_a_manifest() {
        // Shapes after the real installed product's manifest, observed
        // during indexing. This file documents that the extraction covers
        // that exact form, so a regression is caught before it matters.
        let manifest = r#"<Application Executable="Raycast\Raycast.exe" EntryPoint="Windows.FullTrustApplication"/>"#;
        assert_eq!(
            extract_executable(manifest).as_deref(),
            Some(r"Raycast\Raycast.exe")
        );
    }

    #[test]
    fn takes_the_first_executable_in_a_manifest() {
        let manifest = concat!(
            r#"<Application Executable="App\App.exe" EntryPoint="Windows.FullTrustApplication"/>"#,
            r#"<uap:Extension Executable="App\Helper.exe"/>"#,
        );
        assert_eq!(
            extract_executable(manifest).as_deref(),
            Some(r"App\App.exe")
        );
    }

    #[test]
    fn handles_an_unquoted_executable_attribute() {
        let manifest = r#"<Application Executable=App.exe />"#;
        assert_eq!(extract_executable(manifest).as_deref(), Some("App.exe"));
    }

    #[test]
    fn a_manifest_without_an_executable_yields_nothing() {
        assert_eq!(extract_executable("<Package/>"), None);
    }

    #[test]
    fn identifiers_ignore_name_case() {
        assert_eq!(
            identifier("Notepad", Path::new(r"C:\np.exe")),
            identifier("notepad", Path::new(r"C:\np.exe"))
        );
    }
}
