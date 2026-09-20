//! Request-path error type. Every non-2xx response body is a
//! `shared::ApiError` JSON object (server/CLAUDE.md). Handlers never panic:
//! they return `Result<_, AppError>` and this maps it to a status.
//!
//! CONVENTION (root CLAUDE.md): no `use` statements — full paths at use site.

pub struct AppError {
    pub status: axum::http::StatusCode,
    pub code: &'static str,
    pub message: String,
}

impl AppError {
    /// Missing/malformed/expired/revoked credential on a protected directory.
    // TODO(S6): drop this allow — unauthorized()/forbidden() get their callers
    // when /download starts enforcing auth::decide().
    #[allow(dead_code)]
    pub fn unauthorized() -> Self {
        Self {
            status: axum::http::StatusCode::UNAUTHORIZED,
            code: "unauthorized",
            message: "A valid access key is required for this package.".to_string(),
        }
    }

    /// Valid credential, but it does not grant this directory.
    #[allow(dead_code)] // TODO(S6): see above.
    pub fn forbidden() -> Self {
        Self {
            status: axum::http::StatusCode::FORBIDDEN,
            code: "forbidden",
            message: "This access key does not grant that package.".to_string(),
        }
    }

    pub fn not_found() -> Self {
        Self {
            status: axum::http::StatusCode::NOT_FOUND,
            code: "not_found",
            message: "No such package.".to_string(),
        }
    }

    /// Something broke on our side. The cause is logged, never returned.
    pub fn internal(context: &str, error: impl std::fmt::Display) -> Self {
        tracing::error!(error = %error, "{context}");
        Self {
            status: axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            code: "internal",
            message: "Internal server error.".to_string(),
        }
    }
}

impl axum::response::IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let body = pacenotes_shared::ApiError {
            code: self.code.to_string(),
            message: self.message,
        };
        (self.status, axum::Json(body)).into_response()
    }
}
