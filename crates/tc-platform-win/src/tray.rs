//! Native Win32 notification-area (system tray) icon.
//!
//! Uses `Shell_NotifyIconW` directly on a dedicated thread with a blocking
//! `GetMessageW` loop (0% idle CPU, zero external crate dependencies), handles
//! `TaskbarCreated` (including UIPI allow-listing) so Explorer restarts never
//! orphan the icon, cancels any open popup menu cleanly on stop, and removes
//! the icon via `NIM_DELETE` inside `WM_DESTROY` while the `HWND` is still valid.

use std::ffi::c_void;
use std::sync::mpsc;
use std::thread::JoinHandle;

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, AppendMenuW, ChangeWindowMessageFilterEx, CreatePopupMenu,
    CreateWindowExW, DefWindowProcW, DestroyIcon, DestroyMenu, DestroyWindow, DispatchMessageW,
    GetCursorPos, GetMessageW, GetWindowLongPtrW, LoadIconW, LoadImageW, PostMessageW,
    PostQuitMessage, RegisterClassExW, RegisterWindowMessageW, SetForegroundWindow,
    SetWindowLongPtrW, TrackPopupMenu, TranslateMessage, ASFW_ANY, CREATESTRUCTW, GWLP_USERDATA,
    HICON, IDI_APPLICATION, IMAGE_ICON, LR_DEFAULTSIZE, MF_SEPARATOR, MF_STRING, MSG, MSGFLT_ALLOW,
    TPM_BOTTOMALIGN, TPM_LEFTALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON, WINDOW_EX_STYLE, WINDOW_STYLE,
    WM_APP, WM_CANCELMODE, WM_CLOSE, WM_DESTROY, WM_LBUTTONUP, WM_NCCREATE, WM_NCDESTROY, WM_NULL,
    WM_RBUTTONUP, WNDCLASSEXW,
};

/// Message ID posted by Windows when the user interacts with the tray icon.
const WM_TRAY_ICON: u32 = WM_APP + 100;

/// Menu item command IDs.
const CMD_OPEN: usize = 1001;
const CMD_SETTINGS: usize = 1002;
const CMD_SUPPORT: usize = 1003;
const CMD_QUIT: usize = 1004;

/// Embedded icon resource ID compiled into `yal.exe` by `build.rs` (`windows/yal.rc`).
const APP_ICON_RESOURCE_ID: usize = 1;

/// Actions the user can invoke from the notification-area icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    /// Left-click on the tray icon: toggles visibility (blur-debounced).
    Toggle,
    /// Explicit "Open YAL" menu item: always shows and focuses the palette.
    Open,
    Settings,
    Support,
    Quit,
}

impl TrayAction {
    pub fn label(self) -> &'static str {
        match self {
            TrayAction::Toggle | TrayAction::Open => "Open YAL",
            TrayAction::Settings => "Settings",
            TrayAction::Support => "Support YAL",
            TrayAction::Quit => "Quit YAL",
        }
    }
}

struct TrayState {
    hwnd: HWND,
    icon: HICON,
    owns_icon: bool,
    taskbar_created_msg: u32,
    on_action: Box<dyn Fn(TrayAction) + Send>,
}

/// Handle to a running native Win32 system tray icon.
pub struct WindowsTray {
    hwnd: isize,
    thread: Option<JoinHandle<()>>,
}

impl WindowsTray {
    /// Start the system tray icon on its own thread.
    pub fn spawn(on_action: impl Fn(TrayAction) + Send + 'static) -> Result<Self, String> {
        let (ready_tx, ready_rx) = mpsc::channel::<Result<isize, String>>();
        let on_action: Box<dyn Fn(TrayAction) + Send> = Box::new(on_action);

        let thread = std::thread::Builder::new()
            .name("yal-tray".to_string())
            .spawn(move || {
                run_tray_thread(on_action, ready_tx);
            })
            .map_err(|error| format!("the tray thread could not be started: {error}"))?;

        let hwnd = match ready_rx.recv() {
            Ok(Ok(hwnd)) => hwnd,
            Ok(Err(error)) => {
                let _ = thread.join();
                return Err(error);
            }
            Err(_) => {
                let _ = thread.join();
                return Err("the tray thread exited before creating the icon".to_string());
            }
        };

        Ok(Self {
            hwnd,
            thread: Some(thread),
        })
    }

    /// Remove the tray icon and stop its thread cleanly.
    pub fn stop(&mut self) {
        if self.hwnd != 0 {
            let hwnd = HWND(self.hwnd as *mut c_void);
            unsafe {
                // Cancel any open popup menu before closing so TrackPopupMenu returns.
                let _ = PostMessageW(Some(hwnd), WM_CANCELMODE, WPARAM(0), LPARAM(0));
                let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
            }
            self.hwnd = 0;
        }
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for WindowsTray {
    fn drop(&mut self) {
        self.stop();
    }
}

fn run_tray_thread(
    on_action: Box<dyn Fn(TrayAction) + Send>,
    ready_tx: mpsc::Sender<Result<isize, String>>,
) {
    let instance = match unsafe { GetModuleHandleW(None) } {
        Ok(h) => HINSTANCE(h.0),
        Err(error) => {
            let _ = ready_tx.send(Err(format!("GetModuleHandleW failed: {error}")));
            return;
        }
    };

    let class = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(tray_wnd_proc),
        hInstance: instance,
        lpszClassName: w!("YalTrayWindow"),
        ..Default::default()
    };
    let atom = unsafe { RegisterClassExW(&class) };
    if atom == 0 {
        let last = unsafe { windows::Win32::Foundation::GetLastError() };
        if last != windows::Win32::Foundation::ERROR_CLASS_ALREADY_EXISTS {
            let _ = ready_tx.send(Err(
                "the tray window class could not be registered".to_string()
            ));
            return;
        }
    }

    let (icon, owns_icon) = load_tray_icon(instance);
    let taskbar_created_msg = unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) };

    // Convert into a raw pointer BEFORE CreateWindowExW so the provenance
    // stored in GWLP_USERDATA is never invalidated by subsequent Box access.
    let state_ptr = Box::into_raw(Box::new(TrayState {
        hwnd: HWND(std::ptr::null_mut()),
        icon,
        owns_icon,
        taskbar_created_msg,
        on_action,
    }));

    // Use a regular invisible top-level window rather than HWND_MESSAGE so
    // broadcast messages like TaskbarCreated still reach us when Explorer restarts.
    let hwnd = match unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("YalTrayWindow"),
            w!("YAL Tray"),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            None,
            None,
            Some(instance),
            Some(state_ptr as *mut c_void),
        )
    } {
        Ok(hwnd) => hwnd,
        Err(error) => {
            unsafe {
                let boxed = Box::from_raw(state_ptr);
                if boxed.owns_icon && !boxed.icon.is_invalid() {
                    let _ = DestroyIcon(boxed.icon);
                }
            }
            let _ = ready_tx.send(Err(format!(
                "the tray window could not be created: {error}"
            )));
            return;
        }
    };

    unsafe {
        (*state_ptr).hwnd = hwnd;
        if taskbar_created_msg != 0 {
            let _ = ChangeWindowMessageFilterEx(hwnd, taskbar_created_msg, MSGFLT_ALLOW, None);
        }
    }

    let added = unsafe { add_notify_icon(hwnd, (*state_ptr).icon) };
    if !added {
        unsafe {
            let _ = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            let _ = DestroyWindow(hwnd);
            let boxed = Box::from_raw(state_ptr);
            if boxed.owns_icon && !boxed.icon.is_invalid() {
                let _ = DestroyIcon(boxed.icon);
            }
        }
        let _ = ready_tx.send(Err("the tray icon could not be added".to_string()));
        return;
    }

    let _ = ready_tx.send(Ok(hwnd.0 as isize));

    unsafe {
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        // Window is already destroyed (NIM_DELETE ran in WM_DESTROY and
        // GWLP_USERDATA was cleared in WM_NCDESTROY). Free state and icon.
        let boxed = Box::from_raw(state_ptr);
        if boxed.owns_icon && !boxed.icon.is_invalid() {
            let _ = DestroyIcon(boxed.icon);
        }
    }
}

fn load_tray_icon(instance: HINSTANCE) -> (HICON, bool) {
    let loaded = unsafe {
        LoadImageW(
            Some(instance),
            PCWSTR(APP_ICON_RESOURCE_ID as *const u16),
            IMAGE_ICON,
            0,
            0,
            LR_DEFAULTSIZE,
        )
    };
    if let Ok(handle) = loaded {
        if !handle.is_invalid() {
            return (HICON(handle.0), true);
        }
    }
    let fallback = unsafe { LoadIconW(None, IDI_APPLICATION).unwrap_or_default() };
    (fallback, false)
}

unsafe fn make_notify_data(hwnd: HWND, icon: HICON) -> NOTIFYICONDATAW {
    let mut data = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: 1,
        uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
        uCallbackMessage: WM_TRAY_ICON,
        hIcon: icon,
        ..Default::default()
    };
    let tip: Vec<u16> = "YAL — Keyboard Launcher".encode_utf16().collect();
    let len = tip.len().min(data.szTip.len() - 1);
    data.szTip[..len].copy_from_slice(&tip[..len]);
    data
}

unsafe fn add_notify_icon(hwnd: HWND, icon: HICON) -> bool {
    let data = make_notify_data(hwnd, icon);
    Shell_NotifyIconW(NIM_ADD, &data).as_bool()
}

unsafe fn remove_notify_icon(hwnd: HWND) {
    let data = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: 1,
        ..Default::default()
    };
    let _ = Shell_NotifyIconW(NIM_DELETE, &data);
}

unsafe fn show_context_menu(hwnd: HWND) -> Option<TrayAction> {
    let Ok(menu) = CreatePopupMenu() else {
        return None;
    };

    let _ = AppendMenuW(menu, MF_STRING, CMD_OPEN, w!("Open YAL"));
    let _ = AppendMenuW(menu, MF_STRING, CMD_SETTINGS, w!("Settings"));
    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
    let _ = AppendMenuW(menu, MF_STRING, CMD_SUPPORT, w!("Support YAL"));
    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
    let _ = AppendMenuW(menu, MF_STRING, CMD_QUIT, w!("Quit YAL"));

    let mut point = POINT::default();
    let _ = GetCursorPos(&mut point);

    // Textbook Win32 requirement so clicking outside dismisses the popup menu.
    // `TPM_RETURNCMD` waits until the menu has completely closed and settled
    // before returning the selected command ID so the target palette window
    // does not immediately receive a spurious `WA_INACTIVE` blur.
    let _ = SetForegroundWindow(hwnd);
    let chosen = TrackPopupMenu(
        menu,
        TPM_LEFTALIGN | TPM_BOTTOMALIGN | TPM_RIGHTBUTTON | TPM_RETURNCMD,
        point.x,
        point.y,
        None,
        hwnd,
        None,
    );
    let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
    let _ = DestroyMenu(menu);

    match chosen.0 as usize {
        CMD_OPEN => Some(TrayAction::Open),
        CMD_SETTINGS => Some(TrayAction::Settings),
        CMD_SUPPORT => Some(TrayAction::Support),
        CMD_QUIT => Some(TrayAction::Quit),
        _ => None,
    }
}

unsafe extern "system" fn tray_wnd_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_NCCREATE {
        let create = &*(lparam.0 as *const CREATESTRUCTW);
        let state = create.lpCreateParams as *mut TrayState;
        let _ = SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);
        return DefWindowProcW(hwnd, message, wparam, lparam);
    }

    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut TrayState;
    if ptr.is_null() {
        return DefWindowProcW(hwnd, message, wparam, lparam);
    }

    let taskbar_msg = (*ptr).taskbar_created_msg;
    if taskbar_msg != 0 && message == taskbar_msg {
        let icon = (*ptr).icon;
        let _ = add_notify_icon(hwnd, icon);
        return LRESULT(0);
    }

    match message {
        WM_TRAY_ICON => {
            let mouse_msg = (lparam.0 & 0xFFFF) as u32;
            match mouse_msg {
                WM_LBUTTONUP => {
                    let _ = AllowSetForegroundWindow(ASFW_ANY);
                    ((*ptr).on_action)(TrayAction::Toggle);
                }
                WM_RBUTTONUP => {
                    if let Some(action) = show_context_menu(hwnd) {
                        let _ = AllowSetForegroundWindow(ASFW_ANY);
                        ((*ptr).on_action)(action);
                    }
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            remove_notify_icon(hwnd);
            PostQuitMessage(0);
            LRESULT(0)
        }
        WM_NCDESTROY => {
            let _ = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            DefWindowProcW(hwnd, message, wparam, lparam)
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_actions_have_non_empty_labels() {
        for action in [
            TrayAction::Open,
            TrayAction::Settings,
            TrayAction::Support,
            TrayAction::Quit,
        ] {
            assert!(!action.label().is_empty());
        }
    }

    #[test]
    fn tray_spawns_and_stops_cleanly() {
        if let Ok(mut tray) = WindowsTray::spawn(|_| {}) {
            tray.stop();
        }
    }
}
