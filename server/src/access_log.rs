//! One INFO line per response on target `access`, since production runs at
//! INFO, where `TraceLayer` says nothing.

/// Middleware logging method, target, status, bytes, time to the response head
/// (a download's body is still streaming), the key's hash prefix and the agent.
pub async fn log_response(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let started = std::time::Instant::now();
    let method = request.method().clone();
    let target = request
        .uri()
        .path_and_query()
        .map(|path_and_query| path_and_query.as_str().to_string())
        .unwrap_or_default();
    let key = key_label(request.headers());
    let agent = agent_label(request.headers());

    let response = next.run(request).await;
    tracing::info!(
        target: "access",
        method = %method,
        target_uri = %target,
        status = response.status().as_u16(),
        bytes = %bytes_label(&response),
        ms = started.elapsed().as_millis() as u64,
        key = %key,
        agent = %agent,
        "request"
    );
    response
}

/// `-` without a key, `malformed` for one that could not be minted, otherwise
/// the first 12 hex chars of its hash, as `token list` prints them; never the key.
fn key_label(headers: &axum::http::HeaderMap) -> String {
    match crate::auth::bearer_from_headers(headers) {
        None => "-".to_string(),
        Some(raw) if !crate::auth::well_formed(&raw) => "malformed".to_string(),
        Some(raw) => crate::auth::hash_token(&raw)[..12].to_string(),
    }
}

/// Bounded, and `to_str` admits only visible ASCII, so the agent cannot forge log lines.
fn agent_label(headers: &axum::http::HeaderMap) -> String {
    let agent = headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("-");
    agent.chars().take(200).collect()
}

/// `Content-Length` when the handler set it, otherwise the body's exact size,
/// which hyper only turns into a header later.
fn bytes_label(response: &axum::response::Response) -> String {
    response
        .headers()
        .get(axum::http::header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
        .or_else(|| {
            <axum::body::Body as axum::body::HttpBody>::size_hint(response.body())
                .exact()
                .map(|size| size.to_string())
        })
        .unwrap_or_else(|| "-".to_string())
}

#[cfg(test)]
mod tests {
    fn authorization(value: &str) -> axum::http::HeaderMap {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            axum::http::HeaderValue::from_str(value).unwrap(),
        );
        headers
    }

    #[test]
    fn the_access_log_never_sees_a_raw_key() {
        let key = crate::auth::mint_token();
        let label = super::key_label(&authorization(&format!("Bearer {key}")));
        assert_eq!(label, crate::auth::hash_token(&key)[..12]);
        assert_eq!(super::key_label(&axum::http::HeaderMap::new()), "-");
        assert_eq!(
            super::key_label(&authorization("Bearer garbage")),
            "malformed"
        );
    }
}
