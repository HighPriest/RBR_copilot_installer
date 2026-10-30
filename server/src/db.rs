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
    connection
        .execute_batch(MIGRATION_0001)
        .await
        .map_err(|e| anyhow::anyhow!("applying migrations/0001_init.sql: {e}"))?;
    Ok(connection)
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
