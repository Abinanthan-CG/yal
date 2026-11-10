use tc_platform::SingleInstance;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Threading::{
    CreateMutexW, OpenMutexW, ReleaseMutex, SYNCHRONIZATION_ACCESS_RIGHTS,
};
use windows::core::PCWSTR;

/// The name of the mutex that keeps one instance running.
const MUTEX_NAME: &str = r"Local\YAL-single-instance";

/// A lock backed by a named mutex.
#[derive(Debug)]
pub struct WindowsSingleInstance {
    handle: std::sync::Mutex<Option<HANDLE>>,
}

// A window handle is a plain kernel handle; it is safe to move between threads.
unsafe impl Send for WindowsSingleInstance {}
unsafe impl Sync for WindowsSingleInstance {}

impl WindowsSingleInstance {
    pub fn new() -> WindowsSingleInstance {
        WindowsSingleInstance {
            handle: std::sync::Mutex::new(None),
        }
    }
}

impl Default for WindowsSingleInstance {
    fn default() -> Self {
        WindowsSingleInstance::new()
    }
}

impl SingleInstance for WindowsSingleInstance {
    fn acquire(&self) -> bool {
        let name: Vec<u16> = MUTEX_NAME.encode_utf16().chain(std::iter::once(0)).collect();
        // A mutex that already exists is how the second instance finds the
        // first one.
        let created = match unsafe { CreateMutexW(None, true, PCWSTR(name.as_ptr())) } {
            Ok(handle) => handle,
            Err(_) => return false,
        };
        if created.is_invalid() {
            return false;
        }
        if unsafe { windows::Win32::Foundation::GetLastError() }
            == windows::Win32::Foundation::ERROR_ALREADY_EXISTS
        {
            unsafe {
                let _ = CloseHandle(created);
            }
            return false;
        }
        *self.handle.lock().expect("instance lock") = Some(created);
        true
    }

    fn release(&self) {
        let taken = self.handle.lock().expect("instance lock").take();
        if let Some(handle) = taken {
            unsafe {
                let _ = ReleaseMutex(handle);
                let _ = CloseHandle(handle);
            }
        }
    }
}

/// Whether another instance already holds the lock, without taking it.
pub fn another_instance_is_running() -> bool {
    let name: Vec<u16> = MUTEX_NAME.encode_utf16().chain(std::iter::once(0)).collect();
    let existing = match unsafe {
        OpenMutexW(
            SYNCHRONIZATION_ACCESS_RIGHTS(0x00100000),
            false,
            PCWSTR(name.as_ptr()),
        )
    } {
        Ok(handle) => handle,
        Err(_) => return false,
    };
    if existing.is_invalid() {
        false
    } else {
        unsafe {
            let _ = CloseHandle(existing);
        }
        true
    }
}
