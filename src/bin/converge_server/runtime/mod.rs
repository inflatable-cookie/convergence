use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;

mod app;
mod identity;
mod listener;
mod shutdown;

use self::app::{build_app_router, build_state, load_repos_into_state};
use self::identity::load_or_bootstrap_identity;
use self::listener::{bind_listener, maybe_write_addr_file};
use self::shutdown::shutdown_signal;

#[derive(Parser)]
#[command(name = "converge-server")]
#[command(about = "Convergence central authority (development)", long_about = None)]
pub(super) struct Args {
    /// Address to listen on
    #[arg(long, default_value = "127.0.0.1:8080")]
    pub(super) addr: SocketAddr,

    /// Write bound address to this file (dev/test convenience)
    #[arg(long)]
    pub(super) addr_file: Option<PathBuf>,

    /// Data directory (future use)
    #[arg(long, default_value = "./converge-data")]
    pub(super) data_dir: PathBuf,

    /// Database URL (future use)
    #[arg(long)]
    pub(super) db_url: Option<String>,

    /// One-time bootstrap bearer token that allows `POST /bootstrap` to create the first admin.
    ///
    /// When set and no admin exists yet, the server will start with no users/tokens and require
    /// bootstrapping before any authenticated endpoints can be used.
    #[arg(long)]
    pub(super) bootstrap_token: Option<String>,

    /// Development user name
    #[arg(long, default_value = "dev")]
    pub(super) dev_user: String,

    /// Development bearer token (bootstrap-only)
    #[arg(long, default_value = "dev")]
    pub(super) dev_token: String,
}

pub(super) async fn run() -> Result<()> {
    let args = Args::parse();
    let _ = args.db_url;
    std::fs::create_dir_all(&args.data_dir)
        .with_context(|| format!("create data dir {}", args.data_dir.display()))?;

    let (users, tokens) = load_or_bootstrap_identity(&args)?;
    let state = build_state(&args, users, tokens);
    load_repos_into_state(&state).await?;

    let app = build_app_router(state);
    let listener = bind_listener(args.addr).await?;
    let local_addr = listener.local_addr().context("read listener local addr")?;
    eprintln!("converge-server listening on {}", local_addr);
    maybe_write_addr_file(args.addr_file.as_ref(), local_addr)?;

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server error")?;

    Ok(())
}
