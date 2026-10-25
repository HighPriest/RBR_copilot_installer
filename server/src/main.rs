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
        /// Publish although values the current manifest carries would
        /// disappear; without it, `gen` refuses and names them.
        #[arg(long)]
        accept_metadata_loss: bool,
        /// Publish although a game being served would vanish because its
        /// schema.toml is missing or broken; without it, `gen` refuses.
        #[arg(long)]
        accept_unpublish: bool,
    },
    /// Copy values that live only in manifest.json into sidecars, so a
    /// regeneration keeps them.
    MigrateToSidecars {
        #[arg(long, default_value = "./data", env = "PACENOTES_DATA_DIR")]
        data: String,
    },
}

/// How long a stop waits for in-flight requests, since a slow 40 MB download
/// would otherwise outlast podman's stop timeout and end in SIGKILL anyway.
const DRAIN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

/// `manifest gen` wrote the manifest but withheld something; 1 means it could not run.
const EXIT_PUBLISHED_WITH_ERRORS: u8 = 2;

#[tokio::main]
async fn main() -> anyhow::Result<std::process::ExitCode> {
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
        } => serve(listen, data, db_url)
            .await
            .map(|()| std::process::ExitCode::SUCCESS),
        Command::Token { db_url, command } => administer_tokens(&db_url, command)
            .await
            .map(|()| std::process::ExitCode::SUCCESS),
        Command::Manifest {
            command:
                ManifestCommand::Gen {
                    data,
                    accept_metadata_loss,
                    accept_unpublish,
                },
        } => generate_manifest(
            std::path::Path::new(&data),
            accept_metadata_loss,
            accept_unpublish,
        ),
        Command::Manifest {
            command: ManifestCommand::MigrateToSidecars { data },
        } => migrate_to_sidecars(std::path::Path::new(&data)),
    }
}

/// Ctrl-C or SIGTERM, which `podman stop` sends and which the kernel drops for
/// a container's PID 1 that does not handle it.
async fn shutdown_signal() {
    let interrupt = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(e) => {
                tracing::warn!(error = %e, "cannot listen for SIGTERM; ctrl-c only");
                std::future::pending::<()>().await;
            }
        }
    };
    // The installer's tests run the server on Windows, which has no SIGTERM.
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = interrupt => tracing::info!("interrupt received, shutting down"),
        () = terminate => tracing::info!("SIGTERM received, shutting down"),
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
    // The same signal starts the drain and a clock that cuts off stragglers.
    let (signalled, mut on_signal) = tokio::sync::watch::channel(false);
    let server = std::future::IntoFuture::into_future(
        axum::serve(listener, pacenotes_server::routes::router(state)).with_graceful_shutdown(
            async move {
                shutdown_signal().await;
                let _ = signalled.send(true);
            },
        ),
    );
    tokio::pin!(server);
    tokio::select! {
        result = &mut server => result?,
        () = async {
            let _ = on_signal.wait_for(|fired| *fired).await;
            tokio::time::sleep(DRAIN_TIMEOUT).await;
        } => tracing::warn!(
            timeout_secs = DRAIN_TIMEOUT.as_secs(),
            "drain timed out; closing the remaining connections"
        ),
    }
    tracing::info!("stopped");
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

/// Exit 1 without writing anything when a live game or metadata would vanish
/// unannounced and the matching `accept_*` flag is not set.
fn generate_manifest(
    data: &std::path::Path,
    accept_metadata_loss: bool,
    accept_unpublish: bool,
) -> anyhow::Result<std::process::ExitCode> {
    let generated = pacenotes_server::store::generate(data)?;
    let manifest = &generated.manifest;
    let current = current_manifest(data);
    let vanishing = pacenotes_server::store::unpublished_games(&current, manifest);
    if !vanishing.is_empty() && !accept_unpublish {
        for error in &generated.errors {
            eprintln!("ERROR: {error}");
        }
        eprintln!(
            "REFUSING to publish: these games are live now and would be taken off the \
             server entirely, because their schema.toml is missing or broken:"
        );
        for game in &vanishing {
            eprintln!(
                "  {game}  (fix: cp schemas/{game}.toml <data>/{game}/{})",
                pacenotes_shared::SCHEMA_FILE
            );
        }
        eprintln!(
            "Nothing was written; the current manifest keeps being served. Re-run with \
             --accept-unpublish if removing the game is intended."
        );
        return Ok(std::process::ExitCode::FAILURE);
    }

    let losses = pacenotes_server::store::metadata_losses(&current, manifest);
    if !losses.is_empty() && !accept_metadata_loss {
        eprintln!("REFUSING to publish: this would drop metadata the current manifest carries:");
        for loss in &losses {
            eprintln!("  {loss}");
        }
        eprintln!(
            "Nothing was written. If these values should stay, run \
             `manifest migrate-to-sidecars` first. If removing them is \
             intended, re-run with --accept-metadata-loss."
        );
        return Ok(std::process::ExitCode::FAILURE);
    }
    for loss in &losses {
        eprintln!("warning: dropping {loss} (--accept-metadata-loss)");
    }
    let path = pacenotes_server::store::write_manifest(data, manifest)?;

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
    for error in &generated.errors {
        eprintln!("ERROR: {error}");
    }
    println!(
        "wrote {} ({} package(s))",
        path.display(),
        manifest.packages.len()
    );
    if generated.errors.is_empty() {
        return Ok(std::process::ExitCode::SUCCESS);
    }
    eprintln!(
        "{} problem(s) kept something from being published — see ERROR lines above",
        generated.errors.len()
    );
    Ok(std::process::ExitCode::from(EXIT_PUBLISHED_WITH_ERRORS))
}

/// The manifest being served, which a publish is checked against; an
/// unreadable one leaves nothing to compare.
fn current_manifest(data: &std::path::Path) -> pacenotes_shared::Manifest {
    let path = data.join(pacenotes_server::store::MANIFEST_FILE);
    pacenotes_server::store::read_manifest(&path).unwrap_or_else(|e| {
        eprintln!("warning: the current manifest does not load ({e}); nothing to compare");
        pacenotes_shared::Manifest::default()
    })
}

/// Exit 2 when some packages could not be migrated.
fn migrate_to_sidecars(data: &std::path::Path) -> anyhow::Result<std::process::ExitCode> {
    let migration = pacenotes_server::store::migrate_to_sidecars(data)?;
    for line in &migration.written {
        println!("wrote {line}");
    }
    for error in &migration.errors {
        eprintln!("ERROR: {error}");
    }
    if migration.written.is_empty() && migration.errors.is_empty() {
        println!("nothing to migrate: every value is already in a sidecar");
    }
    Ok(if migration.errors.is_empty() {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::from(EXIT_PUBLISHED_WITH_ERRORS)
    })
}
