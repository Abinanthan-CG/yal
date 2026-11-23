//! Minimal pure-Win32 HTTPS GET client backed by `WinHttp`.
//!
//! Zero external HTTP/TLS crates — uses Windows' built-in Schannel TLS stack
//! and system proxy settings, with strict timeouts and a hard response-size cap.

use std::ffi::c_void;

use windows::core::PCWSTR;
use windows::Win32::Networking::WinHttp::{
    WinHttpCloseHandle, WinHttpConnect, WinHttpCrackUrl, WinHttpOpen, WinHttpOpenRequest,
    WinHttpQueryDataAvailable, WinHttpQueryHeaders, WinHttpReadData, WinHttpReceiveResponse,
    WinHttpSendRequest, WinHttpSetTimeouts, URL_COMPONENTS, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
    WINHTTP_FLAG_SECURE, WINHTTP_INTERNET_SCHEME, WINHTTP_QUERY_FLAG_NUMBER,
    WINHTTP_QUERY_STATUS_CODE,
};

/// Maximum response body size (2 MiB — plenty for exchange rate JSON feeds).
const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;

/// RAII guard for a `WinHttp` `HINTERNET` handle (`*mut c_void`).
struct WinHttpHandle(*mut c_void);

impl Drop for WinHttpHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                let _ = WinHttpCloseHandle(self.0);
            }
        }
    }
}

/// Perform a blocking HTTPS/HTTP GET request and return the UTF-8 response body.
pub fn https_get(url: &str, timeout_ms: i32) -> Result<String, String> {
    let wide_url: Vec<u16> = url.encode_utf16().chain(std::iter::once(0)).collect();

    let mut host_buf = vec![0u16; 256];
    let mut path_buf = vec![0u16; 2048];
    let mut comps = URL_COMPONENTS {
        dwStructSize: std::mem::size_of::<URL_COMPONENTS>() as u32,
        lpszHostName: windows::core::PWSTR(host_buf.as_mut_ptr()),
        dwHostNameLength: host_buf.len() as u32,
        lpszUrlPath: windows::core::PWSTR(path_buf.as_mut_ptr()),
        dwUrlPathLength: path_buf.len() as u32,
        ..Default::default()
    };

    unsafe {
        WinHttpCrackUrl(&wide_url[..wide_url.len() - 1], 0, &mut comps)
            .map_err(|e| format!("invalid URL: {e}"))?;
    }

    let is_https = comps.nScheme == WINHTTP_INTERNET_SCHEME(2);
    let port = comps.nPort;
    let host_len = comps.dwHostNameLength as usize;
    let path_len = comps.dwUrlPathLength as usize;

    let mut host: Vec<u16> = host_buf[..host_len].to_vec();
    host.push(0);

    let mut path: Vec<u16> = if path_len > 0 {
        path_buf[..path_len].to_vec()
    } else {
        vec![b'/' as u16]
    };
    path.push(0);

    let agent: Vec<u16> = "YAL-Launcher/1.0\0".encode_utf16().collect();
    let session = unsafe {
        WinHttpOpen(
            PCWSTR(agent.as_ptr()),
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
            PCWSTR::null(),
            PCWSTR::null(),
            0,
        )
    };
    if session.is_null() {
        return Err("WinHttpOpen failed".to_string());
    }
    let session = WinHttpHandle(session);

    let timeout = timeout_ms.max(1000);
    unsafe {
        let _ = WinHttpSetTimeouts(session.0, timeout, timeout, timeout, timeout);
    }

    let connect = unsafe { WinHttpConnect(session.0, PCWSTR(host.as_ptr()), port, 0) };
    if connect.is_null() {
        return Err("WinHttpConnect failed".to_string());
    }
    let connect = WinHttpHandle(connect);

    let verb: Vec<u16> = "GET\0".encode_utf16().collect();
    let flags = if is_https {
        WINHTTP_FLAG_SECURE
    } else {
        Default::default()
    };
    let request = unsafe {
        WinHttpOpenRequest(
            connect.0,
            PCWSTR(verb.as_ptr()),
            PCWSTR(path.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            std::ptr::null(),
            flags,
        )
    };
    if request.is_null() {
        return Err("WinHttpOpenRequest failed".to_string());
    }
    let request = WinHttpHandle(request);

    unsafe {
        WinHttpSendRequest(request.0, None, None, 0, 0, 0)
            .map_err(|e| format!("WinHttpSendRequest failed: {e}"))?;
        WinHttpReceiveResponse(request.0, std::ptr::null_mut())
            .map_err(|e| format!("WinHttpReceiveResponse failed: {e}"))?;
    }

    let mut status_code: u32 = 0;
    let mut status_len: u32 = std::mem::size_of::<u32>() as u32;
    unsafe {
        WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            PCWSTR::null(),
            Some(&mut status_code as *mut u32 as *mut c_void),
            &mut status_len,
            std::ptr::null_mut(),
        )
        .map_err(|e| format!("WinHttpQueryHeaders failed: {e}"))?;
    }
    if !(200..300).contains(&status_code) {
        return Err(format!("HTTP status {status_code}"));
    }

    let mut body = Vec::new();
    loop {
        let mut available: u32 = 0;
        unsafe {
            WinHttpQueryDataAvailable(request.0, &mut available)
                .map_err(|e| format!("WinHttpQueryDataAvailable failed: {e}"))?;
        }
        if available == 0 {
            break;
        }
        let chunk_len = (available as usize).min(64 * 1024);
        if body.len().saturating_add(chunk_len) > MAX_RESPONSE_BYTES {
            return Err("HTTP response exceeded size limit".to_string());
        }
        let start = body.len();
        body.resize(start + chunk_len, 0);
        let mut read: u32 = 0;
        unsafe {
            WinHttpReadData(
                request.0,
                body[start..].as_mut_ptr() as *mut c_void,
                chunk_len as u32,
                &mut read,
            )
            .map_err(|e| format!("WinHttpReadData failed: {e}"))?;
        }
        body.truncate(start + read as usize);
        if read == 0 {
            break;
        }
    }

    String::from_utf8(body).map_err(|e| format!("response was not valid UTF-8: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_malformed_urls_without_panicking() {
        assert!(https_get("not a url", 2000).is_err());
        assert!(https_get("", 2000).is_err());
    }
}
