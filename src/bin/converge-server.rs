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
use axum::routing::get;
use axum::{Json, Router, extract::Path};
use clap::Parser;
use tokio::sync::RwLock;

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
    readers: HashSet<String>,
    publishers: HashSet<String>,
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
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct Gate {
    id: String,
    name: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct GateGraph {
    version: u32,
    terminal_gate: String,
    gates: Vec<GateDef>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct GateDef {
    id: String,
    name: String,
    upstream: Vec<String>,

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
    created_at: String,

    promotable: bool,
    reasons: Vec<String>,

    #[serde(default)]
    approvals: Vec<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct Promotion {
    id: String,
    bundle_id: String,
    scope: String,
    from_gate: String,
    to_gate: String,
    promoted_by: String,
    promoted_at: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct Lane {
    id: String,
    members: HashSet<String>,

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

fn can_read(repo: &Repo, user: &str) -> bool {
    repo.owner == user || repo.readers.contains(user)
}

fn can_publish(repo: &Repo, user: &str) -> bool {
    repo.owner == user || repo.publishers.contains(user)
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
        let (u, t) = bootstrap_identity(&args.dev_user, &args.dev_token);
        users.insert(u.id.clone(), u);
        tokens.insert(t.id.clone(), t);
        persist_identity_to_disk(&args.data_dir, &users, &tokens).context("persist identity")?;
    }

    let default_user = users
        .values()
        .find(|u| u.admin)
        .or_else(|| users.values().next())
        .map(|u| u.handle.clone())
        .unwrap_or_else(|| "dev".to_string());

    let token_hash_index: HashMap<String, String> =
        tokens.values().map(|t| (t.token_hash.clone(), t.id.clone())).collect();

    let state = Arc::new(AppState {
        default_user,
        data_dir: args.data_dir,
        repos: Arc::new(RwLock::new(HashMap::new())),

        users: Arc::new(RwLock::new(users)),
        tokens: Arc::new(RwLock::new(tokens)),
        token_hash_index: Arc::new(RwLock::new(token_hash_index)),
    });

    // Best-effort load repos from disk so the dev server survives restarts.
    let loaded = load_repos_from_disk(state.as_ref()).context("load repos from disk")?;
    {
        let mut repos = state.repos.write().await;
        *repos = loaded;
    }

    let authed = Router::new()
        .route("/whoami", get(whoami))
        .route("/tokens", get(list_tokens).post(create_token))
        .route(
            "/tokens/:token_id/revoke",
            axum::routing::post(revoke_token),
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
        if let Some(exp) = &t.expires_at {
            if let Ok(exp) = time::OffsetDateTime::parse(
                exp,
                &time::format_description::well_known::Rfc3339,
            ) {
                if time::OffsetDateTime::now_utc() > exp {
                    return unauthorized();
                }
            }
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

async fn whoami(Extension(subject): Extension<Subject>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "user": subject.user,
        "user_id": subject.user_id,
        "admin": subject.admin,
    }))
}

#[derive(Debug, serde::Serialize)]
struct TokenView {
    id: String,
    label: Option<String>,
    created_at: String,
    last_used_at: Option<String>,
    revoked_at: Option<String>,
    expires_at: Option<String>,
}

async fn list_tokens(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
) -> Result<Json<Vec<TokenView>>, Response> {
    let tokens = state.tokens.read().await;
    let mut out = Vec::new();
    for t in tokens.values() {
        if t.user_id != subject.user_id {
            continue;
        }
        out.push(TokenView {
            id: t.id.clone(),
            label: t.label.clone(),
            created_at: t.created_at.clone(),
            last_used_at: t.last_used_at.clone(),
            revoked_at: t.revoked_at.clone(),
            expires_at: t.expires_at.clone(),
        });
    }
    out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(Json(out))
}

#[derive(Debug, serde::Deserialize)]
struct CreateTokenRequest {
    #[serde(default)]
    label: Option<String>,
}

#[derive(Debug, serde::Serialize)]
struct CreateTokenResponse {
    id: String,
    token: String,
    created_at: String,
}

async fn create_token(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Json(payload): Json<CreateTokenRequest>,
) -> Result<Json<CreateTokenResponse>, Response> {
    let created_at = now_ts();

    let token = generate_token_secret().map_err(internal_error)?;
    let token_hash = hash_token(&token);
    let token_id = {
        let mut h = blake3::Hasher::new();
        h.update(subject.user_id.as_bytes());
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
                user_id: subject.user_id.clone(),
                token_hash: token_hash.clone(),
                label: payload.label,
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

    // Persist best-effort.
    {
        let users = state.users.read().await;
        let tokens = state.tokens.read().await;
        if let Err(err) = persist_identity_to_disk(&state.data_dir, &users, &tokens) {
            return Err(internal_error(err));
        }
    }

    Ok(Json(CreateTokenResponse {
        id: token_id,
        token,
        created_at,
    }))
}

async fn revoke_token(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path(token_id): Path<String>,
) -> Result<Json<serde_json::Value>, Response> {
    let revoked_at = now_ts();

    {
        let mut tokens = state.tokens.write().await;
        let Some(t) = tokens.get_mut(&token_id) else {
            return Err(not_found());
        };
        if t.user_id != subject.user_id && !subject.admin {
            return Err(forbidden());
        }
        t.revoked_at = Some(revoked_at.clone());
    }

    // Persist best-effort.
    {
        let users = state.users.read().await;
        let tokens = state.tokens.read().await;
        if let Err(err) = persist_identity_to_disk(&state.data_dir, &users, &tokens) {
            return Err(internal_error(err));
        }
    }

    Ok(Json(serde_json::json!({"revoked": true, "token_id": token_id, "revoked_at": revoked_at})))
}

#[derive(Debug, serde::Deserialize)]
struct CreateRepoRequest {
    id: String,
}

async fn create_repo(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Json(payload): Json<CreateRepoRequest>,
) -> Result<Json<Repo>, Response> {
    validate_repo_id(&payload.id).map_err(bad_request)?;

    let mut repos = state.repos.write().await;
    if repos.contains_key(&payload.id) {
        return Err(conflict("repo already exists"));
    }

    let mut readers = HashSet::new();
    readers.insert(subject.user.clone());
    let mut publishers = HashSet::new();
    publishers.insert(subject.user.clone());

    let mut members = HashSet::new();
    members.insert(subject.user.clone());
    let default_lane = Lane {
        id: "default".to_string(),
        members,
        heads: HashMap::new(),
        head_history: HashMap::new(),
    };
    let mut lanes = HashMap::new();
    lanes.insert(default_lane.id.clone(), default_lane);

    let gate_graph = GateGraph {
        version: 1,
        terminal_gate: "dev-intake".to_string(),
        gates: vec![GateDef {
            id: "dev-intake".to_string(),
            name: "Dev Intake".to_string(),
            upstream: vec![],
            allow_superpositions: false,
            allow_metadata_only_publications: false,
            required_approvals: 0,
        }],
    };

    let mut scopes = HashSet::new();
    scopes.insert("main".to_string());

    let snaps = HashSet::new();
    let publications = Vec::new();
    let bundles = Vec::new();
    let pinned_bundles = HashSet::new();
    let promotions = Vec::new();
    let promotion_state = HashMap::new();

    let repo = Repo {
        id: payload.id.clone(),
        owner: subject.user.clone(),
        readers,
        publishers,
        lanes,

        gate_graph,
        scopes,

        snaps,
        publications,

        bundles,

        pinned_bundles,

        promotions,
        promotion_state,
    };
    repos.insert(repo.id.clone(), repo.clone());

    std::fs::create_dir_all(repo_data_dir(&state, &repo.id))
        .map_err(|e| internal_error(anyhow::anyhow!(e)))?;

    std::fs::create_dir_all(repo_data_dir(&state, &repo.id).join("bundles"))
        .map_err(|e| internal_error(anyhow::anyhow!(e)))?;

    std::fs::create_dir_all(repo_data_dir(&state, &repo.id).join("promotions"))
        .map_err(|e| internal_error(anyhow::anyhow!(e)))?;

    persist_repo(state.as_ref(), &repo).map_err(internal_error)?;

    Ok(Json(repo))
}

async fn list_repos(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
) -> Result<Json<Vec<Repo>>, Response> {
    let repos = state.repos.read().await;
    let mut out = Vec::new();
    for repo in repos.values() {
        if can_read(repo, &subject.user) {
            out.push(repo.clone());
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(Json(out))
}

async fn get_repo(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path(repo_id): Path<String>,
) -> Result<Json<Repo>, Response> {
    let repos = state.repos.read().await;
    let repo = repos.get(&repo_id).ok_or_else(not_found)?;
    if !can_read(repo, &subject.user) {
        return Err(forbidden());
    }
    Ok(Json(repo.clone()))
}

async fn get_repo_permissions(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path(repo_id): Path<String>,
) -> Result<Json<serde_json::Value>, Response> {
    let repos = state.repos.read().await;
    let repo = repos.get(&repo_id).ok_or_else(not_found)?;
    Ok(Json(serde_json::json!({
        "read": can_read(repo, &subject.user),
        "publish": can_publish(repo, &subject.user)
    })))
}

async fn list_lanes(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path(repo_id): Path<String>,
) -> Result<Json<Vec<Lane>>, Response> {
    let repos = state.repos.read().await;
    let repo = repos.get(&repo_id).ok_or_else(not_found)?;
    if !can_read(repo, &subject.user) {
        return Err(forbidden());
    }

    let mut out: Vec<Lane> = repo.lanes.values().cloned().collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(Json(out))
}

#[derive(Debug, serde::Deserialize)]
struct UpdateLaneHeadRequest {
    snap_id: String,

    #[serde(default)]
    client_id: Option<String>,
}

async fn update_lane_head_me(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path((repo_id, lane_id)): Path<(String, String)>,
    Json(payload): Json<UpdateLaneHeadRequest>,
) -> Result<Json<LaneHead>, Response> {
    validate_lane_id(&lane_id).map_err(bad_request)?;
    validate_object_id(&payload.snap_id).map_err(bad_request)?;

    let mut repos = state.repos.write().await;
    let repo = repos.get_mut(&repo_id).ok_or_else(not_found)?;
    if !can_publish(repo, &subject.user) {
        return Err(forbidden());
    }

    let lane = repo.lanes.get_mut(&lane_id).ok_or_else(not_found)?;
    if !lane.members.contains(&subject.user) {
        return Err(forbidden());
    }

    if !repo.snaps.contains(&payload.snap_id) {
        return Err(bad_request(anyhow::anyhow!(
            "unknown snap (upload snap first)"
        )));
    }

    let updated_at = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|e| internal_error(anyhow::anyhow!(e)))?;

    let head = LaneHead {
        snap_id: payload.snap_id,
        updated_at,
        client_id: payload.client_id,
    };
    lane.heads.insert(subject.user.clone(), head.clone());

    let hist = lane.head_history.entry(subject.user.clone()).or_default();
    // Keep newest first.
    hist.insert(0, head.clone());
    if hist.len() > LANE_HEAD_HISTORY_KEEP_LAST {
        hist.truncate(LANE_HEAD_HISTORY_KEEP_LAST);
    }
    persist_repo(state.as_ref(), repo).map_err(internal_error)?;
    Ok(Json(head))
}

async fn get_lane_head(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path((repo_id, lane_id, user)): Path<(String, String, String)>,
) -> Result<Json<LaneHead>, Response> {
    validate_lane_id(&lane_id).map_err(bad_request)?;

    let repos = state.repos.read().await;
    let repo = repos.get(&repo_id).ok_or_else(not_found)?;
    if !can_read(repo, &subject.user) {
        return Err(forbidden());
    }
    let lane = repo.lanes.get(&lane_id).ok_or_else(not_found)?;
    if !lane.members.contains(&subject.user) {
        return Err(forbidden());
    }

    let head = lane.heads.get(&user).ok_or_else(not_found)?;
    Ok(Json(head.clone()))
}

async fn list_gates(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path(repo_id): Path<String>,
) -> Result<Json<Vec<Gate>>, Response> {
    let repos = state.repos.read().await;
    let repo = repos.get(&repo_id).ok_or_else(not_found)?;
    if !can_read(repo, &subject.user) {
        return Err(forbidden());
    }

    let gates = repo
        .gate_graph
        .gates
        .iter()
        .map(|g| Gate {
            id: g.id.clone(),
            name: g.name.clone(),
        })
        .collect();
    Ok(Json(gates))
}

async fn get_gate_graph(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path(repo_id): Path<String>,
) -> Result<Json<GateGraph>, Response> {
    let repos = state.repos.read().await;
    let repo = repos.get(&repo_id).ok_or_else(not_found)?;
    if !can_read(repo, &subject.user) {
        return Err(forbidden());
    }
    Ok(Json(repo.gate_graph.clone()))
}

async fn put_gate_graph(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path(repo_id): Path<String>,
    Json(graph): Json<GateGraph>,
) -> Result<Json<GateGraph>, Response> {
    validate_gate_graph(&graph).map_err(bad_request)?;

    let mut repos = state.repos.write().await;
    let repo = repos.get_mut(&repo_id).ok_or_else(not_found)?;
    if !can_publish(repo, &subject.user) {
        return Err(forbidden());
    }

    repo.gate_graph = graph.clone();
    persist_repo(state.as_ref(), repo).map_err(internal_error)?;
    Ok(Json(graph))
}

#[derive(Debug, serde::Deserialize)]
struct CreateScopeRequest {
    id: String,
}

async fn create_scope(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path(repo_id): Path<String>,
    Json(payload): Json<CreateScopeRequest>,
) -> Result<Json<serde_json::Value>, Response> {
    validate_scope_id(&payload.id).map_err(bad_request)?;

    let mut repos = state.repos.write().await;
    let repo = repos.get_mut(&repo_id).ok_or_else(not_found)?;
    if !can_publish(repo, &subject.user) {
        return Err(forbidden());
    }

    if !repo.scopes.insert(payload.id.clone()) {
        return Err(conflict("scope already exists"));
    }

    persist_repo(state.as_ref(), repo).map_err(internal_error)?;

    Ok(Json(serde_json::json!({"id": payload.id})))
}

async fn list_scopes(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path(repo_id): Path<String>,
) -> Result<Json<Vec<String>>, Response> {
    let repos = state.repos.read().await;
    let repo = repos.get(&repo_id).ok_or_else(not_found)?;
    if !can_read(repo, &subject.user) {
        return Err(forbidden());
    }

    let mut out: Vec<String> = repo.scopes.iter().cloned().collect();
    out.sort();
    Ok(Json(out))
}

async fn put_blob(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path((repo_id, blob_id)): Path<(String, String)>,
    body: axum::body::Bytes,
) -> Result<StatusCode, Response> {
    validate_object_id(&blob_id).map_err(bad_request)?;

    {
        let repos = state.repos.read().await;
        let repo = repos.get(&repo_id).ok_or_else(not_found)?;
        if !can_publish(repo, &subject.user) {
            return Err(forbidden());
        }
    }

    let actual = blake3::hash(&body).to_hex().to_string();
    if actual != blob_id {
        return Err(bad_request(anyhow::anyhow!(
            "blob hash mismatch (expected {}, got {})",
            blob_id,
            actual
        )));
    }

    let path = repo_data_dir(&state, &repo_id)
        .join("objects/blobs")
        .join(&blob_id);
    write_if_absent(&path, &body).map_err(internal_error)?;
    Ok(StatusCode::CREATED)
}

async fn get_blob(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path((repo_id, blob_id)): Path<(String, String)>,
) -> Result<axum::body::Bytes, Response> {
    validate_object_id(&blob_id).map_err(bad_request)?;

    {
        let repos = state.repos.read().await;
        let repo = repos.get(&repo_id).ok_or_else(not_found)?;
        if !can_read(repo, &subject.user) {
            return Err(forbidden());
        }
    }

    let path = repo_data_dir(&state, &repo_id)
        .join("objects/blobs")
        .join(&blob_id);
    if !path.exists() {
        return Err(not_found());
    }
    let bytes = std::fs::read(&path)
        .with_context(|| format!("read {}", path.display()))
        .map_err(|e| internal_error(anyhow::anyhow!(e)))?;
    let actual = blake3::hash(&bytes).to_hex().to_string();
    if actual != blob_id {
        return Err(internal_error(anyhow::anyhow!(
            "blob integrity check failed"
        )));
    }
    Ok(axum::body::Bytes::from(bytes))
}

async fn put_manifest(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path((repo_id, manifest_id)): Path<(String, String)>,
    Query(q): Query<PutObjectQuery>,
    body: axum::body::Bytes,
) -> Result<StatusCode, Response> {
    validate_object_id(&manifest_id).map_err(bad_request)?;

    {
        let repos = state.repos.read().await;
        let repo = repos.get(&repo_id).ok_or_else(not_found)?;
        if !can_publish(repo, &subject.user) {
            return Err(forbidden());
        }
    }

    let actual = blake3::hash(&body).to_hex().to_string();
    if actual != manifest_id {
        return Err(bad_request(anyhow::anyhow!(
            "manifest hash mismatch (expected {}, got {})",
            manifest_id,
            actual
        )));
    }

    // Basic schema validation.
    let manifest: converge::model::Manifest =
        serde_json::from_slice(&body).map_err(|e| bad_request(anyhow::anyhow!(e)))?;
    if manifest.version != 1 {
        return Err(bad_request(anyhow::anyhow!("unsupported manifest version")));
    }

    // Default behavior: require referenced objects to exist.
    // When allow_missing_blobs is set, we allow dangling blob references so that early
    // gates can accept metadata-only publications.
    for entry in &manifest.entries {
        validate_manifest_entry_refs(&state, &repo_id, &entry.kind, q.allow_missing_blobs)?;
    }

    let path = repo_data_dir(&state, &repo_id)
        .join("objects/manifests")
        .join(format!("{}.json", manifest_id));
    write_if_absent(&path, &body).map_err(internal_error)?;
    Ok(StatusCode::CREATED)
}

async fn put_recipe(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path((repo_id, recipe_id)): Path<(String, String)>,
    Query(q): Query<PutObjectQuery>,
    body: axum::body::Bytes,
) -> Result<StatusCode, Response> {
    validate_object_id(&recipe_id).map_err(bad_request)?;

    {
        let repos = state.repos.read().await;
        let repo = repos.get(&repo_id).ok_or_else(not_found)?;
        if !can_publish(repo, &subject.user) {
            return Err(forbidden());
        }
    }

    let actual = blake3::hash(&body).to_hex().to_string();
    if actual != recipe_id {
        return Err(bad_request(anyhow::anyhow!(
            "recipe hash mismatch (expected {}, got {})",
            recipe_id,
            actual
        )));
    }

    let recipe: converge::model::FileRecipe =
        serde_json::from_slice(&body).map_err(|e| bad_request(anyhow::anyhow!(e)))?;
    if recipe.version != 1 {
        return Err(bad_request(anyhow::anyhow!("unsupported recipe version")));
    }

    for c in &recipe.chunks {
        validate_object_id(c.blob.as_str()).map_err(bad_request)?;
        if !q.allow_missing_blobs {
            let p = repo_data_dir(&state, &repo_id)
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

    let path = repo_data_dir(&state, &repo_id)
        .join("objects/recipes")
        .join(format!("{}.json", recipe_id));
    write_if_absent(&path, &body).map_err(internal_error)?;
    Ok(StatusCode::CREATED)
}

#[derive(Debug, Default, serde::Deserialize)]
struct PutObjectQuery {
    #[serde(default)]
    allow_missing_blobs: bool,
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

async fn get_manifest(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path((repo_id, manifest_id)): Path<(String, String)>,
) -> Result<Response, Response> {
    validate_object_id(&manifest_id).map_err(bad_request)?;

    {
        let repos = state.repos.read().await;
        let repo = repos.get(&repo_id).ok_or_else(not_found)?;
        if !can_read(repo, &subject.user) {
            return Err(forbidden());
        }
    }

    let path = repo_data_dir(&state, &repo_id)
        .join("objects/manifests")
        .join(format!("{}.json", manifest_id));
    if !path.exists() {
        return Err(not_found());
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
    // Validate JSON schema (and fail fast on corruption).
    let _: converge::model::Manifest =
        serde_json::from_slice(&bytes).map_err(|e| internal_error(anyhow::anyhow!(e)))?;
    Ok(json_bytes(bytes))
}

async fn get_recipe(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path((repo_id, recipe_id)): Path<(String, String)>,
) -> Result<Response, Response> {
    validate_object_id(&recipe_id).map_err(bad_request)?;

    {
        let repos = state.repos.read().await;
        let repo = repos.get(&repo_id).ok_or_else(not_found)?;
        if !can_read(repo, &subject.user) {
            return Err(forbidden());
        }
    }

    let path = repo_data_dir(&state, &repo_id)
        .join("objects/recipes")
        .join(format!("{}.json", recipe_id));
    if !path.exists() {
        return Err(not_found());
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

    let _: converge::model::FileRecipe =
        serde_json::from_slice(&bytes).map_err(|e| internal_error(anyhow::anyhow!(e)))?;

    Ok(json_bytes(bytes))
}

async fn put_snap(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path((repo_id, snap_id)): Path<(String, String)>,
    Json(snap): Json<converge::model::SnapRecord>,
) -> Result<StatusCode, Response> {
    validate_object_id(&snap_id).map_err(bad_request)?;

    {
        let repos = state.repos.read().await;
        let repo = repos.get(&repo_id).ok_or_else(not_found)?;
        if !can_publish(repo, &subject.user) {
            return Err(forbidden());
        }
    }

    if snap.id != snap_id {
        return Err(bad_request(anyhow::anyhow!(
            "snap id mismatch (path {}, body {})",
            snap_id,
            snap.id
        )));
    }

    if snap.version != 1 {
        return Err(bad_request(anyhow::anyhow!("unsupported snap version")));
    }

    // For Phase 2 we accept the snap record as-is (client is authoritative on created_at).
    let bytes = serde_json::to_vec_pretty(&snap).map_err(|e| internal_error(anyhow::anyhow!(e)))?;
    let path = repo_data_dir(&state, &repo_id)
        .join("objects/snaps")
        .join(format!("{}.json", snap_id));
    write_if_absent(&path, &bytes).map_err(internal_error)?;

    // Record snap existence for later publication validation.
    {
        let mut repos = state.repos.write().await;
        if let Some(repo) = repos.get_mut(&repo_id) {
            repo.snaps.insert(snap_id);
            persist_repo(state.as_ref(), repo).map_err(internal_error)?;
        }
    }

    Ok(StatusCode::CREATED)
}

async fn get_snap(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path((repo_id, snap_id)): Path<(String, String)>,
) -> Result<Response, Response> {
    validate_object_id(&snap_id).map_err(bad_request)?;

    {
        let repos = state.repos.read().await;
        let repo = repos.get(&repo_id).ok_or_else(not_found)?;
        if !can_read(repo, &subject.user) {
            return Err(forbidden());
        }
    }

    let path = repo_data_dir(&state, &repo_id)
        .join("objects/snaps")
        .join(format!("{}.json", snap_id));
    if !path.exists() {
        return Err(not_found());
    }
    let bytes = std::fs::read(&path)
        .with_context(|| format!("read {}", path.display()))
        .map_err(|e| internal_error(anyhow::anyhow!(e)))?;
    let _snap: converge::model::SnapRecord =
        serde_json::from_slice(&bytes).map_err(|e| internal_error(anyhow::anyhow!(e)))?;
    Ok(json_bytes(bytes))
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
}

async fn gc_repo(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path(repo_id): Path<String>,
    Query(q): Query<GcQuery>,
) -> Result<Json<serde_json::Value>, Response> {
    let mut repos = state.repos.write().await;
    let repo = repos.get_mut(&repo_id).ok_or_else(not_found)?;
    if !can_publish(repo, &subject.user) {
        return Err(forbidden());
    }

    if !q.prune_metadata && !q.dry_run {
        return Err(bad_request(anyhow::anyhow!(
            "refusing destructive GC with prune_metadata=false (would create dangling references); use dry_run=true or prune_metadata=true"
        )));
    }

    // Retention roots: pinned bundles and current promotion-state pointers.
    let mut keep_bundles: HashSet<String> = repo.pinned_bundles.iter().cloned().collect();
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

    if q.prune_metadata && !q.dry_run {
        repo.bundles.retain(|b| keep_bundles.contains(&b.id));
        repo.pinned_bundles.retain(|b| keep_bundles.contains(b));
        repo.publications
            .retain(|p| keep_publications.contains(&p.id));
        repo.snaps = keep_snaps.clone();
        persist_repo(state.as_ref(), repo).map_err(internal_error)?;
    }

    Ok(Json(serde_json::json!({
        "dry_run": q.dry_run,
        "prune_metadata": q.prune_metadata,
        "kept": {
            "bundles": keep_bundles.len(),
            "publications": keep_publications.len(),
            "snaps": keep_snaps.len(),
            "blobs": kept_blobs_count,
            "manifests": kept_manifests_count,
            "recipes": kept_recipes_count
        },
        "deleted": {
            "bundles": deleted_bundles,
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
        if !can_publish(repo, &subject.user) {
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
    if !can_publish(repo, &subject.user) {
        return Err(forbidden());
    }
    if !repo.scopes.contains(&payload.scope) {
        return Err(bad_request(anyhow::anyhow!("unknown scope")));
    }
    if !repo.gate_graph.gates.iter().any(|g| g.id == payload.gate) {
        return Err(bad_request(anyhow::anyhow!("unknown gate")));
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
    if !can_read(repo, &subject.user) {
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
    if !can_publish(repo, &subject.user) {
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
        created_at,

        promotable,
        reasons,

        approvals: Vec::new(),
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
    if !can_read(repo, &subject.user) {
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
    if !can_read(repo, &subject.user) {
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
    if !can_publish(repo, &subject.user) {
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
    if !can_read(repo, &subject.user) {
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
    if !can_publish(repo, &subject.user) {
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
    if !can_publish(repo, &subject.user) {
        return Err(forbidden());
    }

    repo.pinned_bundles.remove(&bundle_id);
    persist_repo(state.as_ref(), repo).map_err(internal_error)?;
    Ok(Json(
        serde_json::json!({"bundle_id": bundle_id, "pinned": false}),
    ))
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
    if !can_publish(repo, &subject.user) {
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
    if !can_read(repo, &subject.user) {
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
    if !can_read(repo, &subject.user) {
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

fn validate_object_id(id: &str) -> Result<()> {
    if id.len() != 64 {
        return Err(anyhow::anyhow!("object id must be 64 hex chars"));
    }
    if !id.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')) {
        return Err(anyhow::anyhow!("object id must be lowercase hex"));
    }
    Ok(())
}

fn repo_data_dir(state: &AppState, repo_id: &str) -> PathBuf {
    state.data_dir.join(repo_id)
}

fn repo_state_path(state: &AppState, repo_id: &str) -> PathBuf {
    repo_data_dir(state, repo_id).join("repo.json")
}

fn persist_repo(state: &AppState, repo: &Repo) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(repo).context("serialize repo")?;
    let path = repo_state_path(state, &repo.id);
    write_atomic_overwrite(&path, &bytes).context("write repo.json")?;
    Ok(())
}

fn load_repos_from_disk(state: &AppState) -> Result<HashMap<String, Repo>> {
    let mut out = HashMap::new();
    if !state.data_dir.is_dir() {
        return Ok(out);
    }

    for entry in std::fs::read_dir(&state.data_dir).context("read data dir")? {
        let entry = entry.context("read data dir entry")?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let repo_id = entry
            .file_name()
            .into_string()
            .map_err(|_| anyhow::anyhow!("non-utf8 repo dir name"))?;

        let repo = load_repo_from_disk(state, &repo_id)
            .with_context(|| format!("load repo {}", repo_id))?;
        out.insert(repo_id, repo);
    }

    Ok(out)
}

fn load_repo_from_disk(state: &AppState, repo_id: &str) -> Result<Repo> {
    let mut repo = if repo_state_path(state, repo_id).exists() {
        let bytes = std::fs::read(repo_state_path(state, repo_id)).context("read repo.json")?;
        serde_json::from_slice::<Repo>(&bytes).context("parse repo.json")?
    } else {
        default_repo_state(state, repo_id)
    };

    // Ensure id matches directory (best-effort).
    repo.id = repo_id.to_string();

    // Hydrate lists from existing on-disk records (needed for older data dirs).
    let snaps = load_snap_ids_from_disk(state, repo_id).unwrap_or_default();
    if !snaps.is_empty() {
        repo.snaps = snaps;
    }

    let bundles = load_bundles_from_disk(state, repo_id).unwrap_or_default();
    if !bundles.is_empty() {
        repo.bundles = bundles;
    }

    let promotions = load_promotions_from_disk(state, repo_id).unwrap_or_default();
    if !promotions.is_empty() {
        repo.promotions = promotions;
        repo.promotion_state = rebuild_promotion_state(&repo.promotions);
    }

    Ok(repo)
}

fn default_repo_state(state: &AppState, repo_id: &str) -> Repo {
    let mut readers = HashSet::new();
    readers.insert(state.default_user.clone());
    let mut publishers = HashSet::new();
    publishers.insert(state.default_user.clone());

    let mut members = HashSet::new();
    members.insert(state.default_user.clone());
    let default_lane = Lane {
        id: "default".to_string(),
        members,
        heads: HashMap::new(),
        head_history: HashMap::new(),
    };
    let mut lanes = HashMap::new();
    lanes.insert(default_lane.id.clone(), default_lane);

    let gate_graph = GateGraph {
        version: 1,
        terminal_gate: "dev-intake".to_string(),
        gates: vec![GateDef {
            id: "dev-intake".to_string(),
            name: "Dev Intake".to_string(),
            upstream: vec![],
            allow_superpositions: false,
            allow_metadata_only_publications: false,
            required_approvals: 0,
        }],
    };

    let mut scopes = HashSet::new();
    scopes.insert("main".to_string());

    Repo {
        id: repo_id.to_string(),
        owner: state.default_user.clone(),
        readers,
        publishers,
        lanes,
        gate_graph,
        scopes,
        snaps: HashSet::new(),
        publications: Vec::new(),
        bundles: Vec::new(),
        pinned_bundles: HashSet::new(),
        promotions: Vec::new(),
        promotion_state: HashMap::new(),
    }
}

fn load_snap_ids_from_disk(state: &AppState, repo_id: &str) -> Result<HashSet<String>> {
    let dir = repo_data_dir(state, repo_id).join("objects/snaps");
    if !dir.is_dir() {
        return Ok(HashSet::new());
    }

    let mut out = HashSet::new();
    for entry in std::fs::read_dir(&dir).context("read snaps dir")? {
        let entry = entry.context("read snaps dir entry")?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if stem.len() == 64 {
            out.insert(stem.to_string());
        }
    }
    Ok(out)
}

fn load_bundles_from_disk(state: &AppState, repo_id: &str) -> Result<Vec<Bundle>> {
    let dir = repo_data_dir(state, repo_id).join("bundles");
    if !dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).context("read bundles dir")? {
        let entry = entry.context("read bundles dir entry")?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let bytes = std::fs::read(&path).with_context(|| format!("read {}", path.display()))?;
        let bundle: Bundle =
            serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))?;
        out.push(bundle);
    }
    out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(out)
}

fn load_promotions_from_disk(state: &AppState, repo_id: &str) -> Result<Vec<Promotion>> {
    let dir = repo_data_dir(state, repo_id).join("promotions");
    if !dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).context("read promotions dir")? {
        let entry = entry.context("read promotions dir entry")?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let bytes = std::fs::read(&path).with_context(|| format!("read {}", path.display()))?;
        let p: Promotion =
            serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))?;
        out.push(p);
    }
    out.sort_by(|a, b| b.promoted_at.cmp(&a.promoted_at));
    Ok(out)
}

fn rebuild_promotion_state(promotions: &[Promotion]) -> HashMap<String, HashMap<String, String>> {
    let mut tmp: HashMap<String, HashMap<String, (String, String)>> = HashMap::new();
    for p in promotions {
        let scope_entry = tmp.entry(p.scope.clone()).or_default();
        match scope_entry.get(&p.to_gate) {
            None => {
                scope_entry.insert(
                    p.to_gate.clone(),
                    (p.promoted_at.clone(), p.bundle_id.clone()),
                );
            }
            Some((prev_time, _prev_bundle)) => {
                if p.promoted_at > *prev_time {
                    scope_entry.insert(
                        p.to_gate.clone(),
                        (p.promoted_at.clone(), p.bundle_id.clone()),
                    );
                }
            }
        }
    }

    tmp.into_iter()
        .map(|(scope, m)| {
            let m = m
                .into_iter()
                .map(|(to_gate, (_t, bundle_id))| (to_gate, bundle_id))
                .collect::<HashMap<_, _>>();
            (scope, m)
        })
        .collect()
}

fn write_if_absent(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create dir {}", parent.display()))?;
    }
    std::fs::write(path, bytes).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

fn write_atomic_overwrite(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create dir {}", parent.display()))?;
    }
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    std::fs::write(&tmp, bytes).with_context(|| format!("write {}", tmp.display()))?;
    std::fs::rename(&tmp, path)
        .with_context(|| format!("rename {} -> {}", tmp.display(), path.display()))?;
    Ok(())
}

fn now_ts() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "<time>".to_string())
}

fn hash_token(secret: &str) -> String {
    blake3::hash(secret.as_bytes()).to_hex().to_string()
}

fn identity_users_path(data_dir: &std::path::Path) -> std::path::PathBuf {
    data_dir.join("users.json")
}

fn identity_tokens_path(data_dir: &std::path::Path) -> std::path::PathBuf {
    data_dir.join("tokens.json")
}

fn load_identity_from_disk(
    data_dir: &std::path::Path,
) -> Result<(HashMap<String, User>, HashMap<String, AccessToken>)> {
    let mut users: HashMap<String, User> = HashMap::new();
    let mut tokens: HashMap<String, AccessToken> = HashMap::new();

    let users_path = identity_users_path(data_dir);
    if users_path.exists() {
        let bytes = std::fs::read(&users_path).context("read users.json")?;
        let list: Vec<User> = serde_json::from_slice(&bytes).context("parse users.json")?;
        for u in list {
            users.insert(u.id.clone(), u);
        }
    }

    let tokens_path = identity_tokens_path(data_dir);
    if tokens_path.exists() {
        let bytes = std::fs::read(&tokens_path).context("read tokens.json")?;
        let list: Vec<AccessToken> = serde_json::from_slice(&bytes).context("parse tokens.json")?;
        for t in list {
            tokens.insert(t.id.clone(), t);
        }
    }

    Ok((users, tokens))
}

fn persist_identity_to_disk(
    data_dir: &std::path::Path,
    users: &HashMap<String, User>,
    tokens: &HashMap<String, AccessToken>,
) -> Result<()> {
    let mut user_list: Vec<User> = users.values().cloned().collect();
    user_list.sort_by(|a, b| a.handle.cmp(&b.handle));
    let users_bytes = serde_json::to_vec_pretty(&user_list).context("serialize users")?;
    write_atomic_overwrite(&identity_users_path(data_dir), &users_bytes)
        .context("write users.json")?;

    let mut token_list: Vec<AccessToken> = tokens.values().cloned().collect();
    token_list.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    let tokens_bytes = serde_json::to_vec_pretty(&token_list).context("serialize tokens")?;
    write_atomic_overwrite(&identity_tokens_path(data_dir), &tokens_bytes)
        .context("write tokens.json")?;

    Ok(())
}

fn bootstrap_identity(handle: &str, token_secret: &str) -> (User, AccessToken) {
    let created_at = now_ts();
    let user_id = {
        let mut h = blake3::Hasher::new();
        h.update(handle.as_bytes());
        h.update(b"\n");
        h.update(created_at.as_bytes());
        h.finalize().to_hex().to_string()
    };
    let user = User {
        id: user_id.clone(),
        handle: handle.to_string(),
        display_name: None,
        admin: true,
        created_at: created_at.clone(),
    };

    let token_hash = hash_token(token_secret);
    let token_id = {
        let mut h = blake3::Hasher::new();
        h.update(user_id.as_bytes());
        h.update(b"\n");
        h.update(token_hash.as_bytes());
        h.finalize().to_hex().to_string()
    };
    let token = AccessToken {
        id: token_id,
        user_id,
        token_hash,
        label: Some("bootstrap".to_string()),
        created_at,
        last_used_at: None,
        revoked_at: None,
        expires_at: None,
    };

    (user, token)
}

fn generate_token_secret() -> Result<String> {
    // 32 bytes of entropy, hex-encoded.
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes)
        .map_err(|e| anyhow::anyhow!("getrandom: {:?}", e))?;
    let mut out = String::with_capacity(64);
    for b in &bytes {
        out.push_str(&format!("{:02x}", b));
    }
    Ok(out)
}

fn internal_error(err: anyhow::Error) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({"error": err.to_string()})),
    )
        .into_response()
}

fn validate_repo_id(id: &str) -> Result<()> {
    if id.is_empty() {
        return Err(anyhow::anyhow!("repo id cannot be empty"));
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(anyhow::anyhow!("repo id must be lowercase alnum or '-'"));
    }
    Ok(())
}

fn validate_scope_id(id: &str) -> Result<()> {
    if id.is_empty() {
        return Err(anyhow::anyhow!("scope id cannot be empty"));
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '/')
    {
        return Err(anyhow::anyhow!(
            "scope id must be lowercase alnum or '-', '/'"
        ));
    }
    Ok(())
}

fn validate_gate_id(id: &str) -> Result<()> {
    if id.is_empty() {
        return Err(anyhow::anyhow!("gate id cannot be empty"));
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(anyhow::anyhow!("gate id must be lowercase alnum or '-'"));
    }
    Ok(())
}

fn validate_lane_id(id: &str) -> Result<()> {
    if id.is_empty() {
        return Err(anyhow::anyhow!("lane id cannot be empty"));
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(anyhow::anyhow!("lane id must be lowercase alnum or '-'"));
    }
    Ok(())
}

fn validate_gate_graph(graph: &GateGraph) -> Result<()> {
    if graph.version != 1 {
        return Err(anyhow::anyhow!("unsupported gate graph version"));
    }

    if graph.gates.is_empty() {
        return Err(anyhow::anyhow!("gate graph must contain at least one gate"));
    }

    let mut ids = HashSet::new();
    for g in &graph.gates {
        validate_gate_id(&g.id)?;
        if g.name.trim().is_empty() {
            return Err(anyhow::anyhow!("gate name cannot be empty"));
        }
        if !ids.insert(g.id.clone()) {
            return Err(anyhow::anyhow!("duplicate gate id {}", g.id));
        }
    }

    validate_gate_id(&graph.terminal_gate)?;
    if !ids.contains(&graph.terminal_gate) {
        return Err(anyhow::anyhow!("terminal_gate does not exist"));
    }

    // Validate upstream references exist.
    for g in &graph.gates {
        for up in &g.upstream {
            validate_gate_id(up)?;
            if !ids.contains(up) {
                return Err(anyhow::anyhow!(
                    "gate {} references unknown upstream {}",
                    g.id,
                    up
                ));
            }
        }
    }

    // Acyclic check via DFS.
    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();
    for g in &graph.gates {
        dfs_gate(g, graph, &mut visiting, &mut visited)?;
    }

    Ok(())
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
