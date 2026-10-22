//! Open browser tabs, found from the browser windows themselves.
//!
//! Chromium-family browsers title every window "Tab name - Browser name".
//! The tab title is the part before the separator, so the tabs that are
//! open can be offered without asking the browser for anything, which is
//! how the launcher stays read-only and stays out of the browser's way.

/// One open browser tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserTab {
    /// The tab's title.
    pub title: String,

    /// The browser it belongs to.
    pub browser: String,

    /// The handle of the window holding it.
    pub window: u64,
}

/// What a browser calls its windows, and the process it runs as.
#[cfg(windows)]
const BROWSERS: &[(&str, &str)] = &[
    ("Google Chrome", "chrome"),
    ("Microsoft Edge", "msedge"),
    ("Vivaldi", "vivaldi"),
    ("Brave", "brave"),
    ("Brave Browser", "brave"),
    ("Chromium", "chromium"),
    ("Opera", "opera"),
    ("Firefox", "firefox"),
    ("Mozilla Firefox", "firefox"),
];

/// Every open tab of every supported browser.
#[cfg(windows)]
pub fn open_tabs() -> Vec<BrowserTab> {
    let windows = match crate::windows::list_windows() {
        Ok(windows) => windows,
        Err(_) => return Vec::new(),
    };

    let mut tabs = Vec::new();
    for window in windows {
        let Some(process) = window.process_name.as_deref() else {
            continue;
        };
        // The process name arrives with or without its extension depending
        // on which call produced it, so a stem is matched either way.
        let stem = process.to_ascii_lowercase();
        let stem = stem.trim_end_matches(".exe").to_string();
        if !BROWSERS.iter().any(|(_, name)| *name == stem) {
            continue;
        }
        // "Tab title - Browser" is the form; a window without the separator
        // is a browser page with an unnamed document.
        let title = window.title.trim();
        if title.is_empty() || title.matches(" - ").count() == 0 {
            continue;
        }
        let Some((tab, _)) = title.split_once(" - ") else {
            continue;
        };
        let tab = tab.trim();
        if tab.is_empty() {
            continue;
        }

        tabs.push(BrowserTab {
            title: tab.to_string(),
            browser: browser_display_name(&stem),
            window: window.handle,
        });
    }

    tabs.sort_by_key(|tab| tab.title.to_lowercase());
    tabs.dedup_by(|a, b| a.title == b.title && a.window == b.window);
    tabs
}

/// Every open tab on a host without window handles.
#[cfg(not(windows))]
pub fn open_tabs() -> Vec<BrowserTab> {
    Vec::new()
}

/// The browser's display name for its process stem.
#[cfg(windows)]
fn browser_display_name(stem: &str) -> String {
    for (display, name) in BROWSERS {
        if *name == stem {
            return display.to_string();
        }
    }
    stem.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_supported_browser_has_a_display_name() {
        for (display, stem) in BROWSERS {
            assert!(!display.is_empty());
            assert!(!stem.is_empty());
        }
    }
}
