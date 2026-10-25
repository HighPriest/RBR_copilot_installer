//! The installer's only HTTP client: WinHTTP, whose TLS is the OS's own Schannel.
//! Blocking, so it runs on the worker thread, never the UI thread.

mod http_error;
mod request_handle;
mod url_target;

pub use http_error::HttpError;

/// `(bytes_so_far, content_length)`; the length is `None` for a chunked response.
pub type ProgressFn<'a> = &'a mut dyn FnMut(u64, Option<u64>);

/// Without timeouts a dead server parks the worker forever, since Cancel is
/// only noticed between chunks.
const TIMEOUT_RESOLVE_MS: i32 = 10_000;
const TIMEOUT_CONNECT_MS: i32 = 10_000;
const TIMEOUT_SEND_MS: i32 = 30_000;
const TIMEOUT_RECEIVE_MS: i32 = 30_000;

/// Small enough that Cancel is noticed promptly.
const CHUNK_SIZE: usize = 64 * 1024;

const USER_AGENT: &str = "Pacenotes-Installer/0.1";

/// The whole body in memory: right for a manifest, wrong for a package zip.
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

/// Hands each chunk to `sink` as it arrives; `sink` returns false to abort.
/// The bearer token is a credential and is never logged.
pub fn get_streaming(
    url: &str,
    bearer: Option<&str>,
    sink: &mut dyn FnMut(&[u8]) -> bool,
    progress: ProgressFn,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<(), HttpError> {
    send("GET", url, bearer, None, sink, progress, cancel)
}

/// Posts JSON without a token, since what the installer sends carries no
/// identity, and discards the response body.
pub fn post_json(
    url: &str,
    body: &[u8],
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<(), HttpError> {
    let mut ignore_progress = |_: u64, _: Option<u64>| {};
    send(
        "POST",
        url,
        None,
        Some(body),
        &mut |_| true,
        &mut ignore_progress,
        cancel,
    )
}

fn send(
    method: &str,
    url: &str,
    bearer: Option<&str>,
    body: Option<&[u8]>,
    sink: &mut dyn FnMut(&[u8]) -> bool,
    progress: ProgressFn,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<(), HttpError> {
    let target = url_target::UrlTarget::parse(url)?;
    let open = request_handle::OpenRequest::send(&target, method, bearer, body)?;

    let status = open
        .query_number(windows_sys::Win32::Networking::WinHttp::WINHTTP_QUERY_STATUS_CODE)?
        as u16;
    if !(200..300).contains(&status) {
        return Err(HttpError::logical(
            format!("HTTP {status} for {url}"),
            Some(status),
        ));
    }
    let content_length = open
        .query_number(windows_sys::Win32::Networking::WinHttp::WINHTTP_QUERY_CONTENT_LENGTH)
        .ok()
        .map(u64::from);

    let bytes_read = read_body(&open, sink, progress, content_length, cancel)?;
    if let Some(expected) = content_length {
        if bytes_read != expected {
            return Err(HttpError::logical(
                format!("truncated response: expected {expected} bytes, read {bytes_read}"),
                Some(status),
            ));
        }
    }
    Ok(())
}

fn read_body(
    open: &request_handle::OpenRequest,
    sink: &mut dyn FnMut(&[u8]) -> bool,
    progress: ProgressFn,
    content_length: Option<u64>,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<u64, HttpError> {
    let mut bytes_read: u64 = 0;
    let mut chunk = [0u8; CHUNK_SIZE];
    loop {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(HttpError::cancelled());
        }
        let read = open.read(&mut chunk)?;
        if read == 0 {
            return Ok(bytes_read);
        }
        if !sink(&chunk[..read]) {
            return Err(HttpError::logical("aborted by caller", None));
        }
        bytes_read += read as u64;
        progress(bytes_read, content_length);
    }
}
