//! Credential administration (task S7): the `token` subcommands.
//!
//! SECURITY: a raw token exists exactly once, in `mint`'s return value, and
//! the CLI prints it once. Everything else — storage, lookup, every argument
//! a later command takes — works on the sha256 hash.
//!
//! CONVENTION (root CLAUDE.md): full paths at the point of use.

/// One row of `token list`.
#[derive(Debug)]
pub struct Credential {
    pub token_hash: String,
    pub label: Option<String>,
    pub created_at: i64,
    pub expires_at: Option<i64>,
    pub revoked: bool,
    pub grants: Vec<String>,
}

impl Credential {
    /// Enough of the hash to identify a row without inviting anyone to treat
    /// it as the credential itself.
    pub fn short_hash(&self) -> &str {
        &self.token_hash[..self.token_hash.len().min(12)]
    }
}

/// Accepts either a raw token (`pcn_…`, hashed here) or a hash that an
/// earlier `token list` printed — full or the 12-char prefix.
/// A prefix that matches more than one credential is an error rather than a
/// coin flip.
pub async fn resolve_hash(
    connection: &libsql::Connection,
    token_or_hash: &str,
) -> anyhow::Result<String> {
    if token_or_hash.starts_with(crate::auth::TOKEN_PREFIX) {
        return Ok(crate::auth::hash_token(token_or_hash));
    }

    let mut rows = connection
        .query(
            "SELECT token_hash FROM tokens WHERE token_hash LIKE ?1 || '%'",
            libsql::params![token_or_hash],
        )
        .await
        .map_err(|e| anyhow::anyhow!("looking up credential: {e}"))?;

    let mut matches: Vec<String> = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|e| anyhow::anyhow!("reading credential row: {e}"))?
    {
        matches.push(row.get::<String>(0).unwrap_or_default());
    }
    match matches.len() {
        0 => anyhow::bail!("no credential matches {token_or_hash:?}"),
        1 => Ok(matches.remove(0)),
        n => anyhow::bail!("{token_or_hash:?} matches {n} credentials — use the full hash"),
    }
}

/// Mints a credential and stores its hash plus any grants. Returns the raw
/// token: the ONLY moment it exists.
pub async fn mint(
    connection: &libsql::Connection,
    label: Option<&str>,
    grants: &[String],
    expires_days: Option<i64>,
) -> anyhow::Result<String> {
    for directory in grants {
        check_grantable(directory)?;
    }

    let token = crate::auth::mint_token();
    let hash = crate::auth::hash_token(&token);
    let created_at = crate::auth::now();
    let expires_at = expires_days.map(|days| created_at + days * 86_400);

    connection
        .execute(
            "INSERT INTO tokens (token_hash, label, created_at, expires_at, revoked)
             VALUES (?1, ?2, ?3, ?4, 0)",
            libsql::params![hash.clone(), label, created_at, expires_at],
        )
        .await
        .map_err(|e| anyhow::anyhow!("storing credential: {e}"))?;

    for directory in grants {
        insert_grant(connection, &hash, directory).await?;
    }
    Ok(token)
}

/// Adds a directory grant. Idempotent: granting twice is not an error.
pub async fn grant(
    connection: &libsql::Connection,
    token_or_hash: &str,
    directory: &str,
) -> anyhow::Result<String> {
    check_grantable(directory)?;
    let hash = resolve_hash(connection, token_or_hash).await?;
    insert_grant(connection, &hash, directory).await?;
    Ok(hash)
}

/// Removes a directory grant. Takes effect on the next request: grants are
/// read per request, never cached.
pub async fn ungrant(
    connection: &libsql::Connection,
    token_or_hash: &str,
    directory: &str,
) -> anyhow::Result<String> {
    let hash = resolve_hash(connection, token_or_hash).await?;
    connection
        .execute(
            "DELETE FROM grants WHERE token_hash = ?1 AND directory = ?2",
            libsql::params![hash.clone(), directory],
        )
        .await
        .map_err(|e| anyhow::anyhow!("removing grant: {e}"))?;
    Ok(hash)
}

/// Revokes a credential. The row stays (so `token list` keeps showing it);
/// `auth::decide` treats revoked exactly like an expired credential.
pub async fn revoke(
    connection: &libsql::Connection,
    token_or_hash: &str,
) -> anyhow::Result<String> {
    let hash = resolve_hash(connection, token_or_hash).await?;
    connection
        .execute(
            "UPDATE tokens SET revoked = 1 WHERE token_hash = ?1",
            libsql::params![hash.clone()],
        )
        .await
        .map_err(|e| anyhow::anyhow!("revoking credential: {e}"))?;
    Ok(hash)
}

/// Every credential with its grants, newest first.
pub async fn list(connection: &libsql::Connection) -> anyhow::Result<Vec<Credential>> {
    let mut rows = connection
        .query(
            "SELECT t.token_hash, t.label, t.created_at, t.expires_at, t.revoked, g.directory
             FROM tokens t
             LEFT JOIN grants g ON g.token_hash = t.token_hash
             ORDER BY t.created_at DESC, t.token_hash, g.directory",
            (),
        )
        .await
        .map_err(|e| anyhow::anyhow!("listing credentials: {e}"))?;

    let mut credentials: Vec<Credential> = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|e| anyhow::anyhow!("reading credential row: {e}"))?
    {
        let token_hash = row.get::<String>(0).unwrap_or_default();
        if credentials.last().map(|c| c.token_hash.as_str()) != Some(token_hash.as_str()) {
            credentials.push(Credential {
                token_hash: token_hash.clone(),
                label: row.get::<Option<String>>(1).unwrap_or_default(),
                created_at: row.get::<i64>(2).unwrap_or_default(),
                expires_at: row.get::<Option<i64>>(3).unwrap_or_default(),
                revoked: row.get::<i64>(4).unwrap_or_default() != 0,
                grants: Vec::new(),
            });
        }
        if let (Some(credential), Ok(Some(directory))) =
            (credentials.last_mut(), row.get::<Option<String>>(5))
        {
            credential.grants.push(directory);
        }
    }
    Ok(credentials)
}

async fn insert_grant(
    connection: &libsql::Connection,
    hash: &str,
    directory: &str,
) -> anyhow::Result<()> {
    connection
        .execute(
            "INSERT OR IGNORE INTO grants (token_hash, directory) VALUES (?1, ?2)",
            libsql::params![hash, directory],
        )
        .await
        .map_err(|e| anyhow::anyhow!("storing grant: {e}"))?;
    Ok(())
}

/// `public` is reserved for anonymous access and must never appear in
/// `grants` (server/CLAUDE.md); a grant for a name no directory can have
/// would silently never match, so both are refused up front.
fn check_grantable(directory: &str) -> anyhow::Result<()> {
    if directory == pacenotes_shared::PUBLIC_DIR {
        anyhow::bail!(
            "{:?} is visible to everyone and cannot be granted",
            pacenotes_shared::PUBLIC_DIR
        );
    }
    if !crate::store::valid_directory(directory) {
        anyhow::bail!("{directory:?} is not a valid directory name [a-z0-9_-]+");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    async fn db() -> libsql::Connection {
        crate::db::open(":memory:").await.unwrap()
    }

    #[tokio::test]
    async fn mint_stores_only_the_hash() {
        let connection = db().await;
        let token = super::mint(&connection, Some("Jan"), &["beta".into()], None)
            .await
            .unwrap();
        assert!(token.starts_with(crate::auth::TOKEN_PREFIX));

        let credentials = super::list(&connection).await.unwrap();
        assert_eq!(credentials.len(), 1);
        assert_eq!(credentials[0].token_hash, crate::auth::hash_token(&token));
        assert_eq!(credentials[0].label.as_deref(), Some("Jan"));
        assert_eq!(credentials[0].grants, vec!["beta".to_string()]);
        assert!(!credentials[0].revoked);
        assert!(credentials[0].expires_at.is_none());

        // The raw token appears nowhere in the database.
        let loaded = crate::auth::load_token(&connection, &token).await.unwrap();
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn expires_days_becomes_an_absolute_deadline() {
        let connection = db().await;
        let before = crate::auth::now();
        super::mint(&connection, None, &[], Some(7)).await.unwrap();
        let expires_at = super::list(&connection).await.unwrap()[0]
            .expires_at
            .unwrap();
        assert!((expires_at - before - 7 * 86_400).abs() <= 2);
    }

    #[tokio::test]
    async fn grant_and_ungrant_round_trip_by_token_or_hash() {
        let connection = db().await;
        let token = super::mint(&connection, None, &[], None).await.unwrap();
        let hash = crate::auth::hash_token(&token);

        // By raw token...
        super::grant(&connection, &token, "beta").await.unwrap();
        // ...by full hash...
        super::grant(&connection, &hash, "premium-voices")
            .await
            .unwrap();
        // ...and by the short hash `token list` prints.
        super::grant(&connection, &hash[..12], "early-access")
            .await
            .unwrap();
        // Granting twice is not an error.
        super::grant(&connection, &token, "beta").await.unwrap();

        let mut grants = super::list(&connection).await.unwrap()[0].grants.clone();
        grants.sort();
        assert_eq!(grants, vec!["beta", "early-access", "premium-voices"]);

        super::ungrant(&connection, &token, "beta").await.unwrap();
        let grants = super::list(&connection).await.unwrap()[0].grants.clone();
        assert!(!grants.contains(&"beta".to_string()), "{grants:?}");

        // Ungranting something that was never granted is a no-op, not a crash.
        super::ungrant(&connection, &token, "beta").await.unwrap();
    }

    #[tokio::test]
    async fn public_and_invalid_directories_cannot_be_granted() {
        let connection = db().await;
        let token = super::mint(&connection, None, &[], None).await.unwrap();

        let err = super::grant(&connection, &token, pacenotes_shared::PUBLIC_DIR)
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("everyone"), "{err}");

        let err = super::grant(&connection, &token, "Premium Voices")
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("not a valid directory name"), "{err}");

        // The same check applies at mint time, before anything is stored.
        let err = super::mint(&connection, None, &["public".into()], None)
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("everyone"), "{err}");
        assert_eq!(super::list(&connection).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn revoke_marks_the_row_and_decide_refuses_it() {
        let connection = db().await;
        let token = super::mint(&connection, None, &["beta".into()], None)
            .await
            .unwrap();
        super::revoke(&connection, &token).await.unwrap();

        assert!(super::list(&connection).await.unwrap()[0].revoked);
        let loaded = crate::auth::load_token(&connection, &crate::auth::hash_token(&token))
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            crate::auth::decide(Some(&loaded), "beta", crate::auth::now()),
            crate::auth::Decision::Unauthorized
        ));
    }

    #[tokio::test]
    async fn unknown_and_ambiguous_references_are_refused() {
        let connection = db().await;
        let err = super::resolve_hash(&connection, "deadbeef")
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("no credential matches"), "{err}");

        // Two credentials, then a prefix short enough to hit both.
        super::mint(&connection, None, &[], None).await.unwrap();
        super::mint(&connection, None, &[], None).await.unwrap();
        let err = super::resolve_hash(&connection, "")
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("use the full hash"), "{err}");
    }
}
