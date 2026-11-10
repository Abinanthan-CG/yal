//! Asking a running instance to quit.
//!
//! The installer replaces the program files, so it has to close a running
//! instance first. A second `yal.exe --quit-running` raises a named event; the
//! running instance is waiting on that event and exits when it fires.
//!
//! The event is named and lives in the caller's own session, so the signal
//! never reaches another user and never leaves the machine.

use std::sync::mpsc;
use std::sync::Mutex;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{
    CloseHandle, DuplicateHandle, DUPLICATE_SAME_ACCESS, HANDLE, WAIT_OBJECT_0,
};
use windows::Win32::System::Threading::{
    CreateEventW, GetCurrentProcess, OpenEventW, SetEvent, WaitForSingleObject, EVENT_MODIFY_STATE,
};

/// The name of the event that asks the running instance to quit.
const QUIT_EVENT_NAME: &str = r"Local\YAL-quit";

/// How long the watcher waits at a time before looking again.
///
/// The wait is interrupted by the event, so the timeout only bounds how often
/// the thread wakes up.
const WATCH_INTERVAL_MILLIS: u32 = 250;

/// A kernel handle that is safe to move between threads.
///
/// A handle is a plain value; Windows does not care which thread uses it, as
/// long as only one thread uses it at a time.
#[derive(Debug, Clone, Copy)]
struct SendableHandle(HANDLE);

// A kernel handle is a plain value; it is safe to move between threads.
unsafe impl Send for SendableHandle {}
unsafe impl Sync for SendableHandle {}

/// The handle of the quit event.
///
/// The watcher and its thread each own a handle to the same event, so that
/// closing one does not pull the handle out from under the other.
#[derive(Debug)]
pub struct QuitWatcher {
    handle: Mutex<Option<HANDLE>>,
}

// A kernel handle is a plain value; it is safe to move between threads.
unsafe impl Send for QuitWatcher {}
unsafe impl Sync for QuitWatcher {}

impl QuitWatcher {
    /// Create the quit event and watch for a request.
    ///
    /// The callback runs on a background thread, so it must not block for long
    /// and must be safe to call from another thread.
    pub fn spawn(on_quit: impl Fn() + Send + 'static) -> Result<QuitWatcher, String> {
        let name: Vec<u16> = QUIT_EVENT_NAME
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let handle = unsafe { CreateEventW(None, true, false, PCWSTR(name.as_ptr())) }
            .map_err(|error| format!("the quit signal could not be created: {error}"))?;
        if handle.is_invalid() {
            return Err("the quit signal could not be created".to_string());
        }

        // The thread gets a handle of its own, so that the watcher can close
        // its own without ending the thread's wait.
        let mut thread_handle = HANDLE::default();
        unsafe {
            DuplicateHandle(
                GetCurrentProcess(),
                handle,
                GetCurrentProcess(),
                std::ptr::addr_of_mut!(thread_handle),
                0,
                false,
                DUPLICATE_SAME_ACCESS,
            )
            .map_err(|error| {
                let _ = CloseHandle(handle);
                format!("the quit signal could not be duplicated: {error}")
            })?;
        }

        let watcher = QuitWatcher {
            handle: Mutex::new(Some(handle)),
        };
        let (ready_tx, ready_rx) = mpsc::channel::<()>();
        let waiting = SendableHandle(thread_handle);
        std::thread::spawn(move || {
            // Take the whole wrapper, so the thread owns the handle's type
            // rather than a bare kernel handle.
            let waiting = waiting;
            // Tell the caller the watcher is live, so a test does not signal
            // before the event is being waited on.
            let _ = ready_tx.send(());
            loop {
                let result = unsafe { WaitForSingleObject(waiting.0, WATCH_INTERVAL_MILLIS) };
                if result == WAIT_OBJECT_0 {
                    on_quit();
                    break;
                }
            }
            let _ = unsafe { CloseHandle(waiting.0) };
        });
        let _ = ready_rx.recv();
        Ok(watcher)
    }
}

impl Drop for QuitWatcher {
    fn drop(&mut self) {
        let taken = self.handle.lock().expect("quit signal lock").take();
        if let Some(handle) = taken {
            unsafe {
                let _ = CloseHandle(handle);
            }
        }
    }
}

/// Ask a running instance to quit.
///
/// The answer says whether anyone was listening. `false` means no instance was
/// running, which is not a failure: the caller wanted it gone, and it is gone.
pub fn signal_quit() -> bool {
    let name: Vec<u16> = QUIT_EVENT_NAME
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
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
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    #[test]
    fn a_signal_reaches_the_watcher_and_stops_when_nobody_listens() {
        let fired = std::sync::Arc::new(AtomicBool::new(false));
        let seen = fired.clone();
        let watcher = QuitWatcher::spawn(move || {
            seen.store(true, Ordering::SeqCst);
        })
        .expect("the quit signal could not be created");

        assert!(signal_quit(), "the running instance was not signalled");

        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !fired.load(Ordering::SeqCst) && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(25));
        }
        assert!(fired.load(Ordering::SeqCst), "the watcher did not fire");

        // Once the watcher is gone the event is gone with it, so a signal has
        // nobody to reach and says so.
        drop(watcher);
        let name: Vec<u16> = QUIT_EVENT_NAME
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let exists = unsafe {
            OpenEventW(EVENT_MODIFY_STATE, false, PCWSTR(name.as_ptr()))
                .map(|handle| {
                    let _ = CloseHandle(handle);
                    true
                })
                .unwrap_or(false)
        };
        assert!(!exists, "a quit event was left behind");
        assert!(!signal_quit(), "a signal was reported with no listener");
    }

    #[test]
    fn the_watcher_keeps_the_event_alive_while_it_exists() {
        // The watcher holds the event open. A watcher that is dropped straight
        // away would take the event with it, and the running instance would
        // never hear the signal.
        let watcher = QuitWatcher::spawn(|| {}).expect("the quit signal could not be created");
        assert!(
            signal_quit(),
            "the event was not alive while the watcher existed"
        );
        drop(watcher);
    }
}
