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
        .with_state(state)
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

/// Never 401 or 403: an unusable credential silently gets the anonymous
/// listing, so probing cannot reveal directory names.
async fn list_packages(
    axum::extract::State(state): SharedState,
    axum::extract::Query(query): axum::extract::Query<PackagesQuery>,
    headers: axum::http::HeaderMap,
) -> Result<axum::Json<pacenotes_shared::Manifest>, crate::error::AppError> {
    let grants = match caller(&state, &headers).await? {
        Some(token) if token.is_live(crate::auth::now()) => token.grants,
        _ => Vec::new(),
    };
    let mut manifest = state.store.current().visible_to(&grants);
    if let Some(game) = query.game.as_deref() {
        manifest = manifest.for_game(game);
    }
    Ok(axum::Json(manifest))
}

/// An unknown package is 404 whatever the credential, because access is
/// decided only after the manifest lookup succeeds.
async fn download(
    axum::extract::State(state): SharedState,
    axum::extract::Path((game, directory, name)): axum::extract::Path<(String, String, String)>,
    headers: axum::http::HeaderMap,
) -> Result<axum::response::Response, crate::error::AppError> {
    let Some((package, path)) = state.store.zip_path(&game, &directory, &name) else {
        return Err(crate::error::AppError::not_found());
    };

    let token = caller(&state, &headers).await?;
    match crate::auth::decide(token.as_ref(), &package.directory, crate::auth::now()) {
        crate::auth::Decision::Allow => {}
        crate::auth::Decision::Unauthorized => return Err(crate::error::AppError::unauthorized()),
        crate::auth::Decision::Forbidden => return Err(crate::error::AppError::forbidden()),
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

/// The caller's credential, or `None` for anonymous and unknown tokens alike.
async fn caller(
    state: &AppState,
    headers: &axum::http::HeaderMap,
) -> Result<Option<crate::auth::TokenRow>, crate::error::AppError> {
    let Some(raw) = crate::auth::bearer_from_headers(headers) else {
        return Ok(None);
    };
    crate::auth::load_token(&state.db, &crate::auth::hash_token(&raw))
        .await
        .map_err(|e| crate::error::AppError::internal("loading credential", e))
}
