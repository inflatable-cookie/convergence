use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::Router;
use axum::routing::{get, post};
use clap::Parser;
use tokio::sync::RwLock;

use super::handlers_system::{bootstrap, healthz};
use super::identity_store::{
    bootstrap_identity, hash_token, load_identity_from_disk, persist_identity_to_disk,
};
use super::persistence::load_repos_from_disk;
use super::routes::authed_router;
use super::types::{AccessToken, AppState, User};

#[derive(Parser)]
#[command(name = "converge-server")]
#[command(about = "Convergence central authority (development)", long_about = None)]
struct Args {
    /// Address to listen on
    #[arg(long, default_value = "127.0.0.1:8080")]
    addr: SocketAddr,

    /// Write bound address to this file (dev/test convenience)
    #[arg(long)]
    addr_file: Option<PathBuf>,

    /// Data directory (future use)
    #[arg(long, default_value = "./converge-data")]
    data_dir: PathBuf,

    /// Database URL (future use)
    #[arg(long)]
    db_url: Option<String>,

    /// One-time bootstrap bearer token that allows `POST /bootstrap` to create the first admin.
    ///
    /// When set and no admin exists yet, the server will start with no users/tokens and require
    /// bootstrapping before any authenticated endpoints can be used.
    #[arg(long)]
    bootstrap_token: Option<String>,

    /// Development user name
    #[arg(long, default_value = "dev")]
    dev_user: String,

    /// Development bearer token (bootstrap-only)
    #[arg(long, default_value = "dev")]
    dev_token: String,
}

pub(super) async fn run() -> Result<()> {
    let args = Args::parse();
    let _ = args.db_url;
    std::fs::create_dir_all(&args.data_dir)
        .with_context(|| format!("create data dir {}", args.data_dir.display()))?;

    let (users, tokens) = load_or_bootstrap_identity(&args)?;

    let default_user = users
        .values()
        .find(|u| u.admin)
        .or_else(|| users.values().next())
        .map(|u| u.handle.clone())
        .unwrap_or_else(|| "dev".to_string());

    let handle_to_id: HashMap<String, String> = users
        .values()
        .map(|u| (u.handle.clone(), u.id.clone()))
        .collect();

    let token_hash_index: HashMap<String, String> = tokens
        .values()
        .map(|t| (t.token_hash.clone(), t.id.clone()))
        .collect();

    let state = Arc::new(AppState {
        default_user,
        data_dir: args.data_dir.clone(),
        repos: Arc::new(RwLock::new(HashMap::new())),
        users: Arc::new(RwLock::new(users)),
        tokens: Arc::new(RwLock::new(tokens)),
        token_hash_index: Arc::new(RwLock::new(token_hash_index)),
        bootstrap_token_hash: args.bootstrap_token.as_deref().map(hash_token),
    });

    // Best-effort load repos from disk so the dev server survives restarts.
    let loaded =
        load_repos_from_disk(state.as_ref(), &handle_to_id).context("load repos from disk")?;
    {
        let mut repos = state.repos.write().await;
        *repos = loaded;
    }

    let authed = authed_router(state.clone());
    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/bootstrap", post(bootstrap))
        .merge(authed)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(args.addr)
        .await
        .with_context(|| format!("bind {}", args.addr))?;

    let local_addr = listener.local_addr().context("read listener local addr")?;
    eprintln!("converge-server listening on {}", local_addr);

    if let Some(addr_file) = &args.addr_file {
        std::fs::write(addr_file, local_addr.to_string())
            .with_context(|| format!("write addr file {}", addr_file.display()))?;
    }

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server error")?;

    Ok(())
}

fn load_or_bootstrap_identity(
    args: &Args,
) -> Result<(HashMap<String, User>, HashMap<String, AccessToken>)> {
    let (mut users, mut tokens) =
        load_identity_from_disk(&args.data_dir).context("load identity")?;

    if users.is_empty() || tokens.is_empty() {
        if args.bootstrap_token.is_some() {
            if !(users.is_empty() && tokens.is_empty()) {
                anyhow::bail!(
                    "identity store inconsistent (users/tokens missing); remove {} to re-bootstrap",
                    args.data_dir.display()
                );
            }
        } else {
            let (u, t) = bootstrap_identity(&args.dev_user, &args.dev_token);
            users.insert(u.id.clone(), u);
            tokens.insert(t.id.clone(), t);
            persist_identity_to_disk(&args.data_dir, &users, &tokens)
                .context("persist identity")?;
        }
    }

    Ok((users, tokens))
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
