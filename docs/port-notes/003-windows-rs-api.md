# Port note 003 — The Windows API shapes in windows-rs 0.62

**Date.** 2026-11-10

---

## The problem

`windows-rs` 0.62 reorganised several of the Win32 bindings. Code written
against an older version does not compile, and the compiler's errors are
sometimes about types that have moved rather than about the thing you got
wrong.

This note records the shapes that the port actually uses, so that the next
person does not spend an afternoon discovering them.

## Where things live

| Thing | Where it is in 0.62 |
|---|---|
| `BOOL` | `windows::core::BOOL`, **not** `windows::Win32::Foundation::BOOL` |
| `HANDLE`, `HGLOBAL` | `windows::Win32::Foundation` |
| `CF_UNICODETEXT` | `windows::Win32::System::Ole` |
| `TOKEN_QUERY` | `windows::Win32::Security` |
| `REG_SAM_FLAGS`, `KEY_READ` | `windows::Win32::System::Registry` |
| `SYNCHRONIZATION_ACCESS_RIGHTS`, `EVENT_MODIFY_STATE` | `windows::Win32::System::Threading` |
| `WAIT_EVENT`, `WAIT_OBJECT_0` | `windows::Win32::Foundation` |
| `PCWSTR` | `windows::core` |

## Functions return `Result`

Several functions that used to return a raw value now return
`windows::core::Result`:

```rust
// GetWindowRect, OpenProcessToken, GetTokenInformation, CreateMutexW,
// OpenMutexW, SetWindowsHookExW, CreateEventW, OpenEventW, SetEvent
```

So the check is `result.is_ok()` rather than `result != 0`.

## Out-parameters are pointers

`GetWindowRect` and friends take `Option<*mut RECT>`:

```rust
let mut rect = RECT::default();
GetWindowRect(window, Some(&mut rect)).ok();
```

The token functions take `Option<*mut c_void>` for the buffer:

```rust
let mut buffer: *mut c_void = std::ptr::null_mut();
GetTokenInformation(token, TokenIntegrityLevel, Some(&mut buffer), size, &mut returned).ok();
```

## The registry

`RegOpenKeyExW` has the shape:

```rust
pub unsafe fn RegOpenKeyExW<P1>(
    hkey: HKEY,
    lpsubkey: P1,
    uloptions: Option<u32>,
    samdesired: REG_SAM_FLAGS,
    phkresult: *mut HKEY,
) -> WIN32_ERROR
```

Three things to get right:

- `uloptions` is an `Option<u32>`, so pass `None`.
- `phkresult` is a **raw pointer**, not a reference. Pass
  `std::ptr::addr_of_mut!(handle)`.
- The hive and the subkey are separate arguments. Do not build
  `"HKEY_LOCAL_MACHINE\SOFTWARE\..."` and pass it as the subkey; pass
  `HKEY_LOCAL_MACHINE` and `"SOFTWARE\..."`.

`RRF_RT_REGISTRY` is not in this crate's `Registry` module, and is not
needed: `RegOpenKeyExW` is the right call.

## Raw pointers are not `Send`

`HANDLE` is `HANDLE(pub *mut c_void)`, and `*mut c_void` is `!Send`. So a
kernel handle cannot be moved into a thread.

The fix is a newtype with an explicit `Send`:

```rust
#[derive(Debug, Clone, Copy)]
struct SendableHandle(HANDLE);

unsafe impl Send for SendableHandle {}
unsafe impl Sync for SendableHandle {}
```

## Closure capture is precise — and that bites

Rust 2021 captures the **fields** a closure uses, not the whole variable. So:

```rust
let waiting = SendableHandle(handle);
std::thread::spawn(move || {
    let result = unsafe { WaitForSingleObject(waiting.0, WATCH_INTERVAL_MILLIS) };
});
```

captures `waiting.0`, which is the raw `HANDLE`, not the `SendableHandle`. The
`Send` impl on the newtype is therefore **not** used, and the closure is still
`!Send`.

The fix is to bind the whole value inside the closure:

```rust
std::thread::spawn(move || {
    let waiting = waiting;
    // now `waiting` is the whole SendableHandle
});
```

## `unsafe` blocks inside `unsafe` blocks

In edition 2021 an inner `unsafe` block inside an outer one triggers
`unused_unsafe`. Write it as:

```rust
let result = unsafe {
    OpenEventW(...)
        .map(|handle| {
            let _ = CloseHandle(handle);   // already in an unsafe block
            true
        })
        .unwrap_or(false)
};
```

## Named events

The quit signal, which the installer uses to close a running instance:

```rust
CreateEventW(None, true, false, PCWSTR(name.as_ptr()))   // manual reset, initially clear
OpenEventW(EVENT_MODIFY_STATE, false, PCWSTR(name.as_ptr()))
SetEvent(event)
WaitForSingleObject(handle, millis)
```

`OpenEventW` returns `Err` when the event does not exist, which is exactly
the "nobody is running" case.

## See also

- [`docs/DECISIONS.md`](../DECISIONS.md)
- [`crates/tc-platform-win/`](../crates/tc-platform-win/) — the code that uses
  all of this
