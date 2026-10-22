//! Display control: brightness and the light or dark theme.
//!
//! Brightness is set on the physical monitors attached to the primary
//! adapter. The theme is the Windows personalise setting, which is a
//! registry value followed by a settings-change broadcast.

/// A failure while changing the display.
#[derive(Debug, thiserror::Error)]
pub enum DisplayError {
    #[error("no controllable monitor was found")]
    NoMonitor,

    #[error("the display operation failed: {0}")]
    Failed(String),
}

/// The brightness of every controllable monitor.
///
/// A host whose monitors cannot report brightness — older laptops, desktop
/// speakers of the display variety — returns an empty list.
#[cfg(windows)]
pub fn read_brightness() -> Result<Vec<(u32, f64)>, DisplayError> {
    use windows::Win32::Devices::Display::{
        GetMonitorBrightness, GetNumberOfPhysicalMonitorsFromHMONITOR,
        GetPhysicalMonitorsFromHMONITOR, PHYSICAL_MONITOR,
    };
    use windows::Win32::Foundation::{LPARAM, RECT};
    use windows::Win32::Graphics::Gdi::{
        EnumDisplayMonitors, HDC, HMONITOR,
    };

    let mut monitors: Vec<HMONITOR> = Vec::new();

    unsafe extern "system" fn callback(
        monitor: HMONITOR,
        _context: HDC,
        _clip: *mut RECT,
        parameter: LPARAM,
    ) -> windows::core::BOOL {
        use windows::Win32::Foundation::FALSE;
        let pointer = parameter.0 as *mut Vec<HMONITOR>;
        if pointer.is_null() {
            return FALSE;
        }
        unsafe { (*pointer).push(monitor) };
        windows::core::BOOL::from(true)
    }

    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(callback),
            LPARAM(&mut monitors as *mut Vec<HMONITOR> as isize),
        );
    }

    let mut readings = Vec::new();
    for monitor in monitors {
        unsafe {
            let mut count = 0u32;
            if GetNumberOfPhysicalMonitorsFromHMONITOR(monitor, &mut count).is_err() {
                continue;
            }
            let mut physicals = vec![PHYSICAL_MONITOR::default(); count as usize];
            if GetPhysicalMonitorsFromHMONITOR(monitor, &mut physicals).is_err() {
                continue;
            }
            for physical in &physicals {
                let mut minimum = 0u32;
                let mut current = 0u32;
                let mut maximum = 0u32;
                if GetMonitorBrightness(physical.hPhysicalMonitor, &mut minimum, &mut current, &mut maximum)
                    == 0
                {
                    continue;
                }
                if maximum <= minimum {
                    continue;
                }
                let fraction = f64::from(current - minimum) / f64::from(maximum - minimum);
                readings.push((monitor.0 as u32, fraction * 100.0));
            }
        }
    }

    if readings.is_empty() {
        Err(DisplayError::NoMonitor)
    } else {
        Ok(readings)
    }
}

/// Read the brightness on a host without controllable monitors.
#[cfg(not(windows))]
pub fn read_brightness() -> Result<Vec<(u32, f64)>, DisplayError> {
    Err(DisplayError::NoMonitor)
}

/// Set the brightness of every controllable monitor, on the 0 to 100 scale.
#[cfg(windows)]
pub fn set_brightness(level: f64) -> Result<(), DisplayError> {
    use windows::Win32::Devices::Display::{
        GetMonitorBrightness, GetNumberOfPhysicalMonitorsFromHMONITOR,
        GetPhysicalMonitorsFromHMONITOR, SetMonitorBrightness, PHYSICAL_MONITOR,
    };
    use windows::Win32::Foundation::{LPARAM, RECT};
    use windows::Win32::Graphics::Gdi::{EnumDisplayMonitors, HDC, HMONITOR};

    if !(0.0..=100.0).contains(&level) {
        return Err(DisplayError::Failed(format!(
            "{level} is not a brightness between 0 and 100"
        )));
    }

    let mut monitors: Vec<HMONITOR> = Vec::new();

    unsafe extern "system" fn callback(
        monitor: HMONITOR,
        _context: HDC,
        _clip: *mut RECT,
        parameter: LPARAM,
    ) -> windows::core::BOOL {
        let pointer = parameter.0 as *mut Vec<HMONITOR>;
        if pointer.is_null() {
            return windows::core::BOOL::from(false);
        }
        unsafe { (*pointer).push(monitor) };
        windows::core::BOOL::from(true)
    }

    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(callback),
            LPARAM(&mut monitors as *mut Vec<HMONITOR> as isize),
        );
    }

    let mut changed = 0usize;
    for monitor in monitors {
        unsafe {
            let mut count = 0u32;
            if GetNumberOfPhysicalMonitorsFromHMONITOR(monitor, &mut count).is_err() {
                continue;
            }
            let mut physicals = vec![PHYSICAL_MONITOR::default(); count as usize];
            if GetPhysicalMonitorsFromHMONITOR(monitor, &mut physicals).is_err() {
                continue;
            }
            for physical in &physicals {
                let mut minimum = 0u32;
                let mut current = 0u32;
                let mut maximum = 0u32;
                if GetMonitorBrightness(
                    physical.hPhysicalMonitor,
                    &mut minimum,
                    &mut current,
                    &mut maximum,
                ) == 0
                {
                    continue;
                }
                if maximum <= minimum {
                    continue;
                }
                let desired = minimum + ((f64::from(maximum - minimum) * level / 100.0) as u32);
                if SetMonitorBrightness(physical.hPhysicalMonitor, desired) != 0 {
                    changed += 1;
                }
            }
        }
    }

    if changed == 0 {
        Err(DisplayError::NoMonitor)
    } else {
        Ok(())
    }
}

/// Set the brightness on a host without controllable monitors.
#[cfg(not(windows))]
pub fn set_brightness(_level: f64) -> Result<(), DisplayError> {
    Err(DisplayError::NoMonitor)
}

/// The registry value that holds the application theme.
#[cfg(windows)]
const THEME_VALUE: &str = "AppsUseLightTheme";

#[cfg(windows)]
const PERSONALISE_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize";

/// Whether applications are drawn dark.
///
/// `true` means dark, `false` means light. An absent value — a host where
/// the personalise key has never been written — reports `true`, which is
/// what a fresh Windows install uses.
#[cfg(windows)]
pub fn is_dark_mode() -> Result<bool, DisplayError> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let machine = RegKey::predef(HKEY_CURRENT_USER);
    let key = machine
        .open_subkey(PERSONALISE_KEY)
        .map_err(|err| DisplayError::Failed(err.to_string()))?;
    match key.get_value::<u32, _>(THEME_VALUE) {
        Ok(0) => Ok(true),
        Ok(_) => Ok(false),
        Err(_) => Ok(true),
    }
}

/// Whether applications are drawn dark on a host without the setting.
#[cfg(not(windows))]
pub fn is_dark_mode() -> Result<bool, DisplayError> {
    Ok(true)
}

/// Turn dark mode on or off, and tell applications the setting changed.
#[cfg(windows)]
pub fn set_dark_mode(dark: bool) -> Result<(), DisplayError> {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_SET_VALUE};
    use winreg::RegKey;

    let machine = RegKey::predef(HKEY_CURRENT_USER);
    let key = machine
        .open_subkey_with_flags(PERSONALISE_KEY, KEY_SET_VALUE)
        .map_err(|err| DisplayError::Failed(err.to_string()))?;
    key.set_value(THEME_VALUE, &u32::from(!dark))
        .map_err(|err| DisplayError::Failed(err.to_string()))?;

    // Applications only re-read the theme when they are told it changed.
    announce_theme_change();

    Ok(())
}

/// Turn dark mode on or off on a host without the setting.
#[cfg(not(windows))]
pub fn set_dark_mode(_dark: bool) -> Result<(), DisplayError> {
    Err(DisplayError::Failed(
        "theme control is not implemented on this platform".into(),
    ))
}

/// Broadcast the settings change so running applications re-read the theme.
#[cfg(windows)]
fn announce_theme_change() {
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
    };

    unsafe {
        let _ = SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            WPARAM(0),
            LPARAM(0),
            SMTO_ABORTIFHUNG,
            2_000,
            None,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn the_theme_can_be_read_and_restored() {
        let before = is_dark_mode().unwrap();
        // Toggle and restore so the machine is left as it was.
        let _ = set_dark_mode(!before);
        let changed = is_dark_mode().unwrap();
        assert_eq!(changed, !before);
        let _ = set_dark_mode(before);
        let restored = is_dark_mode().unwrap();
        assert_eq!(restored, before);
    }

    #[cfg(windows)]
    #[test]
    fn rejects_an_out_of_range_brightness() {
        assert!(set_brightness(200.0).is_err());
        assert!(set_brightness(-5.0).is_err());
    }
}
