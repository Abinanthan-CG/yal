//! Windows application discovery.
//!
//! Applications are found in four places, and each contributes a different
//! kind of entry:
//!
//! | Source | What it yields |
//! | --- | --- |
//! | Start Menu and desktop shortcuts | User-facing apps, `.lnk` and `.url`, with their launch target, arguments and working directory |
//! | Uninstall registry keys | Apps installed by a package manager, including ones with no shortcut |
//! | MSIX packages | Store apps, which have no shortcut in the Start Menu folder — read through `GetAppListEntries`, never through manifest guessing |
//! | `PATH` directories | Command line programs that never get installed properly |
//!
//! Every source is best-effort. A source that cannot be read is skipped, not
//! fatal, so a partially readable system still yields results.
//!
//! Games ride their launcher protocols: a `.url` shortcut whose URL scheme
//! belongs to a game launcher (`steam:`, Epic, GOG Galaxy, Ubisoft,
//! Battle.net, EA, Legacy Origin, Amazon Games) is an application whose
//! launch is the URL itself. Uninstallers are skipped by name, on the
//! shortcut and on the registry target alike.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Where an indexed application was discovered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Source {
    /// A Start Menu shortcut.
    StartMenu,
    /// A shortcut on the desktop.
    Desktop,
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
            Source::Desktop => "desktop",
            Source::Msix => "msix",
            Source::Registry => "registry",
            Source::Path => "path",
        }
    }
}

/// One discovered application.
///
/// The launch shape follows the entry's kind: a shortcut carries itself
/// (and the shell honours its stored arguments, working directory and
/// compatibility settings when the shortcut file is handed over), a Store
/// app carries its AUMID, a plain executable carries its program path.
/// One of `shortcut`, `aumid` or `program` is always present for a
/// launchable entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Discovered {
    /// A stable identifier derived from the launch target.
    pub id: String,
    /// The display name.
    pub name: String,
    /// A path that stands for the application: the resolved target of a
    /// shortcut, the executable of the rest. What the reveal act opens.
    pub path: PathBuf,
    /// Where the entry came from.
    pub source: Source,
    /// Extra search terms.
    pub keywords: Vec<String>,
    /// The shortcut file to hand the shell, for `.lnk` and `.url` entries.
    pub shortcut: Option<PathBuf>,
    /// Arguments the entry carries, for launches that pass them to a program.
    pub arguments: Option<String>,
    /// The working directory the program starts in.
    pub working_directory: Option<PathBuf>,
    /// The Store application's AUMID, when this entry is one.
    pub aumid: Option<String>,
    /// The URL, for `.url` shortcuts.
    pub url: Option<String>,
}

impl Discovered {
    /// An entry with no launch extras: the common shape for plain paths.
    pub fn plain(name: impl Into<String>, path: impl Into<PathBuf>, source: Source) -> Self {
        let name = name.into();
        let path: PathBuf = path.into();
        Self {
            id: identifier(&name, &path),
            name,
            path,
            source,
            keywords: Vec::new(),
            shortcut: None,
            arguments: None,
            working_directory: None,
            aumid: None,
            url: None,
        }
    }
}

/// Discover the applications installed on this host.
///
/// The Store is read first: a Start Menu shortcut whose AUMID names a
/// package that is already indexed is dropped in favour of the Store row,
/// the way the shell itself answers one application with one identity.
pub fn discover() -> Vec<Discovered> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut results: Vec<Discovered> = Vec::new();

    // Store apps: display name and AUMID straight from the package
    // manager's launchable-entry list.
    for (name, aumid, install) in msix_applications() {
        let key = format!("uwp:{}", aumid.to_lowercase());
        if !seen.insert(key) {
            continue;
        }
        let mut entry = Discovered::plain(name.clone(), install.clone().unwrap_or_default(), Source::Msix);
        entry.aumid = Some(aumid);
        entry.name = name;
        results.push(entry);
    }

    // Start Menu and desktop shortcuts: link and Internet-shortcut files.
    for (shortcut, source) in shortcut_files() {
        add_shortcut_entry(&mut seen, &mut results, &shortcut, source);
    }

    // Uninstall registry keys: installed programs without shortcuts.
    for (name, target) in registry_applications() {
        if looks_like_uninstaller(&target) {
            continue;
        }
        let key = format!("win32:{}", target.to_string_lossy().to_lowercase());
        if !seen.insert(key) {
            continue;
        }
        results.push(Discovered::plain(name, target, Source::Registry));
    }

    // PATH: command-line programs.
    for (name, target) in path_applications() {
        if looks_like_uninstaller(&target) {
            continue;
        }
        let key = format!("win32:{}", target.to_string_lossy().to_lowercase());
        if !seen.insert(key) {
            continue;
        }
        results.push(Discovered::plain(name, target, Source::Path));
    }

    results.sort_by_key(|application| application.name.to_lowercase());
    results
}

/// One Start Menu or desktop shortcut, of any readable kind.
fn add_shortcut_entry(
    seen: &mut BTreeSet<String>,
    results: &mut Vec<Discovered>,
    file: &Path,
    source: Source,
) {
    let Some(name) = file.file_stem().map(|stem| stem.to_string_lossy().into_owned()) else {
        return;
    };

    let extension = file
        .extension()
        .map(|ext| ext.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    let mut entry = match extension.as_str() {
        "lnk" => {
            let Some(resolved) = crate::lnk::resolve(file) else {
                return;
            };
            if is_msi_uninstall(&resolved.target, resolved.arguments.as_deref()) {
                return;
            }
            let key = format!(
                "win32:{} {}",
                resolved.target.to_string_lossy().to_lowercase(),
                resolved
                    .arguments
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
            );
            if !seen.insert(key) {
                return;
            }
            let mut entry = Discovered::plain(name, resolved.target, source);
            entry.shortcut = Some(file.to_path_buf());
            entry.arguments = resolved.arguments;
            entry.working_directory = resolved.working_directory;
            entry
        }
        "url" => {
            let Some(url) = crate::lnk::url_target(file) else {
                return;
            };
            let key = format!("win32:{}", url.to_lowercase());
            if !seen.insert(key) {
                return;
            }
            let mut entry = Discovered::plain(name, file.to_path_buf(), source);
            entry.shortcut = Some(file.to_path_buf());
            entry.url = Some(url);
            entry
        }
        // `.appref-ms` ClickOnce applications and anything else unknown:
        // the shortcut file itself launches through the shell, whatever
        // the format carries inside.
        _ if file.to_string_lossy().to_lowercase().ends_with(".appref-ms") => {
            let key = format!("win32:{}", file.to_string_lossy().to_lowercase());
            if !seen.insert(key) {
                return;
            }
            let mut entry = Discovered::plain(name, file.to_path_buf(), source);
            entry.shortcut = Some(file.to_path_buf());
            entry
        }
        _ => return,
    };

    if looks_like_uninstaller(&entry.path) {
        return;
    }
    entry.id = identifier(&entry.name, &entry.path);
    results.push(entry);
}

/// The game-launcher URL schemes: a `.url` shortcut with one of these is
/// a game whose launch act is the URL itself.
pub fn is_game_url(url: &str) -> bool {
    const SCHEMES: [&str; 9] = [
        "steam:",
        "com.epicgames.launcher:",
        "goggalaxy:",
        "uplay:",
        "ubisoft:",
        "battlenet:",
        "origin:",
        "link2ea:",
        "amazon-games:",
    ];
    let lower = url.to_lowercase();
    SCHEMES.iter().any(|scheme| lower.starts_with(scheme))
}

/// A file whose name marks it as an uninstaller: skipped, because a
/// launcher does not offer to uninstall things by accident.
fn looks_like_uninstaller(path: &Path) -> bool {
    path.file_name()
        .map(|name| name.to_string_lossy().to_lowercase().contains("unins"))
        .unwrap_or(false)
}

/// `msiexec.exe /x ...`: the registry's uninstall shape, never launchable.
fn is_msi_uninstall(target: &Path, arguments: Option<&str>) -> bool {
    target
        .file_name()
        .map(|name| name.to_string_lossy().to_lowercase() == "msiexec.exe")
        .unwrap_or(false)
        && arguments
            .map(|args| args.to_lowercase().contains("/x"))
            .unwrap_or(false)
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

// ---- shortcuts ------------------------------------------------------------

/// The shortcut files of the Start Menu folders and of both desktops,
/// each paired with its source.
///
/// The roots come from environment variables rather than the
/// known-folder COM API: the four folders live at fixed bearings from
/// `APPDATA`, `ProgramData`, `USERPROFILE` and `PUBLIC`, and those
/// bearings carry no COM apartment requirement to honour.
fn shortcut_files() -> Vec<(PathBuf, Source)> {
    let mut roots: Vec<(PathBuf, Source)> = Vec::new();
    for (variable, join_from, source) in [
        ("APPDATA", "Microsoft\\Windows\\Start Menu\\Programs", Source::StartMenu),
        ("ProgramData", "Microsoft\\Windows\\Start Menu\\Programs", Source::StartMenu),
        ("USERPROFILE", "Desktop", Source::Desktop),
        ("PUBLIC", "Desktop", Source::Desktop),
    ] {
        if let Some(value) = std::env::var_os(variable) {
            roots.push((PathBuf::from(value).join(join_from), source));
        }
    }

    let mut files = Vec::new();
    for (root, source) in roots {
        collect_shortcut_files(&root, source, 0, &mut files);
    }
    files
}

/// Walk a folder collecting shortcut files, up to a depth limit.
fn collect_shortcut_files(root: &Path, source: Source, depth: usize, out: &mut Vec<(PathBuf, Source)>) {
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
            collect_shortcut_files(&path, source, depth + 1, out);
        } else {
            out.push((path, source));
        }
    }
}

// ---- MSIX -----------------------------------------------------------------

/// Store applications: the display name, the AUMID and the install folder
/// of every launchable entry.
///
/// Store apps are enumerated through the Windows Runtime package manager
/// rather than by listing `Program Files\WindowsApps`, which is not
/// listable by an ordinary user. `FindPackagesForUser` with an empty
/// identifier means "the current user" and works from an ordinary
/// process; the elevated all-user enumeration is only a fallback.
///
/// The launchable shape comes from `GetAppListEntries` — exactly the
/// entries the Start Menu itself lists, each under its real AUMID and
/// display name. The first pipeline instead guessed from the manifest's
/// `Executable=` attribute, which most Store applications do not even
/// carry, and silently dropped them.
///
/// When the package manager is unavailable the source yields nothing
/// rather than failing.
#[cfg(windows)]
fn msix_applications() -> Vec<(String, String, Option<PathBuf>)> {
    use windows::core::HSTRING;
    use windows::Management::Deployment::PackageManager;
    use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};

    // The Windows Runtime must have an apartment on this thread before any
    // WinRT object is activated. Initialising it more than once is
    // harmless: the second call reports the existing apartment.
    unsafe {
        let _ = RoInitialize(RO_INIT_MULTITHREADED);
    }

    let Some(manager) = PackageManager::new().ok() else {
        return Vec::new();
    };

    // The current user's packages only: no elevation needed.
    let Ok(packages) = manager.FindPackagesByUserSecurityId(&HSTRING::new()) else {
        return Vec::new();
    };

    let mut found: Vec<(String, String, Option<PathBuf>)> = Vec::new();
    for package in packages {
        // Framework and resource packages are the scaffolding an app
        // rides on, not applications.
        if package.IsFramework().unwrap_or(true) || package.IsResourcePackage().unwrap_or(true) {
            continue;
        }
        let install = package
            .InstalledLocation()
            .ok()
            .and_then(|folder| folder.Path().ok())
            .map(|path| PathBuf::from(path.to_string()));

        let Ok(entries) = package.GetAppListEntries() else {
            // One bad package does not abort the scan.
            continue;
        };
        for entry in entries {
            let Ok(aumid) = entry.AppUserModelId() else { continue };
            if aumid.is_empty() {
                continue;
            }
            let name = entry
                .DisplayInfo()
                .ok()
                .and_then(|info| info.DisplayName().ok())
                .map(|display| display.to_string())
                .unwrap_or_default();
            if name.is_empty() {
                continue;
            }
            found.push((name, aumid.to_string(), install.clone()));
        }
    }
    found
}

#[cfg(not(windows))]
fn msix_applications() -> Vec<(String, String, Option<PathBuf>)> {
    Vec::new()
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
    fn game_launcher_urls_are_classified() {
        assert!(is_game_url("steam://rungameid/12345"));
        assert!(is_game_url("STEAM://Rungameid/12345"));
        assert!(is_game_url("com.epicgames.launcher://apps/fortnite"));
        assert!(is_game_url("link2ea://launchgame/1234"));
        assert!(!is_game_url("https://example.com"));
        assert!(!is_game_url("steamunofficial://x"));
    }

    #[test]
    fn uninstallers_are_recognised_by_name() {
        assert!(looks_like_uninstaller(Path::new(r"C:\app\unins000.exe")));
        assert!(looks_like_uninstaller(Path::new(r"C:\app\Uninstall Tool.exe")));
        assert!(!looks_like_uninstaller(Path::new(r"C:\app\game.exe")));
    }

    #[test]
    fn msi_uninstalls_are_recognised() {
        assert!(is_msi_uninstall(
            Path::new(r"C:\Windows\System32\msiexec.exe"),
            Some("/X {GUID}")
        ));
        assert!(!is_msi_uninstall(Path::new(r"C:\game\steam.exe"), Some("-applaunch 123")));
    }

    #[test]
    fn identifiers_ignore_name_case() {
        assert_eq!(
            identifier("Notepad", Path::new(r"C:\np.exe")),
            identifier("notepad", Path::new(r"C:\np.exe"))
        );
    }
}
