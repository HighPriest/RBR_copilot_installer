//! The `token` subcommands. A raw token exists only in `mint`'s return value;
//! every other command works on its hash.

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
    pub fn short_hash(&self) -> &str {
        short_hash(&self.token_hash)
    }
}

/// Identifies a credential without looking like a usable one.
pub fn short_hash(token_hash: &str) -> &str {
    &token_hash[..token_hash.len().min(SHORT_HASH_LENGTH)]
}

const SHORT_HASH_LENGTH: usize = 12;
const SECONDS_PER_DAY: i64 = 86_400;

/// A raw `pcn_…` token, a full hash, or a prefix of one that matches exactly one credential.
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
        count => anyhow::bail!("{token_or_hash:?} matches {count} credentials — use the full hash"),
    }
}

/// Stores the hash and grants and returns the raw token, which nothing keeps.
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
    let expires_at = expires_days.map(|days| created_at + days * SECONDS_PER_DAY);

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

/// Granting a directory twice is not an error.
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

/// Effective on the next request, since grants are read per request.
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

/// Keeps the row, so `token list` still shows it.
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

/// Newest first.
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
        let is_new_credential = credentials
            .last()
            .is_none_or(|credential| credential.token_hash != token_hash);
        if is_new_credential {
            credentials.push(Credential {
                token_hash,
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

/// `public` is everyone's already, and an invalid name could never match a directory.
fn check_grantable(directory: &str) -> anyhow::Result<()> {
    if directory == pacenotes_shared::PUBLIC_DIR {
        anyhow::bail!(
            "{:?} is visible to everyone and cannot be granted",
            pacenotes_shared::PUBLIC_DIR
        );
    }
    if !pacenotes_shared::valid_id(directory) {
        anyhow::bail!("{directory:?} is not a valid directory name [a-z0-9_-]+");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    async fn open_database() -> libsql::Connection {
        crate::db::open(":memory:").await.unwrap()
    }

    #[tokio::test]
    async fn mint_stores_only_the_hash() {
        let connection = open_database().await;
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

        let looked_up_by_raw_token = crate::auth::load_token(&connection, &token).await.unwrap();
        assert!(looked_up_by_raw_token.is_none());
    }

    #[tokio::test]
    async fn expires_days_becomes_an_absolute_deadline() {
        let connection = open_database().await;
        let before = crate::auth::now();
        super::mint(&connection, None, &[], Some(7)).await.unwrap();
        let expires_at = super::list(&connection).await.unwrap()[0]
            .expires_at
            .unwrap();
        let clock_slack = 2;
        assert!((expires_at - before - 7 * super::SECONDS_PER_DAY).abs() <= clock_slack);
    }

    #[tokio::test]
    async fn grant_and_ungrant_round_trip_by_token_or_hash() {
        let connection = open_database().await;
        let token = super::mint(&connection, None, &[], None).await.unwrap();
        let hash = crate::auth::hash_token(&token);

        super::grant(&connection, &token, "beta").await.unwrap();
        super::grant(&connection, &hash, "premium-voices")
            .await
            .unwrap();
        super::grant(
            &connection,
            &hash[..super::SHORT_HASH_LENGTH],
            "early-access",
        )
        .await
        .unwrap();
        super::grant(&connection, &token, "beta").await.unwrap();

        let mut grants = super::list(&connection).await.unwrap()[0].grants.clone();
        grants.sort();
        assert_eq!(grants, vec!["beta", "early-access", "premium-voices"]);

        super::ungrant(&connection, &token, "beta").await.unwrap();
        let grants = super::list(&connection).await.unwrap()[0].grants.clone();
        assert!(!grants.contains(&"beta".to_string()), "{grants:?}");

        super::ungrant(&connection, &token, "beta").await.unwrap();
    }

    #[tokio::test]
    async fn public_and_invalid_directories_cannot_be_granted() {
        let connection = open_database().await;
        let token = super::mint(&connection, None, &[], None).await.unwrap();

        let error = super::grant(&connection, &token, pacenotes_shared::PUBLIC_DIR)
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("everyone"), "{error}");

        let error = super::grant(&connection, &token, "Premium Voices")
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("not a valid directory name"), "{error}");

        let error = super::mint(&connection, None, &["public".into()], None)
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("everyone"), "{error}");
        assert_eq!(super::list(&connection).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn revoke_marks_the_row_and_decide_refuses_it() {
        let connection = open_database().await;
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
        let connection = open_database().await;
        let error = super::resolve_hash(&connection, "deadbeef")
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("no credential matches"), "{error}");

        super::mint(&connection, None, &[], None).await.unwrap();
        super::mint(&connection, None, &[], None).await.unwrap();
        let prefix_of_every_hash = "";
        let error = super::resolve_hash(&connection, prefix_of_every_hash)
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("use the full hash"), "{error}");
    }
}
