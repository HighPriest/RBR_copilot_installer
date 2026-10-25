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
) -> Result<axum::Json<pacenotes_shared::Manifest>, crate::error::AppError> {
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
    Ok(axum::Json(manifest))
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
