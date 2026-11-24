//! Windows Settings (`ms-settings:`) and classic Control Panel / Admin Tools
//! search provider for YAL.

/// Prefix carried by a Windows Settings row id.
pub const WIN_SETTING_PREFIX: &str = "win-setting:";

/// Section heading under which Windows Settings rows appear in the palette.
pub const WIN_SETTING_SECTION: &str = "Windows Settings";

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// A built-in Windows Settings page or classic administrative tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WinSettingItem {
    pub id: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    pub target: &'static str,
    pub keywords: &'static [&'static str],
}

const CATALOGUE: &[WinSettingItem] = &[
    // System
    WinSettingItem {
        id: "display",
        name: "Display",
        category: "System",
        target: "ms-settings:display",
        keywords: &[
            "monitor",
            "screen",
            "resolution",
            "scale",
            "dpi",
            "hdr",
            "refresh rate",
            "brightness",
        ],
    },
    WinSettingItem {
        id: "sound",
        name: "Sound",
        category: "System",
        target: "ms-settings:sound",
        keywords: &[
            "audio",
            "volume",
            "speaker",
            "speakers",
            "headphones",
            "microphone",
            "output",
            "input",
        ],
    },
    WinSettingItem {
        id: "notifications",
        name: "Notifications",
        category: "System",
        target: "ms-settings:notifications",
        keywords: &[
            "alerts",
            "banners",
            "toast",
            "do not disturb",
            "dnd",
            "action center",
        ],
    },
    WinSettingItem {
        id: "focus",
        name: "Focus",
        category: "System",
        target: "ms-settings:quiethours",
        keywords: &[
            "focus assist",
            "quiet hours",
            "do not disturb",
            "dnd",
            "pomodoro",
            "distractions",
        ],
    },
    WinSettingItem {
        id: "power-battery",
        name: "Power & Battery",
        category: "System",
        target: "ms-settings:powersleep",
        keywords: &[
            "sleep",
            "screen timeout",
            "power mode",
            "hibernate",
            "energy",
            "lid",
            "charging",
        ],
    },
    WinSettingItem {
        id: "battery-saver",
        name: "Battery Saver",
        category: "System",
        target: "ms-settings:batterysaver",
        keywords: &["energy saver", "low power", "battery life", "power saving"],
    },
    WinSettingItem {
        id: "storage",
        name: "Storage",
        category: "System",
        target: "ms-settings:storagesense",
        keywords: &[
            "disk space",
            "storage sense",
            "drive",
            "cleanup",
            "temporary files",
            "free space",
        ],
    },
    WinSettingItem {
        id: "multitasking",
        name: "Multitasking",
        category: "System",
        target: "ms-settings:multitasking",
        keywords: &[
            "snap",
            "snap layouts",
            "alt tab",
            "virtual desktops",
            "window snapping",
        ],
    },
    WinSettingItem {
        id: "about",
        name: "About",
        category: "System",
        target: "ms-settings:about",
        keywords: &[
            "system info",
            "pc specs",
            "hardware",
            "ram",
            "cpu",
            "windows version",
            "rename pc",
            "device name",
        ],
    },
    WinSettingItem {
        id: "night-light",
        name: "Night Light",
        category: "System",
        target: "ms-settings:nightlight",
        keywords: &[
            "blue light",
            "warm colors",
            "eye strain",
            "flux",
            "color temperature",
            "display",
        ],
    },
    // Bluetooth & Devices
    WinSettingItem {
        id: "bluetooth",
        name: "Bluetooth & Devices",
        category: "Bluetooth & Devices",
        target: "ms-settings:bluetooth",
        keywords: &[
            "pair",
            "wireless",
            "headphones",
            "earbuds",
            "controller",
            "peripherals",
            "devices",
        ],
    },
    WinSettingItem {
        id: "printers",
        name: "Printers & Scanners",
        category: "Bluetooth & Devices",
        target: "ms-settings:printers",
        keywords: &[
            "print",
            "printer",
            "scanner",
            "fax",
            "print queue",
            "spooler",
        ],
    },
    WinSettingItem {
        id: "mouse",
        name: "Mouse",
        category: "Bluetooth & Devices",
        target: "ms-settings:mousetouchpad",
        keywords: &[
            "cursor",
            "pointer",
            "scroll",
            "sensitivity",
            "dpi",
            "primary button",
        ],
    },
    WinSettingItem {
        id: "touchpad",
        name: "Touchpad",
        category: "Bluetooth & Devices",
        target: "ms-settings:devices-touchpad",
        keywords: &[
            "trackpad",
            "gestures",
            "tap to click",
            "scrolling",
            "precision touchpad",
        ],
    },
    WinSettingItem {
        id: "keyboard",
        name: "Keyboard",
        category: "Bluetooth & Devices",
        target: "ms-settings:typing",
        keywords: &[
            "typing",
            "autocorrect",
            "spellcheck",
            "touch keyboard",
            "text suggestions",
        ],
    },
    // Network & Internet
    WinSettingItem {
        id: "network",
        name: "Network & Internet",
        category: "Network & Internet",
        target: "ms-settings:network-status",
        keywords: &[
            "internet",
            "connection",
            "network status",
            "offline",
            "online",
            "data usage",
        ],
    },
    WinSettingItem {
        id: "wifi",
        name: "Wi-Fi",
        category: "Network & Internet",
        target: "ms-settings:network-wifi",
        keywords: &["wifi", "wireless", "wlan", "ssid", "hotspot", "internet"],
    },
    WinSettingItem {
        id: "ethernet",
        name: "Ethernet",
        category: "Network & Internet",
        target: "ms-settings:network-ethernet",
        keywords: &["lan", "wired", "cable", "nic", "ip address", "dns"],
    },
    WinSettingItem {
        id: "vpn",
        name: "VPN",
        category: "Network & Internet",
        target: "ms-settings:network-vpn",
        keywords: &[
            "virtual private network",
            "tunnel",
            "wireguard",
            "openvpn",
            "corporate network",
        ],
    },
    WinSettingItem {
        id: "proxy",
        name: "Proxy",
        category: "Network & Internet",
        target: "ms-settings:network-proxy",
        keywords: &[
            "http proxy",
            "pac",
            "manual proxy",
            "automatic proxy",
            "network",
        ],
    },
    // Personalization
    WinSettingItem {
        id: "personalization",
        name: "Personalization",
        category: "Personalization",
        target: "ms-settings:personalization",
        keywords: &["appearance", "customize", "desktop", "look and feel", "ui"],
    },
    WinSettingItem {
        id: "background",
        name: "Background",
        category: "Personalization",
        target: "ms-settings:personalization-background",
        keywords: &[
            "wallpaper",
            "desktop background",
            "slideshow",
            "spotlight",
            "picture",
        ],
    },
    WinSettingItem {
        id: "colors",
        name: "Colors",
        category: "Personalization",
        target: "ms-settings:colors",
        keywords: &[
            "dark mode",
            "light mode",
            "accent color",
            "transparency",
            "theme mode",
        ],
    },
    WinSettingItem {
        id: "themes",
        name: "Themes",
        category: "Personalization",
        target: "ms-settings:themes",
        keywords: &[
            "desktop theme",
            "cursor theme",
            "sound scheme",
            "appearance",
        ],
    },
    WinSettingItem {
        id: "lock-screen",
        name: "Lock Screen",
        category: "Personalization",
        target: "ms-settings:lockscreen",
        keywords: &[
            "lockscreen",
            "screen saver",
            "screensaver",
            "spotlight",
            "login background",
        ],
    },
    WinSettingItem {
        id: "taskbar",
        name: "Taskbar",
        category: "Personalization",
        target: "ms-settings:taskbar",
        keywords: &[
            "system tray",
            "auto hide",
            "alignment",
            "start button",
            "widgets",
            "corner icons",
        ],
    },
    WinSettingItem {
        id: "fonts",
        name: "Fonts",
        category: "Personalization",
        target: "ms-settings:fonts",
        keywords: &["typeface", "typography", "ttf", "otf", "install font"],
    },
    // Apps
    WinSettingItem {
        id: "apps-features",
        name: "Apps & Features",
        category: "Apps",
        target: "ms-settings:appsfeatures",
        keywords: &[
            "programs",
            "uninstall",
            "modify",
            "remove app",
            "applications",
        ],
    },
    WinSettingItem {
        id: "installed-apps",
        name: "Installed Apps",
        category: "Apps",
        target: "ms-settings:installed-apps",
        keywords: &[
            "uninstall",
            "programs",
            "software",
            "remove",
            "applications",
            "app list",
        ],
    },
    WinSettingItem {
        id: "default-apps",
        name: "Default Apps",
        category: "Apps",
        target: "ms-settings:defaultapps",
        keywords: &[
            "file associations",
            "default browser",
            "open with",
            "protocols",
            "extensions",
        ],
    },
    WinSettingItem {
        id: "startup-apps",
        name: "Startup Apps",
        category: "Apps",
        target: "ms-settings:startupapps",
        keywords: &[
            "autostart",
            "boot",
            "login items",
            "launch at startup",
            "startup programs",
        ],
    },
    WinSettingItem {
        id: "optional-features",
        name: "Optional Features",
        category: "Apps",
        target: "ms-settings:optionalfeatures",
        keywords: &[
            "windows features",
            "openssh",
            "rsat",
            "add feature",
            "components",
        ],
    },
    // Accounts
    WinSettingItem {
        id: "accounts",
        name: "Accounts",
        category: "Accounts",
        target: "ms-settings:yourinfo",
        keywords: &[
            "profile",
            "user account",
            "microsoft account",
            "your info",
            "avatar",
        ],
    },
    WinSettingItem {
        id: "sign-in-options",
        name: "Sign-in Options",
        category: "Accounts",
        target: "ms-settings:signinoptions",
        keywords: &[
            "password",
            "pin",
            "windows hello",
            "fingerprint",
            "face recognition",
            "passkey",
            "security key",
        ],
    },
    // Time & Language
    WinSettingItem {
        id: "time-language",
        name: "Time & Language",
        category: "Time & Language",
        target: "ms-settings:regionlanguage",
        keywords: &[
            "language",
            "locale",
            "keyboard layout",
            "display language",
            "speech",
        ],
    },
    WinSettingItem {
        id: "date-time",
        name: "Date & Time",
        category: "Time & Language",
        target: "ms-settings:dateandtime",
        keywords: &[
            "clock",
            "timezone",
            "time zone",
            "ntp",
            "sync time",
            "calendar",
            "daylight saving",
        ],
    },
    WinSettingItem {
        id: "language-region",
        name: "Language & Region",
        category: "Time & Language",
        target: "ms-settings:regionformatting",
        keywords: &[
            "country",
            "region",
            "regional format",
            "currency",
            "date format",
            "locale",
        ],
    },
    // Gaming
    WinSettingItem {
        id: "gaming",
        name: "Gaming",
        category: "Gaming",
        target: "ms-settings:gaming-gamebar",
        keywords: &[
            "xbox",
            "game bar",
            "captures",
            "screenshots",
            "recording",
            "overlay",
        ],
    },
    WinSettingItem {
        id: "game-mode",
        name: "Game Mode",
        category: "Gaming",
        target: "ms-settings:gaming-gamemode",
        keywords: &["gaming", "fps", "performance", "optimize games", "xbox"],
    },
    // Accessibility
    WinSettingItem {
        id: "accessibility",
        name: "Accessibility",
        category: "Accessibility",
        target: "ms-settings:easeofaccess-display",
        keywords: &[
            "ease of access",
            "text size",
            "magnifier",
            "narrator",
            "high contrast",
            "color filters",
        ],
    },
    // Privacy & Security
    WinSettingItem {
        id: "privacy-security",
        name: "Privacy & Security",
        category: "Privacy & Security",
        target: "ms-settings:privacy",
        keywords: &[
            "permissions",
            "diagnostics",
            "telemetry",
            "location",
            "security",
        ],
    },
    WinSettingItem {
        id: "windows-security",
        name: "Windows Security",
        category: "Privacy & Security",
        target: "ms-settings:windowsdefender",
        keywords: &[
            "defender",
            "antivirus",
            "virus",
            "threat protection",
            "malware",
            "firewall",
            "smartscreen",
        ],
    },
    WinSettingItem {
        id: "camera-privacy",
        name: "Camera Privacy",
        category: "Privacy & Security",
        target: "ms-settings:privacy-webcam",
        keywords: &["webcam", "video", "camera permissions", "privacy"],
    },
    WinSettingItem {
        id: "microphone-privacy",
        name: "Microphone Privacy",
        category: "Privacy & Security",
        target: "ms-settings:privacy-microphone",
        keywords: &["mic", "audio permissions", "voice", "recording", "privacy"],
    },
    // Windows Update & System Maintenance
    WinSettingItem {
        id: "windows-update",
        name: "Windows Update",
        category: "Windows Update",
        target: "ms-settings:windowsupdate",
        keywords: &[
            "updates",
            "patches",
            "upgrade",
            "check for updates",
            "restart",
            "drivers",
        ],
    },
    WinSettingItem {
        id: "recovery",
        name: "Recovery",
        category: "System",
        target: "ms-settings:recovery",
        keywords: &[
            "reset pc",
            "factory reset",
            "advanced startup",
            "safe mode",
            "go back",
            "troubleshoot",
        ],
    },
    WinSettingItem {
        id: "developer-settings",
        name: "Developer Settings",
        category: "System",
        target: "ms-settings:developers",
        keywords: &[
            "developer mode",
            "for developers",
            "sideload",
            "sudo",
            "end task",
            "powershell",
            "terminal",
        ],
    },
    WinSettingItem {
        id: "clipboard-settings",
        name: "Clipboard Settings",
        category: "System",
        target: "ms-settings:clipboard",
        keywords: &["clipboard history", "sync clipboard", "win+v", "copy paste"],
    },
    // Classic Control Panel & Admin Tools
    WinSettingItem {
        id: "task-manager",
        name: "Task Manager",
        category: "Admin Tools",
        target: "taskmgr.exe",
        keywords: &[
            "taskmgr",
            "processes",
            "cpu",
            "memory",
            "gpu",
            "performance",
            "kill process",
            "end task",
        ],
    },
    WinSettingItem {
        id: "device-manager",
        name: "Device Manager",
        category: "Admin Tools",
        target: "devmgmt.msc",
        keywords: &["devmgmt", "drivers", "hardware", "usb", "pci", "devices"],
    },
    WinSettingItem {
        id: "disk-management",
        name: "Disk Management",
        category: "Admin Tools",
        target: "diskmgmt.msc",
        keywords: &[
            "diskmgmt",
            "partitions",
            "volumes",
            "format drive",
            "drive letter",
            "storage",
        ],
    },
    WinSettingItem {
        id: "services",
        name: "Services",
        category: "Admin Tools",
        target: "services.msc",
        keywords: &[
            "windows services",
            "daemons",
            "background services",
            "service control",
        ],
    },
    WinSettingItem {
        id: "registry-editor",
        name: "Registry Editor",
        category: "Admin Tools",
        target: "regedit.exe",
        keywords: &["regedit", "registry", "hkey", "hkcu", "hklm"],
    },
    WinSettingItem {
        id: "event-viewer",
        name: "Event Viewer",
        category: "Admin Tools",
        target: "eventvwr.msc",
        keywords: &[
            "eventvwr",
            "logs",
            "system logs",
            "application logs",
            "crash logs",
            "diagnostics",
        ],
    },
    WinSettingItem {
        id: "control-panel",
        name: "Control Panel",
        category: "Control Panel",
        target: "control.exe",
        keywords: &["classic settings", "applets", "windows control panel"],
    },
    WinSettingItem {
        id: "programs-features",
        name: "Programs and Features",
        category: "Control Panel",
        target: "appwiz.cpl",
        keywords: &[
            "appwiz",
            "uninstall",
            "change program",
            "remove software",
            "turn windows features",
        ],
    },
    WinSettingItem {
        id: "network-connections",
        name: "Network Connections",
        category: "Control Panel",
        target: "ncpa.cpl",
        keywords: &[
            "ncpa",
            "adapters",
            "network adapters",
            "ethernet",
            "wifi",
            "ip",
            "dns",
        ],
    },
    WinSettingItem {
        id: "sound-control-panel",
        name: "Sound Control Panel",
        category: "Control Panel",
        target: "mmsys.cpl",
        keywords: &[
            "mmsys",
            "audio devices",
            "playback",
            "recording",
            "sound devices",
            "speakers",
        ],
    },
    WinSettingItem {
        id: "system-properties",
        name: "System Properties",
        category: "Control Panel",
        target: "sysdm.cpl",
        keywords: &[
            "sysdm",
            "environment variables",
            "computer name",
            "virtual memory",
            "pagefile",
            "advanced system settings",
        ],
    },
    WinSettingItem {
        id: "windows-defender-firewall",
        name: "Windows Defender Firewall",
        category: "Admin Tools",
        target: "wf.msc",
        keywords: &[
            "firewall",
            "inbound rules",
            "outbound rules",
            "ports",
            "network security",
            "wf",
        ],
    },
    WinSettingItem {
        id: "credential-manager",
        name: "Credential Manager",
        category: "Control Panel",
        target: "control.exe /name Microsoft.CredentialManager",
        keywords: &[
            "credentials",
            "passwords",
            "windows credentials",
            "web credentials",
            "vault",
        ],
    },
    WinSettingItem {
        id: "resource-monitor",
        name: "Resource Monitor",
        category: "Admin Tools",
        target: "resmon.exe",
        keywords: &[
            "resmon",
            "cpu",
            "disk",
            "network",
            "memory",
            "handles",
            "performance",
        ],
    },
    WinSettingItem {
        id: "system-information",
        name: "System Information",
        category: "Admin Tools",
        target: "msinfo32.exe",
        keywords: &[
            "msinfo32",
            "sysinfo",
            "bios",
            "motherboard",
            "hardware",
            "diagnostics",
        ],
    },
    WinSettingItem {
        id: "disk-cleanup",
        name: "Disk Cleanup",
        category: "Admin Tools",
        target: "cleanmgr.exe",
        keywords: &["cleanmgr", "temp files", "free space", "cleanup", "storage"],
    },
];

/// Returns the complete built-in catalogue of Windows Settings and admin tools.
pub fn all() -> &'static [WinSettingItem] {
    CATALOGUE
}

/// Searches the catalogue using case-insensitive multi-token matching.
///
/// Returns an empty list for an empty or whitespace-only query.
pub fn search(query: &str, limit: usize) -> Vec<&'static WinSettingItem> {
    let trimmed = query.trim();
    if trimmed.is_empty() || limit == 0 {
        return Vec::new();
    }

    let tokens: Vec<String> = trimmed.split_whitespace().map(str::to_lowercase).collect();
    if tokens.is_empty() {
        return Vec::new();
    }
    let full_query = tokens.join(" ");

    let mut matches: Vec<(u8, usize, &'static WinSettingItem)> = CATALOGUE
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            score_item(item, &full_query, &tokens).map(|score| (score, index, item))
        })
        .collect();

    matches.sort_by_key(|&(score, index, _)| (score, index));
    matches.truncate(limit);
    matches.into_iter().map(|(_, _, item)| item).collect()
}

/// Finds a Windows Settings item by bare id or `win-setting:<id>`.
pub fn find_by_id(id: &str) -> Option<&'static WinSettingItem> {
    let bare = id.strip_prefix(WIN_SETTING_PREFIX).unwrap_or(id);
    CATALOGUE.iter().find(|item| item.id == bare)
}

/// Launches a Windows Settings URI or classic administrative tool target.
pub fn launch(target: &str) -> Result<(), String> {
    let trimmed = target.trim();
    if trimmed.is_empty() {
        return Err("Windows setting target is empty".to_string());
    }

    let mut parts = trimmed.split_whitespace();
    let Some(program) = parts.next() else {
        return Err("Windows setting target is empty".to_string());
    };
    let args: Vec<&str> = parts.collect();

    if !args.is_empty() {
        let mut cmd = std::process::Command::new(program);
        cmd.args(args);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        cmd.spawn()
            .map(|_| ())
            .map_err(|error| format!("Windows could not launch `{trimmed}`: {error}"))
    } else {
        tc_platform_win::open(trimmed)
    }
}

/// Converts a Windows Settings item into a launcher palette result row.
pub fn to_result_row(item: &'static WinSettingItem) -> crate::launcher::ResultRow {
    crate::launcher::ResultRow {
        id: format!("{WIN_SETTING_PREFIX}{}", item.id),
        name: item.name.to_string(),
        detail: format!("{} · {}", item.category, item.target),
        kind: WIN_SETTING_SECTION.to_string(),
        target: item.target.to_string(),
        ..crate::launcher::ResultRow::default()
    }
}

fn score_item(item: &WinSettingItem, full_query: &str, tokens: &[String]) -> Option<u8> {
    let name = item.name.to_lowercase();
    let category = item.category.to_lowercase();
    let target = item.target.to_lowercase();
    let keywords: Vec<String> = item.keywords.iter().map(|k| k.to_lowercase()).collect();

    let all_tokens_match = tokens.iter().all(|token| {
        let t = token.as_str();
        name.contains(t)
            || category.contains(t)
            || target.contains(t)
            || keywords.iter().any(|k| k.contains(t))
    });
    if !all_tokens_match {
        return None;
    }

    let score = if name == full_query {
        0
    } else if name.starts_with(full_query) {
        1
    } else if name.contains(full_query) {
        2
    } else if tokens.iter().all(|t| name.contains(t.as_str())) {
        3
    } else if keywords.iter().any(|k| k == full_query) {
        4
    } else if keywords.iter().any(|k| k.starts_with(full_query)) {
        5
    } else if keywords.iter().any(|k| k.contains(full_query)) {
        6
    } else if tokens
        .iter()
        .all(|t| name.contains(t.as_str()) || keywords.iter().any(|k| k.contains(t.as_str())))
    {
        7
    } else if category.contains(full_query) || target.contains(full_query) {
        8
    } else {
        9
    };

    Some(score)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn catalogue_has_at_least_50_unique_valid_entries() {
        let items = all();
        assert!(
            items.len() >= 50,
            "expected at least 50 items, got {}",
            items.len()
        );

        let mut seen_ids = HashSet::new();
        for item in items {
            assert!(!item.id.trim().is_empty(), "item id must not be empty");
            assert!(
                seen_ids.insert(item.id),
                "duplicate win setting id: {}",
                item.id
            );
            assert!(
                !item.name.trim().is_empty(),
                "item name must not be empty for {}",
                item.id
            );
            assert!(
                !item.category.trim().is_empty(),
                "item category must not be empty for {}",
                item.id
            );
            assert!(
                !item.target.trim().is_empty(),
                "item target must not be empty for {}",
                item.id
            );
        }
    }

    #[test]
    fn search_display_ranks_display_first() {
        let results = search("display", 10);
        assert!(!results.is_empty());
        assert_eq!(results[0].id, "display");
        assert_eq!(results[0].target, "ms-settings:display");
    }

    #[test]
    fn search_bluetooth_finds_bluetooth_uri() {
        let results = search("bluetooth", 10);
        assert!(!results.is_empty());
        assert_eq!(results[0].target, "ms-settings:bluetooth");
    }

    #[test]
    fn search_task_manager_by_executable_and_name() {
        let by_exe = search("taskmgr", 10);
        assert!(!by_exe.is_empty());
        assert_eq!(by_exe[0].target, "taskmgr.exe");

        let by_name = search("task manager", 10);
        assert!(!by_name.is_empty());
        assert_eq!(by_name[0].target, "taskmgr.exe");
    }

    #[test]
    fn search_credential_finds_credential_manager() {
        let results = search("credential", 10);
        assert!(!results.is_empty());
        assert_eq!(results[0].name, "Credential Manager");
        assert_eq!(
            results[0].target,
            "control.exe /name Microsoft.CredentialManager"
        );
    }

    #[test]
    fn search_empty_query_returns_empty() {
        assert!(search("", 10).is_empty());
        assert!(search("   ", 10).is_empty());
    }

    #[test]
    fn find_by_id_accepts_bare_and_prefixed_ids() {
        let bare = find_by_id("display");
        let prefixed = find_by_id(&format!("{WIN_SETTING_PREFIX}display"));
        assert!(bare.is_some());
        assert_eq!(bare, prefixed);
        assert_eq!(bare.map(|item| item.target), Some("ms-settings:display"));
        assert!(find_by_id("nonexistent-setting").is_none());
    }

    #[test]
    fn to_result_row_formats_fields_as_expected() {
        let Some(item) = find_by_id("display") else {
            panic!("display item missing from catalogue");
        };
        let row = to_result_row(item);
        assert_eq!(row.id, "win-setting:display");
        assert_eq!(row.name, "Display");
        assert_eq!(row.detail, "System · ms-settings:display");
        assert_eq!(row.kind, WIN_SETTING_SECTION);
        assert_eq!(row.target, "ms-settings:display");
    }
}
