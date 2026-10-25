/// `dwHeadersLength = -1`: the header string is NUL-terminated.
const NUL_TERMINATED: u32 = u32::MAX;

/// A WinHTTP handle, closed on drop so every early return unwinds the chain.
struct Handle(*mut core::ffi::c_void);

impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { windows_sys::Win32::Networking::WinHttp::WinHttpCloseHandle(self.0) };
        }
    }
}

impl Handle {
    fn checked(raw: *mut core::ffi::c_void, call: &str) -> Result<Handle, super::HttpError> {
        if raw.is_null() {
            Err(super::HttpError::last(call))
        } else {
            Ok(Handle(raw))
        }
    }
}

/// A sent request whose response headers have arrived; fields drop in
/// declaration order, request before connection before session.
pub struct OpenRequest {
    request: Handle,
    _connection: Handle,
    _session: Handle,
}

impl OpenRequest {
    /// `body`, when given, goes as `application/json`, the only type the installer posts.
    pub fn send(
        target: &super::url_target::UrlTarget,
        method: &str,
        bearer: Option<&str>,
        body: Option<&[u8]>,
    ) -> Result<OpenRequest, super::HttpError> {
        let user_agent = crate::i18n::to_wide(super::USER_AGENT);
        let session = Handle::checked(
            unsafe {
                windows_sys::Win32::Networking::WinHttp::WinHttpOpen(
                    user_agent.as_ptr(),
                    windows_sys::Win32::Networking::WinHttp::WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
                    core::ptr::null(),
                    core::ptr::null(),
                    0,
                )
            },
            "WinHttpOpen",
        )?;
        unsafe {
            windows_sys::Win32::Networking::WinHttp::WinHttpSetTimeouts(
                session.0,
                super::TIMEOUT_RESOLVE_MS,
                super::TIMEOUT_CONNECT_MS,
                super::TIMEOUT_SEND_MS,
                super::TIMEOUT_RECEIVE_MS,
            )
        };

        let host = crate::i18n::to_wide(&target.host);
        let connection = Handle::checked(
            unsafe {
                windows_sys::Win32::Networking::WinHttp::WinHttpConnect(
                    session.0,
                    host.as_ptr(),
                    target.port,
                    0,
                )
            },
            "WinHttpConnect",
        )?;

        let verb = crate::i18n::to_wide(method);
        let path = crate::i18n::to_wide(&target.path);
        let flags = if target.secure {
            windows_sys::Win32::Networking::WinHttp::WINHTTP_FLAG_SECURE
        } else {
            0
        };
        let request = Handle::checked(
            unsafe {
                windows_sys::Win32::Networking::WinHttp::WinHttpOpenRequest(
                    connection.0,
                    verb.as_ptr(),
                    path.as_ptr(),
                    core::ptr::null(),
                    core::ptr::null(),
                    core::ptr::null(),
                    flags,
                )
            },
            "WinHttpOpenRequest",
        )?;

        if let Some(token) = bearer {
            let line = format!(
                "{}: {}{token}",
                pacenotes_shared::AUTH_HEADER,
                pacenotes_shared::BEARER_PREFIX
            );
            add_header(&request, &line)?;
        }
        if body.is_some() {
            add_header(&request, "Content-Type: application/json")?;
        }
        let payload = body.unwrap_or(&[]);
        let payload_pointer = if body.is_some() {
            payload.as_ptr() as *const core::ffi::c_void
        } else {
            core::ptr::null()
        };
        let sent = unsafe {
            windows_sys::Win32::Networking::WinHttp::WinHttpSendRequest(
                request.0,
                core::ptr::null(),
                0,
                payload_pointer,
                payload.len() as u32,
                payload.len() as u32,
                0,
            )
        };
        if sent == 0 {
            return Err(super::HttpError::last("WinHttpSendRequest"));
        }
        let received = unsafe {
            windows_sys::Win32::Networking::WinHttp::WinHttpReceiveResponse(
                request.0,
                core::ptr::null_mut(),
            )
        };
        if received == 0 {
            return Err(super::HttpError::last("WinHttpReceiveResponse"));
        }

        Ok(OpenRequest {
            request,
            _connection: connection,
            _session: session,
        })
    }

    /// A numeric response header, such as the status code or content length.
    pub fn query_number(&self, info: u32) -> Result<u32, super::HttpError> {
        let mut value: u32 = 0;
        let mut length = core::mem::size_of::<u32>() as u32;
        let found = unsafe {
            windows_sys::Win32::Networking::WinHttp::WinHttpQueryHeaders(
                self.request.0,
                info | windows_sys::Win32::Networking::WinHttp::WINHTTP_QUERY_FLAG_NUMBER,
                core::ptr::null(),
                &mut value as *mut u32 as *mut core::ffi::c_void,
                &mut length,
                core::ptr::null_mut(),
            )
        };
        if found == 0 {
            return Err(super::HttpError::last("WinHttpQueryHeaders"));
        }
        Ok(value)
    }

    /// The next part of the body; 0 at its end.
    pub fn read(&self, buffer: &mut [u8]) -> Result<usize, super::HttpError> {
        let mut read: u32 = 0;
        let succeeded = unsafe {
            windows_sys::Win32::Networking::WinHttp::WinHttpReadData(
                self.request.0,
                buffer.as_mut_ptr() as *mut core::ffi::c_void,
                buffer.len() as u32,
                &mut read,
            )
        };
        if succeeded == 0 {
            return Err(super::HttpError::last("WinHttpReadData"));
        }
        Ok(read as usize)
    }
}

fn add_header(request: &Handle, line: &str) -> Result<(), super::HttpError> {
    let header = crate::i18n::to_wide(line);
    let added = unsafe {
        windows_sys::Win32::Networking::WinHttp::WinHttpAddRequestHeaders(
            request.0,
            header.as_ptr(),
            NUL_TERMINATED,
            windows_sys::Win32::Networking::WinHttp::WINHTTP_ADDREQ_FLAG_ADD,
        )
    };
    if added == 0 {
        return Err(super::HttpError::last("WinHttpAddRequestHeaders"));
    }
    Ok(())
}
