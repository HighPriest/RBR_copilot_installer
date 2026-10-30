/// The credential with this hash and all its grants, in one query; `None`
/// when no such credential exists.
pub async fn load_token(
    connection: &libsql::Connection,
    token_hash: &str,
) -> anyhow::Result<Option<crate::auth::TokenRow>> {
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

    let mut loaded: Option<crate::auth::TokenRow> = None;
    while let Some(row) = rows
        .next()
        .await
        .map_err(|e| anyhow::anyhow!("reading credential row: {e}"))?
    {
        let token = loaded.get_or_insert_with(|| crate::auth::TokenRow {
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

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn folds_grants_into_one_row() {
        let connection = crate::db::open(":memory:").await.unwrap();
        connection
            .execute_batch(
                "INSERT INTO tokens (token_hash, label, created_at, expires_at, revoked)
                 VALUES ('h1', 'Jan', 1, 99, 0);
                 INSERT INTO grants (token_hash, directory) VALUES ('h1', 'beta');
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

        let without_grants = super::load_token(&connection, "h2").await.unwrap().unwrap();
        assert!(without_grants.grants.is_empty());
        assert!(without_grants.label.is_none());
        assert!(without_grants.expires_at.is_none());

        assert!(super::load_token(&connection, "nope")
            .await
            .unwrap()
            .is_none());
    }
}
