/// The body of every non-2xx response.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ApiError {
    /// Machine-readable: `unauthorized`, `not_found`, `rate_limited`, …
    pub code: String,
    /// Safe to show to the user as-is.
    pub message: String,
}
