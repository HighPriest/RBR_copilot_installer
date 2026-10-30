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
        (self.status, axum::Json(body)).into_response()
    }
}
