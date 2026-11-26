use tc_platform::{IntegrityLevel, WindowInfo, WindowRect, WindowSystem};
use windows::core::{BOOL, PWSTR};
use windows::Win32::Foundation::{HANDLE, HWND, LPARAM, RECT};
use windows::Win32::Security::{
    GetSidSubAuthority, GetSidSubAuthorityCount, GetTokenInformation, TokenIntegrityLevel,
    TOKEN_MANDATORY_LABEL, TOKEN_QUERY,
};
use windows::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, QueryFullProcessImageNameW,
    PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetForegroundWindow, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId,
    IsWindowVisible,
};

/// The real window system.
#[derive(Debug, Default)]
pub struct WindowsWindows;

impl WindowsWindows {
    pub fn new() -> WindowsWindows {
        WindowsWindows
    }
}

impl WindowSystem for WindowsWindows {
    fn foreground_window(&self) -> Option<WindowInfo> {
        let handle = unsafe { GetForegroundWindow() };
        if handle.0.is_null() {
            return None;
        }
        window_info(handle)
    }

    fn switchable_windows(&self) -> Vec<WindowInfo> {
        let mut found: Vec<WindowInfo> = Vec::new();
        unsafe {
            let _ = EnumWindows(
                Some(collect_window),
                LPARAM(&mut found as *mut Vec<WindowInfo> as isize),
            );
        }
        found
    }

    fn integrity_level(&self, process_id: u32) -> IntegrityLevel {
        integrity_level_of(process_id)
    }
}

/// The callback `EnumWindows` uses.
unsafe extern "system" fn collect_window(handle: HWND, parameter: LPARAM) -> BOOL {
    let list = &mut *(parameter.0 as *mut Vec<WindowInfo>);
    if let Some(info) = window_info(handle) {
        list.push(info);
    }
    BOOL(1)
}

/// Read what is known about a window.
fn window_info(handle: HWND) -> Option<WindowInfo> {
    unsafe {
        if !IsWindowVisible(handle).as_bool() {
            return None;
        }
        let mut process_id = 0u32;
        GetWindowThreadProcessId(handle, Some(&mut process_id));
        let title = window_title(handle);
        let mut rect = RECT::default();
        if GetWindowRect(handle, &mut rect).is_err() {
            return None;
        }
        Some(WindowInfo {
            handle: handle.0 as usize as u64,
            title,
            process_id,
            rect: WindowRect {
                left: rect.left,
                top: rect.top,
                right: rect.right,
                bottom: rect.bottom,
            },
        })
    }
}

/// A window's title.
fn window_title(handle: HWND) -> String {
    unsafe {
        let mut buffer = [0u16; 512];
        let length = GetWindowTextW(handle, &mut buffer);
        if length <= 0 {
            return String::new();
        }
        String::from_utf16_lossy(&buffer[..length as usize])
    }
}

/// The integrity level of a process.
///
/// A normal process cannot read the windows of an elevated one, so the app
/// needs to know before it tries.
pub fn integrity_level_of(process_id: u32) -> IntegrityLevel {
    unsafe {
        let process = match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id) {
            Ok(handle) => handle,
            Err(_) => return IntegrityLevel::Medium,
        };
        let level = token_integrity_level(process);
        let _ = windows::Win32::Foundation::CloseHandle(process);
        level
    }
}

/// The integrity level of this process.
pub fn own_integrity_level() -> IntegrityLevel {
    unsafe {
        let process = GetCurrentProcess();
        token_integrity_level(process)
    }
}

/// Full executable path of the current foreground window's owning process,
/// when readable (used to tag Clipboard History items with their source app
/// so the list can show the app's real icon and name).
pub fn foreground_process_exe() -> Option<String> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return None;
        }
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_FORMAT(0),
            PWSTR(buf.as_mut_ptr()),
            &mut len,
        )
        .is_ok();
        let _ = windows::Win32::Foundation::CloseHandle(process);
        if !ok || len == 0 {
            return None;
        }
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        let trimmed = path.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }
}

/// The integrity level carried by a process token.
unsafe fn token_integrity_level(process: HANDLE) -> IntegrityLevel {
    let mut token = HANDLE::default();
    if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) }.is_err() {
        return IntegrityLevel::Medium;
    }
    let mut needed = 0u32;
    let _ = unsafe {
        GetTokenInformation(
            token,
            TokenIntegrityLevel,
            Some(std::ptr::null_mut()),
            0,
            &mut needed,
        )
    };
    if needed == 0 {
        let _ = windows::Win32::Foundation::CloseHandle(token);
        return IntegrityLevel::Medium;
    }
    let mut buffer = vec![0u8; needed as usize];
    let read = unsafe {
        GetTokenInformation(
            token,
            TokenIntegrityLevel,
            Some(buffer.as_mut_ptr() as *mut std::ffi::c_void),
            needed,
            &mut needed,
        )
    };
    let _ = windows::Win32::Foundation::CloseHandle(token);
    if read.is_err() {
        return IntegrityLevel::Medium;
    }
    let label = &*(buffer.as_ptr() as *const TOKEN_MANDATORY_LABEL);
    let sid = label.Label.Sid;
    let count = *GetSidSubAuthorityCount(sid);
    if count < 1 {
        return IntegrityLevel::Medium;
    }
    let authority = *GetSidSubAuthority(sid, (count - 1) as u32);
    match authority {
        0x0000 => IntegrityLevel::Untrusted,
        0x1000 => IntegrityLevel::Low,
        0x2000 => IntegrityLevel::Medium,
        0x3000 => IntegrityLevel::High,
        _ => IntegrityLevel::System,
    }
}

/// The extended frame bounds of a window, which is what the user sees.
pub fn extended_frame_bounds(handle: HWND) -> Option<WindowRect> {
    unsafe {
        let mut rect = RECT::default();
        let result = windows::Win32::Graphics::Dwm::DwmGetWindowAttribute(
            handle,
            windows::Win32::Graphics::Dwm::DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut rect as *mut RECT as *mut std::ffi::c_void,
            std::mem::size_of::<RECT>() as u32,
        );
        if result.is_err() {
            return None;
        }
        Some(WindowRect {
            left: rect.left,
            top: rect.top,
            right: rect.right,
            bottom: rect.bottom,
        })
    }
}
