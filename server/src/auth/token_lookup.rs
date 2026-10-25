/// The credential with this hash and all its grants, in one query; `None`
/// when no such credential exists.
///
/// Decodes fail-closed, since defaulting `revoked` or `expires_at` after a
/// schema drift would turn a dead key live; raw values are read because libsql
/// 0.6's typed `row.get` panics on a type mismatch.
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
        let column = |index: i32| {
            row.get_value(index)
                .map_err(|e| anyhow::anyhow!("reading credential column {index}: {e}"))
        };
        if loaded.is_none() {
            loaded = Some(crate::auth::TokenRow {
                token_hash: text(column(0)?, "token_hash")?,
                label: optional_text(column(1)?, "label")?,
                expires_at: optional_integer(column(2)?, "expires_at")?,
                revoked: integer(column(3)?, "revoked")? != 0,
                grants: Vec::new(),
            });
        }
        let directory = optional_text(column(4)?, "grant")?;
        if let (Some(token), Some(directory)) = (loaded.as_mut(), directory) {
            token.grants.push(directory);
        }
    }
    Ok(loaded)
}

fn integer(value: libsql::Value, column: &str) -> anyhow::Result<i64> {
    match value {
        libsql::Value::Integer(integer) => Ok(integer),
        other => anyhow::bail!("credential {column}: expected an integer, found {other:?}"),
    }
}

fn optional_integer(value: libsql::Value, column: &str) -> anyhow::Result<Option<i64>> {
    match value {
        libsql::Value::Null => Ok(None),
        other => integer(other, column).map(Some),
    }
}

fn text(value: libsql::Value, column: &str) -> anyhow::Result<String> {
    match value {
        libsql::Value::Text(text) => Ok(text),
        other => anyhow::bail!("credential {column}: expected text, found {other:?}"),
    }
}

fn optional_text(value: libsql::Value, column: &str) -> anyhow::Result<Option<String>> {
    match value {
        libsql::Value::Null => Ok(None),
        other => text(other, column).map(Some),
    }
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

    /// SQLite's dynamic typing lets text land in `revoked`, which used to decode as live.
    #[tokio::test]
    async fn an_undecodable_row_fails_closed() {
        let connection = crate::db::open(":memory:").await.unwrap();
        connection
            .execute(
                "INSERT INTO tokens (token_hash, created_at, revoked) VALUES ('h', 1, 'yes')",
                (),
            )
            .await
            .unwrap();
        assert!(super::load_token(&connection, "h").await.is_err());
    }
}
