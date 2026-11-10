use tc_platform::Hotkey;
use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT, WH_KEYBOARD_LL,
};

/// The low-level keyboard hook, used for global shortcuts, modifier taps and
/// keyword expansion.
///
/// The hook is installed on demand and removed as soon as nothing needs it, so
/// a feature that is off costs nothing.
#[derive(Debug, Default)]
pub struct WindowsHotkey {
    hook: std::sync::Mutex<Option<HHOOK>>,
}

// A hook handle is a plain kernel handle; it is safe to move between threads.
unsafe impl Send for WindowsHotkey {}
unsafe impl Sync for WindowsHotkey {}

impl WindowsHotkey {
    pub fn new() -> WindowsHotkey {
        WindowsHotkey::default()
    }
}

impl Hotkey for WindowsHotkey {
    fn is_available(&self) -> bool {
        // Installing and removing the hook is the only reliable way to know:
        // a hook fails when the desktop is secure or the session is locked.
        match self.install() {
            Some(hook) => {
                unsafe {
                    let _ = UnhookWindowsHookEx(hook);
                }
                self.hook.lock().expect("hook lock").take();
                true
            }
            None => false,
        }
    }

    fn unavailable_reason(&self) -> Option<String> {
        if self.is_available() {
            None
        } else {
            Some(
                "The keyboard hook is unavailable. The secure desktop, the lock screen and \
                 administrator windows all block it."
                    .to_string(),
            )
        }
    }
}

impl WindowsHotkey {
    /// Install the hook, keeping it for later.
    pub fn install(&self) -> Option<HHOOK> {
        let mut guard = self.hook.lock().expect("hook lock");
        if let Some(hook) = *guard {
            return Some(hook);
        }
        let module = unsafe { GetModuleHandleW(None).ok()? };
        let module = Some(HINSTANCE(module.0));
        let hook = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_procedure), module, 0) }.ok()?;
        *guard = Some(hook);
        Some(hook)
    }

    /// Remove the hook.
    pub fn uninstall(&self) {
        let taken = self.hook.lock().expect("hook lock").take();
        if let Some(hook) = taken {
            unsafe {
                let _ = UnhookWindowsHookEx(hook);
            }
        }
    }
}

/// The hook callback. It must return quickly, so it does nothing but pass the
/// event on.
unsafe extern "system" fn hook_procedure(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code >= 0 {
        let _ = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        let _ = wparam;
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}
