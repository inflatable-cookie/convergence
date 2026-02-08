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
#[path = "converge_server/handlers_publications.rs"]
mod handlers_publications;
use self::handlers_publications::*;
#[path = "converge_server/handlers_release.rs"]
mod handlers_release;
use self::handlers_release::*;
#[path = "converge_server/handlers_gc.rs"]
mod handlers_gc;
use self::handlers_gc::*;
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
