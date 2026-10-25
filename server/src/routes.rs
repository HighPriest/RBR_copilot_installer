//! The HTTP surface. Handlers stay thin: access rules live in `auth`, package
//! lookups in `store`.

pub struct AppState {
    pub store: crate::store::ManifestStore,
    pub db: libsql::Connection,
}

type SharedState = axum::extract::State<std::sync::Arc<AppState>>;

pub fn router(state: std::sync::Arc<AppState>) -> axum::Router {
    axum::Router::new()
        .route("/health", axum::routing::get(health))
        .route("/packages", axum::routing::get(list_packages))
        .route(
            "/download/{game}/{directory}/{*name}",
            axum::routing::get(download),
        )
        .route("/motd", axum::routing::get(motd))
        .route("/whoami", axum::routing::get(whoami))
        .with_state(state)
        .layer(axum::middleware::from_fn(crate::access_log::log_response))
        .layer(tower_http::trace::TraceLayer::new_for_http())
}

async fn health() -> &'static str {
    "ok"
}

#[derive(serde::Deserialize)]
pub struct PackagesQuery {
    /// Absent means every game in one document.
    game: Option<String>,
}

/// Never 4xx: an unusable credential silently gets the anonymous listing, so
/// probing cannot reveal directory names, and so does a failed credential
/// lookup, since the public catalogue is still right.
async fn list_packages(
    axum::extract::State(state): SharedState,
    axum::extract::Query(query): axum::extract::Query<PackagesQuery>,
    headers: axum::http::HeaderMap,
) -> Result<axum::response::Response, crate::error::AppError> {
    let grants = match caller(&state, &headers).await {
        Ok(Some(token)) if token.is_live(crate::auth::now()) => token.grants,
        Ok(_) => Vec::new(),
        Err(e) => {
            tracing::error!(error = %e, "credential lookup failed; serving the public listing");
            Vec::new()
        }
    };
    let mut manifest = state.store.current().visible_to(&grants);
    if let Some(game) = query.game.as_deref() {
        manifest = manifest.for_game(game);
    }
    // The listing differs per key, so no shared cache may hand one to another caller.
    Ok(axum::response::IntoResponse::into_response((
        [
            (axum::http::header::CACHE_CONTROL, "private, no-store"),
            (axum::http::header::VARY, "Authorization"),
        ],
        axum::Json(manifest),
    )))
}

/// The status never reveals what a protected directory holds: an impossible
/// key is 404, a protected directory without a live key is 401 whether or not
/// the package exists, and a missing grant answers the same 404 as a missing
/// package. Public packages never touch the auth database, so a database
/// outage cannot take the free catalogue down.
async fn download(
    axum::extract::State(state): SharedState,
    axum::extract::Path((game, directory, name)): axum::extract::Path<(String, String, String)>,
    headers: axum::http::HeaderMap,
) -> Result<axum::response::Response, crate::error::AppError> {
    if !pacenotes_shared::valid_id(&game)
        || !pacenotes_shared::valid_id(&directory)
        || !pacenotes_shared::valid_name(&name)
    {
        return Err(crate::error::AppError::not_found());
    }
    let found = state.store.zip_path(&game, &directory, &name);

    if directory != pacenotes_shared::PUBLIC_DIR {
        let token = caller(&state, &headers)
            .await
            .map_err(|e| crate::error::AppError::internal("loading credential", e))?;
        match crate::auth::decide(token.as_ref(), &directory, crate::auth::now()) {
            crate::auth::Decision::Allow => {}
            crate::auth::Decision::Unauthorized => {
                return Err(crate::error::AppError::unauthorized())
            }
            crate::auth::Decision::Hidden => return Err(crate::error::AppError::not_found()),
        }
    }
    let Some((package, path)) = found else {
        return Err(crate::error::AppError::not_found());
    };

    // Checked after authorization, so a protected validator is never confirmed to a stranger.
    let etag = format!("\"{}\"", package.sha256);
    let cache_control = if directory == pacenotes_shared::PUBLIC_DIR {
        // A republish changes a URL's bytes, so caches revalidate rather than trust an age.
        "no-cache"
    } else {
        // The key travels in a header, so a cache keyed on the URL would leak the zip.
        "private, no-store"
    };
    if if_none_match(&headers, &etag) {
        return Ok(axum::response::IntoResponse::into_response((
            axum::http::StatusCode::NOT_MODIFIED,
            [
                (axum::http::header::ETAG, etag),
                (axum::http::header::CACHE_CONTROL, cache_control.to_string()),
            ],
        )));
    }

    let file = match tokio::fs::File::open(&path).await {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            tracing::warn!(path = %path.display(), "manifest entry without a file on disk");
            return Err(crate::error::AppError::not_found());
        }
        Err(e) => return Err(crate::error::AppError::internal("opening package", e)),
    };
    let size = file
        .metadata()
        .await
        .map_err(|e| crate::error::AppError::internal("stat package", e))?
        .len();
    // Bytes of another size cannot match the client's sha, and would only read as corruption.
    if size != package.size {
        tracing::error!(
            path = %path.display(),
            on_disk = size,
            manifest = package.size,
            "package size differs from the manifest; refusing to serve it"
        );
        return Err(crate::error::AppError::unavailable());
    }

    let body = axum::body::Body::from_stream(tokio_util::io::ReaderStream::new(file));
    let headers = [
        (
            axum::http::header::CONTENT_TYPE,
            "application/zip".to_string(),
        ),
        (axum::http::header::CONTENT_LENGTH, size.to_string()),
        (
            axum::http::header::CONTENT_DISPOSITION,
            content_disposition(package.leaf()),
        ),
        (axum::http::header::ETAG, etag),
        (axum::http::header::CACHE_CONTROL, cache_control.to_string()),
    ];
    Ok(axum::response::IntoResponse::into_response((headers, body)))
}

/// Whether `If-None-Match` names this ETag, in a list, as a weak validator
/// (which matches for a GET) or as `*`.
fn if_none_match(headers: &axum::http::HeaderMap, etag: &str) -> bool {
    headers
        .get_all(axum::http::header::IF_NONE_MATCH)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .any(|candidate| {
            candidate == "*" || candidate.strip_prefix("W/").unwrap_or(candidate) == etag
        })
}

/// Names the LEAF, since a whole name contains `/`: an ASCII `filename` that
/// cannot close its quotes, and the exact name as RFC 6266 `filename*`.
fn content_disposition(leaf: &str) -> String {
    let ascii: String = leaf
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ' ') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let encoded: String = leaf
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
                (byte as char).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect();
    format!("attachment; filename=\"{ascii}.zip\"; filename*=UTF-8''{encoded}.zip")
}

#[derive(serde::Deserialize)]
pub struct MotdQuery {
    /// Absent means the fallback language.
    #[serde(default)]
    lang: String,
}

/// No auth; 404 when there is no text at all, which the installer treats as being offline.
async fn motd(
    axum::extract::State(state): SharedState,
    axum::extract::Query(query): axum::extract::Query<MotdQuery>,
) -> Result<axum::response::Response, crate::error::AppError> {
    let Some(text) = state.store.motd(&query.lang) else {
        return Err(crate::error::AppError::not_found());
    };
    Ok(axum::response::IntoResponse::into_response((
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; charset=utf-8",
        )],
        text,
    )))
}

/// `GET /whoami`: 200 with `shared::KeyStatus` for a live key, 401 for every
/// other, which is how a client tells a mistyped key from a good one.
async fn whoami(
    axum::extract::State(state): SharedState,
    headers: axum::http::HeaderMap,
) -> Result<axum::response::Response, crate::error::AppError> {
    let token = caller(&state, &headers)
        .await
        .map_err(|e| crate::error::AppError::internal("loading credential", e))?;
    match token {
        Some(token) if token.is_live(crate::auth::now()) => {
            Ok(axum::response::IntoResponse::into_response((
                [(axum::http::header::CACHE_CONTROL, "no-store")],
                axum::Json(pacenotes_shared::KeyStatus {
                    expires_at: token.expires_at,
                }),
            )))
        }
        _ => Err(crate::error::AppError::invalid_key()),
    }
}

/// The caller's credential: `None` for anonymous, malformed and unknown keys
/// alike, `Err` when the lookup itself failed, which each caller handles.
async fn caller(
    state: &AppState,
    headers: &axum::http::HeaderMap,
) -> anyhow::Result<Option<crate::auth::TokenRow>> {
    let Some(raw) = crate::auth::bearer_from_headers(headers) else {
        return Ok(None);
    };
    if !crate::auth::well_formed(&raw) {
        return Ok(None);
    }
    crate::auth::load_token(&state.db, &crate::auth::hash_token(&raw)).await
}

#[cfg(test)]
mod tests {
    #[test]
    fn content_disposition_names_the_leaf_safely() {
        assert_eq!(
            super::content_disposition("Kamil"),
            "attachment; filename=\"Kamil.zip\"; filename*=UTF-8''Kamil.zip"
        );
        assert_eq!(
            super::content_disposition("Numeric Standard"),
            "attachment; filename=\"Numeric Standard.zip\"; \
             filename*=UTF-8''Numeric%20Standard.zip"
        );
        assert_eq!(
            super::content_disposition("Wiśniewski"),
            "attachment; filename=\"Wi_niewski.zip\"; filename*=UTF-8''Wi%C5%9Bniewski.zip"
        );
    }

    #[test]
    fn a_quote_or_backslash_cannot_close_the_filename_early() {
        let hostile = super::content_disposition("a\"b\\c");
        assert!(
            hostile.starts_with("attachment; filename=\"a_b_c.zip\";"),
            "{hostile}"
        );
    }

    #[test]
    fn if_none_match_handles_lists_weak_tags_and_star() {
        let etag = "\"abc\"";
        let with = |value: &str| {
            let mut headers = axum::http::HeaderMap::new();
            headers.insert(
                axum::http::header::IF_NONE_MATCH,
                axum::http::HeaderValue::from_str(value).unwrap(),
            );
            headers
        };
        for matching in ["\"abc\"", "\"x\", \"abc\"", "W/\"abc\"", "*"] {
            assert!(super::if_none_match(&with(matching), etag), "{matching}");
        }
        assert!(!super::if_none_match(&with("\"abd\""), etag));
        assert!(!super::if_none_match(&axum::http::HeaderMap::new(), etag));
    }
}
