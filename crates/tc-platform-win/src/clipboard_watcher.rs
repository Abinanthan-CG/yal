//! Native Win32 background clipboard watcher (`AddClipboardFormatListener`).
//!
//! Runs on a dedicated thread with a blocking `GetMessageW` loop (0% idle CPU)
//! and invokes a callback whenever another application updates the clipboard.
//! Cleans up via `RemoveClipboardFormatListener` inside `WM_DESTROY`.

use std::ffi::c_void;
use std::sync::mpsc;
use std::thread::JoinHandle;

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::DataExchange::{
    AddClipboardFormatListener, RemoveClipboardFormatListener,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetForegroundWindow,
    GetMessageW, GetWindowLongPtrW, GetWindowTextW, IsWindow, PostMessageW, PostQuitMessage,
    RegisterClassExW, SetWindowLongPtrW, TranslateMessage, CREATESTRUCTW, GWLP_USERDATA,
    HWND_MESSAGE, MSG, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLIPBOARDUPDATE, WM_CLOSE, WM_DESTROY,
    WM_NCCREATE, WM_NCDESTROY, WNDCLASSEXW,
};

use tc_platform::{Clipboard, WindowSystem};

use crate::clipboard::{is_sensitive_clipboard, WindowsClipboard};
use crate::windows::WindowsWindows;

const FNV1A_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV1A_PRIME: u64 = 0x0000_0100_0000_01b3;

const PASSWORD_MANAGER_PATTERNS: &[&str] = &[
    "1password",
    "bitwarden",
    "keepass",
    "dashlane",
    "lastpass",
    "proton pass",
    "enpass",
    "roboform",
    "windows security",
];

/// A rich clipboard payload captured by [`ClipboardWatcher::spawn_rich`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardUpdate {
    Text(String),
    Files(Vec<String>),
    Image {
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    },
}

/// Returns `true` if `title` matches a known password manager or Windows Security window.
pub fn is_password_manager_title(title: &str) -> bool {
    let lower = title.to_ascii_lowercase();
    PASSWORD_MANAGER_PATTERNS
        .iter()
        .any(|pattern| lower.contains(pattern))
}

type OnCopy = Box<dyn Fn(String, Option<String>) + Send>;
type OnRichUpdate = Box<dyn Fn(ClipboardUpdate) + Send>;

enum WatcherState {
    Text {
        on_copy: OnCopy,
        last_text: Option<String>,
    },
    Rich {
        on_update: OnRichUpdate,
        last_hash: Option<u64>,
    },
}

/// Live background watcher listening for `WM_CLIPBOARDUPDATE`.
pub struct ClipboardWatcher {
    hwnd: isize,
    thread: Option<JoinHandle<()>>,
}

impl ClipboardWatcher {
    /// Start watching the Windows clipboard on a background thread.
    ///
    /// `on_copy` receives `(text, source_window_title)` whenever new text is copied.
    pub fn spawn(
        on_copy: impl Fn(String, Option<String>) + Send + 'static,
    ) -> Result<Self, String> {
        let initial_text = if is_sensitive_clipboard() {
            None
        } else {
            WindowsClipboard::new().read_text()
        };
        Self::spawn_state(WatcherState::Text {
            on_copy: Box::new(on_copy),
            last_text: initial_text,
        })
    }

    /// Start watching the Windows clipboard for files, images (`CF_DIB`), and text.
    ///
    /// Skips sensitive clipboard payloads and password manager foreground windows,
    /// and deduplicates consecutive identical items by 64-bit FNV-1a fingerprint.
    pub fn spawn_rich(on_update: impl Fn(ClipboardUpdate) + Send + 'static) -> Option<Self> {
        let last_hash = if is_sensitive_clipboard() {
            None
        } else {
            read_rich_clipboard().as_ref().map(fingerprint_update)
        };
        Self::spawn_state(WatcherState::Rich {
            on_update: Box::new(on_update),
            last_hash,
        })
        .ok()
    }

    fn spawn_state(state: WatcherState) -> Result<Self, String> {
        let (ready_tx, ready_rx) = mpsc::channel::<Result<isize, String>>();

        let thread = std::thread::Builder::new()
            .name("yal-clipboard-watcher".to_string())
            .spawn(move || {
                run_watcher_thread(state, ready_tx);
            })
            .map_err(|error| format!("the clipboard watcher thread could not start: {error}"))?;

        let hwnd = match ready_rx.recv() {
            Ok(Ok(hwnd)) => hwnd,
            Ok(Err(error)) => {
                let _ = thread.join();
                return Err(error);
            }
            Err(_) => {
                let _ = thread.join();
                return Err("the clipboard watcher exited before registering".to_string());
            }
        };

        Ok(Self {
            hwnd,
            thread: Some(thread),
        })
    }

    /// Stop watching and release the hidden message window cleanly.
    pub fn stop(&mut self) {
        if self.hwnd != 0 {
            let hwnd = HWND(self.hwnd as *mut c_void);
            // SAFETY: `hwnd` was created on the watcher thread and `PostMessageW` is
            // thread-safe; if the window already closed, `PostMessageW` safely returns Err.
            unsafe {
                let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
            }
            self.hwnd = 0;
        }
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for ClipboardWatcher {
    fn drop(&mut self) {
        self.stop();
    }
}

fn foreground_window_title() -> String {
    // SAFETY: `GetForegroundWindow` has no preconditions and returns a null HWND if none.
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.0.is_null() {
        return String::new();
    }
    let mut buf = [0u16; 512];
    // SAFETY: `buf` is a valid mutable UTF-16 buffer passed to `GetWindowTextW`.
    let len = unsafe { GetWindowTextW(hwnd, &mut buf) };
    if len <= 0 {
        return String::new();
    }
    let count = (len as usize).min(buf.len());
    String::from_utf16_lossy(&buf[..count])
}

fn read_rich_clipboard() -> Option<ClipboardUpdate> {
    let clipboard = WindowsClipboard::new();
    if let Some(files) = clipboard.read_files() {
        if !files.is_empty() {
            return Some(ClipboardUpdate::Files(files));
        }
    }
    if let Some((width, height, rgba)) = clipboard.read_dib_rgba() {
        return Some(ClipboardUpdate::Image {
            width,
            height,
            rgba,
        });
    }
    if let Some(text) = clipboard.read_text() {
        if !text.is_empty() {
            return Some(ClipboardUpdate::Text(text));
        }
    }
    None
}

#[inline]
fn fnv1a_update(mut hash: u64, bytes: &[u8]) -> u64 {
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV1A_PRIME);
    }
    hash
}

fn fingerprint_update(update: &ClipboardUpdate) -> u64 {
    let mut hash = FNV1A_OFFSET_BASIS;
    match update {
        ClipboardUpdate::Text(text) => {
            hash = fnv1a_update(hash, &[1]);
            hash = fnv1a_update(hash, &(text.len() as u64).to_le_bytes());
            hash = fnv1a_update(hash, text.as_bytes());
        }
        ClipboardUpdate::Files(files) => {
            hash = fnv1a_update(hash, &[2]);
            hash = fnv1a_update(hash, &(files.len() as u64).to_le_bytes());
            for path in files {
                hash = fnv1a_update(hash, &(path.len() as u64).to_le_bytes());
                hash = fnv1a_update(hash, path.as_bytes());
                hash = fnv1a_update(hash, &[0]);
            }
        }
        ClipboardUpdate::Image {
            width,
            height,
            rgba,
        } => {
            hash = fnv1a_update(hash, &[3]);
            hash = fnv1a_update(hash, &width.to_le_bytes());
            hash = fnv1a_update(hash, &height.to_le_bytes());
            hash = fnv1a_update(hash, &(rgba.len() as u64).to_le_bytes());
            hash = fnv1a_update(hash, rgba);
        }
    }
    hash
}

fn run_watcher_thread(state: WatcherState, ready_tx: mpsc::Sender<Result<isize, String>>) {
    // SAFETY: Passing `None` queries the handle of the current executable module.
    let instance = match unsafe { GetModuleHandleW(None) } {
        Ok(h) => HINSTANCE(h.0),
        Err(error) => {
            let _ = ready_tx.send(Err(format!("GetModuleHandleW failed: {error}")));
            return;
        }
    };

    let class = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(watcher_wnd_proc),
        hInstance: instance,
        lpszClassName: w!("YalClipboardWatcher"),
        ..Default::default()
    };
    // SAFETY: `class` has a valid `cbSize`, static UTF-16 class name, and valid `wndproc`.
    if unsafe { RegisterClassExW(&class) } == 0 {
        // SAFETY: `GetLastError` reads thread-local error state set by `RegisterClassExW`.
        let last = unsafe { windows::Win32::Foundation::GetLastError() };
        if last != windows::Win32::Foundation::ERROR_CLASS_ALREADY_EXISTS {
            let _ = ready_tx.send(Err(
                "the clipboard watcher window class could not be registered".to_string(),
            ));
            return;
        }
    }

    let state_ptr = Box::into_raw(Box::new(state));

    // SAFETY: Class is registered, `HWND_MESSAGE` requests a message-only window, and
    // `state_ptr` stays valid until freed after the message loop exits.
    let hwnd = match unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("YalClipboardWatcher"),
            PCWSTR::null(),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            Some(instance),
            Some(state_ptr as *mut c_void),
        )
    } {
        Ok(hwnd) => hwnd,
        Err(error) => {
            // SAFETY: Window creation failed so `watcher_wnd_proc` no longer holds `state_ptr`.
            unsafe {
                drop(Box::from_raw(state_ptr));
            }
            let _ = ready_tx.send(Err(format!(
                "the clipboard watcher window could not be created: {error}"
            )));
            return;
        }
    };

    // SAFETY: `hwnd` is a valid window owned by the current thread.
    if unsafe { AddClipboardFormatListener(hwnd) }.is_err() {
        // SAFETY: Detach `GWLP_USERDATA` and destroy `hwnd` before freeing `state_ptr`.
        unsafe {
            let _ = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            let _ = DestroyWindow(hwnd);
            drop(Box::from_raw(state_ptr));
        }
        let _ = ready_tx.send(Err(
            "AddClipboardFormatListener refused registration".to_string()
        ));
        return;
    }

    let _ = ready_tx.send(Ok(hwnd.0 as isize));

    // SAFETY: Message loop runs exclusively on the thread owning `hwnd`; `state_ptr`
    // is detached on `WM_NCDESTROY` or defensively below before `Box::from_raw`.
    unsafe {
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        // Defensively detach GWLP_USERDATA and destroy hwnd if GetMessageW
        // returned -1 before WM_NCDESTROY ran.
        if IsWindow(Some(hwnd)).as_bool() {
            let _ = RemoveClipboardFormatListener(hwnd);
            let _ = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            let _ = DestroyWindow(hwnd);
        }
        drop(Box::from_raw(state_ptr));
    }
}

fn handle_clipboard_update(state: &mut WatcherState) {
    if is_sensitive_clipboard() {
        return;
    }
    let fg_title = foreground_window_title();
    if is_password_manager_title(&fg_title) {
        return;
    }

    match state {
        WatcherState::Text { on_copy, last_text } => {
            if let Some(text) = WindowsClipboard::new().read_text() {
                if !text.is_empty() && last_text.as_deref() != Some(text.as_str()) {
                    *last_text = Some(text.clone());
                    let source = WindowsWindows::new()
                        .foreground_window()
                        .map(|w| w.title)
                        .filter(|t| !t.trim().is_empty() && t != "YAL");
                    on_copy(text, source);
                }
            }
        }
        WatcherState::Rich {
            on_update,
            last_hash,
        } => {
            if let Some(update) = read_rich_clipboard() {
                let hash = fingerprint_update(&update);
                if *last_hash != Some(hash) {
                    *last_hash = Some(hash);
                    on_update(update);
                }
            }
        }
    }
}

unsafe extern "system" fn watcher_wnd_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_NCCREATE {
        // SAFETY: Windows passes a valid `*const CREATESTRUCTW` in `lparam` for `WM_NCCREATE`.
        let create = unsafe { &*(lparam.0 as *const CREATESTRUCTW) };
        let state = create.lpCreateParams as *mut WatcherState;
        // SAFETY: `hwnd` is the window being created; storing our boxed `WatcherState` pointer.
        unsafe {
            let _ = SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);
            return DefWindowProcW(hwnd, message, wparam, lparam);
        }
    }

    // SAFETY: `GWLP_USERDATA` holds either null or our valid `*mut WatcherState`.
    let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut WatcherState;
    if ptr.is_null() {
        // SAFETY: Forwarding unhandled message when no state is attached.
        return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
    }

    match message {
        WM_CLIPBOARDUPDATE => {
            // SAFETY: `ptr` is non-null, properly aligned, exclusively accessed on the
            // watcher thread, and remains alive until after the message loop exits.
            let state = unsafe { &mut *ptr };
            handle_clipboard_update(state);
            LRESULT(0)
        }
        WM_CLOSE => {
            // SAFETY: `hwnd` is owned by the current thread.
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            // SAFETY: Unregister clipboard listener and terminate this thread's message loop.
            unsafe {
                let _ = RemoveClipboardFormatListener(hwnd);
                PostQuitMessage(0);
            }
            LRESULT(0)
        }
        WM_NCDESTROY => {
            // SAFETY: Detach `GWLP_USERDATA` before the window handle becomes invalid.
            unsafe {
                let _ = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                DefWindowProcW(hwnd, message, wparam, lparam)
            }
        }
        // SAFETY: Forwarding all other messages to the default window procedure.
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipboard_watcher_spawns_and_stops_cleanly() {
        let mut watcher =
            ClipboardWatcher::spawn(|_, _| {}).expect("clipboard watcher should spawn on Windows");
        watcher.stop();
    }

    #[test]
    fn rich_clipboard_watcher_spawns_and_stops_cleanly() {
        let mut watcher =
            ClipboardWatcher::spawn_rich(|_| {}).expect("rich clipboard watcher should spawn");
        watcher.stop();
    }

    #[test]
    fn password_manager_title_detection_is_case_insensitive() {
        assert!(is_password_manager_title("1Password - Personal Vault"));
        assert!(is_password_manager_title("BITWARDEN Desktop"));
        assert!(is_password_manager_title("KeePassXC - database.kdbx"));
        assert!(is_password_manager_title("Dashlane"));
        assert!(is_password_manager_title("LastPass Vault"));
        assert!(is_password_manager_title("Proton Pass"));
        assert!(is_password_manager_title("Enpass Password Manager"));
        assert!(is_password_manager_title("RoboForm Editor"));
        assert!(is_password_manager_title("Windows Security"));

        assert!(!is_password_manager_title("Visual Studio Code"));
        assert!(!is_password_manager_title("Notepad"));
        assert!(!is_password_manager_title(""));
    }

    #[test]
    fn fingerprint_distinguishes_variants_and_dimensions() {
        let t = fingerprint_update(&ClipboardUpdate::Text("hello".into()));
        let f = fingerprint_update(&ClipboardUpdate::Files(vec!["hello".into()]));
        let i1 = fingerprint_update(&ClipboardUpdate::Image {
            width: 1,
            height: 2,
            rgba: vec![0u8; 8],
        });
        let i2 = fingerprint_update(&ClipboardUpdate::Image {
            width: 2,
            height: 1,
            rgba: vec![0u8; 8],
        });
        assert_ne!(t, f);
        assert_ne!(t, i1);
        assert_ne!(i1, i2);
    }
}
