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
        let url = format!(
            "{}/packages?game={}",
            self.base_url,
            super::url_encoding::encode_segment(game)
        );
        let body = crate::http::get(&url, self.token.as_deref(), &mut ignore_progress, cancel)?;
        serde_json::from_slice(&body).map_err(|e| {
            crate::http::HttpError::parse(format!("the server's package list was unreadable: {e}"))
        })
    }

    /// `GET /motd?lang=xx`; `None` when there is no text, which is normal.
    pub fn fetch_motd(&self, lang: &str, cancel: &std::sync::atomic::AtomicBool) -> Option<String> {
        let url = format!(
            "{}/motd?lang={}",
            self.base_url,
            super::url_encoding::encode_segment(lang)
        );
        crate::http::get(&url, None, &mut ignore_progress, cancel)
            .ok()
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .filter(|text| !text.trim().is_empty())
    }

    pub fn health(&self, cancel: &std::sync::atomic::AtomicBool) -> bool {
        let url = format!("{}/health", self.base_url);
        crate::http::get(&url, None, &mut ignore_progress, cancel).is_ok()
    }

    /// `GET /whoami`, the only way to validate a key, since `/packages` answers
    /// a key it does not accept with the public listing. A 401 covers wrong,
    /// unknown, expired and revoked alike; a 200 we cannot read is not a
    /// rejection, or a protocol change would lock a user out.
    pub fn whoami(&self, cancel: &std::sync::atomic::AtomicBool) -> crate::worker::SecretOutcome {
        let url = format!("{}/whoami", self.base_url);
        match crate::http::get(&url, self.token.as_deref(), &mut ignore_progress, cancel) {
            Ok(body) => match serde_json::from_slice::<pacenotes_shared::KeyStatus>(&body) {
                Ok(status) => crate::worker::SecretOutcome::Accepted {
                    expires_at: status.expires_at,
                },
                Err(_) => crate::worker::SecretOutcome::Unreachable,
            },
            Err(e) if e.status == Some(401) => crate::worker::SecretOutcome::Rejected,
            Err(_) => crate::worker::SecretOutcome::Unreachable,
        }
    }

    /// Built from `PackageInfo::download_path()`, whose leading game keeps two
    /// games' identical `directory/name` apart.
    pub fn download_url(&self, package: &pacenotes_shared::PackageInfo) -> String {
        format!(
            "{}/download/{}",
            self.base_url,
            super::url_encoding::encode_path(&package.download_path())
        )
    }
}

fn ignore_progress(_bytes_so_far: u64, _content_length: Option<u64>) {}
