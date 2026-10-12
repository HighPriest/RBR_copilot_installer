//! HTTP surface: router + handlers. Handlers stay thin — access logic lives in
//! `auth.rs`, package/metadata logic in `store.rs` (server/CLAUDE.md).
//!
//! CONVENTION (root CLAUDE.md): no `use` statements — full paths at use site.

pub struct AppState {
    pub store: crate::store::ManifestStore,
    pub db: libsql::Connection,
}

/// The caller's credential, resolved once per request.
/// `None` = anonymous (or a token that does not exist): either way only
/// `public` is visible.
async fn caller(
    state: &AppState,
    headers: &axum::http::HeaderMap,
) -> Result<Option<crate::auth::TokenRow>, crate::error::AppError> {
    let Some(raw) = crate::auth::bearer_from_headers(headers) else {
        return Ok(None);
    };
    // SECURITY: only the hash leaves this line; the raw token is never logged
    // and never reaches an error message.
    crate::auth::load_token(&state.db, &crate::auth::hash_token(&raw))
        .await
        .map_err(|e| crate::error::AppError::internal("loading credential", e))
}

pub fn router(state: std::sync::Arc<AppState>) -> axum::Router {
    axum::Router::new()
        .route("/health", axum::routing::get(health))
        .route("/packages", axum::routing::get(list_packages))
        // `{*name}` (wildcard) because a package name is its path under the
        // access directory and spans the remaining segments:
        // /download/public/codriver/polish/male/Kamil.
        .route(
            "/download/{directory}/{*name}",
            axum::routing::get(download),
        )
        .route("/motd", axum::routing::get(motd))
        .with_state(state)
        .layer(tower_http::trace::TraceLayer::new_for_http())
}

async fn health() -> &'static str {
    "ok"
}

/// `GET /packages` — never 401s or 403s: it silently filters to the
/// directories the caller may see, so a caller cannot enumerate directory
/// names by probing (server/CLAUDE.md). A revoked, expired or unknown token
/// therefore yields exactly the anonymous listing.
async fn list_packages(
    axum::extract::State(state): axum::extract::State<std::sync::Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<axum::Json<pacenotes_shared::Manifest>, crate::error::AppError> {
    let token = caller(&state, &headers).await?;
    let grants = match token {
        Some(t) if t.is_live(crate::auth::now()) => t.grants,
        _ => Vec::new(),
    };
    Ok(axum::Json(state.store.current().visible_to(&grants)))
}

/// `GET /download/{directory}/{*name}` — `name` spans every remaining
/// segment, because it is the package's path under its access directory.
/// Both halves stay manifest lookup KEYS, never path fragments
/// (server/CLAUDE.md): only a manifest hit yields a path on disk, so an
/// unknown pair — including every traversal attempt — is a 404.
async fn download(
    axum::extract::State(state): axum::extract::State<std::sync::Arc<AppState>>,
    axum::extract::Path((directory, name)): axum::extract::Path<(String, String)>,
    headers: axum::http::HeaderMap,
) -> Result<axum::response::Response, crate::error::AppError> {
    let Some((package, path)) = state.store.zip_path(&directory, &name) else {
        return Err(crate::error::AppError::not_found());
    };

    // Access is decided AFTER the lookup, so the status tells the caller
    // about a package that exists; a package that does not is always 404,
    // whatever credential was presented.
    let token = caller(&state, &headers).await?;
    match crate::auth::decide(token.as_ref(), &package.directory, crate::auth::now()) {
        crate::auth::Decision::Allow => {}
        crate::auth::Decision::Unauthorized => return Err(crate::error::AppError::unauthorized()),
        crate::auth::Decision::Forbidden => return Err(crate::error::AppError::forbidden()),
    }

    let file = match tokio::fs::File::open(&path).await {
        Ok(f) => f,
        // Manifest hit but no file on disk: the manifest is stale, not the
        // caller's problem — report it as a missing package and log it.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            tracing::warn!(path = %path.display(), "manifest entry without a file on disk");
            return Err(crate::error::AppError::not_found());
        }
        Err(e) => return Err(crate::error::AppError::internal("opening package", e)),
    };

    // Trust the file, not the manifest, for Content-Length.
    let size = match file.metadata().await {
        Ok(m) => m.len(),
        Err(e) => return Err(crate::error::AppError::internal("stat package", e)),
    };

    let body = axum::body::Body::from_stream(tokio_util::io::ReaderStream::new(file));
    let headers = [
        (
            axum::http::header::CONTENT_TYPE,
            "application/zip".to_string(),
        ),
        (axum::http::header::CONTENT_LENGTH, size.to_string()),
        (
            axum::http::header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}.zip\"", package.name),
        ),
    ];
    Ok(axum::response::IntoResponse::into_response((headers, body)))
}

#[derive(serde::Deserialize)]
pub struct MotdQuery {
    /// Language tag; validated in the store before it touches the
    /// filesystem. Absent = the fallback language.
    #[serde(default)]
    lang: String,
}

/// `GET /motd?lang=xx` → the message of the day as `text/plain`, falling back
/// to English. No auth: it is shown before the user has entered anything.
///
/// 404 when there is no text at all, which the installer treats exactly like
/// being offline — so an operator who publishes no MOTD costs the user
/// nothing.
async fn motd(
    axum::extract::State(state): axum::extract::State<std::sync::Arc<AppState>>,
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
