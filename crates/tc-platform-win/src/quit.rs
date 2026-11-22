//! Cross-process named event signals for a running YAL instance.
//!
//! - `Local\YAL-quit` (`QuitWatcher` / `signal_quit`): lets `yal.exe --quit-running`
//!   and the installer ask a running instance to exit cleanly.
//! - `Local\YAL-show` (`ShowWatcher` / `signal_show`): lets a second launch of
//!   `yal.exe` wake and toggle the already-running instance's native palette.
//!
//! Both watchers block in `WaitForMultipleObjects` with `INFINITE` timeout plus
//! a private stop event (0% idle CPU, immediate clean shutdown on drop).

use std::sync::mpsc;
use std::sync::Mutex;
use std::thread::JoinHandle;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE, WAIT_OBJECT_0,
};
use windows::Win32::System::Threading::{
    CreateEventW, OpenEventW, ResetEvent, SetEvent, WaitForMultipleObjects, EVENT_MODIFY_STATE,
    INFINITE,
};

/// The name of the event that asks the running instance to quit.
const QUIT_EVENT_NAME: &str = r"Local\YAL-quit";

/// The name of the event that asks the running instance to toggle its palette.
const SHOW_EVENT_NAME: &str = r"Local\YAL-show";

/// Non-owning handle wrapper transferred across threads when owned elsewhere.
#[derive(Debug, Clone, Copy)]
struct SharedHandle(HANDLE);

unsafe impl Send for SharedHandle {}
unsafe impl Sync for SharedHandle {}

/// RAII handle wrapper guaranteeing `CloseHandle` even if a callback panics.
#[derive(Debug)]
struct OwnedHandle(HANDLE);

unsafe impl Send for OwnedHandle {}
unsafe impl Sync for OwnedHandle {}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

/// Watches a named event and invokes a callback once when signaled.
#[derive(Debug)]
pub struct QuitWatcher {
    stop_event: Mutex<Option<OwnedHandle>>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl QuitWatcher {
    /// Create the `Local\YAL-quit` event and watch for a quit request.
    pub fn spawn(on_quit: impl Fn() + Send + 'static) -> Result<QuitWatcher, String> {
        Self::spawn_named(QUIT_EVENT_NAME, on_quit)
    }

    fn spawn_named(name: &str, on_quit: impl Fn() + Send + 'static) -> Result<QuitWatcher, String> {
        let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let raw_signal = unsafe { CreateEventW(None, true, false, PCWSTR(wide.as_ptr())) }
            .map_err(|error| format!("the quit signal could not be created: {error}"))?;
        if raw_signal.is_invalid() {
            return Err("the quit signal could not be created".to_string());
        }
        // If a rapid restart attached to an already-signaled event, reset it so
        // the new instance does not immediately self-terminate.
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            unsafe {
                let _ = ResetEvent(raw_signal);
            }
        }
        let signal_event = OwnedHandle(raw_signal);

        let raw_stop = unsafe { CreateEventW(None, true, false, PCWSTR::null()) }
            .map_err(|error| format!("the watcher stop event could not be created: {error}"))?;
        let stop_event = OwnedHandle(raw_stop);
        let stop_shared = SharedHandle(stop_event.0);

        let (ready_tx, ready_rx) = mpsc::channel::<()>();

        let thread = std::thread::Builder::new()
            .name("yal-quit-watcher".to_string())
            .spawn(move || {
                let signal_event = signal_event;
                let stop_shared = stop_shared;
                let handles = [signal_event.0, stop_shared.0];
                let _ = ready_tx.send(());
                let wait = unsafe { WaitForMultipleObjects(&handles, false, INFINITE) };
                if wait == WAIT_OBJECT_0 {
                    on_quit();
                }
                drop(signal_event);
            })
            .map_err(|error| format!("the quit watcher thread could not be started: {error}"))?;

        let _ = ready_rx.recv();
        Ok(QuitWatcher {
            stop_event: Mutex::new(Some(stop_event)),
            thread: Mutex::new(Some(thread)),
        })
    }
}

impl Drop for QuitWatcher {
    fn drop(&mut self) {
        if let Some(stop) = self.stop_event.lock().expect("stop lock").take() {
            unsafe {
                let _ = SetEvent(stop.0);
            }
            if let Some(thread) = self.thread.lock().expect("thread lock").take() {
                let _ = thread.join();
            }
            drop(stop);
        }
    }
}

/// Watches `Local\YAL-show` (auto-reset) and calls `on_show` every time a
/// second instance launches.
#[derive(Debug)]
pub struct ShowWatcher {
    stop_event: Mutex<Option<OwnedHandle>>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl ShowWatcher {
    /// Create the `Local\YAL-show` event and invoke `on_show` on every signal.
    pub fn spawn(on_show: impl Fn() + Send + 'static) -> Result<ShowWatcher, String> {
        Self::spawn_named(SHOW_EVENT_NAME, on_show)
    }

    fn spawn_named(name: &str, on_show: impl Fn() + Send + 'static) -> Result<ShowWatcher, String> {
        let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        // Auto-reset event (`false`) so repeated second launches each fire cleanly.
        let raw_signal = unsafe { CreateEventW(None, false, false, PCWSTR(wide.as_ptr())) }
            .map_err(|error| format!("the show signal could not be created: {error}"))?;
        if raw_signal.is_invalid() {
            return Err("the show signal could not be created".to_string());
        }
        let signal_event = OwnedHandle(raw_signal);

        let raw_stop =
            unsafe { CreateEventW(None, true, false, PCWSTR::null()) }.map_err(|error| {
                format!("the show watcher stop event could not be created: {error}")
            })?;
        let stop_event = OwnedHandle(raw_stop);
        let stop_shared = SharedHandle(stop_event.0);

        let (ready_tx, ready_rx) = mpsc::channel::<()>();

        let thread = std::thread::Builder::new()
            .name("yal-show-watcher".to_string())
            .spawn(move || {
                let signal_event = signal_event;
                let stop_shared = stop_shared;
                let handles = [signal_event.0, stop_shared.0];
                let _ = ready_tx.send(());
                loop {
                    let wait = unsafe { WaitForMultipleObjects(&handles, false, INFINITE) };
                    if wait == WAIT_OBJECT_0 {
                        on_show();
                    } else {
                        break;
                    }
                }
                drop(signal_event);
            })
            .map_err(|error| format!("the show watcher thread could not be started: {error}"))?;

        let _ = ready_rx.recv();
        Ok(ShowWatcher {
            stop_event: Mutex::new(Some(stop_event)),
            thread: Mutex::new(Some(thread)),
        })
    }
}

impl Drop for ShowWatcher {
    fn drop(&mut self) {
        if let Some(stop) = self.stop_event.lock().expect("stop lock").take() {
            unsafe {
                let _ = SetEvent(stop.0);
            }
            if let Some(thread) = self.thread.lock().expect("thread lock").take() {
                let _ = thread.join();
            }
            drop(stop);
        }
    }
}

/// Ask a running instance to quit.
pub fn signal_quit() -> bool {
    signal_named(QUIT_EVENT_NAME)
}

/// Ask a running instance to show/toggle its palette.
pub fn signal_show() -> bool {
    signal_named(SHOW_EVENT_NAME)
}

fn signal_named(name: &str) -> bool {
    let name: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let event = match unsafe { OpenEventW(EVENT_MODIFY_STATE, false, PCWSTR(name.as_ptr())) } {
        Ok(handle) => handle,
        Err(_) => return false,
    };
    if event.is_invalid() {
        return false;
    }
    let signalled = unsafe { SetEvent(event) }.is_ok();
    unsafe {
        let _ = CloseHandle(event);
    }
    signalled
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
    use std::time::Duration;

    static NEXT_EVENT: AtomicU64 = AtomicU64::new(0);

    fn private_event_name(kind: &str) -> String {
        let number = NEXT_EVENT.fetch_add(1, Ordering::SeqCst);
        format!(r"Local\YAL-{kind}-test-{}-{number}", std::process::id())
    }

    fn event_exists(name: &str) -> bool {
        let name: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            OpenEventW(EVENT_MODIFY_STATE, false, PCWSTR(name.as_ptr()))
                .map(|handle| {
                    let _ = CloseHandle(handle);
                    true
                })
                .unwrap_or(false)
        }
    }

    #[test]
    fn a_signal_reaches_the_watcher_and_stops_when_nobody_listens() {
        let name = private_event_name("quit");
        let fired = std::sync::Arc::new(AtomicBool::new(false));
        let seen = fired.clone();
        let watcher = QuitWatcher::spawn_named(&name, move || {
            seen.store(true, Ordering::SeqCst);
        })
        .expect("the quit signal could not be created");

        assert!(
            signal_named(&name),
            "the running instance was not signalled"
        );

        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !fired.load(Ordering::SeqCst) && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(fired.load(Ordering::SeqCst), "the watcher did not fire");

        drop(watcher);
        assert!(!event_exists(&name), "a quit event was left behind");
        assert!(
            !signal_named(&name),
            "a signal was reported with no listener"
        );
    }

    #[test]
    fn the_watcher_keeps_the_event_alive_while_it_exists() {
        let name = private_event_name("quit");
        let watcher =
            QuitWatcher::spawn_named(&name, || {}).expect("the quit signal could not be created");
        assert!(event_exists(&name), "the event was taken away too early");
        drop(watcher);
        assert!(!event_exists(&name), "the event stayed open after drop");
    }

    #[test]
    fn show_watcher_fires_multiple_times_and_cleans_up_on_drop() {
        let name = private_event_name("show");
        let count = std::sync::Arc::new(AtomicUsize::new(0));
        let seen = count.clone();
        let watcher = ShowWatcher::spawn_named(&name, move || {
            seen.fetch_add(1, Ordering::SeqCst);
        })
        .expect("the show signal could not be created");

        assert!(signal_named(&name));
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while count.load(Ordering::SeqCst) < 1 && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(15));
        }
        assert_eq!(count.load(Ordering::SeqCst), 1);

        assert!(signal_named(&name));
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while count.load(Ordering::SeqCst) < 2 && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(15));
        }
        assert_eq!(count.load(Ordering::SeqCst), 2);

        drop(watcher);
        assert!(!event_exists(&name));
    }
}
