use tc_platform::Paths;

/// The folders the app uses on Windows.
///
/// The data folder mirrors upstream so a backup moves between systems, and
/// the settings file keeps the same relative path so one dotfiles repository
/// serves both.
pub fn windows_paths() -> Paths {
    let home = std::env::var_os("USERPROFILE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let app_data = std::env::var_os("APPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| home.join("AppData").join("Roaming"));
    let local_app_data = std::env::var_os("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| home.join("AppData").join("Local"));

    let data_dir = app_data.join("yal");
    let cache_dir = local_app_data.join("yal");
    let settings_file = home.join(".config").join("tinycast").join("settings.json");

    Paths {
        data_dir,
        cache_dir,
        home_dir: home,
        settings_file,
    }
}

/// Whether the app is running from a portable folder, which keeps its data
/// beside the executable.
pub fn is_portable() -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    exe.parent()
        .map(|folder| folder.join("portable").exists())
        .unwrap_or(false)
}