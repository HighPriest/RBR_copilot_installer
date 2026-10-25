/// A request failure, answered with a status and a `shared::ApiError` body.
pub struct AppError {
    pub status: axum::http::StatusCode,
    pub code: &'static str,
    pub message: String,
}

impl AppError {
    pub fn unauthorized() -> Self {
        Self {
            status: axum::http::StatusCode::UNAUTHORIZED,
            code: "unauthorized",
            message: "A valid access key is required for this package.".to_string(),
        }
    }

    /// `/whoami` without a live key: the code of `unauthorized`, worded for the key itself.
    pub fn invalid_key() -> Self {
        Self {
            status: axum::http::StatusCode::UNAUTHORIZED,
            code: "unauthorized",
            message: "This access key is not valid, or has expired.".to_string(),
        }
    }

    pub fn not_found() -> Self {
        Self {
            status: axum::http::StatusCode::NOT_FOUND,
            code: "not_found",
            message: "No such package.".to_string(),
        }
    }

    /// The file on disk differs from what the manifest promises, almost always
    /// a package being replaced mid-publish, so the client should retry.
    pub fn unavailable() -> Self {
        Self {
            status: axum::http::StatusCode::SERVICE_UNAVAILABLE,
            code: "unavailable",
            message: "This package is being updated on the server. Try again shortly.".to_string(),
        }
    }

    /// Logs the cause and answers 500 without it.
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
        let mut response = (self.status, axum::Json(body)).into_response();
        // RFC 9110: a 401 names the scheme it wants.
        if self.status == axum::http::StatusCode::UNAUTHORIZED {
            response.headers_mut().insert(
                axum::http::header::WWW_AUTHENTICATE,
                axum::http::HeaderValue::from_static("Bearer"),
            );
        }
        response
    }
}
