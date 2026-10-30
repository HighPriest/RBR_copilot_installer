/// A credential and its grants, as stored.
pub struct TokenRow {
    pub token_hash: String,
    pub label: Option<String>,
    /// Unix seconds.
    pub expires_at: Option<i64>,
    pub revoked: bool,
    /// The directories this credential may list and download.
    pub grants: Vec<String>,
}

impl TokenRow {
    /// The one definition of "usable" that both `decide` and the `/packages` filter rely on.
    pub fn is_live(&self, now: i64) -> bool {
        !self.revoked && self.expires_at.is_none_or(|deadline| deadline > now)
    }
}

pub enum Decision {
    Allow,
    /// 401: no live credential where one is needed.
    Unauthorized,
    /// 403: a live credential without a grant for the directory.
    Forbidden,
}

/// `public` is always allowed; any other directory needs a live credential granted it.
pub fn decide(token: Option<&TokenRow>, directory: &str, now: i64) -> Decision {
    if directory == pacenotes_shared::PUBLIC_DIR {
        return Decision::Allow;
    }
    match token {
        Some(token) if !token.is_live(now) => Decision::Unauthorized,
        Some(token) if token.grants.iter().any(|grant| grant == directory) => Decision::Allow,
        Some(_) => Decision::Forbidden,
        None => Decision::Unauthorized,
    }
}

/// Current unix seconds.
pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since_epoch| since_epoch.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    fn token(revoked: bool, expires_at: Option<i64>, grants: &[&str]) -> Option<super::TokenRow> {
        Some(super::TokenRow {
            token_hash: "x".into(),
            label: None,
            expires_at,
            revoked,
            grants: grants.iter().map(|grant| grant.to_string()).collect(),
        })
    }

    fn is_allow(decision: &super::Decision) -> bool {
        matches!(decision, super::Decision::Allow)
    }

    fn is_unauthorized(decision: &super::Decision) -> bool {
        matches!(decision, super::Decision::Unauthorized)
    }

    fn is_forbidden(decision: &super::Decision) -> bool {
        matches!(decision, super::Decision::Forbidden)
    }

    type Case = (
        &'static str,
        Option<super::TokenRow>,
        &'static str,
        fn(&super::Decision) -> bool,
    );

    #[test]
    fn decision_table() {
        let now = 1_000;
        let public = pacenotes_shared::PUBLIC_DIR;
        let premium = "premium-voices";
        let cases: Vec<Case> = vec![
            ("public, anonymous", None, public, is_allow),
            (
                "public, revoked",
                token(true, Some(1), &[]),
                public,
                is_allow,
            ),
            ("anonymous", None, premium, is_unauthorized),
            ("no grants", token(false, None, &[]), premium, is_forbidden),
            (
                "other grant",
                token(false, None, &["beta"]),
                premium,
                is_forbidden,
            ),
            (
                "matching grant",
                token(false, None, &[premium]),
                premium,
                is_allow,
            ),
            (
                "one of several",
                token(false, None, &["beta", premium]),
                premium,
                is_allow,
            ),
            (
                "revoked",
                token(true, None, &[premium]),
                premium,
                is_unauthorized,
            ),
            (
                "expired",
                token(false, Some(999), &[premium]),
                premium,
                is_unauthorized,
            ),
            (
                "expires now",
                token(false, Some(1_000), &[premium]),
                premium,
                is_unauthorized,
            ),
            (
                "not yet expired",
                token(false, Some(1_001), &[premium]),
                premium,
                is_allow,
            ),
        ];
        for (case, token, directory, expected) in &cases {
            let decision = super::decide(token.as_ref(), directory, now);
            assert!(expected(&decision), "{case}");
        }
    }
}
