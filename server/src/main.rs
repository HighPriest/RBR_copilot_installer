//! Pacenotes server — axum + libSQL. See server/CLAUDE.md and TASKS.md.

#[derive(clap::Parser)]
#[command(name = "pacenotes-server")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(clap::Subcommand)]
enum Cmd {
    /// Run the HTTP server.
    Serve {
        #[arg(long, default_value = "127.0.0.1:8080", env = "PACENOTES_LISTEN")]
        listen: String,
        /// Data root: <data>/<game>/packages/<directory>/<name>.zip,
        /// <data>/motd/<lang>.txt, <data>/manifest.json.
        #[arg(long, default_value = "./data", env = "PACENOTES_DATA_DIR")]
        data: String,
        #[arg(long, default_value = "file:./data/auth.db", env = "PACENOTES_DB_URL")]
        db_url: String,
    },
    /// Credential administration (mint/grant/ungrant/revoke/list).
    Token {
        #[arg(long, default_value = "file:./data/auth.db", env = "PACENOTES_DB_URL")]
        db_url: String,
        #[command(subcommand)]
        cmd: TokenCmd,
    },
    /// Package manifest maintenance.
    Manifest {
        #[command(subcommand)]
        cmd: ManifestCmd,
    },
}

/// Every command but `mint` identifies a credential by `<token-or-hash>`:
/// the raw `pcn_…` key, or the hash (full or the short form `token list`
/// prints). The raw key is only ever shown once, by `mint`.
#[derive(clap::Subcommand)]
enum TokenCmd {
    /// Mint a credential and print it ONCE.
    Mint {
        #[arg(long)]
        label: Option<String>,
        /// Directory this credential may list and download. Repeatable.
        /// A credential with no grants is valid; it just sees only `public`.
        #[arg(long = "grant", value_name = "DIRECTORY")]
        grants: Vec<String>,
        #[arg(long)]
        expires_days: Option<i64>,
    },
    /// Add a directory grant.
    Grant {
        token_or_hash: String,
        directory: String,
    },
    /// Remove a directory grant (effective on the next request).
    Ungrant {
        token_or_hash: String,
        directory: String,
    },
    /// Revoke a credential for good.
    Revoke { token_or_hash: String },
    /// Show every credential: label, grants, expiry, revoked.
    List,
}

#[derive(clap::Subcommand)]
enum ManifestCmd {
    /// Rebuild <data>/manifest.json from the zips under
    /// <data>/<game>/packages/<directory>/.
    Gen {
        #[arg(long, default_value = "./data", env = "PACENOTES_DATA_DIR")]
        data: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    match <Cli as clap::Parser>::parse().cmd {
        Cmd::Serve {
            listen,
            data,
            db_url,
        } => serve(listen, data, db_url).await,
        Cmd::Token { db_url, cmd } => token_admin(&db_url, cmd).await,
        Cmd::Manifest {
            cmd: ManifestCmd::Gen { data },
        } => manifest_gen(std::path::Path::new(&data)),
    }
}

async fn serve(listen: String, data: String, db_url: String) -> anyhow::Result<()> {
    tracing::info!(%listen, %data, %db_url, "starting");

    let state = std::sync::Arc::new(pacenotes_server::routes::AppState {
        store: pacenotes_server::store::ManifestStore::new(&data),
        db: pacenotes_server::db::open(&db_url).await?,
    });
    // Load once at startup so a broken manifest shows up in the log
    // immediately instead of on the first request.
    let loaded = state.store.current().packages.len();
    tracing::info!(packages = loaded, "manifest loaded");

    let listener = tokio::net::TcpListener::bind(&listen).await?;
    tracing::info!(addr = %listener.local_addr()?, "listening");
    axum::serve(listener, pacenotes_server::routes::router(state))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}

/// `token …`: credential administration against the auth DB.
async fn token_admin(db_url: &str, cmd: TokenCmd) -> anyhow::Result<()> {
    let connection = pacenotes_server::db::open(db_url).await?;
    match cmd {
        TokenCmd::Mint {
            label,
            grants,
            expires_days,
        } => {
            let token =
                pacenotes_server::admin::mint(&connection, label.as_deref(), &grants, expires_days)
                    .await?;
            // The only time this value exists. Printed to stdout so it can be
            // piped; never logged.
            println!("{token}");
            eprintln!(
                "Store it now — only its hash is kept, so it cannot be shown again.{}",
                if grants.is_empty() {
                    " This credential has no grants: it sees only the public directory."
                } else {
                    ""
                }
            );
        }
        TokenCmd::Grant {
            token_or_hash,
            directory,
        } => {
            let hash =
                pacenotes_server::admin::grant(&connection, &token_or_hash, &directory).await?;
            println!("granted {directory} to {}", &hash[..12]);
        }
        TokenCmd::Ungrant {
            token_or_hash,
            directory,
        } => {
            let hash =
                pacenotes_server::admin::ungrant(&connection, &token_or_hash, &directory).await?;
            println!("removed {directory} from {}", &hash[..12]);
        }
        TokenCmd::Revoke { token_or_hash } => {
            let hash = pacenotes_server::admin::revoke(&connection, &token_or_hash).await?;
            println!("revoked {}", &hash[..12]);
        }
        TokenCmd::List => {
            let credentials = pacenotes_server::admin::list(&connection).await?;
            if credentials.is_empty() {
                println!("no credentials");
            }
            for credential in &credentials {
                println!(
                    "{}  {:<20} grants: {:<40} expires: {:<12} {}",
                    credential.short_hash(),
                    credential.label.as_deref().unwrap_or("-"),
                    if credential.grants.is_empty() {
                        "-".to_string()
                    } else {
                        credential.grants.join(",")
                    },
                    credential
                        .expires_at
                        .map(|e| e.to_string())
                        .unwrap_or_else(|| "never".to_string()),
                    if credential.revoked { "REVOKED" } else { "" }
                );
            }
        }
    }
    Ok(())
}

/// `manifest gen`: scan the data root and rewrite manifest.json.
/// Publishing flow = drop a zip into <game>/packages/<directory>/… → run
/// this.
fn manifest_gen(data: &std::path::Path) -> anyhow::Result<()> {
    let generated = pacenotes_server::store::ManifestStore::generate(data)?;
    let manifest = generated.manifest;
    let path = data.join("manifest.json");
    let json = serde_json::to_string_pretty(&manifest)?;
    std::fs::write(&path, format!("{json}\n"))
        .map_err(|e| anyhow::anyhow!("writing {}: {e}", path.display()))?;

    for p in &manifest.packages {
        println!(
            "{} {} [{}] ({} bytes)",
            p.download_path(),
            p.version,
            p.category(),
            p.size
        );
    }
    for warning in &generated.warnings {
        eprintln!("warning: {warning}");
    }
    println!(
        "wrote {} ({} package(s))",
        path.display(),
        manifest.packages.len()
    );
    Ok(())
}
