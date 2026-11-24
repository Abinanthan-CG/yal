//! Pure Win32 overlapped directory watcher (`ReadDirectoryChangesW`).
//!
//! Runs one lightweight thread per watched root using overlapped I/O plus a
//! shared manual-reset stop event (0% idle CPU, instant cancellation on drop)
//! and coalesces bursts of file-system events on a single blocking debouncer
//! thread.

use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{
    CloseHandle, ERROR_IO_PENDING, HANDLE, WAIT_EVENT, WAIT_OBJECT_0,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, ReadDirectoryChangesW, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OVERLAPPED,
    FILE_LIST_DIRECTORY, FILE_NOTIFY_CHANGE_DIR_NAME, FILE_NOTIFY_CHANGE_FILE_NAME,
    FILE_NOTIFY_CHANGE_LAST_WRITE, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    OPEN_EXISTING,
};
use windows::Win32::System::Threading::{
    CreateEventW, ResetEvent, SetEvent, WaitForMultipleObjects, INFINITE,
};
use windows::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};

const DEBOUNCE_WINDOW: Duration = Duration::from_millis(500);
const CHANGE_BUFFER_DWORDS: usize = 4096;

/// RAII wrapper for a Win32 kernel `HANDLE` that guarantees `CloseHandle` on drop.
#[derive(Debug)]
struct OwnedHandle(HANDLE);

// SAFETY: Win32 kernel event and file handles are process-wide kernel object
// references safe to transfer between threads.
unsafe impl Send for OwnedHandle {}

// SAFETY: Concurrent Win32 wait/signal calls (`WaitForMultipleObjects`, `SetEvent`)
// on a shared kernel handle are thread-safe.
unsafe impl Sync for OwnedHandle {}

impl OwnedHandle {
    fn raw(&self) -> HANDLE {
        self.0
    }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            // SAFETY: `self.0` is an owned valid kernel handle not closed elsewhere.
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

enum DebounceMsg {
    Changed,
    Stop,
}

/// Watches one or more directory trees for changes and invokes a debounced callback.
pub struct FileWatcher {
    stop_event: Arc<OwnedHandle>,
    debounce_tx: Option<Sender<DebounceMsg>>,
    watcher_threads: Vec<JoinHandle<()>>,
    debounce_thread: Option<JoinHandle<()>>,
}

impl FileWatcher {
    /// Start watching each existing directory in `roots` recursively.
    ///
    /// `on_changed` is called on a background debouncer thread after a quiet
    /// window of 500 ms following file/directory creations, renames, deletions,
    /// or writes.
    pub fn start(
        roots: Vec<PathBuf>,
        on_changed: impl Fn() + Send + Sync + 'static,
    ) -> Result<Self, String> {
        // SAFETY: Creating an unnamed manual-reset, initially non-signaled Win32 event.
        let raw_stop = unsafe { CreateEventW(None, true, false, PCWSTR::null()) }
            .map_err(|error| format!("file watcher stop event could not be created: {error}"))?;
        if raw_stop.is_invalid() {
            return Err("file watcher stop event handle was invalid".to_string());
        }
        let stop_event = Arc::new(OwnedHandle(raw_stop));

        let (debounce_tx, debounce_rx) = mpsc::channel::<DebounceMsg>();
        let debounce_thread = std::thread::Builder::new()
            .name("yal-file-watcher-debounce".to_string())
            .spawn(move || run_debouncer(debounce_rx, on_changed))
            .map_err(|error| format!("file watcher debouncer thread could not start: {error}"))?;

        let mut watcher_threads = Vec::new();
        for (index, root) in roots.into_iter().enumerate() {
            if !root.is_dir() {
                continue;
            }
            let dir_handle = match open_directory_overlapped(&root) {
                Ok(handle) => handle,
                Err(_) => continue,
            };
            // SAFETY: Creating a per-thread unnamed manual-reset Win32 event for overlapped I/O.
            let raw_io = match unsafe { CreateEventW(None, true, false, PCWSTR::null()) } {
                Ok(handle) if !handle.is_invalid() => handle,
                _ => continue,
            };
            let io_event = OwnedHandle(raw_io);
            let stop_clone = Arc::clone(&stop_event);
            let tx_clone = debounce_tx.clone();
            let (ready_tx, ready_rx) = mpsc::channel::<()>();

            match std::thread::Builder::new()
                .name(format!("yal-file-watcher-{index}"))
                .spawn(move || {
                    run_directory_watcher(dir_handle, io_event, stop_clone, tx_clone, ready_tx);
                }) {
                Ok(handle) => {
                    let _ = ready_rx.recv();
                    watcher_threads.push(handle);
                }
                Err(_) => continue,
            }
        }

        Ok(Self {
            stop_event,
            debounce_tx: Some(debounce_tx),
            watcher_threads,
            debounce_thread: Some(debounce_thread),
        })
    }

    /// Stop all directory watcher threads and the debouncer thread cleanly.
    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        // SAFETY: `self.stop_event` is a valid manual-reset event kept alive via `Arc`.
        unsafe {
            let _ = SetEvent(self.stop_event.raw());
        }
        if let Some(tx) = self.debounce_tx.take() {
            let _ = tx.send(DebounceMsg::Stop);
        }
        for handle in self.watcher_threads.drain(..) {
            let _ = handle.join();
        }
        if let Some(handle) = self.debounce_thread.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for FileWatcher {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn open_directory_overlapped(path: &Path) -> Result<OwnedHandle, String> {
    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    // SAFETY: `wide` is a valid null-terminated UTF-16 path. Opening a directory
    // with `FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OVERLAPPED` is required for
    // asynchronous `ReadDirectoryChangesW`.
    let handle = unsafe {
        CreateFileW(
            PCWSTR(wide.as_ptr()),
            FILE_LIST_DIRECTORY.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OVERLAPPED,
            None,
        )
    }
    .map_err(|error| {
        format!(
            "could not open watched directory {}: {error}",
            path.display()
        )
    })?;
    if handle.is_invalid() {
        return Err(format!(
            "opened directory handle was invalid: {}",
            path.display()
        ));
    }
    Ok(OwnedHandle(handle))
}

fn run_directory_watcher(
    dir_handle: OwnedHandle,
    io_event: OwnedHandle,
    stop_event: Arc<OwnedHandle>,
    debounce_tx: Sender<DebounceMsg>,
    ready_tx: Sender<()>,
) {
    // DWORD-aligned 16 KB buffer required by `ReadDirectoryChangesW`.
    let mut buf = vec![0u32; CHANGE_BUFFER_DWORDS];
    let buf_byte_len = (buf.len() * std::mem::size_of::<u32>()) as u32;
    let notify_filter =
        FILE_NOTIFY_CHANGE_FILE_NAME | FILE_NOTIFY_CHANGE_DIR_NAME | FILE_NOTIFY_CHANGE_LAST_WRITE;
    let wait_handles = [stop_event.raw(), io_event.raw()];
    let io_signaled = WAIT_EVENT(WAIT_OBJECT_0.0 + 1);
    let mut ready_tx = Some(ready_tx);

    loop {
        // SAFETY: `io_event` is an owned valid manual-reset event.
        if unsafe { ResetEvent(io_event.raw()) }.is_err() {
            break;
        }

        let mut overlapped = OVERLAPPED {
            hEvent: io_event.raw(),
            ..Default::default()
        };

        // SAFETY: `dir_handle` was opened with `FILE_FLAG_OVERLAPPED`, `buf` is
        // DWORD-aligned and lives until after `GetOverlappedResult` completes or
        // cancels, and `overlapped` stays pinned at the same stack address until
        // the pending I/O finishes or is cancelled and awaited below.
        let issue_result = unsafe {
            ReadDirectoryChangesW(
                dir_handle.raw(),
                buf.as_mut_ptr() as *mut c_void,
                buf_byte_len,
                true,
                notify_filter,
                None,
                Some(&mut overlapped),
                None,
            )
        };

        let queued = match issue_result {
            Ok(()) => true,
            Err(error) => error.code() == ERROR_IO_PENDING.to_hresult(),
        };

        if let Some(tx) = ready_tx.take() {
            let _ = tx.send(());
        }

        if !queued {
            break;
        }

        // SAFETY: Both handles in `wait_handles` are valid kernel events kept alive
        // for the full duration of `WaitForMultipleObjects`.
        let wait = unsafe { WaitForMultipleObjects(&wait_handles, false, INFINITE) };
        if wait == io_signaled {
            let mut transferred = 0u32;
            // SAFETY: `io_event` was signaled so the overlapped operation has completed;
            // `overlapped` and `buf` are still alive.
            let res = unsafe {
                GetOverlappedResult(dir_handle.raw(), &overlapped, &mut transferred, false)
            };
            if res.is_err() {
                break;
            }
            if debounce_tx.send(DebounceMsg::Changed).is_err() {
                break;
            }
        } else {
            // Stop requested (`WAIT_OBJECT_0`) or unexpected wait failure.
            // Cancel the pending overlapped read and ALWAYS wait for the kernel
            // to finish touching `buf` and `overlapped` before dropping them.
            let mut transferred = 0u32;
            // SAFETY: `dir_handle` and `overlapped` remain valid until `GetOverlappedResult`
            // returns with `bwait = true`, preventing any use-after-free by the kernel.
            unsafe {
                let _ = CancelIoEx(dir_handle.raw(), Some(&overlapped));
                let _ = GetOverlappedResult(dir_handle.raw(), &overlapped, &mut transferred, true);
            }
            break;
        }
    }
}

fn run_debouncer(rx: mpsc::Receiver<DebounceMsg>, on_changed: impl Fn() + Send + Sync + 'static) {
    while let Ok(msg) = rx.recv() {
        match msg {
            DebounceMsg::Stop => return,
            DebounceMsg::Changed => loop {
                match rx.recv_timeout(DEBOUNCE_WINDOW) {
                    Ok(DebounceMsg::Changed) => continue,
                    Ok(DebounceMsg::Stop) | Err(RecvTimeoutError::Disconnected) => return,
                    Err(RecvTimeoutError::Timeout) => {
                        on_changed();
                        break;
                    }
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Instant;

    #[test]
    fn file_watcher_detects_new_file_and_stops_cleanly() {
        let unique = format!(
            "yal-file-watcher-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );
        let dir = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&dir).expect("create temp test directory");

        let fired = Arc::new(AtomicUsize::new(0));
        let fired_cb = Arc::clone(&fired);
        let watcher = FileWatcher::start(vec![dir.clone()], move || {
            fired_cb.fetch_add(1, Ordering::SeqCst);
        })
        .expect("file watcher should start");

        let file_path = dir.join("probe.txt");
        std::fs::write(&file_path, b"hello").expect("write probe file");

        let deadline = Instant::now() + Duration::from_secs(3);
        while fired.load(Ordering::SeqCst) == 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(25));
        }
        assert!(
            fired.load(Ordering::SeqCst) >= 1,
            "file watcher callback did not fire within 3 seconds"
        );

        watcher.stop();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
