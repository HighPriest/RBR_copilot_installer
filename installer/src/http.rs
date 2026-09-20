//! WinHTTP wrapper — the ONLY http/tls code allowed in the installer.
//! Synchronous; must be called from a worker thread, never the UI thread.
//! Task I3.

#![allow(dead_code)]

pub struct HttpError {
    /// GetLastError()/WinHTTP error code, 0 for logical errors (e.g. non-2xx).
    pub code: u32,
    pub message: String,
    /// HTTP status if a response was received (401/403 drive auth UX).
    pub status: Option<u16>,
}

pub type ProgressFn<'a> = &'a mut dyn FnMut(u64, Option<u64>);

/// GET `url`, optionally with `Authorization: Bearer <token>`.
/// Streams the body, invoking `progress(bytes_read, content_length)`.
/// `cancel` is checked between chunks (task I4 cancel support).
///
/// Implementation outline (I3):
///   WinHttpOpen (UA "Pacenotes-Installer/0.1", automatic proxy) →
///   WinHttpCrackUrl → WinHttpConnect → WinHttpOpenRequest
///   (WINHTTP_FLAG_SECURE iff https) → add auth header →
///   WinHttpSendRequest / WinHttpReceiveResponse →
///   WinHttpQueryHeaders (status, Content-Length) →
///   loop WinHttpReadData into 64 KiB chunks.
/// All handles closed via a small RAII guard (WinHttpCloseHandle).
pub fn get(
    _url: &str,
    _bearer: Option<&str>,
    _progress: ProgressFn,
    _cancel: &std::sync::atomic::AtomicBool,
) -> Result<Vec<u8>, HttpError> {
    // TODO(I3)
    Err(HttpError {
        code: 0,
        message: "not implemented".into(),
        status: None,
    })
}
