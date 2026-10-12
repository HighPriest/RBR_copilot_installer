//! Server API layer: manifest fetch and package download URLs.
//!
//! JSON usage is isolated HERE (with cache.rs and ledger.rs) so serde_json can
//! be swapped for a minimal parser if the size budget demands it
//! (installer/CLAUDE.md §5). Task I5.
//!
//! This module builds URLs and parses bodies; the transport is `http.rs` and
//! the thread is `worker.rs`.

#![allow(dead_code)]

pub struct Api {
    /// e.g. "https://packages.example.com", no trailing slash.
    pub base_url: String,
    /// Bearer access key from scene 2. None = anonymous.
    pub token: Option<String>,
}

impl Api {
    pub fn new(base_url: &str, token: Option<String>) -> Api {
        Api {
            base_url: base_url.trim_end_matches('/').to_string(),
            token,
        }
    }

    /// `GET /packages?game=<id>` → the manifest for ONE game.
    ///
    /// The filter is a server-side one on purpose: one instance serves every
    /// game, so without it the installer would pull every game's catalogue and
    /// throw most of it away.
    pub fn fetch_manifest(
        &self,
        game: &str,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<pacenotes_shared::Manifest, crate::http::HttpError> {
        let url = format!("{}/packages?game={}", self.base_url, encode_segment(game));
        let mut ignore = |_: u64, _: Option<u64>| {};
        let body = crate::http::get(&url, self.token.as_deref(), &mut ignore, cancel)?;
        serde_json::from_slice(&body).map_err(|e| {
            crate::http::HttpError::parse(format!("the server's package list was unreadable: {e}"))
        })
    }

    /// `GET /motd?lang=xx`. `None` when the server has nothing to say — a 404
    /// is the normal answer when no MOTD is published, and the caller shows
    /// its offline copy either way.
    pub fn fetch_motd(&self, lang: &str, cancel: &std::sync::atomic::AtomicBool) -> Option<String> {
        let url = format!("{}/motd?lang={}", self.base_url, encode_segment(lang));
        let mut ignore = |_: u64, _: Option<u64>| {};
        crate::http::get(&url, None, &mut ignore, cancel)
            .ok()
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .filter(|text| !text.trim().is_empty())
    }

    /// `GET /health`.
    pub fn health(&self, cancel: &std::sync::atomic::AtomicBool) -> bool {
        let mut ignore = |_: u64, _: Option<u64>| {};
        crate::http::get(
            &format!("{}/health", self.base_url),
            None,
            &mut ignore,
            cancel,
        )
        .is_ok()
    }

    /// Where to fetch a package from.
    ///
    /// The tail is `PackageInfo::download_path()` — `game/directory/name` —
    /// never assembled by hand: the route leads with the game because two
    /// games can hold the same directory+name, and getting that wrong would
    /// quietly fetch the other game's bytes.
    pub fn download_url(&self, package: &pacenotes_shared::PackageInfo) -> String {
        format!(
            "{}/download/{}",
            self.base_url,
            encode_path(&package.download_path())
        )
    }
}

/// Percent-encode a path, keeping `/` as the separator.
///
/// Real package names contain spaces (`style/Numeric Standard`), and a raw
/// space in a request line is not a valid URL — the server would see a
/// truncated path and answer 404.
pub fn encode_path(path: &str) -> String {
    path.split('/')
        .map(encode_segment)
        .collect::<Vec<_>>()
        .join("/")
}

/// Percent-encode one segment, including any `/` it contains.
///
/// Unreserved set from RFC 3986 plus `~`; everything else is escaped. Erring
/// toward escaping is safe here because the server percent-decodes before
/// looking the name up.
pub fn encode_segment(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for byte in segment.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}
