#![allow(clippy::result_large_err)]

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::{collections::HashMap, collections::HashSet};

use anyhow::{Context, Result};
use axum::extract::{Extension, Query, State};
use axum::http::{StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router, extract::Path};
use clap::Parser;
use tokio::sync::RwLock;

#[path = "converge_server/persistence.rs"]
mod persistence;
use self::persistence::*;
#[path = "converge_server/identity_store.rs"]
mod identity_store;
use self::identity_store::*;
#[path = "converge_server/access.rs"]
mod access;
use self::access::*;
#[path = "converge_server/validators.rs"]
mod validators;
use self::validators::*;
#[path = "converge_server/http_error.rs"]
mod http_error;
use self::http_error::*;
#[path = "converge_server/gate_graph_validation.rs"]
mod gate_graph_validation;
use self::gate_graph_validation::*;
#[path = "converge_server/handlers_identity.rs"]
mod handlers_identity;
use self::handlers_identity::*;
#[path = "converge_server/handlers_system.rs"]
mod handlers_system;
use self::handlers_system::*;
#[path = "converge_server/handlers_repo.rs"]
mod handlers_repo;
use self::handlers_repo::*;
#[path = "converge_server/handlers_gates.rs"]
mod handlers_gates;
use self::handlers_gates::*;
#[path = "converge_server/handlers_objects.rs"]
mod handlers_objects;
use self::handlers_objects::*;
#[path = "converge_server/handlers_publications.rs"]
mod handlers_publications;
use self::handlers_publications::*;
#[path = "converge_server/handlers_release.rs"]
mod handlers_release;
use self::handlers_release::*;
#[path = "converge_server/handlers_gc.rs"]
mod handlers_gc;
use self::handlers_gc::*;
#[path = "converge_server/object_graph/mod.rs"]
mod object_graph;
use self::object_graph::*;
#[path = "converge_server/routes.rs"]
mod routes;
use self::routes::*;

#[derive(Clone, Debug)]
struct Subject {
    user_id: String,
    user: String,

    #[allow(dead_code)]
    admin: bool,
}

#[derive(Clone)]
struct AppState {
    // Used only for best-effort defaults when hydrating old on-disk repos.
    default_user: String,

    data_dir: PathBuf,

    repos: Arc<RwLock<HashMap<String, Repo>>>,

    users: Arc<RwLock<HashMap<String, User>>>,
    tokens: Arc<RwLock<HashMap<String, AccessToken>>>,
    token_hash_index: Arc<RwLock<HashMap<String, String>>>,

    // Optional one-time bootstrap token (hash) used to create the first admin.
    // Enabled only when the server is started with `--bootstrap-token`.
    bootstrap_token_hash: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct User {
    id: String,
    handle: String,

    #[serde(default)]
    display_name: Option<String>,

    #[serde(default)]
    admin: bool,

    created_at: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct AccessToken {
    id: String,
    user_id: String,

    // Stored hash of the bearer token secret.
    token_hash: String,

    #[serde(default)]
    label: Option<String>,

    created_at: String,

    #[serde(default)]
    last_used_at: Option<String>,

    #[serde(default)]
    revoked_at: Option<String>,

    #[serde(default)]
    expires_at: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct Repo {
    id: String,
    owner: String,

    #[serde(default)]
    owner_user_id: Option<String>,

    readers: HashSet<String>,

    #[serde(default)]
    reader_user_ids: HashSet<String>,

    publishers: HashSet<String>,

    #[serde(default)]
    publisher_user_ids: HashSet<String>,

    lanes: HashMap<String, Lane>,

    gate_graph: GateGraph,
    scopes: HashSet<String>,

    snaps: HashSet<String>,
    publications: Vec<Publication>,

    bundles: Vec<Bundle>,

    #[serde(default)]
    pinned_bundles: HashSet<String>,

    promotions: Vec<Promotion>,
    promotion_state: HashMap<String, HashMap<String, String>>,

    #[serde(default)]
    releases: Vec<Release>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct Gate {
    id: String,
    name: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct GateGraph {
    version: u32,
    gates: Vec<GateDef>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct GateDef {
    id: String,
    name: String,
    upstream: Vec<String>,

    #[serde(default = "default_true")]
    allow_releases: bool,

    #[serde(default)]
    allow_superpositions: bool,

    #[serde(default)]
    allow_metadata_only_publications: bool,

    #[serde(default)]
    required_approvals: u32,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct Publication {
    id: String,
    snap_id: String,
    scope: String,
    gate: String,
    publisher: String,

    #[serde(default)]
    publisher_user_id: Option<String>,
    created_at: String,

    #[serde(default)]
    resolution: Option<PublicationResolution>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct PublicationResolution {
    bundle_id: String,
    root_manifest: String,
    resolved_root_manifest: String,
    created_at: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct Bundle {
    id: String,
    scope: String,
    gate: String,
    root_manifest: String,
    input_publications: Vec<String>,
    created_by: String,

    #[serde(default)]
    created_by_user_id: Option<String>,
    created_at: String,

    promotable: bool,
    reasons: Vec<String>,

    #[serde(default)]
    approvals: Vec<String>,

    #[serde(default)]
    approval_user_ids: Vec<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct Promotion {
    id: String,
    bundle_id: String,
    scope: String,
    from_gate: String,
    to_gate: String,
    promoted_by: String,

    #[serde(default)]
    promoted_by_user_id: Option<String>,
    promoted_at: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct Release {
    id: String,
    channel: String,
    bundle_id: String,
    scope: String,
    gate: String,

    released_by: String,

    #[serde(default)]
    released_by_user_id: Option<String>,

    released_at: String,

    #[serde(default)]
    notes: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct Lane {
    id: String,
    members: HashSet<String>,

    #[serde(default)]
    member_user_ids: HashSet<String>,

    #[serde(default)]
    heads: HashMap<String, LaneHead>,

    // Retention roots for unpublished collaboration. We keep a bounded history of head
    // updates so the server can GC aggressively without losing recent WIP context.
    #[serde(default)]
    head_history: HashMap<String, Vec<LaneHead>>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct LaneHead {
    snap_id: String,
    updated_at: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    client_id: Option<String>,
}

const LANE_HEAD_HISTORY_KEEP_LAST: usize = 5;

fn default_true() -> bool {
    true
}

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

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("{:#}", err);
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let args = Args::parse();
    let _ = args.db_url;
    std::fs::create_dir_all(&args.data_dir)
        .with_context(|| format!("create data dir {}", args.data_dir.display()))?;

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
        data_dir: args.data_dir,
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

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
