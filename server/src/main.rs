//! Pacenotes server — axum + libSQL. See server/CLAUDE.md and TASKS.md.
//! Skeleton compiles the shape; handlers are filled in tasks S2..S7.

mod auth;
mod store;

use anyhow::Result;
use axum::{routing::get, Router};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "pacenotes-server")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
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
    /// Regenerate packages/manifest.json from zips on disk. Task S3.
    Manifest {
        // TODO(S3): gen --dir
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    match Cli::parse().cmd {
        Cmd::Serve { listen, packages, db_url } => serve(listen, packages, db_url).await,
        Cmd::Token {} => {
            // TODO(S7)
            anyhow::bail!("token admin: not implemented (task S7)")
        }
        Cmd::Manifest {} => {
            // TODO(S3)
            anyhow::bail!("manifest gen: not implemented (task S3)")
        }
    }
}

async fn serve(listen: String, packages: String, db_url: String) -> Result<()> {
    tracing::info!(%listen, %packages, %db_url, "starting");

    // TODO(S5): open libsql, run migrations/0001_init.sql (idempotent).
    // TODO(S3): store::ManifestStore::new(packages) with lazy reload.

    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        // TODO(S3): .route("/packages", get(handlers::list_packages))
        // TODO(S4): .route("/download/{directory}/{name}", get(handlers::download))
        .layer(tower_http::trace::TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(&listen).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
