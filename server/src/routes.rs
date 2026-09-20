//! HTTP surface: router + handlers. Handlers stay thin — access logic lives in
//! `auth.rs`, package/metadata logic in `store.rs` (server/CLAUDE.md).
//!
//! CONVENTION (root CLAUDE.md): no `use` statements — full paths at use site.

pub struct AppState {
    pub store: crate::store::ManifestStore,
}

pub fn router(state: std::sync::Arc<AppState>) -> axum::Router {
    axum::Router::new()
        .route("/health", axum::routing::get(health))
        .route("/packages", axum::routing::get(list_packages))
        .route("/download/{directory}/{name}", axum::routing::get(download))
        .with_state(state)
        .layer(tower_http::trace::TraceLayer::new_for_http())
}

async fn health() -> &'static str {
    "ok"
}

/// `GET /packages` — never 403s: it silently filters to the directories the
/// caller may see, so directory names can't be enumerated (task S3/S6).
/// Auth-based filtering arrives with S6; for now only `public` is listed.
async fn list_packages(
    axum::extract::State(state): axum::extract::State<std::sync::Arc<AppState>>,
) -> axum::Json<pacenotes_shared::Manifest> {
    axum::Json(state.store.current().public_only())
}

/// `GET /download/{directory}/{name}` — `directory` and `name` are manifest
/// lookup keys, never path fragments (server/CLAUDE.md). Unknown pair → 404,
/// which is also what a traversal attempt gets.
async fn download(
    axum::extract::State(state): axum::extract::State<std::sync::Arc<AppState>>,
    axum::extract::Path((directory, name)): axum::extract::Path<(String, String)>,
) -> Result<axum::response::Response, crate::error::AppError> {
    let Some((package, path)) = state.store.zip_path(&directory, &name) else {
        return Err(crate::error::AppError::not_found());
    };

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
