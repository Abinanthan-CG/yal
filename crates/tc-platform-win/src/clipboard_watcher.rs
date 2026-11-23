//! Native Win32 background clipboard watcher (`AddClipboardFormatListener`).
//!
//! Runs on a dedicated thread with a blocking `GetMessageW` loop (0% idle CPU)
//! and invokes a callback whenever another application updates the clipboard
//! with text. Cleans up via `RemoveClipboardFormatListener` inside `WM_DESTROY`.

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
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
    GetWindowLongPtrW, IsWindow, PostMessageW, PostQuitMessage, RegisterClassExW,
    SetWindowLongPtrW, TranslateMessage, CREATESTRUCTW, GWLP_USERDATA, HWND_MESSAGE, MSG,
    WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLIPBOARDUPDATE, WM_CLOSE, WM_DESTROY, WM_NCCREATE,
    WM_NCDESTROY, WNDCLASSEXW,
};

use tc_platform::{Clipboard, WindowSystem};

use crate::clipboard::WindowsClipboard;
use crate::windows::WindowsWindows;

type OnCopy = Box<dyn Fn(String, Option<String>) + Send>;

struct WatcherState {
    on_copy: OnCopy,
    last_text: Option<String>,
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
        let (ready_tx, ready_rx) = mpsc::channel::<Result<isize, String>>();
        let on_copy: OnCopy = Box::new(on_copy);

        let thread = std::thread::Builder::new()
            .name("yal-clipboard-watcher".to_string())
            .spawn(move || {
                run_watcher_thread(on_copy, ready_tx);
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

fn run_watcher_thread(on_copy: OnCopy, ready_tx: mpsc::Sender<Result<isize, String>>) {
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
    if unsafe { RegisterClassExW(&class) } == 0 {
        let last = unsafe { windows::Win32::Foundation::GetLastError() };
        if last != windows::Win32::Foundation::ERROR_CLASS_ALREADY_EXISTS {
            let _ = ready_tx.send(Err(
                "the clipboard watcher window class could not be registered".to_string(),
            ));
            return;
        }
    }

    let initial_text = WindowsClipboard::new().read_text();
    let state_ptr = Box::into_raw(Box::new(WatcherState {
        on_copy,
        last_text: initial_text,
    }));

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
            unsafe {
                drop(Box::from_raw(state_ptr));
            }
            let _ = ready_tx.send(Err(format!(
                "the clipboard watcher window could not be created: {error}"
            )));
            return;
        }
    };

    if unsafe { AddClipboardFormatListener(hwnd) }.is_err() {
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

unsafe extern "system" fn watcher_wnd_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_NCCREATE {
        let create = &*(lparam.0 as *const CREATESTRUCTW);
        let state = create.lpCreateParams as *mut WatcherState;
        let _ = SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);
        return DefWindowProcW(hwnd, message, wparam, lparam);
    }

    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WatcherState;
    if ptr.is_null() {
        return DefWindowProcW(hwnd, message, wparam, lparam);
    }

    match message {
        WM_CLIPBOARDUPDATE => {
            if let Some(text) = WindowsClipboard::new().read_text() {
                if !text.is_empty() && (*ptr).last_text.as_deref() != Some(text.as_str()) {
                    (*ptr).last_text = Some(text.clone());
                    let source = WindowsWindows::new()
                        .foreground_window()
                        .map(|w| w.title)
                        .filter(|t| !t.trim().is_empty() && t != "YAL");
                    ((*ptr).on_copy)(text, source);
                }
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            let _ = RemoveClipboardFormatListener(hwnd);
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
    fn clipboard_watcher_spawns_and_stops_cleanly() {
        let mut watcher =
            ClipboardWatcher::spawn(|_, _| {}).expect("clipboard watcher should spawn on Windows");
        watcher.stop();
    }
}
