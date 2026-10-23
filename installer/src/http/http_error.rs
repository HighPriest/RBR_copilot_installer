const CANCELLED: &str = "cancelled";

pub struct HttpError {
    /// The WinHTTP or `GetLastError` code; 0 for failures with no OS code.
    pub code: u32,
    /// Readable as-is in a MessageBox or the log.
    pub message: String,
    /// Set when a response arrived, so 401/403 can be told from "unreachable".
    pub status: Option<u16>,
}

impl HttpError {
    /// Renders `GetLastError` immediately, while the thread's error is still ours.
    pub(super) fn last(context: &str) -> HttpError {
        let code = unsafe { windows_sys::Win32::Foundation::GetLastError() };
        HttpError {
            code,
            message: format!("{context}: {} (0x{code:08X})", error_text(code)),
            status: None,
        }
    }

    /// A reachable server sent a body that could not be understood, which a retry will not fix.
    pub fn parse(message: impl Into<String>) -> HttpError {
        HttpError::logical(message, None)
    }

    pub(super) fn logical(message: impl Into<String>, status: Option<u16>) -> HttpError {
        HttpError {
            code: 0,
            message: message.into(),
            status,
        }
    }

    pub(super) fn cancelled() -> HttpError {
        HttpError::logical(CANCELLED, None)
    }

    /// The caller asked to stop, which is not a failure to report.
    pub fn is_cancelled(&self) -> bool {
        self.code == 0 && self.status.is_none() && self.message == CANCELLED
    }
}

/// WinHTTP's own codes (12001+) live in winhttp.dll's message table, not the
/// system's, so it must be named or every network failure is a bare number.
fn error_text(code: u32) -> String {
    let module = unsafe {
        windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(
            crate::i18n::to_wide("winhttp.dll").as_ptr(),
        )
    };
    let mut buffer = [0u16; 512];
    let length = unsafe {
        windows_sys::Win32::System::Diagnostics::Debug::FormatMessageW(
            windows_sys::Win32::System::Diagnostics::Debug::FORMAT_MESSAGE_FROM_SYSTEM
                | windows_sys::Win32::System::Diagnostics::Debug::FORMAT_MESSAGE_FROM_HMODULE
                | windows_sys::Win32::System::Diagnostics::Debug::FORMAT_MESSAGE_IGNORE_INSERTS,
            module as *const core::ffi::c_void,
            code,
            0,
            buffer.as_mut_ptr(),
            buffer.len() as u32,
            core::ptr::null(),
        )
    };
    if length == 0 {
        return "unknown error".into();
    }
    String::from_utf16_lossy(&buffer[..length as usize])
        .trim_end_matches(['\r', '\n', ' '])
        .to_string()
}
