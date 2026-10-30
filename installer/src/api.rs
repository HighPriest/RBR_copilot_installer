//! The server's endpoints: URLs in, parsed bodies out. The transport is `http`
//! and the thread is `worker`.

pub struct Api {
    /// Without a trailing slash.
    pub base_url: String,
    /// The access key from scene 2; `None` is anonymous.
    pub token: Option<String>,
}

impl Api {
    pub fn new(base_url: &str, token: Option<String>) -> Api {
        Api {
            base_url: base_url.trim_end_matches('/').to_string(),
            token,
        }
    }

    /// `GET /packages?game=<id>`: one game's catalogue, filtered server-side.
    pub fn fetch_manifest(
        &self,
        game: &str,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<pacenotes_shared::Manifest, crate::http::HttpError> {
        let url = format!("{}/packages?game={}", self.base_url, encode_segment(game));
        let body = crate::http::get(&url, self.token.as_deref(), &mut ignore_progress, cancel)?;
        serde_json::from_slice(&body).map_err(|e| {
            crate::http::HttpError::parse(format!("the server's package list was unreadable: {e}"))
        })
    }

    /// `GET /motd?lang=xx`; `None` when there is no text, which is normal.
    pub fn fetch_motd(&self, lang: &str, cancel: &std::sync::atomic::AtomicBool) -> Option<String> {
        let url = format!("{}/motd?lang={}", self.base_url, encode_segment(lang));
        crate::http::get(&url, None, &mut ignore_progress, cancel)
            .ok()
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .filter(|text| !text.trim().is_empty())
    }

    pub fn health(&self, cancel: &std::sync::atomic::AtomicBool) -> bool {
        let url = format!("{}/health", self.base_url);
        crate::http::get(&url, None, &mut ignore_progress, cancel).is_ok()
    }
}

fn ignore_progress(_bytes_so_far: u64, _content_length: Option<u64>) {}

/// Escapes everything outside RFC 3986's unreserved set, `/` included.
pub fn encode_segment(segment: &str) -> String {
    let mut encoded = String::with_capacity(segment.len());
    for byte in segment.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char)
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}
