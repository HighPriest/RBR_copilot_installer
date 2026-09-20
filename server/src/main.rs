//! Pacenotes server — axum + libSQL. See server/CLAUDE.md and TASKS.md.

mod auth;
mod error;
mod routes;
mod store;

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
        #[arg(long, default_value = "./packages", env = "PACENOTES_PACKAGES_DIR")]
        packages: String,
        #[arg(long, default_value = "file:./data/auth.db", env = "PACENOTES_DB_URL")]
        db_url: String,
    },
    /// Credential administration (mint/grant/ungrant/revoke/list). Task S7.
    Token {
        // TODO(S7): #[command(subcommand)] mint --grant DIR... /
        // grant / ungrant / revoke / list
    },
    /// Package manifest maintenance.
    Manifest {
        #[command(subcommand)]
        cmd: ManifestCmd,
    },
}

#[derive(clap::Subcommand)]
enum ManifestCmd {
    /// Rebuild <dir>/manifest.json from the zips under <dir>/<directory>/.
    Gen {
        #[arg(long, default_value = "./packages", env = "PACENOTES_PACKAGES_DIR")]
        dir: String,
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
            packages,
            db_url,
        } => serve(listen, packages, db_url).await,
        Cmd::Token {} => {
            // TODO(S7)
            anyhow::bail!("token admin: not implemented (task S7)")
        }
        Cmd::Manifest {
            cmd: ManifestCmd::Gen { dir },
        } => manifest_gen(std::path::Path::new(&dir)),
    }
}

async fn serve(listen: String, packages: String, db_url: String) -> anyhow::Result<()> {
    tracing::info!(%listen, %packages, %db_url, "starting");

    // TODO(S5): open libsql, run migrations/0001_init.sql (idempotent).

    let state = std::sync::Arc::new(routes::AppState {
        store: store::ManifestStore::new(&packages),
    });
    // Load once at startup so a broken manifest shows up in the log
    // immediately instead of on the first request.
    let loaded = state.store.current().packages.len();
    tracing::info!(packages = loaded, "manifest loaded");

    let listener = tokio::net::TcpListener::bind(&listen).await?;
    tracing::info!(addr = %listener.local_addr()?, "listening");
    axum::serve(listener, routes::router(state))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}

/// `manifest gen`: scan the packages tree and rewrite manifest.json.
/// Publishing flow = drop a zip into packages/<directory>/ → run this.
fn manifest_gen(dir: &std::path::Path) -> anyhow::Result<()> {
    let generated = store::ManifestStore::generate(dir)?;
    let manifest = generated.manifest;
    let path = dir.join("manifest.json");
    let json = serde_json::to_string_pretty(&manifest)?;
    std::fs::write(&path, format!("{json}\n"))
        .map_err(|e| anyhow::anyhow!("writing {}: {e}", path.display()))?;

    for p in &manifest.packages {
        println!(
            "{}/{} {} [{}/{}] ({} bytes)",
            p.directory, p.name, p.version, p.game, p.category, p.size
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
