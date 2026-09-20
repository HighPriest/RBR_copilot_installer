//! WinHTTP wrapper — the ONLY http/tls code allowed in the installer.
//!
//! TLS comes from the OS (Schannel), which is what keeps reqwest/hyper/rustls
//! and their tens of kB out of the binary (installer/CLAUDE.md §1).
//!
//! Synchronous, and it blocks for as long as the network takes: call it from
//! the worker thread (task I4), never from the UI thread. Task I3.

#![allow(dead_code)]

use windows_sys::Win32::Networking::WinHttp as wh;

/// What the caller needs to react to a failed request.
pub struct HttpError {
    /// WinHTTP / GetLastError code. 0 for errors of our own making, such as a
    /// non-2xx response or a cancelled transfer, which have no OS code.
    pub code: u32,
    /// Already-readable text, safe to show in a MessageBox or the log pane.
    pub message: String,
    /// Set when a response was actually received. 401 and 403 drive the
    /// access-key UX in task I8, so the caller must be able to tell them from
    /// "the server was unreachable".
    pub status: Option<u16>,
}

impl HttpError {
    /// A WinHTTP call failed: capture GetLastError and render it now, while
    /// the thread-local error is still ours.
    fn last(context: &str) -> HttpError {
        let code = unsafe { windows_sys::Win32::Foundation::GetLastError() };
        HttpError {
            code,
            message: format!("{context}: {} (0x{code:08X})", error_text(code)),
            status: None,
        }
    }

    fn logical(message: impl Into<String>, status: Option<u16>) -> HttpError {
        HttpError {
            code: 0,
            message: message.into(),
            status,
        }
    }

    /// True when the transfer stopped because the caller asked it to, which is
    /// not something to report as a failure.
    pub fn is_cancelled(&self) -> bool {
        self.code == 0 && self.status.is_none() && self.message == CANCELLED
    }
}

const CANCELLED: &str = "cancelled";

/// `progress(bytes_so_far, content_length)`. `content_length` is None when the
/// server did not send one, so a caller driving a progress bar must cope with
/// an unknown total.
pub type ProgressFn<'a> = &'a mut dyn FnMut(u64, Option<u64>);

/// Timeouts in milliseconds: resolve, connect, send, receive. Without these a
/// dead server leaves the worker thread parked forever and the UI's Cancel
/// button does nothing, since cancellation is only checked between chunks.
const TIMEOUT_RESOLVE_MS: i32 = 10_000;
const TIMEOUT_CONNECT_MS: i32 = 10_000;
const TIMEOUT_SEND_MS: i32 = 30_000;
const TIMEOUT_RECEIVE_MS: i32 = 30_000;

/// Read granularity. Big enough that syscall overhead is irrelevant, small
/// enough that Cancel is noticed promptly and a stalled transfer is not
/// holding a large buffer.
const CHUNK: usize = 64 * 1024;

/// Closes its WinHTTP handle on drop, so every `?`/early return unwinds the
/// session/connection/request chain without a leak.
struct Handle(*mut core::ffi::c_void);

impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { wh::WinHttpCloseHandle(self.0) };
        }
    }
}

/// GET `url` into memory, optionally authenticated with a bearer token.
///
/// Fine for manifests. Package zips should use [`get_streaming`] instead: this
/// buffers the whole body, and a voice pack is tens of megabytes.
pub fn get(
    url: &str,
    bearer: Option<&str>,
    progress: ProgressFn,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<Vec<u8>, HttpError> {
    let mut body = Vec::new();
    get_streaming(
        url,
        bearer,
        &mut |chunk| {
            body.extend_from_slice(chunk);
            true
        },
        progress,
        cancel,
    )?;
    Ok(body)
}

/// GET `url`, handing each chunk to `sink` as it arrives.
///
/// `sink` returns false to abort (task I5 uses that when a write to the cache
/// file fails). The body is never fully buffered, so this is what downloads
/// should use.
pub fn get_streaming(
    url: &str,
    bearer: Option<&str>,
    sink: &mut dyn FnMut(&[u8]) -> bool,
    progress: ProgressFn,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<(), HttpError> {
    let target = Target::parse(url)?;

    let session = Handle(unsafe {
        wh::WinHttpOpen(
            crate::i18n::to_wide(USER_AGENT).as_ptr(),
            wh::WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
            core::ptr::null(),
            core::ptr::null(),
            0,
        )
    });
    if session.0.is_null() {
        return Err(HttpError::last("WinHttpOpen"));
    }
    unsafe {
        wh::WinHttpSetTimeouts(
            session.0,
            TIMEOUT_RESOLVE_MS,
            TIMEOUT_CONNECT_MS,
            TIMEOUT_SEND_MS,
            TIMEOUT_RECEIVE_MS,
        )
    };

    let host = crate::i18n::to_wide(&target.host);
    let connect = Handle(unsafe { wh::WinHttpConnect(session.0, host.as_ptr(), target.port, 0) });
    if connect.0.is_null() {
        return Err(HttpError::last("WinHttpConnect"));
    }

    let verb = crate::i18n::to_wide("GET");
    let path = crate::i18n::to_wide(&target.path);
    let request = Handle(unsafe {
        wh::WinHttpOpenRequest(
            connect.0,
            verb.as_ptr(),
            path.as_ptr(),
            core::ptr::null(),
            core::ptr::null(),
            core::ptr::null(),
            if target.secure {
                wh::WINHTTP_FLAG_SECURE
            } else {
                0
            },
        )
    });
    if request.0.is_null() {
        return Err(HttpError::last("WinHttpOpenRequest"));
    }

    if let Some(token) = bearer {
        // Never logged: server/CLAUDE.md forbids logging Authorization, and
        // the same rule applies on this side.
        let header = crate::i18n::to_wide(&format!("Authorization: Bearer {token}"));
        let added = unsafe {
            wh::WinHttpAddRequestHeaders(
                request.0,
                header.as_ptr(),
                COUNT_THE_STRING,
                wh::WINHTTP_ADDREQ_FLAG_ADD,
            )
        };
        if added == 0 {
            return Err(HttpError::last("WinHttpAddRequestHeaders"));
        }
    }

    let sent = unsafe {
        wh::WinHttpSendRequest(request.0, core::ptr::null(), 0, core::ptr::null(), 0, 0, 0)
    };
    if sent == 0 {
        return Err(HttpError::last("WinHttpSendRequest"));
    }
    if unsafe { wh::WinHttpReceiveResponse(request.0, core::ptr::null_mut()) } == 0 {
        return Err(HttpError::last("WinHttpReceiveResponse"));
    }

    let status = query_number(&request, wh::WINHTTP_QUERY_STATUS_CODE)? as u16;
    // Absent Content-Length is normal (chunked encoding), not an error.
    let total = query_number(&request, wh::WINHTTP_QUERY_CONTENT_LENGTH)
        .ok()
        .map(u64::from);

    if !(200..300).contains(&status) {
        return Err(HttpError::logical(
            format!("HTTP {status} for {url}"),
            Some(status),
        ));
    }

    let mut read_total: u64 = 0;
    let mut chunk = [0u8; CHUNK];
    loop {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(HttpError::logical(CANCELLED, None));
        }
        let mut read: u32 = 0;
        let ok = unsafe {
            wh::WinHttpReadData(
                request.0,
                chunk.as_mut_ptr() as *mut core::ffi::c_void,
                CHUNK as u32,
                &mut read,
            )
        };
        if ok == 0 {
            return Err(HttpError::last("WinHttpReadData"));
        }
        if read == 0 {
            break; // End of response body.
        }
        if !sink(&chunk[..read as usize]) {
            return Err(HttpError::logical("aborted by caller", None));
        }
        read_total += u64::from(read);
        progress(read_total, total);
    }

    // A truncated response that still ends cleanly would otherwise look like a
    // success and get written into the cache as a valid package.
    if let Some(expected) = total {
        if read_total != expected {
            return Err(HttpError::logical(
                format!("truncated response: expected {expected} bytes, read {read_total}"),
                Some(status),
            ));
        }
    }
    Ok(())
}

const USER_AGENT: &str = "Pacenotes-Installer/0.1";
/// `dwHeadersLength = -1` tells WinHTTP the header string is NUL-terminated.
const COUNT_THE_STRING: u32 = u32::MAX;

/// Where a URL points, in the pieces WinHttpConnect/OpenRequest want.
struct Target {
    host: String,
    port: u16,
    /// Path plus query, which WinHTTP treats as separate components.
    path: String,
    secure: bool,
}

impl Target {
    /// Split with WinHttpCrackUrl rather than by hand, so odd-but-legal URLs
    /// are divided the way the rest of the OS stack expects.
    fn parse(url: &str) -> Result<Target, HttpError> {
        let wide = crate::i18n::to_wide(url);
        let mut comp: wh::URL_COMPONENTS = unsafe { core::mem::zeroed() };
        comp.dwStructSize = core::mem::size_of::<wh::URL_COMPONENTS>() as u32;
        // Null pointer + non-zero length asks WinHttpCrackUrl to return
        // pointers INTO `wide` with lengths, instead of copying into buffers
        // we would have to size in advance.
        comp.dwSchemeLength = u32::MAX;
        comp.dwHostNameLength = u32::MAX;
        comp.dwUrlPathLength = u32::MAX;
        comp.dwExtraInfoLength = u32::MAX;

        let ok = unsafe {
            wh::WinHttpCrackUrl(
                wide.as_ptr(),
                (wide.len() - 1) as u32, // without the NUL
                0,
                &mut comp,
            )
        };
        if ok == 0 {
            return Err(HttpError::last(&format!("cannot parse URL {url}")));
        }

        let mut path = slice_at(comp.lpszUrlPath, comp.dwUrlPathLength);
        path.push_str(&slice_at(comp.lpszExtraInfo, comp.dwExtraInfoLength));
        if path.is_empty() {
            path.push('/');
        }
        Ok(Target {
            host: slice_at(comp.lpszHostName, comp.dwHostNameLength),
            // WinHttpCrackUrl fills in the scheme's default when the URL omits
            // a port, so this is never 0 for http/https.
            port: comp.nPort,
            path,
            secure: comp.nScheme == wh::WINHTTP_INTERNET_SCHEME_HTTPS,
        })
    }
}

fn slice_at(ptr: *const u16, len: u32) -> String {
    if ptr.is_null() || len == 0 {
        return String::new();
    }
    unsafe { String::from_utf16_lossy(core::slice::from_raw_parts(ptr, len as usize)) }
}

/// Read a numeric response header (status code, content length).
fn query_number(request: &Handle, info: u32) -> Result<u32, HttpError> {
    let mut value: u32 = 0;
    let mut len = core::mem::size_of::<u32>() as u32;
    let ok = unsafe {
        wh::WinHttpQueryHeaders(
            request.0,
            info | wh::WINHTTP_QUERY_FLAG_NUMBER,
            core::ptr::null(),
            &mut value as *mut u32 as *mut core::ffi::c_void,
            &mut len,
            core::ptr::null_mut(),
        )
    };
    if ok == 0 {
        return Err(HttpError::last("WinHttpQueryHeaders"));
    }
    Ok(value)
}

/// Turn an error code into something a user can act on.
///
/// WinHTTP's own codes (12001+) are not in the system message table — their
/// strings live in winhttp.dll — so the module has to be named explicitly or
/// every network failure shows up as a bare hex number.
fn error_text(code: u32) -> String {
    let module = unsafe {
        windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(
            crate::i18n::to_wide("winhttp.dll").as_ptr(),
        )
    };
    let mut buf = [0u16; 512];
    let len = unsafe {
        windows_sys::Win32::System::Diagnostics::Debug::FormatMessageW(
            windows_sys::Win32::System::Diagnostics::Debug::FORMAT_MESSAGE_FROM_SYSTEM
                | windows_sys::Win32::System::Diagnostics::Debug::FORMAT_MESSAGE_FROM_HMODULE
                | windows_sys::Win32::System::Diagnostics::Debug::FORMAT_MESSAGE_IGNORE_INSERTS,
            module as *const core::ffi::c_void,
            code,
            0,
            buf.as_mut_ptr(),
            buf.len() as u32,
            core::ptr::null(),
        )
    };
    if len == 0 {
        return "unknown error".into();
    }
    String::from_utf16_lossy(&buf[..len as usize])
        .trim_end_matches(['\r', '\n', ' '])
        .to_string()
}
