//! libSQL connection + migrations. The DB holds AUTH ONLY (credentials and
//! their directory grants); packages never touch it.
//!
//! PORTABILITY (server/CLAUDE.md): switching from the embedded file to remote
//! Turso must require changing `--db-url` and nothing else, so every query
//! stays plain SQL and nothing here assumes a local file beyond opening one.
//!
//! CONVENTION (root CLAUDE.md): full paths at the point of use.

/// Schema, applied on every start. Every statement is `IF NOT EXISTS`, so
/// this is idempotent and needs no migration bookkeeping table yet.
const MIGRATION_0001: &str = include_str!("../migrations/0001_init.sql");

/// Opens `db_url` and applies the migrations.
///
/// Accepted forms:
/// - `file:./data/auth.db` or a bare path — embedded local file (parent
///   directories are created)
/// - `:memory:` — ephemeral, used by tests
/// - `libsql://…`, `http://…`, `https://…` — remote Turso; the auth token
///   comes from `PACENOTES_DB_AUTH_TOKEN` (never a flag: it would land in
///   the process list).
pub async fn open(db_url: &str) -> anyhow::Result<libsql::Connection> {
    let database = if db_url.starts_with("libsql://")
        || db_url.starts_with("http://")
        || db_url.starts_with("https://")
    {
        let token = std::env::var("PACENOTES_DB_AUTH_TOKEN").unwrap_or_default();
        libsql::Builder::new_remote(db_url.to_string(), token)
            .build()
            .await
            .map_err(|e| anyhow::anyhow!("connecting to {db_url}: {e}"))?
    } else {
        let path = local_path(db_url);
        if path != std::path::Path::new(":memory:") {
            if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
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

/// `file:./data/auth.db` → `./data/auth.db`; anything else is already a path.
/// Query strings (`?mode=ro`) are not supported and are left alone rather
/// than silently stripped.
fn local_path(db_url: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(db_url.strip_prefix("file:").unwrap_or(db_url))
}

#[cfg(test)]
mod tests {
    #[test]
    fn file_urls_become_paths() {
        assert_eq!(
            super::local_path("file:./data/auth.db"),
            std::path::Path::new("./data/auth.db")
        );
        assert_eq!(
            super::local_path("/var/lib/pacenotes/auth.db"),
            std::path::Path::new("/var/lib/pacenotes/auth.db")
        );
        assert_eq!(
            super::local_path(":memory:"),
            std::path::Path::new(":memory:")
        );
    }

    #[tokio::test]
    async fn migrations_are_idempotent() {
        let connection = super::open(":memory:").await.unwrap();
        // Applying the same schema twice must not fail...
        connection
            .execute_batch(super::MIGRATION_0001)
            .await
            .unwrap();
        // ...and the tables the rest of the server relies on must exist.
        connection
            .execute(
                "INSERT INTO tokens (token_hash, created_at) VALUES ('h', 1)",
                (),
            )
            .await
            .unwrap();
        connection
            .execute(
                "INSERT INTO grants (token_hash, directory) VALUES ('h', 'beta')",
                (),
            )
            .await
            .unwrap();
    }
}
