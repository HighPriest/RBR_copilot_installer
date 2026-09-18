//! Auth: bearer token → sha256 → libSQL lookup → pure access decision.
//! ACCESS MODEL: no tiers. A credential (token) holds grants, each grant
//! naming one package directory. `public` needs no credential. The decision
//! function is pure and exhaustively unit-tested (task S5).
//! SECURITY: raw tokens are never stored or logged (see server/CLAUDE.md).

#![allow(dead_code)]

use pacenotes_shared::PUBLIC_DIR;

pub struct TokenRow {
    pub token_hash: String,
    pub label: Option<String>,
    pub expires_at: Option<i64>, // unix seconds
    pub revoked: bool,
    /// Directory names from the grants table for this token.
    pub grants: Vec<String>,
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
    if directory == PUBLIC_DIR {
        return Decision::Allow;
    }
    match token {
        None => Decision::Unauthorized,
        Some(t) => {
            let expired = t.expires_at.map_or(false, |e| e <= now);
            if t.revoked || expired {
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

/// sha256 of the raw bearer token, lowercase hex — the DB key. Task S5.
pub fn hash_token(raw: &str) -> String {
    use sha2::{Digest, Sha256};
    let d = Sha256::digest(raw.as_bytes());
    d.iter().map(|b| format!("{b:02x}")).collect()
}

// TODO(S5): async fn load_token(db, hash) -> Option<TokenRow>
//           (single query: tokens LEFT JOIN grants, fold rows)
// TODO(S5): fn bearer_from_headers(&HeaderMap) -> Option<String>
//           (strip shared::BEARER_PREFIX, trim)

#[cfg(test)]
mod tests {
    use super::*;

    fn tok(revoked: bool, expires_at: Option<i64>, grants: &[&str]) -> TokenRow {
        TokenRow {
            token_hash: "x".into(),
            label: None,
            expires_at,
            revoked,
            grants: grants.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn decision_table() {
        use Decision::*;
        let now = 1_000;
        let cases: Vec<(Option<TokenRow>, &str, fn(&Decision) -> bool)> = vec![
            // public: always allowed, even with a bad token attached
            (None, PUBLIC_DIR, |d| matches!(d, Allow)),
            (Some(tok(true, Some(1), &[])), PUBLIC_DIR, |d| matches!(d, Allow)),
            // protected dir, no token
            (None, "premium-voices", |d| matches!(d, Unauthorized)),
            // valid token, no grant
            (Some(tok(false, None, &[])), "premium-voices", |d| matches!(d, Forbidden)),
            // valid token, grant for a DIFFERENT dir
            (Some(tok(false, None, &["beta"])), "premium-voices", |d| matches!(d, Forbidden)),
            // valid token with matching grant
            (Some(tok(false, None, &["premium-voices"])), "premium-voices", |d| matches!(d, Allow)),
            // multiple grants, one matches
            (Some(tok(false, None, &["beta", "premium-voices"])), "premium-voices", |d| matches!(d, Allow)),
            // revoked
            (Some(tok(true, None, &["premium-voices"])), "premium-voices", |d| matches!(d, Unauthorized)),
            // expired (expires_at <= now)
            (Some(tok(false, Some(999), &["premium-voices"])), "premium-voices", |d| matches!(d, Unauthorized)),
            (Some(tok(false, Some(1_000), &["premium-voices"])), "premium-voices", |d| matches!(d, Unauthorized)),
            // not yet expired
            (Some(tok(false, Some(1_001), &["premium-voices"])), "premium-voices", |d| matches!(d, Allow)),
        ];
        for (i, (t, dir, check)) in cases.iter().enumerate() {
            let d = decide(t.as_ref(), dir, now);
            assert!(check(&d), "case {i} failed");
        }
    }
}
