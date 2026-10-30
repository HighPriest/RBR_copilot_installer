//! `pacenotes-server`: serve packages, administer credentials, publish the manifest.

#[derive(clap::Parser)]
#[command(name = "pacenotes-server")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand)]
enum Command {
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
        command: TokenCommand,
    },
    /// Package manifest maintenance.
    Manifest {
        #[command(subcommand)]
        command: ManifestCommand,
    },
}

/// Every command but `mint` takes `<token-or-hash>`: the raw `pcn_…` key, or
/// the full or short hash that `token list` prints.
#[derive(clap::Subcommand)]
enum TokenCommand {
    /// Mint a credential and print it ONCE.
    Mint {
        #[arg(long)]
        label: Option<String>,
        /// Directory this credential may list and download. Repeatable; with
        /// none, the credential sees only `public`.
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
enum ManifestCommand {
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

    match <Cli as clap::Parser>::parse().command {
        Command::Serve {
            listen,
            data,
            db_url,
        } => serve(listen, data, db_url).await,
        Command::Token { db_url, command } => administer_tokens(&db_url, command).await,
        Command::Manifest {
            command: ManifestCommand::Gen { data },
        } => generate_manifest(std::path::Path::new(&data)),
    }
}

async fn serve(listen: String, data: String, db_url: String) -> anyhow::Result<()> {
    tracing::info!(%listen, %data, %db_url, "starting");

    let state = std::sync::Arc::new(pacenotes_server::routes::AppState {
        store: pacenotes_server::store::ManifestStore::new(&data),
        db: pacenotes_server::db::open(&db_url).await?,
    });
    let package_count = state.store.current().packages.len();
    tracing::info!(packages = package_count, "manifest loaded");

    let listener = tokio::net::TcpListener::bind(&listen).await?;
    tracing::info!(addr = %listener.local_addr()?, "listening");
    axum::serve(listener, pacenotes_server::routes::router(state))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}

async fn administer_tokens(db_url: &str, command: TokenCommand) -> anyhow::Result<()> {
    let connection = pacenotes_server::db::open(db_url).await?;
    match command {
        TokenCommand::Mint {
            label,
            grants,
            expires_days,
        } => {
            let token =
                pacenotes_server::admin::mint(&connection, label.as_deref(), &grants, expires_days)
                    .await?;
            println!("{token}");
            let grantless_note = if grants.is_empty() {
                " This credential has no grants: it sees only the public directory."
            } else {
                ""
            };
            eprintln!(
                "Store it now — only its hash is kept, so it cannot be shown again.{grantless_note}"
            );
        }
        TokenCommand::Grant {
            token_or_hash,
            directory,
        } => {
            let hash =
                pacenotes_server::admin::grant(&connection, &token_or_hash, &directory).await?;
            println!(
                "granted {directory} to {}",
                pacenotes_server::admin::short_hash(&hash)
            );
        }
        TokenCommand::Ungrant {
            token_or_hash,
            directory,
        } => {
            let hash =
                pacenotes_server::admin::ungrant(&connection, &token_or_hash, &directory).await?;
            println!(
                "removed {directory} from {}",
                pacenotes_server::admin::short_hash(&hash)
            );
        }
        TokenCommand::Revoke { token_or_hash } => {
            let hash = pacenotes_server::admin::revoke(&connection, &token_or_hash).await?;
            println!("revoked {}", pacenotes_server::admin::short_hash(&hash));
        }
        TokenCommand::List => print_credentials(&pacenotes_server::admin::list(&connection).await?),
    }
    Ok(())
}

fn print_credentials(credentials: &[pacenotes_server::admin::Credential]) {
    if credentials.is_empty() {
        println!("no credentials");
    }
    for credential in credentials {
        let grants = if credential.grants.is_empty() {
            "-".to_string()
        } else {
            credential.grants.join(",")
        };
        let expires = credential
            .expires_at
            .map(|deadline| deadline.to_string())
            .unwrap_or_else(|| "never".to_string());
        println!(
            "{}  {:<20} grants: {:<40} expires: {:<12} {}",
            credential.short_hash(),
            credential.label.as_deref().unwrap_or("-"),
            grants,
            expires,
            if credential.revoked { "REVOKED" } else { "" }
        );
    }
}

fn generate_manifest(data: &std::path::Path) -> anyhow::Result<()> {
    let generated = pacenotes_server::store::generate(data)?;
    let manifest = generated.manifest;
    let path = data.join(pacenotes_server::store::MANIFEST_FILE);
    let json = serde_json::to_string_pretty(&manifest)?;
    std::fs::write(&path, format!("{json}\n"))
        .map_err(|e| anyhow::anyhow!("writing {}: {e}", path.display()))?;

    for package in &manifest.packages {
        println!(
            "{} {} [{}] ({} bytes)",
            package.download_path(),
            package.version,
            package.category(),
            package.size
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
