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
#[path = "converge_server/validators.rs"]
mod validators;
use self::validators::*;
#[path = "converge_server/handlers_identity.rs"]
mod handlers_identity;
use self::handlers_identity::*;
#[path = "converge_server/handlers_repo.rs"]
mod handlers_repo;
use self::handlers_repo::*;
#[path = "converge_server/handlers_gates.rs"]
mod handlers_gates;
use self::handlers_gates::*;
#[path = "converge_server/handlers_objects.rs"]
mod handlers_objects;
use self::handlers_objects::*;

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

fn can_read(repo: &Repo, subject: &Subject) -> bool {
    repo.owner == subject.user
        || repo.readers.contains(&subject.user)
        || repo
            .owner_user_id
            .as_ref()
            .is_some_and(|u| u == &subject.user_id)
        || repo.reader_user_ids.contains(&subject.user_id)
}

fn can_publish(repo: &Repo, subject: &Subject) -> bool {
    repo.owner == subject.user
        || repo.publishers.contains(&subject.user)
        || repo
            .owner_user_id
            .as_ref()
            .is_some_and(|u| u == &subject.user_id)
        || repo.publisher_user_ids.contains(&subject.user_id)
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

    let authed = Router::new()
        .route("/whoami", get(whoami))
        .route("/users", get(list_users).post(create_user))
        .route(
            "/users/:user_id/tokens",
            axum::routing::post(create_token_for_user),
        )
        .route("/tokens", get(list_tokens).post(create_token))
        .route(
            "/tokens/:token_id/revoke",
            axum::routing::post(revoke_token),
        )
        .route(
            "/repos/:repo_id/members",
            get(list_repo_members).post(add_repo_member),
        )
        .route(
            "/repos/:repo_id/members/:handle",
            axum::routing::delete(remove_repo_member),
        )
        .route(
            "/repos/:repo_id/lanes/:lane_id/members",
            get(list_lane_members).post(add_lane_member),
        )
        .route(
            "/repos/:repo_id/lanes/:lane_id/members/:handle",
            axum::routing::delete(remove_lane_member),
        )
        .route("/repos", get(list_repos).post(create_repo))
        .route("/repos/:repo_id", get(get_repo))
        .route("/repos/:repo_id/permissions", get(get_repo_permissions))
        .route("/repos/:repo_id/lanes", get(list_lanes))
        .route(
            "/repos/:repo_id/lanes/:lane_id/heads/me",
            axum::routing::post(update_lane_head_me),
        )
        .route(
            "/repos/:repo_id/lanes/:lane_id/heads/:user",
            get(get_lane_head),
        )
        .route("/repos/:repo_id/gates", get(list_gates))
        .route(
            "/repos/:repo_id/gate-graph",
            get(get_gate_graph).put(put_gate_graph),
        )
        .route(
            "/repos/:repo_id/scopes",
            get(list_scopes).post(create_scope),
        )
        .route(
            "/repos/:repo_id/publications",
            get(list_publications).post(create_publication),
        )
        .route(
            "/repos/:repo_id/bundles",
            get(list_bundles).post(create_bundle),
        )
        .route("/repos/:repo_id/bundles/:bundle_id", get(get_bundle))
        .route(
            "/repos/:repo_id/bundles/:bundle_id/pin",
            axum::routing::post(pin_bundle),
        )
        .route(
            "/repos/:repo_id/bundles/:bundle_id/unpin",
            axum::routing::post(unpin_bundle),
        )
        .route("/repos/:repo_id/pins", get(list_pins))
        .route(
            "/repos/:repo_id/bundles/:bundle_id/approve",
            axum::routing::post(approve_bundle),
        )
        .route(
            "/repos/:repo_id/releases",
            get(list_releases).post(create_release),
        )
        .route(
            "/repos/:repo_id/releases/:channel",
            get(get_release_channel),
        )
        .route(
            "/repos/:repo_id/promotions",
            get(list_promotions).post(create_promotion),
        )
        .route("/repos/:repo_id/promotion-state", get(get_promotion_state))
        .route(
            "/repos/:repo_id/objects/blobs/:blob_id",
            axum::routing::put(put_blob).get(get_blob),
        )
        .route(
            "/repos/:repo_id/objects/manifests/:manifest_id",
            axum::routing::put(put_manifest).get(get_manifest),
        )
        .route(
            "/repos/:repo_id/objects/recipes/:recipe_id",
            axum::routing::put(put_recipe).get(get_recipe),
        )
        .route(
            "/repos/:repo_id/objects/snaps/:snap_id",
            axum::routing::put(put_snap).get(get_snap),
        )
        .route(
            "/repos/:repo_id/objects/missing",
            axum::routing::post(find_missing_objects),
        )
        .route("/repos/:repo_id/gc", axum::routing::post(gc_repo))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            require_bearer,
        ));

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

async fn require_bearer(
    State(state): State<Arc<AppState>>,
    req: axum::extract::Request,
    next: Next,
) -> Response {
    let Some(value) = req.headers().get(header::AUTHORIZATION) else {
        return unauthorized();
    };

    let Ok(value) = value.to_str() else {
        return unauthorized();
    };

    let Some(token) = value.strip_prefix("Bearer ") else {
        return unauthorized();
    };

    let token_hash = hash_token(token);

    let token_id = {
        let idx = state.token_hash_index.read().await;
        idx.get(&token_hash).cloned()
    };
    let Some(token_id) = token_id else {
        return unauthorized();
    };

    let (user_id, handle, admin) = {
        let tokens = state.tokens.read().await;
        let Some(t) = tokens.get(&token_id) else {
            return unauthorized();
        };
        if t.revoked_at.is_some() {
            return unauthorized();
        }
        if let Some(exp) = &t.expires_at
            && let Ok(exp) =
                time::OffsetDateTime::parse(exp, &time::format_description::well_known::Rfc3339)
            && time::OffsetDateTime::now_utc() > exp
        {
            return unauthorized();
        }

        let users = state.users.read().await;
        let Some(u) = users.get(&t.user_id) else {
            return unauthorized();
        };
        (u.id.clone(), u.handle.clone(), u.admin)
    };

    // Best-effort last_used tracking (in-memory only).
    {
        let mut tokens = state.tokens.write().await;
        if let Some(t) = tokens.get_mut(&token_id) {
            t.last_used_at = Some(now_ts());
        }
    }

    let mut req = req;
    req.extensions_mut().insert(Subject {
        user_id,
        user: handle,
        admin,
    });
    next.run(req).await
}

fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({"error": "unauthorized"})),
    )
        .into_response()
}

async fn healthz() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok"}))
}

#[derive(Debug, serde::Deserialize)]
struct BootstrapRequest {
    handle: String,

    #[serde(default)]
    display_name: Option<String>,
}

#[derive(Debug, serde::Serialize)]
struct BootstrapResponse {
    user: User,
    token: CreateTokenResponse,
}

async fn bootstrap(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<BootstrapRequest>,
) -> Result<Json<BootstrapResponse>, Response> {
    let Some(expected_hash) = state.bootstrap_token_hash.as_deref() else {
        return Err(not_found());
    };

    let Some(value) = headers.get(header::AUTHORIZATION) else {
        return Err(unauthorized());
    };
    let Ok(value) = value.to_str() else {
        return Err(unauthorized());
    };
    let Some(token) = value.strip_prefix("Bearer ") else {
        return Err(unauthorized());
    };
    if hash_token(token) != expected_hash {
        return Err(unauthorized());
    }

    validate_user_handle(&payload.handle).map_err(bad_request)?;
    let created_at = now_ts();
    let user_id = generate_token_secret().map_err(internal_error)?;

    let user = User {
        id: user_id.clone(),
        handle: payload.handle.clone(),
        display_name: payload.display_name,
        admin: true,
        created_at: created_at.clone(),
    };

    // Enforce one-time semantics per data_dir: only allow bootstrapping if no admin exists.
    {
        let users = state.users.read().await;
        if users.values().any(|u| u.admin) {
            return Err(conflict("already bootstrapped"));
        }
    }

    {
        let mut users = state.users.write().await;
        if users.values().any(|u| u.handle == payload.handle) {
            return Err(conflict("user handle already exists"));
        }
        // Re-check under write lock.
        if users.values().any(|u| u.admin) {
            return Err(conflict("already bootstrapped"));
        }
        users.insert(user_id.clone(), user.clone());
    }

    let token_secret = generate_token_secret().map_err(internal_error)?;
    let token_hash = hash_token(&token_secret);
    let token_id = {
        let mut h = blake3::Hasher::new();
        h.update(user_id.as_bytes());
        h.update(b"\n");
        h.update(token_hash.as_bytes());
        h.update(b"\n");
        h.update(created_at.as_bytes());
        h.finalize().to_hex().to_string()
    };

    {
        let mut tokens = state.tokens.write().await;
        tokens.insert(
            token_id.clone(),
            AccessToken {
                id: token_id.clone(),
                user_id: user_id.clone(),
                token_hash: token_hash.clone(),
                label: Some("bootstrap".to_string()),
                created_at: created_at.clone(),
                last_used_at: None,
                revoked_at: None,
                expires_at: None,
            },
        );
    }
    {
        let mut idx = state.token_hash_index.write().await;
        idx.insert(token_hash, token_id.clone());
    }

    {
        let users = state.users.read().await;
        let tokens = state.tokens.read().await;
        if let Err(err) = persist_identity_to_disk(&state.data_dir, &users, &tokens) {
            return Err(internal_error(err));
        }
    }

    Ok(Json(BootstrapResponse {
        user,
        token: CreateTokenResponse {
            id: token_id,
            token: token_secret,
            created_at,
        },
    }))
}

fn validate_manifest_entry_refs(
    state: &AppState,
    repo_id: &str,
    kind: &converge::model::ManifestEntryKind,
    allow_missing_blobs: bool,
) -> Result<(), Response> {
    match kind {
        converge::model::ManifestEntryKind::File { blob, .. } => {
            validate_object_id(blob.as_str()).map_err(bad_request)?;
            if !allow_missing_blobs {
                let p = repo_data_dir(state, repo_id)
                    .join("objects/blobs")
                    .join(blob.as_str());
                if !p.exists() {
                    return Err(bad_request(anyhow::anyhow!(
                        "missing referenced blob {}",
                        blob.as_str()
                    )));
                }
            }
        }
        converge::model::ManifestEntryKind::FileChunks { recipe, .. } => {
            validate_object_id(recipe.as_str()).map_err(bad_request)?;
            let p = repo_data_dir(state, repo_id)
                .join("objects/recipes")
                .join(format!("{}.json", recipe.as_str()));
            if !p.exists() {
                return Err(bad_request(anyhow::anyhow!(
                    "missing referenced recipe {}",
                    recipe.as_str()
                )));
            }
        }
        converge::model::ManifestEntryKind::Dir { manifest } => {
            validate_object_id(manifest.as_str()).map_err(bad_request)?;
            let p = repo_data_dir(state, repo_id)
                .join("objects/manifests")
                .join(format!("{}.json", manifest.as_str()));
            if !p.exists() {
                return Err(bad_request(anyhow::anyhow!(
                    "missing referenced manifest {}",
                    manifest.as_str()
                )));
            }
        }
        converge::model::ManifestEntryKind::Symlink { .. } => {}
        converge::model::ManifestEntryKind::Superposition { variants } => {
            for v in variants {
                match &v.kind {
                    converge::model::SuperpositionVariantKind::File { blob, .. } => {
                        validate_object_id(blob.as_str()).map_err(bad_request)?;
                        if !allow_missing_blobs {
                            let p = repo_data_dir(state, repo_id)
                                .join("objects/blobs")
                                .join(blob.as_str());
                            if !p.exists() {
                                return Err(bad_request(anyhow::anyhow!(
                                    "missing referenced blob {}",
                                    blob.as_str()
                                )));
                            }
                        }
                    }
                    converge::model::SuperpositionVariantKind::FileChunks { recipe, .. } => {
                        validate_object_id(recipe.as_str()).map_err(bad_request)?;
                        let p = repo_data_dir(state, repo_id)
                            .join("objects/recipes")
                            .join(format!("{}.json", recipe.as_str()));
                        if !p.exists() {
                            return Err(bad_request(anyhow::anyhow!(
                                "missing referenced recipe {}",
                                recipe.as_str()
                            )));
                        }
                    }
                    converge::model::SuperpositionVariantKind::Dir { manifest } => {
                        validate_object_id(manifest.as_str()).map_err(bad_request)?;
                        let p = repo_data_dir(state, repo_id)
                            .join("objects/manifests")
                            .join(format!("{}.json", manifest.as_str()));
                        if !p.exists() {
                            return Err(bad_request(anyhow::anyhow!(
                                "missing referenced manifest {}",
                                manifest.as_str()
                            )));
                        }
                    }
                    converge::model::SuperpositionVariantKind::Symlink { .. } => {}
                    converge::model::SuperpositionVariantKind::Tombstone => {}
                }
            }
        }
    }
    Ok(())
}

fn read_recipe(
    state: &AppState,
    repo_id: &str,
    recipe_id: &str,
) -> Result<converge::model::FileRecipe, Response> {
    validate_object_id(recipe_id).map_err(bad_request)?;
    let path = repo_data_dir(state, repo_id)
        .join("objects/recipes")
        .join(format!("{}.json", recipe_id));
    if !path.exists() {
        return Err(bad_request(anyhow::anyhow!("unknown recipe")));
    }
    let bytes = std::fs::read(&path)
        .with_context(|| format!("read {}", path.display()))
        .map_err(|e| internal_error(anyhow::anyhow!(e)))?;
    let actual = blake3::hash(&bytes).to_hex().to_string();
    if actual != recipe_id {
        return Err(internal_error(anyhow::anyhow!(
            "recipe integrity check failed"
        )));
    }
    let recipe: converge::model::FileRecipe =
        serde_json::from_slice(&bytes).map_err(|e| internal_error(anyhow::anyhow!(e)))?;
    Ok(recipe)
}

fn collect_objects_from_manifest_tree(
    state: &AppState,
    repo_id: &str,
    root_manifest_id: &str,
    blobs: &mut HashSet<String>,
    manifests: &mut HashSet<String>,
    recipes: &mut HashSet<String>,
) -> Result<(), Response> {
    fn visit_recipe(
        state: &AppState,
        repo_id: &str,
        recipe_id: &str,
        blobs: &mut HashSet<String>,
        recipes: &mut HashSet<String>,
        visited: &mut HashSet<String>,
    ) -> Result<(), Response> {
        if !visited.insert(recipe_id.to_string()) {
            return Ok(());
        }
        recipes.insert(recipe_id.to_string());
        let recipe = read_recipe(state, repo_id, recipe_id)?;
        for c in recipe.chunks {
            blobs.insert(c.blob.as_str().to_string());
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn visit_manifest(
        state: &AppState,
        repo_id: &str,
        manifest_id: &str,
        blobs: &mut HashSet<String>,
        manifests: &mut HashSet<String>,
        recipes: &mut HashSet<String>,
        visited_manifests: &mut HashSet<String>,
        visited_recipes: &mut HashSet<String>,
    ) -> Result<(), Response> {
        if !visited_manifests.insert(manifest_id.to_string()) {
            return Ok(());
        }
        manifests.insert(manifest_id.to_string());

        let manifest = read_manifest(state, repo_id, manifest_id)?;
        for e in manifest.entries {
            match e.kind {
                converge::model::ManifestEntryKind::File { blob, .. } => {
                    blobs.insert(blob.as_str().to_string());
                }
                converge::model::ManifestEntryKind::FileChunks { recipe, .. } => {
                    visit_recipe(
                        state,
                        repo_id,
                        recipe.as_str(),
                        blobs,
                        recipes,
                        visited_recipes,
                    )?;
                }
                converge::model::ManifestEntryKind::Dir { manifest } => {
                    visit_manifest(
                        state,
                        repo_id,
                        manifest.as_str(),
                        blobs,
                        manifests,
                        recipes,
                        visited_manifests,
                        visited_recipes,
                    )?;
                }
                converge::model::ManifestEntryKind::Symlink { .. } => {}
                converge::model::ManifestEntryKind::Superposition { variants } => {
                    for v in variants {
                        match v.kind {
                            converge::model::SuperpositionVariantKind::File { blob, .. } => {
                                blobs.insert(blob.as_str().to_string());
                            }
                            converge::model::SuperpositionVariantKind::FileChunks {
                                recipe,
                                ..
                            } => {
                                visit_recipe(
                                    state,
                                    repo_id,
                                    recipe.as_str(),
                                    blobs,
                                    recipes,
                                    visited_recipes,
                                )?;
                            }
                            converge::model::SuperpositionVariantKind::Dir { manifest } => {
                                visit_manifest(
                                    state,
                                    repo_id,
                                    manifest.as_str(),
                                    blobs,
                                    manifests,
                                    recipes,
                                    visited_manifests,
                                    visited_recipes,
                                )?;
                            }
                            converge::model::SuperpositionVariantKind::Symlink { .. } => {}
                            converge::model::SuperpositionVariantKind::Tombstone => {}
                        }
                    }
                }
            }
        }

        Ok(())
    }

    visit_manifest(
        state,
        repo_id,
        root_manifest_id,
        blobs,
        manifests,
        recipes,
        &mut HashSet::new(),
        &mut HashSet::new(),
    )
}

fn validate_manifest_tree_availability(
    state: &AppState,
    repo_id: &str,
    root_manifest_id: &str,
    require_blobs: bool,
) -> Result<(), Response> {
    fn visit_manifest(
        state: &AppState,
        repo_id: &str,
        manifest_id: &str,
        require_blobs: bool,
        visited: &mut HashSet<String>,
    ) -> Result<(), Response> {
        if !visited.insert(manifest_id.to_string()) {
            return Ok(());
        }

        let manifest = read_manifest(state, repo_id, manifest_id)?;
        for e in manifest.entries {
            match e.kind {
                converge::model::ManifestEntryKind::File { blob, .. } => {
                    validate_object_id(blob.as_str()).map_err(bad_request)?;
                    if require_blobs {
                        let p = repo_data_dir(state, repo_id)
                            .join("objects/blobs")
                            .join(blob.as_str());
                        if !p.exists() {
                            return Err(bad_request(anyhow::anyhow!(
                                "missing referenced blob {}",
                                blob.as_str()
                            )));
                        }
                    }
                }
                converge::model::ManifestEntryKind::FileChunks { recipe, .. } => {
                    let recipe = read_recipe(state, repo_id, recipe.as_str())?;
                    for c in recipe.chunks {
                        validate_object_id(c.blob.as_str()).map_err(bad_request)?;
                        if require_blobs {
                            let p = repo_data_dir(state, repo_id)
                                .join("objects/blobs")
                                .join(c.blob.as_str());
                            if !p.exists() {
                                return Err(bad_request(anyhow::anyhow!(
                                    "missing referenced blob {}",
                                    c.blob.as_str()
                                )));
                            }
                        }
                    }
                }
                converge::model::ManifestEntryKind::Dir { manifest } => {
                    visit_manifest(state, repo_id, manifest.as_str(), require_blobs, visited)?;
                }
                converge::model::ManifestEntryKind::Symlink { .. } => {}
                converge::model::ManifestEntryKind::Superposition { variants } => {
                    for v in variants {
                        match v.kind {
                            converge::model::SuperpositionVariantKind::File { blob, .. } => {
                                validate_object_id(blob.as_str()).map_err(bad_request)?;
                                if require_blobs {
                                    let p = repo_data_dir(state, repo_id)
                                        .join("objects/blobs")
                                        .join(blob.as_str());
                                    if !p.exists() {
                                        return Err(bad_request(anyhow::anyhow!(
                                            "missing referenced blob {}",
                                            blob.as_str()
                                        )));
                                    }
                                }
                            }
                            converge::model::SuperpositionVariantKind::FileChunks {
                                recipe,
                                ..
                            } => {
                                let recipe = read_recipe(state, repo_id, recipe.as_str())?;
                                for c in recipe.chunks {
                                    validate_object_id(c.blob.as_str()).map_err(bad_request)?;
                                    if require_blobs {
                                        let p = repo_data_dir(state, repo_id)
                                            .join("objects/blobs")
                                            .join(c.blob.as_str());
                                        if !p.exists() {
                                            return Err(bad_request(anyhow::anyhow!(
                                                "missing referenced blob {}",
                                                c.blob.as_str()
                                            )));
                                        }
                                    }
                                }
                            }
                            converge::model::SuperpositionVariantKind::Dir { manifest } => {
                                visit_manifest(
                                    state,
                                    repo_id,
                                    manifest.as_str(),
                                    require_blobs,
                                    visited,
                                )?;
                            }
                            converge::model::SuperpositionVariantKind::Symlink { .. } => {}
                            converge::model::SuperpositionVariantKind::Tombstone => {}
                        }
                    }
                }
            }
        }

        Ok(())
    }

    visit_manifest(
        state,
        repo_id,
        root_manifest_id,
        require_blobs,
        &mut HashSet::new(),
    )
}

fn read_snap(
    state: &AppState,
    repo_id: &str,
    snap_id: &str,
) -> Result<converge::model::SnapRecord, Response> {
    validate_object_id(snap_id).map_err(bad_request)?;
    let path = repo_data_dir(state, repo_id)
        .join("objects/snaps")
        .join(format!("{}.json", snap_id));
    if !path.exists() {
        return Err(bad_request(anyhow::anyhow!("unknown snap")));
    }
    let bytes = std::fs::read(&path)
        .with_context(|| format!("read {}", path.display()))
        .map_err(|e| internal_error(anyhow::anyhow!(e)))?;
    let snap: converge::model::SnapRecord =
        serde_json::from_slice(&bytes).map_err(|e| internal_error(anyhow::anyhow!(e)))?;
    Ok(snap)
}

fn read_manifest(
    state: &AppState,
    repo_id: &str,
    manifest_id: &str,
) -> Result<converge::model::Manifest, Response> {
    validate_object_id(manifest_id).map_err(bad_request)?;
    let path = repo_data_dir(state, repo_id)
        .join("objects/manifests")
        .join(format!("{}.json", manifest_id));
    if !path.exists() {
        return Err(bad_request(anyhow::anyhow!("unknown manifest")));
    }
    let bytes = std::fs::read(&path)
        .with_context(|| format!("read {}", path.display()))
        .map_err(|e| internal_error(anyhow::anyhow!(e)))?;
    let actual = blake3::hash(&bytes).to_hex().to_string();
    if actual != manifest_id {
        return Err(internal_error(anyhow::anyhow!(
            "manifest integrity check failed"
        )));
    }
    let manifest: converge::model::Manifest =
        serde_json::from_slice(&bytes).map_err(|e| internal_error(anyhow::anyhow!(e)))?;
    Ok(manifest)
}

fn store_manifest(
    state: &AppState,
    repo_id: &str,
    manifest: &converge::model::Manifest,
) -> Result<String, Response> {
    let bytes = serde_json::to_vec(manifest).map_err(|e| internal_error(anyhow::anyhow!(e)))?;
    let id = blake3::hash(&bytes).to_hex().to_string();
    let path = repo_data_dir(state, repo_id)
        .join("objects/manifests")
        .join(format!("{}.json", id));
    write_if_absent(&path, &bytes).map_err(internal_error)?;
    Ok(id)
}

fn coalesce_root_manifest(
    state: &AppState,
    repo_id: &str,
    inputs: &[(String, String)],
) -> Result<String, Response> {
    // inputs: (publication_id, root_manifest_id)
    let mut inputs = inputs.to_vec();
    inputs.sort_by(|a, b| a.0.cmp(&b.0));
    merge_dir_manifests(state, repo_id, &inputs)
}

fn manifest_has_superpositions(
    state: &AppState,
    repo_id: &str,
    root_manifest_id: &str,
) -> Result<bool, Response> {
    fn inner(
        state: &AppState,
        repo_id: &str,
        manifest_id: &str,
        visited: &mut HashSet<String>,
    ) -> Result<bool, Response> {
        if !visited.insert(manifest_id.to_string()) {
            return Ok(false);
        }

        let manifest = read_manifest(state, repo_id, manifest_id)?;
        for e in manifest.entries {
            match e.kind {
                converge::model::ManifestEntryKind::Superposition { .. } => return Ok(true),
                converge::model::ManifestEntryKind::Dir { manifest } => {
                    if inner(state, repo_id, manifest.as_str(), visited)? {
                        return Ok(true);
                    }
                }
                converge::model::ManifestEntryKind::File { .. } => {}
                converge::model::ManifestEntryKind::FileChunks { .. } => {}
                converge::model::ManifestEntryKind::Symlink { .. } => {}
            }
        }
        Ok(false)
    }

    inner(state, repo_id, root_manifest_id, &mut HashSet::new())
}

fn compute_promotability(
    gate: &GateDef,
    has_superpositions: bool,
    approval_count: usize,
) -> (bool, Vec<String>) {
    let mut reasons = Vec::new();
    if has_superpositions && !gate.allow_superpositions {
        reasons.push("superpositions_present".to_string());
    }
    if approval_count < gate.required_approvals as usize {
        reasons.push("approvals_missing".to_string());
    }
    (reasons.is_empty(), reasons)
}

fn merge_dir_manifests(
    state: &AppState,
    repo_id: &str,
    inputs: &[(String, String)],
) -> Result<String, Response> {
    use std::collections::{BTreeMap, BTreeSet};

    // Load each input directory manifest.
    let mut input_maps: Vec<(String, BTreeMap<String, converge::model::ManifestEntryKind>)> =
        Vec::new();
    for (pub_id, mid) in inputs {
        let m = read_manifest(state, repo_id, mid)?;
        let mut map = BTreeMap::new();
        for e in m.entries {
            map.insert(e.name, e.kind);
        }
        input_maps.push((pub_id.clone(), map));
    }

    // Union of entry names.
    let mut names = BTreeSet::new();
    for (_, map) in &input_maps {
        for k in map.keys() {
            names.insert(k.clone());
        }
    }

    let mut out_entries = Vec::new();
    for name in names {
        let mut kinds: Vec<(String, Option<converge::model::ManifestEntryKind>)> = Vec::new();
        for (pub_id, map) in &input_maps {
            kinds.push((pub_id.clone(), map.get(&name).cloned()));
        }

        let all_present = kinds.iter().all(|(_, k)| k.is_some());
        if all_present {
            let all_dirs = kinds
                .iter()
                .all(|(_, k)| matches!(k, Some(converge::model::ManifestEntryKind::Dir { .. })));
            if all_dirs {
                let child_inputs = kinds
                    .iter()
                    .map(|(pub_id, k)| {
                        let converge::model::ManifestEntryKind::Dir { manifest } =
                            k.clone().unwrap()
                        else {
                            unreachable!();
                        };
                        (pub_id.clone(), manifest.as_str().to_string())
                    })
                    .collect::<Vec<_>>();
                let merged_child = merge_dir_manifests(state, repo_id, &child_inputs)?;
                out_entries.push(converge::model::ManifestEntry {
                    name,
                    kind: converge::model::ManifestEntryKind::Dir {
                        manifest: converge::model::ObjectId(merged_child),
                    },
                });
                continue;
            }

            // If all entry kinds are identical (file/symlink), keep it.
            let first = kinds[0].1.clone().unwrap();
            let identical = kinds.iter().all(|(_, k)| match k.clone().unwrap() {
                converge::model::ManifestEntryKind::File { .. } => k.clone().unwrap() == first,
                converge::model::ManifestEntryKind::FileChunks { .. } => {
                    k.clone().unwrap() == first
                }
                converge::model::ManifestEntryKind::Symlink { .. } => k.clone().unwrap() == first,
                _ => false,
            });
            if identical {
                out_entries.push(converge::model::ManifestEntry { name, kind: first });
                continue;
            }
        }

        // Conflict (or missing in some inputs): create a superposition entry.
        let mut variants = Vec::new();
        for (pub_id, kind) in kinds {
            let vkind = match kind {
                Some(converge::model::ManifestEntryKind::File { blob, mode, size }) => {
                    converge::model::SuperpositionVariantKind::File { blob, mode, size }
                }
                Some(converge::model::ManifestEntryKind::FileChunks { recipe, mode, size }) => {
                    converge::model::SuperpositionVariantKind::FileChunks { recipe, mode, size }
                }
                Some(converge::model::ManifestEntryKind::Dir { manifest }) => {
                    converge::model::SuperpositionVariantKind::Dir { manifest }
                }
                Some(converge::model::ManifestEntryKind::Symlink { target }) => {
                    converge::model::SuperpositionVariantKind::Symlink { target }
                }
                Some(converge::model::ManifestEntryKind::Superposition { variants }) => {
                    // Nested superposition: preserve as a variant to avoid losing information.
                    // Represent it by storing it as a derived manifest under a synthetic dir entry.
                    // For v1, treat as tombstone to force explicit resolution later.
                    let _ = variants;
                    converge::model::SuperpositionVariantKind::Tombstone
                }
                None => converge::model::SuperpositionVariantKind::Tombstone,
            };
            variants.push(converge::model::SuperpositionVariant {
                source: pub_id,
                kind: vkind,
            });
        }

        out_entries.push(converge::model::ManifestEntry {
            name,
            kind: converge::model::ManifestEntryKind::Superposition { variants },
        });
    }

    let merged = converge::model::Manifest {
        version: 1,
        entries: out_entries,
    };

    // Ensure references exist before persisting.
    for e in &merged.entries {
        // Bundles should be constructible even when blob bytes are pending.
        validate_manifest_entry_refs(state, repo_id, &e.kind, true)?;
    }

    store_manifest(state, repo_id, &merged)
}

fn json_bytes(bytes: Vec<u8>) -> Response {
    (
        [(header::CONTENT_TYPE, "application/json")],
        axum::body::Bytes::from(bytes),
    )
        .into_response()
}

fn default_true() -> bool {
    true
}

#[derive(Debug, serde::Deserialize)]
struct GcQuery {
    #[serde(default = "default_true")]
    dry_run: bool,
    #[serde(default = "default_true")]
    prune_metadata: bool,

    /// If set, prune release history by keeping only the latest N releases per channel.
    ///
    /// This affects GC roots: pruned releases stop retaining their referenced bundles/objects.
    #[serde(default)]
    prune_releases_keep_last: Option<usize>,
}

async fn gc_repo(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path(repo_id): Path<String>,
    Query(q): Query<GcQuery>,
) -> Result<Json<serde_json::Value>, Response> {
    let mut repos = state.repos.write().await;
    let repo = repos.get_mut(&repo_id).ok_or_else(not_found)?;
    if !can_publish(repo, &subject) {
        return Err(forbidden());
    }

    if !q.prune_metadata && !q.dry_run {
        return Err(bad_request(anyhow::anyhow!(
            "refusing destructive GC with prune_metadata=false (would create dangling references); use dry_run=true or prune_metadata=true"
        )));
    }

    // Optional release history pruning.
    let releases_before = repo.releases.len();
    let mut pruned_releases_keep_last = 0usize;
    if let Some(keep_last) = q.prune_releases_keep_last {
        if keep_last == 0 {
            return Err(bad_request(anyhow::anyhow!(
                "prune_releases_keep_last must be >= 1"
            )));
        }

        let mut by_channel: HashMap<String, Vec<Release>> = HashMap::new();
        for r in repo.releases.clone() {
            by_channel.entry(r.channel.clone()).or_default().push(r);
        }

        let mut kept: Vec<Release> = Vec::new();
        for (_ch, mut rs) in by_channel {
            rs.sort_by(|a, b| b.released_at.cmp(&a.released_at));
            rs.truncate(keep_last);
            kept.extend(rs);
        }
        kept.sort_by(|a, b| b.released_at.cmp(&a.released_at));
        pruned_releases_keep_last = releases_before.saturating_sub(kept.len());
        repo.releases = kept;
    }

    // Retention roots: pinned bundles, releases, and current promotion-state pointers.
    let mut keep_bundles: HashSet<String> = repo.pinned_bundles.iter().cloned().collect();
    for r in &repo.releases {
        keep_bundles.insert(r.bundle_id.clone());
    }
    for per_scope in repo.promotion_state.values() {
        for bid in per_scope.values() {
            keep_bundles.insert(bid.clone());
        }
    }

    let mut keep_publications: HashSet<String> = HashSet::new();
    let mut keep_snaps: HashSet<String> = HashSet::new();
    let mut keep_blobs: HashSet<String> = HashSet::new();
    let mut keep_manifests: HashSet<String> = HashSet::new();
    let mut keep_recipes: HashSet<String> = HashSet::new();

    let mut bundle_roots: Vec<String> = Vec::new();
    for bid in &keep_bundles {
        let bundle = if let Some(b) = repo.bundles.iter().find(|b| b.id == *bid) {
            b.clone()
        } else {
            load_bundle_from_disk(state.as_ref(), &repo_id, bid)?
        };

        bundle_roots.push(bundle.root_manifest.clone());
        for pid in bundle.input_publications {
            keep_publications.insert(pid);
        }
    }

    for p in &repo.publications {
        if keep_publications.contains(&p.id) {
            keep_snaps.insert(p.snap_id.clone());
        }
    }

    // Lane heads are unpublished collaboration roots.
    for lane in repo.lanes.values() {
        for h in lane.heads.values() {
            keep_snaps.insert(h.snap_id.clone());
        }

        for hist in lane.head_history.values() {
            for h in hist {
                keep_snaps.insert(h.snap_id.clone());
            }
        }
    }

    // Collect objects from kept bundle roots.
    for root in &bundle_roots {
        collect_objects_from_manifest_tree(
            state.as_ref(),
            &repo_id,
            root,
            &mut keep_blobs,
            &mut keep_manifests,
            &mut keep_recipes,
        )?;
    }

    // Collect objects from kept snaps (provenance roots).
    for sid in keep_snaps.clone() {
        let path = repo_data_dir(state.as_ref(), &repo_id)
            .join("objects/snaps")
            .join(format!("{}.json", sid));
        if !path.exists() {
            continue;
        }
        let bytes = std::fs::read(&path)
            .with_context(|| format!("read {}", path.display()))
            .map_err(|e| internal_error(anyhow::anyhow!(e)))?;
        let snap: converge::model::SnapRecord =
            serde_json::from_slice(&bytes).map_err(|e| internal_error(anyhow::anyhow!(e)))?;
        collect_objects_from_manifest_tree(
            state.as_ref(),
            &repo_id,
            snap.root_manifest.as_str(),
            &mut keep_blobs,
            &mut keep_manifests,
            &mut keep_recipes,
        )?;
    }

    fn sweep_ids(
        dir: &std::path::Path,
        ext: Option<&str>,
        keep: &HashSet<String>,
        dry_run: bool,
    ) -> Result<(usize, usize), Response> {
        if !dir.is_dir() {
            return Ok((0, 0));
        }
        let mut deleted = 0;
        let mut kept = 0;
        for entry in std::fs::read_dir(dir)
            .with_context(|| format!("read {}", dir.display()))
            .map_err(|e| internal_error(anyhow::anyhow!(e)))?
        {
            let entry = entry
                .with_context(|| format!("read {} entry", dir.display()))
                .map_err(|e| internal_error(anyhow::anyhow!(e)))?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let id = match ext {
                None => path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string()),
                Some(e) => {
                    if path.extension().and_then(|s| s.to_str()) != Some(e) {
                        continue;
                    }
                    path.file_stem()
                        .and_then(|s| s.to_str())
                        .map(|s| s.to_string())
                }
            };
            let Some(id) = id else {
                continue;
            };
            if id.len() != 64 {
                continue;
            }
            if keep.contains(&id) {
                kept += 1;
                continue;
            }
            deleted += 1;
            if !dry_run {
                std::fs::remove_file(&path)
                    .with_context(|| format!("remove {}", path.display()))
                    .map_err(|e| internal_error(anyhow::anyhow!(e)))?;
            }
        }
        Ok((deleted, kept))
    }

    // Sweep objects.
    let objects_root = repo_data_dir(state.as_ref(), &repo_id).join("objects");
    let (deleted_blobs, kept_blobs_count) =
        sweep_ids(&objects_root.join("blobs"), None, &keep_blobs, q.dry_run)?;
    let (deleted_manifests, kept_manifests_count) = sweep_ids(
        &objects_root.join("manifests"),
        Some("json"),
        &keep_manifests,
        q.dry_run,
    )?;
    let (deleted_recipes, kept_recipes_count) = sweep_ids(
        &objects_root.join("recipes"),
        Some("json"),
        &keep_recipes,
        q.dry_run,
    )?;

    let (deleted_snaps, _kept_snaps_count) = if q.prune_metadata {
        sweep_ids(
            &objects_root.join("snaps"),
            Some("json"),
            &keep_snaps,
            q.dry_run,
        )?
    } else {
        (0, 0)
    };

    let (deleted_bundles, _kept_bundles_count) = if q.prune_metadata {
        sweep_ids(
            &repo_data_dir(state.as_ref(), &repo_id).join("bundles"),
            Some("json"),
            &keep_bundles,
            q.dry_run,
        )?
    } else {
        (0, 0)
    };

    // Sweep releases (metadata).
    let keep_release_ids: HashSet<String> = repo
        .releases
        .iter()
        .filter(|r| keep_bundles.contains(&r.bundle_id))
        .map(|r| r.id.clone())
        .collect();

    let (deleted_releases, kept_releases_count) = if q.prune_metadata {
        sweep_ids(
            &repo_data_dir(state.as_ref(), &repo_id).join("releases"),
            Some("json"),
            &keep_release_ids,
            q.dry_run,
        )?
    } else {
        (0, 0)
    };

    if q.prune_metadata && !q.dry_run {
        repo.bundles.retain(|b| keep_bundles.contains(&b.id));
        repo.pinned_bundles.retain(|b| keep_bundles.contains(b));
        repo.releases
            .retain(|r| keep_bundles.contains(&r.bundle_id));
        repo.publications
            .retain(|p| keep_publications.contains(&p.id));
        repo.snaps = keep_snaps.clone();
        persist_repo(state.as_ref(), repo).map_err(internal_error)?;
    }

    Ok(Json(serde_json::json!({
        "dry_run": q.dry_run,
        "prune_metadata": q.prune_metadata,
        "pruned": {
            "releases_keep_last": pruned_releases_keep_last
        },
        "kept": {
            "bundles": keep_bundles.len(),
            "releases": kept_releases_count,
            "publications": keep_publications.len(),
            "snaps": keep_snaps.len(),
            "blobs": kept_blobs_count,
            "manifests": kept_manifests_count,
            "recipes": kept_recipes_count
        },
        "deleted": {
            "bundles": deleted_bundles,
            "releases": deleted_releases,
            "snaps": deleted_snaps,
            "blobs": deleted_blobs,
            "manifests": deleted_manifests,
            "recipes": deleted_recipes
        }
    })))
}

#[derive(Debug, serde::Deserialize)]
struct MissingObjectsRequest {
    blobs: Vec<String>,
    manifests: Vec<String>,
    recipes: Vec<String>,
    snaps: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
struct MissingObjectsResponse {
    missing_blobs: Vec<String>,
    missing_manifests: Vec<String>,
    missing_recipes: Vec<String>,
    missing_snaps: Vec<String>,
}

async fn find_missing_objects(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path(repo_id): Path<String>,
    Json(req): Json<MissingObjectsRequest>,
) -> Result<Json<MissingObjectsResponse>, Response> {
    {
        let repos = state.repos.read().await;
        let repo = repos.get(&repo_id).ok_or_else(not_found)?;
        if !can_publish(repo, &subject) {
            return Err(forbidden());
        }
    }

    for id in req
        .blobs
        .iter()
        .chain(req.manifests.iter())
        .chain(req.recipes.iter())
        .chain(req.snaps.iter())
    {
        validate_object_id(id).map_err(bad_request)?;
    }

    let root = repo_data_dir(&state, &repo_id).join("objects");

    let mut missing_blobs = Vec::new();
    for id in req.blobs {
        let p = root.join("blobs").join(&id);
        if !p.exists() {
            missing_blobs.push(id);
        }
    }

    let mut missing_manifests = Vec::new();
    for id in req.manifests {
        let p = root.join("manifests").join(format!("{}.json", id));
        if !p.exists() {
            missing_manifests.push(id);
        }
    }

    let mut missing_recipes = Vec::new();
    for id in req.recipes {
        let p = root.join("recipes").join(format!("{}.json", id));
        if !p.exists() {
            missing_recipes.push(id);
        }
    }

    let mut missing_snaps = Vec::new();
    for id in req.snaps {
        let p = root.join("snaps").join(format!("{}.json", id));
        if !p.exists() {
            missing_snaps.push(id);
        }
    }

    Ok(Json(MissingObjectsResponse {
        missing_blobs,
        missing_manifests,
        missing_recipes,
        missing_snaps,
    }))
}

#[derive(Debug, serde::Deserialize)]
struct CreatePublicationRequest {
    snap_id: String,
    scope: String,
    gate: String,

    #[serde(default)]
    metadata_only: bool,

    #[serde(default)]
    resolution: Option<PublicationResolution>,
}

async fn create_publication(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path(repo_id): Path<String>,
    Json(payload): Json<CreatePublicationRequest>,
) -> Result<Json<Publication>, Response> {
    validate_object_id(&payload.snap_id).map_err(bad_request)?;
    validate_scope_id(&payload.scope).map_err(bad_request)?;
    validate_gate_id(&payload.gate).map_err(bad_request)?;

    let created_at = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|e| internal_error(anyhow::anyhow!(e)))?;

    let id = {
        let mut hasher = blake3::Hasher::new();
        hasher.update(repo_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(payload.snap_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(payload.scope.as_bytes());
        hasher.update(b"\n");
        hasher.update(payload.gate.as_bytes());
        hasher.update(b"\n");
        hasher.update(subject.user.as_bytes());
        hasher.update(b"\n");
        hasher.update(created_at.as_bytes());
        hasher.finalize().to_hex().to_string()
    };

    let mut repos = state.repos.write().await;
    let repo = repos.get_mut(&repo_id).ok_or_else(not_found)?;
    if !can_publish(repo, &subject) {
        return Err(forbidden());
    }
    if !repo.scopes.contains(&payload.scope) {
        return Err(bad_request(anyhow::anyhow!("unknown scope")));
    }
    if !repo.gate_graph.gates.iter().any(|g| g.id == payload.gate) {
        return Err(bad_request(anyhow::anyhow!("unknown gate")));
    }

    // Enforce at-most-once publication for a given snap+scope+gate.
    // If you need to publish again, create a new snap.
    if repo
        .publications
        .iter()
        .any(|p| p.snap_id == payload.snap_id && p.scope == payload.scope && p.gate == payload.gate)
    {
        return Err(conflict("snap already published to this scope/gate"));
    }

    let gate_def = repo
        .gate_graph
        .gates
        .iter()
        .find(|g| g.id == payload.gate)
        .ok_or_else(|| bad_request(anyhow::anyhow!("unknown gate")))?;
    if payload.metadata_only && !gate_def.allow_metadata_only_publications {
        return Err(bad_request(anyhow::anyhow!(
            "metadata-only publications not allowed in this gate"
        )));
    }
    if !repo.snaps.contains(&payload.snap_id) {
        return Err(bad_request(anyhow::anyhow!(
            "unknown snap (upload snap first)"
        )));
    }

    // For non-metadata-only publications, require full availability of referenced objects.
    // For metadata-only publications, we still require the manifest structure to be present
    // (snaps/manifests/recipes), but allow blob bytes to be pending.
    let snap = read_snap(state.as_ref(), &repo_id, &payload.snap_id)?;
    validate_manifest_tree_availability(
        state.as_ref(),
        &repo_id,
        snap.root_manifest.as_str(),
        !payload.metadata_only,
    )?;

    let pubrec = Publication {
        id,
        snap_id: payload.snap_id,
        scope: payload.scope,
        gate: payload.gate,
        publisher: subject.user,
        publisher_user_id: Some(subject.user_id),
        created_at,
        resolution: payload.resolution,
    };
    repo.publications.push(pubrec.clone());

    persist_repo(state.as_ref(), repo).map_err(internal_error)?;

    Ok(Json(pubrec))
}

async fn list_publications(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path(repo_id): Path<String>,
) -> Result<Json<Vec<Publication>>, Response> {
    let repos = state.repos.read().await;
    let repo = repos.get(&repo_id).ok_or_else(not_found)?;
    if !can_read(repo, &subject) {
        return Err(forbidden());
    }
    Ok(Json(repo.publications.clone()))
}

#[derive(Debug, serde::Deserialize)]
struct CreateBundleRequest {
    scope: String,
    gate: String,
    input_publications: Vec<String>,
}

async fn create_bundle(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path(repo_id): Path<String>,
    Json(payload): Json<CreateBundleRequest>,
) -> Result<Json<Bundle>, Response> {
    validate_scope_id(&payload.scope).map_err(bad_request)?;
    validate_gate_id(&payload.gate).map_err(bad_request)?;
    if payload.input_publications.is_empty() {
        return Err(bad_request(anyhow::anyhow!(
            "bundle must include at least one input publication"
        )));
    }
    for pid in &payload.input_publications {
        validate_object_id(pid).map_err(bad_request)?;
    }

    let created_at = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|e| internal_error(anyhow::anyhow!(e)))?;

    let mut input_publications = payload.input_publications;
    input_publications.sort();
    input_publications.dedup();

    let mut repos = state.repos.write().await;
    let repo = repos.get_mut(&repo_id).ok_or_else(not_found)?;
    if !can_publish(repo, &subject) {
        return Err(forbidden());
    }
    if !repo.scopes.contains(&payload.scope) {
        return Err(bad_request(anyhow::anyhow!("unknown scope")));
    }
    if !repo.gate_graph.gates.iter().any(|g| g.id == payload.gate) {
        return Err(bad_request(anyhow::anyhow!("unknown gate")));
    }

    // Resolve and validate publication ids; gather input snap roots.
    let mut input_roots: Vec<(String, String)> = Vec::new();
    for pid in &input_publications {
        let Some(p) = repo.publications.iter().find(|p| &p.id == pid) else {
            return Err(bad_request(anyhow::anyhow!("unknown publication {}", pid)));
        };
        if p.scope != payload.scope {
            return Err(bad_request(anyhow::anyhow!(
                "publication {} has mismatched scope",
                pid
            )));
        }
        if p.gate != payload.gate {
            return Err(bad_request(anyhow::anyhow!(
                "publication {} has mismatched gate",
                pid
            )));
        }

        let snap = read_snap(&state, &repo_id, &p.snap_id)?;
        input_roots.push((pid.clone(), snap.root_manifest.as_str().to_string()));
    }

    // Derive a new root manifest by coalescing input snap trees.
    let root_manifest = coalesce_root_manifest(&state, &repo_id, &input_roots)?;

    let gate_def = repo
        .gate_graph
        .gates
        .iter()
        .find(|g| g.id == payload.gate)
        .ok_or_else(|| bad_request(anyhow::anyhow!("unknown gate")))?;

    let has_superpositions = manifest_has_superpositions(&state, &repo_id, &root_manifest)?;
    let (promotable, reasons) = compute_promotability(gate_def, has_superpositions, 0);

    let id = {
        let mut hasher = blake3::Hasher::new();
        hasher.update(repo_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(payload.scope.as_bytes());
        hasher.update(b"\n");
        hasher.update(payload.gate.as_bytes());
        hasher.update(b"\n");
        hasher.update(root_manifest.as_bytes());
        hasher.update(b"\n");
        for pid in &input_publications {
            hasher.update(pid.as_bytes());
            hasher.update(b"\n");
        }
        hasher.update(subject.user.as_bytes());
        hasher.update(b"\n");
        hasher.update(created_at.as_bytes());
        hasher.finalize().to_hex().to_string()
    };

    let bundle = Bundle {
        id: id.clone(),
        scope: payload.scope,
        gate: payload.gate,
        root_manifest,
        input_publications,
        created_by: subject.user,
        created_by_user_id: Some(subject.user_id),
        created_at,

        promotable,
        reasons,

        approvals: Vec::new(),
        approval_user_ids: Vec::new(),
    };

    let bytes =
        serde_json::to_vec_pretty(&bundle).map_err(|e| internal_error(anyhow::anyhow!(e)))?;
    let path = repo_data_dir(&state, &repo_id)
        .join("bundles")
        .join(format!("{}.json", id));
    write_if_absent(&path, &bytes).map_err(internal_error)?;

    repo.bundles.push(bundle.clone());
    persist_repo(state.as_ref(), repo).map_err(internal_error)?;
    Ok(Json(bundle))
}

#[derive(Debug, serde::Deserialize)]
struct ListBundlesQuery {
    scope: Option<String>,
    gate: Option<String>,
}

async fn list_bundles(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path(repo_id): Path<String>,
    Query(q): Query<ListBundlesQuery>,
) -> Result<Json<Vec<Bundle>>, Response> {
    let repos = state.repos.read().await;
    let repo = repos.get(&repo_id).ok_or_else(not_found)?;
    if !can_read(repo, &subject) {
        return Err(forbidden());
    }

    let mut out = Vec::new();
    for b in &repo.bundles {
        if let Some(scope) = &q.scope
            && &b.scope != scope
        {
            continue;
        }
        if let Some(gate) = &q.gate
            && &b.gate != gate
        {
            continue;
        }
        out.push(b.clone());
    }
    Ok(Json(out))
}

async fn get_bundle(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path((repo_id, bundle_id)): Path<(String, String)>,
) -> Result<Json<Bundle>, Response> {
    validate_object_id(&bundle_id).map_err(bad_request)?;

    let repos = state.repos.read().await;
    let repo = repos.get(&repo_id).ok_or_else(not_found)?;
    if !can_read(repo, &subject) {
        return Err(forbidden());
    }

    if let Some(b) = repo.bundles.iter().find(|b| b.id == bundle_id) {
        return Ok(Json(b.clone()));
    }

    // Best-effort disk fallback.
    let path = repo_data_dir(&state, &repo_id)
        .join("bundles")
        .join(format!("{}.json", bundle_id));
    if !path.exists() {
        return Err(not_found());
    }
    let bytes = std::fs::read(&path)
        .with_context(|| format!("read {}", path.display()))
        .map_err(|e| internal_error(anyhow::anyhow!(e)))?;
    let bundle: Bundle =
        serde_json::from_slice(&bytes).map_err(|e| internal_error(anyhow::anyhow!(e)))?;
    Ok(Json(bundle))
}

async fn approve_bundle(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path((repo_id, bundle_id)): Path<(String, String)>,
) -> Result<Json<Bundle>, Response> {
    validate_object_id(&bundle_id).map_err(bad_request)?;

    let mut repos = state.repos.write().await;
    let repo = repos.get_mut(&repo_id).ok_or_else(not_found)?;
    if !can_publish(repo, &subject) {
        return Err(forbidden());
    }

    // Load bundle.
    let mut bundle = if let Some(b) = repo.bundles.iter().find(|b| b.id == bundle_id) {
        b.clone()
    } else {
        load_bundle_from_disk(state.as_ref(), &repo_id, &bundle_id)?
    };

    if !bundle.approvals.contains(&subject.user) {
        bundle.approvals.push(subject.user.clone());
        bundle.approvals.sort();
        bundle.approvals.dedup();
    }

    if !bundle
        .approval_user_ids
        .iter()
        .any(|u| u == &subject.user_id)
    {
        bundle.approval_user_ids.push(subject.user_id.clone());
        bundle.approval_user_ids.sort();
        bundle.approval_user_ids.dedup();
    }

    let gate_def = repo
        .gate_graph
        .gates
        .iter()
        .find(|g| g.id == bundle.gate)
        .ok_or_else(|| internal_error(anyhow::anyhow!("bundle gate not found")))?;

    let has_superpositions =
        manifest_has_superpositions(state.as_ref(), &repo_id, &bundle.root_manifest)?;
    let (promotable, reasons) =
        compute_promotability(gate_def, has_superpositions, bundle.approvals.len());
    bundle.promotable = promotable;
    bundle.reasons = reasons;

    // Persist updated bundle.
    let bytes =
        serde_json::to_vec_pretty(&bundle).map_err(|e| internal_error(anyhow::anyhow!(e)))?;
    let path = repo_data_dir(state.as_ref(), &repo_id)
        .join("bundles")
        .join(format!("{}.json", bundle.id));
    write_atomic_overwrite(&path, &bytes).map_err(internal_error)?;

    // Update in-memory copy if present.
    if let Some(existing) = repo.bundles.iter_mut().find(|b| b.id == bundle.id) {
        *existing = bundle.clone();
    } else {
        repo.bundles.push(bundle.clone());
    }

    persist_repo(state.as_ref(), repo).map_err(internal_error)?;

    Ok(Json(bundle))
}

async fn list_pins(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path(repo_id): Path<String>,
) -> Result<Json<serde_json::Value>, Response> {
    let repos = state.repos.read().await;
    let repo = repos.get(&repo_id).ok_or_else(not_found)?;
    if !can_read(repo, &subject) {
        return Err(forbidden());
    }

    let mut bundles: Vec<String> = repo.pinned_bundles.iter().cloned().collect();
    bundles.sort();
    Ok(Json(serde_json::json!({"bundles": bundles})))
}

async fn pin_bundle(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path((repo_id, bundle_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, Response> {
    validate_object_id(&bundle_id).map_err(bad_request)?;

    let mut repos = state.repos.write().await;
    let repo = repos.get_mut(&repo_id).ok_or_else(not_found)?;
    if !can_publish(repo, &subject) {
        return Err(forbidden());
    }

    // Ensure bundle exists (in memory or on disk).
    let _ = if repo.bundles.iter().any(|b| b.id == bundle_id) {
        None
    } else {
        Some(load_bundle_from_disk(state.as_ref(), &repo_id, &bundle_id)?)
    };

    repo.pinned_bundles.insert(bundle_id.clone());
    persist_repo(state.as_ref(), repo).map_err(internal_error)?;

    Ok(Json(
        serde_json::json!({"bundle_id": bundle_id, "pinned": true}),
    ))
}

async fn unpin_bundle(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path((repo_id, bundle_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, Response> {
    validate_object_id(&bundle_id).map_err(bad_request)?;

    let mut repos = state.repos.write().await;
    let repo = repos.get_mut(&repo_id).ok_or_else(not_found)?;
    if !can_publish(repo, &subject) {
        return Err(forbidden());
    }

    repo.pinned_bundles.remove(&bundle_id);
    persist_repo(state.as_ref(), repo).map_err(internal_error)?;
    Ok(Json(
        serde_json::json!({"bundle_id": bundle_id, "pinned": false}),
    ))
}

#[derive(Debug, serde::Deserialize)]
struct CreateReleaseRequest {
    channel: String,
    bundle_id: String,

    #[serde(default)]
    notes: Option<String>,
}

async fn create_release(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path(repo_id): Path<String>,
    Json(payload): Json<CreateReleaseRequest>,
) -> Result<Json<Release>, Response> {
    validate_release_channel(&payload.channel).map_err(bad_request)?;
    validate_object_id(&payload.bundle_id).map_err(bad_request)?;

    let released_at = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|e| internal_error(anyhow::anyhow!(e)))?;

    let mut repos = state.repos.write().await;
    let repo = repos.get_mut(&repo_id).ok_or_else(not_found)?;
    if !can_publish(repo, &subject) {
        return Err(forbidden());
    }

    let bundle = if let Some(b) = repo.bundles.iter().find(|b| b.id == payload.bundle_id) {
        b.clone()
    } else {
        load_bundle_from_disk(state.as_ref(), &repo_id, &payload.bundle_id)?
    };

    let gate_def = repo
        .gate_graph
        .gates
        .iter()
        .find(|g| g.id == bundle.gate)
        .ok_or_else(|| internal_error(anyhow::anyhow!("bundle gate not found")))?;

    if !gate_def.allow_releases {
        return Err(bad_request(anyhow::anyhow!(
            "releases disabled for gate {}",
            bundle.gate
        )));
    }

    // Re-check promotability at release time.
    let has_superpositions =
        manifest_has_superpositions(state.as_ref(), &repo_id, &bundle.root_manifest)?;
    let (promotable, _reasons) =
        compute_promotability(gate_def, has_superpositions, bundle.approvals.len());
    if !promotable {
        return Err(conflict("bundle not promotable"));
    }

    let id = {
        let mut hasher = blake3::Hasher::new();
        hasher.update(repo_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(payload.channel.as_bytes());
        hasher.update(b"\n");
        hasher.update(bundle.id.as_bytes());
        hasher.update(b"\n");
        hasher.update(subject.user.as_bytes());
        hasher.update(b"\n");
        hasher.update(released_at.as_bytes());
        hasher.finalize().to_hex().to_string()
    };

    let release = Release {
        id: id.clone(),
        channel: payload.channel,
        bundle_id: bundle.id.clone(),
        scope: bundle.scope.clone(),
        gate: bundle.gate.clone(),
        released_by: subject.user.clone(),
        released_by_user_id: Some(subject.user_id.clone()),
        released_at,
        notes: payload.notes,
    };

    let bytes =
        serde_json::to_vec_pretty(&release).map_err(|e| internal_error(anyhow::anyhow!(e)))?;
    let path = repo_data_dir(&state, &repo_id)
        .join("releases")
        .join(format!("{}.json", id));
    write_if_absent(&path, &bytes).map_err(internal_error)?;

    repo.releases.push(release.clone());
    persist_repo(state.as_ref(), repo).map_err(internal_error)?;
    Ok(Json(release))
}

async fn list_releases(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path(repo_id): Path<String>,
) -> Result<Json<Vec<Release>>, Response> {
    let repos = state.repos.read().await;
    let repo = repos.get(&repo_id).ok_or_else(not_found)?;
    if !can_read(repo, &subject) {
        return Err(forbidden());
    }
    let mut out = repo.releases.clone();
    out.sort_by(|a, b| b.released_at.cmp(&a.released_at));
    Ok(Json(out))
}

async fn get_release_channel(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path((repo_id, channel)): Path<(String, String)>,
) -> Result<Json<Release>, Response> {
    validate_release_channel(&channel).map_err(bad_request)?;

    let repos = state.repos.read().await;
    let repo = repos.get(&repo_id).ok_or_else(not_found)?;
    if !can_read(repo, &subject) {
        return Err(forbidden());
    }

    let mut best: Option<Release> = None;
    for r in &repo.releases {
        if r.channel != channel {
            continue;
        }
        match best.as_ref() {
            None => best = Some(r.clone()),
            Some(prev) => {
                if r.released_at > prev.released_at {
                    best = Some(r.clone());
                }
            }
        }
    }
    let Some(best) = best else {
        return Err(not_found());
    };
    Ok(Json(best))
}

#[derive(Debug, serde::Deserialize)]
struct CreatePromotionRequest {
    bundle_id: String,
    to_gate: String,
}

async fn create_promotion(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path(repo_id): Path<String>,
    Json(payload): Json<CreatePromotionRequest>,
) -> Result<Json<Promotion>, Response> {
    validate_object_id(&payload.bundle_id).map_err(bad_request)?;
    validate_gate_id(&payload.to_gate).map_err(bad_request)?;

    let promoted_at = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|e| internal_error(anyhow::anyhow!(e)))?;

    let mut repos = state.repos.write().await;
    let repo = repos.get_mut(&repo_id).ok_or_else(not_found)?;
    if !can_publish(repo, &subject) {
        return Err(forbidden());
    }

    let bundle = if let Some(b) = repo.bundles.iter().find(|b| b.id == payload.bundle_id) {
        b.clone()
    } else {
        load_bundle_from_disk(state.as_ref(), &repo_id, &payload.bundle_id)?
    };

    // Re-check promotability at promotion time.
    let gate_def = repo
        .gate_graph
        .gates
        .iter()
        .find(|g| g.id == bundle.gate)
        .ok_or_else(|| internal_error(anyhow::anyhow!("bundle gate not found")))?;
    let has_superpositions =
        manifest_has_superpositions(state.as_ref(), &repo_id, &bundle.root_manifest)?;
    let (promotable, _reasons) =
        compute_promotability(gate_def, has_superpositions, bundle.approvals.len());
    if !promotable {
        return Err(conflict("bundle not promotable"));
    }

    // Validate gate relationship: to_gate must list bundle.gate as upstream.
    let to_gate_def = repo
        .gate_graph
        .gates
        .iter()
        .find(|g| g.id == payload.to_gate)
        .ok_or_else(|| bad_request(anyhow::anyhow!("unknown to_gate")))?;
    if !to_gate_def.upstream.iter().any(|u| u == &bundle.gate) {
        return Err(bad_request(anyhow::anyhow!(
            "to_gate is not downstream of bundle gate"
        )));
    }

    let id = {
        let mut hasher = blake3::Hasher::new();
        hasher.update(repo_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(bundle.id.as_bytes());
        hasher.update(b"\n");
        hasher.update(bundle.scope.as_bytes());
        hasher.update(b"\n");
        hasher.update(bundle.gate.as_bytes());
        hasher.update(b"\n");
        hasher.update(payload.to_gate.as_bytes());
        hasher.update(b"\n");
        hasher.update(subject.user.as_bytes());
        hasher.update(b"\n");
        hasher.update(promoted_at.as_bytes());
        hasher.finalize().to_hex().to_string()
    };

    let promotion = Promotion {
        id: id.clone(),
        bundle_id: bundle.id.clone(),
        scope: bundle.scope.clone(),
        from_gate: bundle.gate.clone(),
        to_gate: payload.to_gate,
        promoted_by: subject.user.clone(),
        promoted_by_user_id: Some(subject.user_id.clone()),
        promoted_at,
    };

    // Update state pointer.
    repo.promotion_state
        .entry(promotion.scope.clone())
        .or_default()
        .insert(promotion.to_gate.clone(), promotion.bundle_id.clone());

    // Persist promotion record.
    let bytes =
        serde_json::to_vec_pretty(&promotion).map_err(|e| internal_error(anyhow::anyhow!(e)))?;
    let path = repo_data_dir(&state, &repo_id)
        .join("promotions")
        .join(format!("{}.json", id));
    write_if_absent(&path, &bytes).map_err(internal_error)?;

    repo.promotions.push(promotion.clone());
    persist_repo(state.as_ref(), repo).map_err(internal_error)?;
    Ok(Json(promotion))
}

#[derive(Debug, serde::Deserialize)]
struct ListPromotionsQuery {
    scope: Option<String>,
    to_gate: Option<String>,
}

async fn list_promotions(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path(repo_id): Path<String>,
    Query(q): Query<ListPromotionsQuery>,
) -> Result<Json<Vec<Promotion>>, Response> {
    let repos = state.repos.read().await;
    let repo = repos.get(&repo_id).ok_or_else(not_found)?;
    if !can_read(repo, &subject) {
        return Err(forbidden());
    }

    let mut out = Vec::new();
    for p in &repo.promotions {
        if let Some(scope) = &q.scope
            && &p.scope != scope
        {
            continue;
        }
        if let Some(to_gate) = &q.to_gate
            && &p.to_gate != to_gate
        {
            continue;
        }
        out.push(p.clone());
    }
    Ok(Json(out))
}

#[derive(Debug, serde::Deserialize)]
struct PromotionStateQuery {
    scope: String,
}

async fn get_promotion_state(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path(repo_id): Path<String>,
    Query(q): Query<PromotionStateQuery>,
) -> Result<Json<HashMap<String, String>>, Response> {
    validate_scope_id(&q.scope).map_err(bad_request)?;
    let repos = state.repos.read().await;
    let repo = repos.get(&repo_id).ok_or_else(not_found)?;
    if !can_read(repo, &subject) {
        return Err(forbidden());
    }
    Ok(Json(
        repo.promotion_state
            .get(&q.scope)
            .cloned()
            .unwrap_or_default(),
    ))
}

fn load_bundle_from_disk(
    state: &AppState,
    repo_id: &str,
    bundle_id: &str,
) -> Result<Bundle, Response> {
    let path = repo_data_dir(state, repo_id)
        .join("bundles")
        .join(format!("{}.json", bundle_id));
    if !path.exists() {
        return Err(not_found());
    }
    let bytes = std::fs::read(&path)
        .with_context(|| format!("read {}", path.display()))
        .map_err(|e| internal_error(anyhow::anyhow!(e)))?;
    let bundle: Bundle =
        serde_json::from_slice(&bytes).map_err(|e| internal_error(anyhow::anyhow!(e)))?;
    Ok(bundle)
}

fn internal_error(err: anyhow::Error) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({"error": err.to_string()})),
    )
        .into_response()
}

#[derive(Clone, Debug, serde::Serialize)]
struct GateGraphIssue {
    code: String,
    message: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    gate: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    upstream: Option<String>,
}

fn validate_gate_graph_issues(graph: &GateGraph) -> Vec<GateGraphIssue> {
    let mut issues: Vec<GateGraphIssue> = Vec::new();

    if graph.version != 1 {
        issues.push(GateGraphIssue {
            code: "unsupported_version".to_string(),
            message: "unsupported gate graph version".to_string(),
            gate: None,
            upstream: None,
        });
        return issues;
    }

    if graph.gates.is_empty() {
        issues.push(GateGraphIssue {
            code: "no_gates".to_string(),
            message: "gate graph must contain at least one gate".to_string(),
            gate: None,
            upstream: None,
        });
        return issues;
    }

    let mut ids = HashSet::new();
    for g in &graph.gates {
        if let Err(err) = validate_gate_id(&g.id) {
            issues.push(GateGraphIssue {
                code: "invalid_gate_id".to_string(),
                message: err.to_string(),
                gate: Some(g.id.clone()),
                upstream: None,
            });
        }
        if g.name.trim().is_empty() {
            issues.push(GateGraphIssue {
                code: "empty_gate_name".to_string(),
                message: "gate name cannot be empty".to_string(),
                gate: Some(g.id.clone()),
                upstream: None,
            });
        }
        if !ids.insert(g.id.clone()) {
            issues.push(GateGraphIssue {
                code: "duplicate_gate_id".to_string(),
                message: format!("duplicate gate id {}", g.id),
                gate: Some(g.id.clone()),
                upstream: None,
            });
        }
    }

    // Upstream references.
    for g in &graph.gates {
        for up in &g.upstream {
            if let Err(err) = validate_gate_id(up) {
                issues.push(GateGraphIssue {
                    code: "invalid_upstream_id".to_string(),
                    message: err.to_string(),
                    gate: Some(g.id.clone()),
                    upstream: Some(up.clone()),
                });
                continue;
            }
            if !ids.contains(up) {
                issues.push(GateGraphIssue {
                    code: "unknown_upstream".to_string(),
                    message: format!("gate {} references unknown upstream {}", g.id, up),
                    gate: Some(g.id.clone()),
                    upstream: Some(up.clone()),
                });
            }
        }
    }

    // Cycle check.
    if issues.iter().any(|i| i.code == "unknown_upstream") {
        // Don't run DFS if upstreams are missing.
        return issues;
    }
    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();
    for g in &graph.gates {
        if let Err(err) = dfs_gate(g, graph, &mut visiting, &mut visited) {
            issues.push(GateGraphIssue {
                code: "cycle".to_string(),
                message: err.to_string(),
                gate: None,
                upstream: None,
            });
            break;
        }
    }

    // Reachability from roots.
    let roots: Vec<&GateDef> = graph
        .gates
        .iter()
        .filter(|g| g.upstream.is_empty())
        .collect();

    if roots.is_empty() {
        issues.push(GateGraphIssue {
            code: "no_root_gate".to_string(),
            message: "gate graph must contain at least one root gate (a gate with no upstream)"
                .to_string(),
            gate: None,
            upstream: None,
        });
        return issues;
    }

    let mut by_id: HashMap<String, &GateDef> = HashMap::new();
    for g in &graph.gates {
        by_id.insert(g.id.clone(), g);
    }

    let mut downstream: HashMap<String, Vec<String>> = HashMap::new();
    for g in &graph.gates {
        for up in &g.upstream {
            downstream.entry(up.clone()).or_default().push(g.id.clone());
        }
    }

    let mut stack: Vec<String> = roots.iter().map(|g| g.id.clone()).collect();
    let mut reachable: HashSet<String> = HashSet::new();
    while let Some(id) = stack.pop() {
        if !reachable.insert(id.clone()) {
            continue;
        }
        if let Some(next) = downstream.get(&id) {
            for nid in next {
                if by_id.contains_key(nid) {
                    stack.push(nid.clone());
                }
            }
        }
    }

    if reachable.len() != graph.gates.len() {
        let mut missing: Vec<String> = graph
            .gates
            .iter()
            .map(|g| g.id.clone())
            .filter(|id| !reachable.contains(id))
            .collect();
        missing.sort();
        issues.push(GateGraphIssue {
            code: "unreachable_gates".to_string(),
            message: format!(
                "unreachable gates (not reachable from any root): {}",
                missing.join(", ")
            ),
            gate: None,
            upstream: None,
        });
    }

    issues
}

fn dfs_gate(
    gate: &GateDef,
    graph: &GateGraph,
    visiting: &mut HashSet<String>,
    visited: &mut HashSet<String>,
) -> Result<()> {
    if visited.contains(&gate.id) {
        return Ok(());
    }
    if !visiting.insert(gate.id.clone()) {
        return Err(anyhow::anyhow!("cycle detected at gate {}", gate.id));
    }

    for up in &gate.upstream {
        let up_gate = graph
            .gates
            .iter()
            .find(|g| g.id == *up)
            .ok_or_else(|| anyhow::anyhow!("unknown upstream {}", up))?;
        dfs_gate(up_gate, graph, visiting, visited)?;
    }

    visiting.remove(&gate.id);
    visited.insert(gate.id.clone());
    Ok(())
}

fn bad_request(err: anyhow::Error) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({"error": err.to_string()})),
    )
        .into_response()
}

fn forbidden() -> Response {
    (
        StatusCode::FORBIDDEN,
        Json(serde_json::json!({"error": "forbidden"})),
    )
        .into_response()
}

fn not_found() -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({"error": "not found"})),
    )
        .into_response()
}

fn conflict(msg: &str) -> Response {
    (
        StatusCode::CONFLICT,
        Json(serde_json::json!({"error": msg})),
    )
        .into_response()
}
