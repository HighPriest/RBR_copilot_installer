//! Auth: bearer token → sha256 → libSQL lookup → pure access decision.
//! ACCESS MODEL: no tiers. A credential (token) holds grants, each grant
//! naming one package directory. `public` needs no credential. The decision
//! function is pure and exhaustively unit-tested (task S5).
//! SECURITY: raw tokens are never stored or logged (see server/CLAUDE.md).
//! CONVENTION (root CLAUDE.md): full paths at the point of use.

pub struct TokenRow {
    pub token_hash: String,
    pub label: Option<String>,
    pub expires_at: Option<i64>, // unix seconds
    pub revoked: bool,
    /// Directory names from the grants table for this token.
    pub grants: Vec<String>,
}

impl TokenRow {
    /// A credential is live when it is neither revoked nor past its
    /// deadline. `decide` and the `/packages` filter must agree on this, so
    /// it is defined once, here.
    pub fn is_live(&self, now: i64) -> bool {
        !self.revoked && self.expires_at.is_none_or(|e| e > now)
    }
}

pub enum Decision {
    Allow,
    /// No/invalid/expired/revoked token where one was needed → 401
    Unauthorized,
    /// Valid token but no grant for this directory → 403
    Forbidden,
}

/// Pure access decision for a package in `directory`. `now` injected for
/// testability. Rules: `public` → always Allow. Any other directory →
/// requires a live token (not revoked/expired) with a grant for exactly
/// that directory.
pub fn decide(token: Option<&TokenRow>, directory: &str, now: i64) -> Decision {
    if directory == pacenotes_shared::PUBLIC_DIR {
        return Decision::Allow;
    }
    match token {
        None => Decision::Unauthorized,
        Some(t) => {
            if !t.is_live(now) {
                return Decision::Unauthorized;
            }
            if t.grants.iter().any(|g| g == directory) {
                Decision::Allow
            } else {
                Decision::Forbidden
            }
        }
    }
}

/// sha256 of the raw bearer token, lowercase hex — the DB key.
/// SECURITY: this is the ONLY representation of a credential the server keeps.
pub fn hash_token(raw: &str) -> String {
    let digest = <sha2::Sha256 as sha2::Digest>::digest(raw.as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// Prefix identifying a Pacenotes access key, so a user can tell what they
/// pasted and so secret scanners have something to match.
pub const TOKEN_PREFIX: &str = "pcn_";

/// Mints a fresh credential: `pcn_` + 32 random bytes, base64url, unpadded.
/// The caller prints it ONCE and stores only `hash_token` of it.
pub fn mint_token() -> String {
    let mut bytes = [0u8; 32];
    <rand::rngs::OsRng as rand::RngCore>::fill_bytes(&mut rand::rngs::OsRng, &mut bytes);
    format!("{TOKEN_PREFIX}{}", base64url(&bytes))
}

/// Unpadded base64url. Hand-rolled to avoid a dependency for 20 lines; the
/// alphabet is URL/header-safe so a token survives being pasted anywhere.
fn base64url(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        let indices = [n >> 18 & 63, n >> 12 & 63, n >> 6 & 63, n & 63];
        // 3 input bytes -> 4 chars, 2 -> 3, 1 -> 2. No padding.
        for index in indices.iter().take(chunk.len() + 1) {
            out.push(ALPHABET[*index as usize] as char);
        }
    }
    out
}

/// Extracts the raw bearer token from request headers, if present and
/// well-formed. SECURITY: the value is a credential — never log it, never put
/// it in an error message or a tracing field.
pub fn bearer_from_headers(headers: &axum::http::HeaderMap) -> Option<String> {
    let value = headers.get(pacenotes_shared::AUTH_HEADER)?.to_str().ok()?;
    let token = value.strip_prefix(pacenotes_shared::BEARER_PREFIX)?.trim();
    if token.is_empty() {
        return None;
    }
    Some(token.to_string())
}

/// Loads a credential and its grants in ONE query (tokens LEFT JOIN grants),
/// folding the rows. `None` = no such credential; a credential with no grants
/// is `Some` with an empty `grants` (it is valid, it just sees only `public`).
pub async fn load_token(
    connection: &libsql::Connection,
    token_hash: &str,
) -> anyhow::Result<Option<TokenRow>> {
    let mut rows = connection
        .query(
            "SELECT t.token_hash, t.label, t.expires_at, t.revoked, g.directory
             FROM tokens t
             LEFT JOIN grants g ON g.token_hash = t.token_hash
             WHERE t.token_hash = ?1",
            libsql::params![token_hash],
        )
        .await
        .map_err(|e| anyhow::anyhow!("loading credential: {e}"))?;

    let mut loaded: Option<TokenRow> = None;
    while let Some(row) = rows
        .next()
        .await
        .map_err(|e| anyhow::anyhow!("reading credential row: {e}"))?
    {
        let token = loaded.get_or_insert_with(|| TokenRow {
            token_hash: row.get::<String>(0).unwrap_or_default(),
            label: row.get::<Option<String>>(1).unwrap_or_default(),
            expires_at: row.get::<Option<i64>>(2).unwrap_or_default(),
            revoked: row.get::<i64>(3).unwrap_or_default() != 0,
            grants: Vec::new(),
        });
        if let Ok(Some(directory)) = row.get::<Option<String>>(4) {
            token.grants.push(directory);
        }
    }
    Ok(loaded)
}

/// Current unix seconds, for expiry checks and `created_at`.
pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    fn tok(revoked: bool, expires_at: Option<i64>, grants: &[&str]) -> super::TokenRow {
        super::TokenRow {
            token_hash: "x".into(),
            label: None,
            expires_at,
            revoked,
            grants: grants.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// (token, directory, predicate over the resulting decision)
    type Case = (
        Option<super::TokenRow>,
        &'static str,
        fn(&super::Decision) -> bool,
    );

    #[test]
    fn hashing_is_stable_and_hides_the_token() {
        // Known vector: sha256("abc").
        assert_eq!(
            super::hash_token("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let token = super::mint_token();
        assert!(!super::hash_token(&token).contains(&token));
    }

    #[test]
    fn minted_tokens_are_prefixed_unique_and_url_safe() {
        let a = super::mint_token();
        let b = super::mint_token();
        assert_ne!(a, b);
        assert!(a.starts_with(super::TOKEN_PREFIX), "{a}");
        // 32 bytes -> 43 unpadded base64url chars.
        assert_eq!(a.len(), super::TOKEN_PREFIX.len() + 43, "{a}");
        assert!(
            a[super::TOKEN_PREFIX.len()..]
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_'),
            "{a}"
        );
    }

    #[test]
    fn base64url_matches_known_vectors() {
        assert_eq!(super::base64url(b""), "");
        assert_eq!(super::base64url(b"f"), "Zg");
        assert_eq!(super::base64url(b"fo"), "Zm8");
        assert_eq!(super::base64url(b"foo"), "Zm9v");
        assert_eq!(super::base64url(b"foobar"), "Zm9vYmFy");
        // The two bytes that differ between base64 and base64url.
        assert_eq!(super::base64url(&[0xfb, 0xff]), "-_8");
    }

    #[test]
    fn bearer_header_parsing() {
        let header = |value: &str| {
            let mut headers = axum::http::HeaderMap::new();
            headers.insert(
                axum::http::header::AUTHORIZATION,
                axum::http::HeaderValue::from_str(value).unwrap(),
            );
            headers
        };
        assert_eq!(
            super::bearer_from_headers(&header("Bearer pcn_abc")).as_deref(),
            Some("pcn_abc")
        );
        assert_eq!(
            super::bearer_from_headers(&header("Bearer  pcn_abc  ")).as_deref(),
            Some("pcn_abc")
        );
        assert!(super::bearer_from_headers(&axum::http::HeaderMap::new()).is_none());
        assert!(super::bearer_from_headers(&header("pcn_abc")).is_none());
        assert!(super::bearer_from_headers(&header("Basic pcn_abc")).is_none());
        assert!(super::bearer_from_headers(&header("Bearer   ")).is_none());
    }

    #[tokio::test]
    async fn load_token_folds_grants_into_one_row() {
        let connection = crate::db::open(":memory:").await.unwrap();
        connection
            .execute(
                "INSERT INTO tokens (token_hash, label, created_at, expires_at, revoked)
                 VALUES ('h1', 'Jan', 1, 99, 0)",
                (),
            )
            .await
            .unwrap();
        connection
            .execute_batch(
                "INSERT INTO grants (token_hash, directory) VALUES ('h1', 'beta');
                 INSERT INTO grants (token_hash, directory) VALUES ('h1', 'premium-voices');
                 INSERT INTO tokens (token_hash, created_at) VALUES ('h2', 1);",
            )
            .await
            .unwrap();

        let loaded = super::load_token(&connection, "h1").await.unwrap().unwrap();
        assert_eq!(loaded.token_hash, "h1");
        assert_eq!(loaded.label.as_deref(), Some("Jan"));
        assert_eq!(loaded.expires_at, Some(99));
        assert!(!loaded.revoked);
        let mut grants = loaded.grants.clone();
        grants.sort();
        assert_eq!(grants, vec!["beta", "premium-voices"]);

        // A credential with no grants is valid and sees only `public`.
        let bare = super::load_token(&connection, "h2").await.unwrap().unwrap();
        assert!(bare.grants.is_empty());
        assert!(bare.label.is_none());
        assert!(bare.expires_at.is_none());

        assert!(super::load_token(&connection, "nope")
            .await
            .unwrap()
            .is_none());
    }

    #[test]
    fn decision_table() {
        use super::Decision::{Allow, Forbidden, Unauthorized};
        let now = 1_000;
        let cases: Vec<Case> = vec![
            // public: always allowed, even with a bad token attached
            (None, pacenotes_shared::PUBLIC_DIR, |d| matches!(d, Allow)),
            (
                Some(tok(true, Some(1), &[])),
                pacenotes_shared::PUBLIC_DIR,
                |d| matches!(d, Allow),
            ),
            // protected dir, no token
            (None, "premium-voices", |d| matches!(d, Unauthorized)),
            // valid token, no grant
            (Some(tok(false, None, &[])), "premium-voices", |d| {
                matches!(d, Forbidden)
            }),
            // valid token, grant for a DIFFERENT dir
            (Some(tok(false, None, &["beta"])), "premium-voices", |d| {
                matches!(d, Forbidden)
            }),
            // valid token with matching grant
            (
                Some(tok(false, None, &["premium-voices"])),
                "premium-voices",
                |d| matches!(d, Allow),
            ),
            // multiple grants, one matches
            (
                Some(tok(false, None, &["beta", "premium-voices"])),
                "premium-voices",
                |d| matches!(d, Allow),
            ),
            // revoked
            (
                Some(tok(true, None, &["premium-voices"])),
                "premium-voices",
                |d| matches!(d, Unauthorized),
            ),
            // expired (expires_at <= now)
            (
                Some(tok(false, Some(999), &["premium-voices"])),
                "premium-voices",
                |d| matches!(d, Unauthorized),
            ),
            (
                Some(tok(false, Some(1_000), &["premium-voices"])),
                "premium-voices",
                |d| matches!(d, Unauthorized),
            ),
            // not yet expired
            (
                Some(tok(false, Some(1_001), &["premium-voices"])),
                "premium-voices",
                |d| matches!(d, Allow),
            ),
        ];
        for (i, (t, dir, check)) in cases.iter().enumerate() {
            let d = super::decide(t.as_ref(), dir, now);
            assert!(check(&d), "case {i} failed");
        }
    }
}
