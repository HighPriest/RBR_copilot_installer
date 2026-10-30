/// A URL in the pieces `WinHttpConnect` and `WinHttpOpenRequest` take.
pub struct UrlTarget {
    pub host: String,
    pub port: u16,
    /// Path and query together.
    pub path: String,
    pub secure: bool,
}

impl UrlTarget {
    /// Split by `WinHttpCrackUrl`, so odd-but-legal URLs divide as the OS stack expects.
    pub fn parse(url: &str) -> Result<UrlTarget, super::HttpError> {
        let wide = crate::i18n::to_wide(url);
        let mut components: windows_sys::Win32::Networking::WinHttp::URL_COMPONENTS =
            unsafe { core::mem::zeroed() };
        components.dwStructSize =
            core::mem::size_of::<windows_sys::Win32::Networking::WinHttp::URL_COMPONENTS>() as u32;
        // A null pointer with a non-zero length asks for pointers into `wide` instead of copies.
        components.dwSchemeLength = u32::MAX;
        components.dwHostNameLength = u32::MAX;
        components.dwUrlPathLength = u32::MAX;
        components.dwExtraInfoLength = u32::MAX;

        let length_without_nul = (wide.len() - 1) as u32;
        let cracked = unsafe {
            windows_sys::Win32::Networking::WinHttp::WinHttpCrackUrl(
                wide.as_ptr(),
                length_without_nul,
                0,
                &mut components,
            )
        };
        if cracked == 0 {
            return Err(super::HttpError::last(&format!("cannot parse URL {url}")));
        }

        let mut path = wide_slice(components.lpszUrlPath, components.dwUrlPathLength);
        path.push_str(&wide_slice(
            components.lpszExtraInfo,
            components.dwExtraInfoLength,
        ));
        if path.is_empty() {
            path.push('/');
        }
        Ok(UrlTarget {
            host: wide_slice(components.lpszHostName, components.dwHostNameLength),
            port: components.nPort,
            path,
            secure: components.nScheme
                == windows_sys::Win32::Networking::WinHttp::WINHTTP_INTERNET_SCHEME_HTTPS,
        })
    }
}

fn wide_slice(start: *const u16, length: u32) -> String {
    if start.is_null() || length == 0 {
        return String::new();
    }
    unsafe { String::from_utf16_lossy(core::slice::from_raw_parts(start, length as usize)) }
}
