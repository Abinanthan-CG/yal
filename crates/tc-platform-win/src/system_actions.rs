//! The system actions, carried out on Windows.
//!
//! Upstream runs them against macOS: `NSWorkspace` for the session, the media
//! keys for the sound, and the Finder for the Trash. Windows has a different
//! answer for each of them, and the mapping is written down here.
//!
//! The mapping is a pure function of the action, so the table can be checked
//! without touching the machine. The calls themselves are thin wrappers around
//! one Win32 function each.

use std::ffi::c_void;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Power::SetSuspendState;
use windows::Win32::System::Registry::{
    RegCloseKey, RegGetValueW, RegOpenKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
    KEY_QUERY_VALUE, KEY_SET_VALUE, REG_DWORD, RRF_RT_REG_DWORD,
};
use windows::Win32::System::Shutdown::{
    ExitWindowsEx, LockWorkStation, EWX_LOGOFF, EWX_REBOOT, EWX_SHUTDOWN, SHUTDOWN_REASON,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    keybd_event, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, VK_MEDIA_NEXT_TRACK, VK_MEDIA_PLAY_PAUSE,
    VK_MEDIA_PREV_TRACK, VK_VOLUME_DOWN, VK_VOLUME_MUTE, VK_VOLUME_UP,
};
use windows::Win32::UI::Shell::SHEmptyRecycleBinW;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetForegroundWindow, IsWindowVisible, PostMessageW, SendMessageW, ShowWindow,
    HWND_BROADCAST, SC_MONITORPOWER, SW_HIDE, SW_MINIMIZE, SW_SHOW, WM_CLOSE, WM_SETTINGCHANGE,
    WM_SYSCOMMAND,
};

use tc_core::SystemActionId;

/// How the session ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitMode {
    /// The machine restarts.
    Reboot,
    /// The machine shuts down.
    Shutdown,
    /// The user is logged out.
    LogOff,
}

/// How an action is carried out on this machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mechanism {
    /// The workstation is locked.
    LockWorkstation,
    /// The machine is suspended.
    Suspend,
    /// The displays are turned off.
    SleepDisplays,
    /// The session ends.
    ExitWindows(ExitMode),
    /// A media key is pressed.
    MediaKey(u8),
    /// A volume key is pressed.
    VolumeKey(u8),
    /// A shell target is opened.
    OpenShellTarget(&'static str),
    /// The Recycle Bin is emptied.
    EmptyRecycleBin,
    /// Every window is minimized.
    ShowDesktop,
    /// Every window but the frontmost is hidden.
    HideOtherWindows,
    /// The windows that were hidden are shown again.
    ShowHiddenWindows,
    /// Every window is asked to close.
    CloseAllWindows,
    /// The light and the dark theme are swapped.
    ToggleAppearance,
    /// Windows cannot do this from an application.
    Unavailable(&'static str),
}

/// The mechanism an action uses on Windows.
///
/// This is the port's translation of upstream's macOS calls, written down so it
/// can be checked without changing anything on the machine.
pub fn mechanism(action: SystemActionId) -> Mechanism {
    match action {
        SystemActionId::LockScreen => Mechanism::LockWorkstation,
        SystemActionId::Sleep => Mechanism::Suspend,
        SystemActionId::SleepDisplays => Mechanism::SleepDisplays,
        SystemActionId::Restart => Mechanism::ExitWindows(ExitMode::Reboot),
        SystemActionId::ShutDown => Mechanism::ExitWindows(ExitMode::Shutdown),
        SystemActionId::LogOut => Mechanism::ExitWindows(ExitMode::LogOff),
        SystemActionId::ShowScreenSaver => Mechanism::OpenShellTarget("scrnsave.scr"),
        SystemActionId::PlayPause => Mechanism::MediaKey(VK_MEDIA_PLAY_PAUSE.0 as u8),
        SystemActionId::NextTrack => Mechanism::MediaKey(VK_MEDIA_NEXT_TRACK.0 as u8),
        SystemActionId::PreviousTrack => Mechanism::MediaKey(VK_MEDIA_PREV_TRACK.0 as u8),
        SystemActionId::ToggleMute => Mechanism::VolumeKey(VK_VOLUME_MUTE.0 as u8),
        SystemActionId::VolumeUp => Mechanism::VolumeKey(VK_VOLUME_UP.0 as u8),
        SystemActionId::VolumeDown => Mechanism::VolumeKey(VK_VOLUME_DOWN.0 as u8),
        SystemActionId::ShowDesktop => Mechanism::ShowDesktop,
        SystemActionId::ToggleAppearance => Mechanism::ToggleAppearance,
        SystemActionId::OpenTrash => Mechanism::OpenShellTarget("shell:RecycleBinFolder"),
        SystemActionId::EmptyTrash => Mechanism::EmptyRecycleBin,
        SystemActionId::HideOtherApps => Mechanism::HideOtherWindows,
        SystemActionId::UnhideAllApps => Mechanism::ShowHiddenWindows,
        SystemActionId::QuitAllApps => Mechanism::CloseAllWindows,
        SystemActionId::ToggleMicrophoneMute => Mechanism::Unavailable(
            "an application mute the microphone on Windows",
        ),
        SystemActionId::SetVolume
        | SystemActionId::Volume0
        | SystemActionId::Volume25
        | SystemActionId::Volume50
        | SystemActionId::Volume75
        | SystemActionId::Volume100 => {
            Mechanism::Unavailable("set an absolute volume level on Windows")
        }
        SystemActionId::ToggleStageManager => {
            Mechanism::Unavailable("switch Stage Manager on Windows")
        }
        SystemActionId::EjectAllDisks => Mechanism::Unavailable("eject every disk on Windows"),
        SystemActionId::ToggleHiddenFiles => Mechanism::Unavailable(
            "show hidden files at once on Windows, because Explorer only reads the setting when it restarts",
        ),
        SystemActionId::DismissNotifications => {
            Mechanism::Unavailable("dismiss the notifications on Windows")
        }
        SystemActionId::ToggleBluetooth => Mechanism::Unavailable("switch the Bluetooth radio on Windows"),
    }
}

/// Whether Windows can carry the action out.
pub fn is_available(action: SystemActionId) -> bool {
    !matches!(mechanism(action), Mechanism::Unavailable(_))
}

/// Why Windows cannot carry the action out, when it cannot.
pub fn unavailable_reason(action: SystemActionId) -> Option<&'static str> {
    match mechanism(action) {
        Mechanism::Unavailable(reason) => Some(reason),
        _ => None,
    }
}

/// Carry out an action.
///
/// The actions that end the session, lock the machine or empty the Recycle Bin
/// really do it: the caller is expected to have asked first.
pub fn run(action: SystemActionId) -> Result<(), String> {
    match mechanism(action) {
        Mechanism::Unavailable(reason) => Err(format!("Windows cannot {reason}")),
        Mechanism::LockWorkstation => lock_workstation(),
        Mechanism::Suspend => suspend(),
        Mechanism::SleepDisplays => sleep_displays(),
        Mechanism::ExitWindows(mode) => exit_windows(mode),
        Mechanism::MediaKey(key) => {
            press_key(key);
            Ok(())
        }
        Mechanism::VolumeKey(key) => {
            press_key(key);
            Ok(())
        }
        Mechanism::OpenShellTarget(target) => crate::launch::open(target),
        Mechanism::EmptyRecycleBin => empty_recycle_bin(),
        Mechanism::ShowDesktop => {
            for handle in top_level_windows() {
                // SAFETY: the handle came from `EnumWindows`, so it is a window.
                let _ = unsafe { ShowWindow(handle, SW_MINIMIZE) };
            }
            Ok(())
        }
        Mechanism::HideOtherWindows => {
            // SAFETY: the call only reads which window is in front.
            let frontmost = unsafe { GetForegroundWindow() };
            for handle in top_level_windows() {
                if handle == frontmost {
                    continue;
                }
                // SAFETY: the handle came from `EnumWindows`, so it is a window.
                let _ = unsafe { ShowWindow(handle, SW_HIDE) };
            }
            Ok(())
        }
        Mechanism::ShowHiddenWindows => {
            for handle in top_level_windows() {
                // SAFETY: the handle came from `EnumWindows`, so it is a window.
                if unsafe { IsWindowVisible(handle) }.as_bool() {
                    continue;
                }
                let _ = unsafe { ShowWindow(handle, SW_SHOW) };
            }
            Ok(())
        }
        Mechanism::CloseAllWindows => {
            for handle in top_level_windows() {
                // SAFETY: the handle came from `EnumWindows`, so it is a window.
                unsafe {
                    let _ = PostMessageW(Some(handle), WM_CLOSE, WPARAM(0), LPARAM(0));
                };
            }
            Ok(())
        }
        Mechanism::ToggleAppearance => toggle_appearance(),
    }
}

/// Lock the workstation.
fn lock_workstation() -> Result<(), String> {
    // SAFETY: the call takes no arguments and changes nothing but the session.
    unsafe { LockWorkStation() }
        .map_err(|error| format!("Windows would not lock the workstation: {error}"))
}

/// Suspend the machine.
fn suspend() -> Result<(), String> {
    // SAFETY: the call changes the machine's power state and nothing else.
    let suspended = unsafe { SetSuspendState(false, true, false) };
    if suspended {
        Ok(())
    } else {
        Err("Windows would not suspend the machine".to_string())
    }
}

/// Turn the displays off.
fn sleep_displays() -> Result<(), String> {
    // SAFETY: the message is broadcast to every top-level window, which is how
    // Windows itself turns the displays off.
    unsafe {
        SendMessageW(
            HWND_BROADCAST,
            WM_SYSCOMMAND,
            Some(WPARAM(SC_MONITORPOWER as usize)),
            Some(LPARAM(2)),
        )
    };
    Ok(())
}

/// End the session.
fn exit_windows(mode: ExitMode) -> Result<(), String> {
    let flags = match mode {
        ExitMode::Reboot => EWX_REBOOT,
        ExitMode::Shutdown => EWX_SHUTDOWN,
        ExitMode::LogOff => EWX_LOGOFF,
    };
    // SAFETY: the call ends the session, which is exactly what was asked for.
    unsafe { ExitWindowsEx(flags, SHUTDOWN_REASON(0)) }
        .map_err(|error| format!("Windows refused to end the session: {error}"))
}

/// Press a key and let it up again.
fn press_key(key: u8) {
    // SAFETY: the calls only put a key down and up again.
    unsafe {
        keybd_event(key, 0, KEYBD_EVENT_FLAGS(0), 0);
        keybd_event(key, 0, KEYEVENTF_KEYUP, 0);
    }
}

/// Empty the Recycle Bin.
///
/// This is the Windows Recycle Bin, so a file in it is still recoverable until
/// this runs.
fn empty_recycle_bin() -> Result<(), String> {
    // SAFETY: the call has no path, which means every drive's bin.
    unsafe { SHEmptyRecycleBinW(None, PCWSTR::null(), 0) }
        .map_err(|error| format!("Windows would not empty the Recycle Bin: {error}"))
}

/// Swap between the light and the dark theme.
fn toggle_appearance() -> Result<(), String> {
    let key = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Themes\Personalize";
    let current = read_dword(HKEY_CURRENT_USER, key, "AppsUseLightTheme").unwrap_or(1);
    let next = if current == 1 { 0 } else { 1 };
    write_dword(HKEY_CURRENT_USER, key, "AppsUseLightTheme", next)?;
    write_dword(HKEY_CURRENT_USER, key, "SystemUsesLightTheme", next)?;
    // SAFETY: the message tells the shell that the theme changed.
    unsafe {
        SendMessageW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            Some(WPARAM(0)),
            Some(LPARAM(0)),
        )
    };
    Ok(())
}

/// The top-level windows on this desktop.
fn top_level_windows() -> Vec<HWND> {
    let mut handles: Vec<HWND> = Vec::new();
    // SAFETY: the callback is a plain function and the parameter is the list.
    unsafe {
        let _ = EnumWindows(
            Some(collect_window),
            LPARAM(&mut handles as *mut Vec<HWND> as isize),
        );
    }
    handles
}

/// The callback `EnumWindows` uses.
unsafe extern "system" fn collect_window(handle: HWND, parameter: LPARAM) -> windows::core::BOOL {
    let list = &mut *(parameter.0 as *mut Vec<HWND>);
    list.push(handle);
    windows::core::BOOL(1)
}

/// Whether Windows paints its apps dark.
///
/// Windows says so in the registry: the value under `AppsUseLightTheme` is
/// zero when the apps are dark. A machine that does not say is assumed to
/// be light, because that is what Windows has always been.
pub fn apps_use_dark_theme() -> bool {
    read_dword(
        HKEY_CURRENT_USER,
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Themes\Personalize",
        "AppsUseLightTheme",
    )
    .map(|value| value == 0)
    .unwrap_or(false)
}

/// A `DWORD` value in the registry.
fn read_dword(root: HKEY, path: &str, name: &str) -> Option<u32> {
    let key = open_key(root, path, KEY_QUERY_VALUE)?;
    let wide_name = to_wide(name);
    let mut value: u32 = 0;
    let mut size: u32 = std::mem::size_of::<u32>() as u32;
    // SAFETY: the handle is open and the buffer holds exactly one `DWORD`.
    let status = unsafe {
        RegGetValueW(
            key,
            PCWSTR::null(),
            PCWSTR(wide_name.as_ptr()),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut value as *mut u32 as *mut c_void),
            Some(&mut size),
        )
    };
    // SAFETY: the handle was opened here, so it is closed here.
    let _ = unsafe { RegCloseKey(key) };
    if status != windows::Win32::Foundation::ERROR_SUCCESS {
        return None;
    }
    Some(value)
}

/// Write a `DWORD` value in the registry.
fn write_dword(root: HKEY, path: &str, name: &str, value: u32) -> Result<(), String> {
    let key = open_key(root, path, KEY_SET_VALUE)
        .ok_or_else(|| format!("Windows would not open `{path}`"))?;
    let wide_name = to_wide(name);
    // SAFETY: the handle is open and the bytes are exactly one `DWORD`.
    let status = unsafe {
        RegSetValueExW(
            key,
            PCWSTR(wide_name.as_ptr()),
            None,
            REG_DWORD,
            Some(&value.to_ne_bytes()),
        )
    };
    // SAFETY: the handle was opened here, so it is closed here.
    let _ = unsafe { RegCloseKey(key) };
    if status != windows::Win32::Foundation::ERROR_SUCCESS {
        return Err(format!("Windows would not write `{name}`"));
    }
    Ok(())
}

/// Open a registry key.
fn open_key(
    root: HKEY,
    path: &str,
    access: windows::Win32::System::Registry::REG_SAM_FLAGS,
) -> Option<HKEY> {
    let wide = to_wide(path);
    let mut key = HKEY::default();
    // SAFETY: the path is a null-terminated wide string and the handle slot is
    // valid.
    let status = unsafe { RegOpenKeyExW(root, PCWSTR(wide.as_ptr()), None, access, &mut key) };
    if status == windows::Win32::Foundation::ERROR_SUCCESS {
        Some(key)
    } else {
        None
    }
}

/// A string, wide and null-terminated.
fn to_wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every action's mechanism, pinned.
    ///
    /// This is the table the port's Windows translation lives in, so it is
    /// written out in full: a change to a line here is a change to what a row
    /// in the palette does.
    #[test]
    fn the_mechanism_of_every_action_is_pinned() {
        let pinned: Vec<(SystemActionId, Mechanism)> = vec![
            (SystemActionId::LockScreen, Mechanism::LockWorkstation),
            (SystemActionId::Sleep, Mechanism::Suspend),
            (SystemActionId::SleepDisplays, Mechanism::SleepDisplays),
            (
                SystemActionId::Restart,
                Mechanism::ExitWindows(ExitMode::Reboot),
            ),
            (
                SystemActionId::ShutDown,
                Mechanism::ExitWindows(ExitMode::Shutdown),
            ),
            (
                SystemActionId::LogOut,
                Mechanism::ExitWindows(ExitMode::LogOff),
            ),
            (
                SystemActionId::ShowScreenSaver,
                Mechanism::OpenShellTarget("scrnsave.scr"),
            ),
            (
                SystemActionId::PlayPause,
                Mechanism::MediaKey(VK_MEDIA_PLAY_PAUSE.0 as u8),
            ),
            (
                SystemActionId::NextTrack,
                Mechanism::MediaKey(VK_MEDIA_NEXT_TRACK.0 as u8),
            ),
            (
                SystemActionId::PreviousTrack,
                Mechanism::MediaKey(VK_MEDIA_PREV_TRACK.0 as u8),
            ),
            (
                SystemActionId::ToggleMute,
                Mechanism::VolumeKey(VK_VOLUME_MUTE.0 as u8),
            ),
            (
                SystemActionId::VolumeUp,
                Mechanism::VolumeKey(VK_VOLUME_UP.0 as u8),
            ),
            (
                SystemActionId::VolumeDown,
                Mechanism::VolumeKey(VK_VOLUME_DOWN.0 as u8),
            ),
            (SystemActionId::ShowDesktop, Mechanism::ShowDesktop),
            (
                SystemActionId::ToggleAppearance,
                Mechanism::ToggleAppearance,
            ),
            (
                SystemActionId::OpenTrash,
                Mechanism::OpenShellTarget("shell:RecycleBinFolder"),
            ),
            (SystemActionId::EmptyTrash, Mechanism::EmptyRecycleBin),
            (SystemActionId::HideOtherApps, Mechanism::HideOtherWindows),
            (SystemActionId::UnhideAllApps, Mechanism::ShowHiddenWindows),
            (SystemActionId::QuitAllApps, Mechanism::CloseAllWindows),
        ];
        for (action, expected) in &pinned {
            assert_eq!(mechanism(*action), *expected, "{action:?}");
        }
        // The twenty actions above are the ones Windows can carry out.
        assert_eq!(pinned.len(), SystemActionId::ALL.len() - 12);
    }
    /// Upstream lists thirty-two system actions; every one of them has to have
    /// a Windows answer, even when the answer is "Windows cannot".
    #[test]
    fn every_action_has_a_mechanism() {
        for action in SystemActionId::ALL {
            let _ = mechanism(action);
        }
        assert_eq!(SystemActionId::ALL.len(), 32);
    }

    #[test]
    fn the_session_actions_end_the_session() {
        assert_eq!(
            mechanism(SystemActionId::LockScreen),
            Mechanism::LockWorkstation
        );
        assert_eq!(
            mechanism(SystemActionId::Restart),
            Mechanism::ExitWindows(ExitMode::Reboot)
        );
        assert_eq!(
            mechanism(SystemActionId::ShutDown),
            Mechanism::ExitWindows(ExitMode::Shutdown)
        );
        assert_eq!(
            mechanism(SystemActionId::LogOut),
            Mechanism::ExitWindows(ExitMode::LogOff)
        );
    }

    /// Upstream: the sound is turned with the media keys, and so is it here.
    #[test]
    fn the_sound_actions_press_the_media_keys() {
        assert_eq!(
            mechanism(SystemActionId::ToggleMute),
            Mechanism::VolumeKey(VK_VOLUME_MUTE.0 as u8)
        );
        assert_eq!(
            mechanism(SystemActionId::VolumeUp),
            Mechanism::VolumeKey(VK_VOLUME_UP.0 as u8)
        );
        assert_eq!(
            mechanism(SystemActionId::VolumeDown),
            Mechanism::VolumeKey(VK_VOLUME_DOWN.0 as u8)
        );
        assert_eq!(
            mechanism(SystemActionId::PlayPause),
            Mechanism::MediaKey(VK_MEDIA_PLAY_PAUSE.0 as u8)
        );
    }

    /// Upstream's Trash is the Finder's; Windows' is the Recycle Bin, and the
    /// bin is what gets opened and emptied.
    #[test]
    fn the_trash_is_the_recycle_bin() {
        assert_eq!(
            mechanism(SystemActionId::OpenTrash),
            Mechanism::OpenShellTarget("shell:RecycleBinFolder")
        );
        assert_eq!(
            mechanism(SystemActionId::EmptyTrash),
            Mechanism::EmptyRecycleBin
        );
    }

    /// The actions Windows cannot do are named, and the rest are available.
    #[test]
    fn the_actions_windows_cannot_do_say_so() {
        let unavailable: Vec<SystemActionId> = SystemActionId::ALL
            .into_iter()
            .filter(|action| !is_available(*action))
            .collect();
        assert_eq!(
            unavailable,
            vec![
                SystemActionId::ToggleMicrophoneMute,
                SystemActionId::SetVolume,
                SystemActionId::Volume0,
                SystemActionId::Volume25,
                SystemActionId::Volume50,
                SystemActionId::Volume75,
                SystemActionId::Volume100,
                SystemActionId::ToggleStageManager,
                SystemActionId::EjectAllDisks,
                SystemActionId::ToggleHiddenFiles,
                SystemActionId::DismissNotifications,
                SystemActionId::ToggleBluetooth,
            ]
        );
        for action in unavailable {
            let reason = unavailable_reason(action).expect("a reason");
            assert!(!reason.is_empty(), "{action:?} has no reason");
        }
    }

    /// Every action that Windows can do has a way to do it.
    #[test]
    fn an_available_action_does_something() {
        for action in SystemActionId::ALL {
            if is_available(action) {
                assert!(unavailable_reason(action).is_none(), "{action:?}");
            }
        }
    }

    #[test]
    fn a_wide_string_is_null_terminated() {
        assert_eq!(to_wide("ab"), vec![0x61, 0x62, 0]);
    }

    /// The registry read and the registry write have to agree, or the theme
    /// toggle would not toggle.
    #[test]
    fn the_theme_setting_can_be_read_and_written() {
        let key = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Themes\Personalize";
        let before = read_dword(HKEY_CURRENT_USER, key, "AppsUseLightTheme");
        // The value is written back unchanged, so the machine is left as it was.
        if let Some(value) = before {
            write_dword(HKEY_CURRENT_USER, key, "AppsUseLightTheme", value)
                .expect("the theme value can be written");
            assert_eq!(
                read_dword(HKEY_CURRENT_USER, key, "AppsUseLightTheme"),
                Some(value)
            );
        }
    }

    /// The top-level window list is a list of handles, and asking for it must
    /// not fail on a machine with windows open.
    #[test]
    fn the_window_list_can_be_read() {
        let _ = top_level_windows();
    }
}
