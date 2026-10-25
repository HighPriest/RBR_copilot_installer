//! The libSQL auth database: credentials and their grants, never packages.
//! Queries stay plain SQL so that moving to remote Turso is only a `--db-url` change.

/// Idempotent (`IF NOT EXISTS`), so it runs on every start.
const MIGRATION_0001: &str = include_str!("../migrations/0001_init.sql");

/// Opens `db_url` and applies the migrations.
///
/// - `file:./data/auth.db` or a bare path: a local file, parent folders created
/// - `:memory:`: ephemeral
/// - `libsql://…`, `http://…`, `https://…`: remote Turso, authenticated by
///   `PACENOTES_DB_AUTH_TOKEN`, an environment variable so it stays out of the process list
pub async fn open(db_url: &str) -> anyhow::Result<libsql::Connection> {
    let is_remote = ["libsql://", "http://", "https://"]
        .iter()
        .any(|scheme| db_url.starts_with(scheme));
    let database = if is_remote {
        let token = std::env::var("PACENOTES_DB_AUTH_TOKEN").unwrap_or_default();
        libsql::Builder::new_remote(db_url.to_string(), token)
            .build()
            .await
            .map_err(|e| anyhow::anyhow!("connecting to {db_url}: {e}"))?
    } else {
        let path = local_path(db_url);
        if path != std::path::Path::new(":memory:") {
            if let Some(parent) = path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
            {
                std::fs::create_dir_all(parent)
                    .map_err(|e| anyhow::anyhow!("creating {}: {e}", parent.display()))?;
            }
        }
        libsql::Builder::new_local(&path)
            .build()
            .await
            .map_err(|e| anyhow::anyhow!("opening {}: {e}", path.display()))?
    };

    let connection = database
        .connect()
        .map_err(|e| anyhow::anyhow!("connecting to {db_url}: {e}"))?;
    if !is_remote {
        tune_local(
            &connection,
            local_path(db_url) == std::path::Path::new(":memory:"),
        )
        .await?;
    }
    connection
        .execute_batch(MIGRATION_0001)
        .await
        .map_err(|e| anyhow::anyhow!("applying migrations/0001_init.sql: {e}"))?;
    Ok(connection)
}

/// Waits up to 5 s for a lock and, outside `:memory:`, switches to WAL, since
/// an admin `token mint` beside live traffic otherwise fails SQLITE_BUSY as a 500.
/// Remote libSQL manages its own storage and is never tuned.
async fn tune_local(connection: &libsql::Connection, in_memory: bool) -> anyhow::Result<()> {
    // Both PRAGMAs answer with a row, so they go through `query`.
    let pragma = |sql: &'static str| async move {
        let mut rows = connection
            .query(sql, ())
            .await
            .map_err(|e| anyhow::anyhow!("{sql}: {e}"))?;
        while rows
            .next()
            .await
            .map_err(|e| anyhow::anyhow!("{sql}: {e}"))?
            .is_some()
        {}
        anyhow::Ok(())
    };
    pragma("PRAGMA busy_timeout = 5000").await?;
    if !in_memory {
        pragma("PRAGMA journal_mode = WAL").await?;
    }
    Ok(())
}

/// `file:./data/auth.db` → `./data/auth.db`; a query string is left in place, not stripped.
fn local_path(db_url: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(db_url.strip_prefix("file:").unwrap_or(db_url))
}

#[cfg(test)]
mod tests {
    #[test]
    fn file_urls_become_paths() {
        for (url, path) in [
            ("file:./data/auth.db", "./data/auth.db"),
            ("/var/lib/pacenotes/auth.db", "/var/lib/pacenotes/auth.db"),
            (":memory:", ":memory:"),
        ] {
            assert_eq!(super::local_path(url), std::path::Path::new(path));
        }
    }

    /// Reads a single-value PRAGMA back.
    async fn pragma(connection: &libsql::Connection, sql: &str) -> libsql::Value {
        let mut rows = connection.query(sql, ()).await.unwrap();
        let row = rows.next().await.unwrap().unwrap();
        row.get_value(0).unwrap()
    }

    #[tokio::test]
    async fn a_local_file_db_uses_wal_and_waits_for_locks() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.db");
        let connection = super::open(&format!("file:{}", path.display()))
            .await
            .unwrap();
        assert_eq!(
            pragma(&connection, "PRAGMA journal_mode").await,
            libsql::Value::Text("wal".into())
        );
        assert_eq!(
            pragma(&connection, "PRAGMA busy_timeout").await,
            libsql::Value::Integer(5000)
        );
    }

    #[tokio::test]
    async fn migrations_are_idempotent() {
        let connection = super::open(":memory:").await.unwrap();
        connection
            .execute_batch(super::MIGRATION_0001)
            .await
            .unwrap();
        connection
            .execute_batch(
                "INSERT INTO tokens (token_hash, created_at) VALUES ('h', 1);
                 INSERT INTO grants (token_hash, directory) VALUES ('h', 'beta');",
            )
            .await
            .unwrap();
    }
}
