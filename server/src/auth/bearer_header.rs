/// The raw token from `Authorization: Bearer <token>`, if well-formed.
/// It is a credential: never log it or put it in an error.
pub fn bearer_from_headers(headers: &axum::http::HeaderMap) -> Option<String> {
    let value = headers.get(pacenotes_shared::AUTH_HEADER)?.to_str().ok()?;
    let token = value.strip_prefix(pacenotes_shared::BEARER_PREFIX)?.trim();
    if token.is_empty() {
        return None;
    }
    Some(token.to_string())
}

#[cfg(test)]
mod tests {
    fn headers(authorization: &str) -> axum::http::HeaderMap {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            axum::http::HeaderValue::from_str(authorization).unwrap(),
        );
        headers
    }

    #[test]
    fn bearer_header_parsing() {
        for (authorization, expected) in [
            ("Bearer pcn_abc", Some("pcn_abc")),
            ("Bearer  pcn_abc  ", Some("pcn_abc")),
            ("pcn_abc", None),
            ("Basic pcn_abc", None),
            ("Bearer   ", None),
        ] {
            assert_eq!(
                super::bearer_from_headers(&headers(authorization)).as_deref(),
                expected,
                "{authorization:?}"
            );
        }
        assert!(super::bearer_from_headers(&axum::http::HeaderMap::new()).is_none());
    }
}
