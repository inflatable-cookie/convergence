use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::{collections::HashMap, collections::HashSet};

use anyhow::{Context, Result};
use axum::extract::{Extension, State};
use axum::http::{header, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{extract::Path, Json, Router};
use clap::Parser;
use tokio::sync::RwLock;

#[derive(Clone, Debug)]
struct Subject {
    user: String,
}

#[derive(Clone)]
struct AppState {
    user: String,
    token: String,

    data_dir: PathBuf,

    repos: Arc<RwLock<HashMap<String, Repo>>>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct Repo {
    id: String,
    owner: String,
    readers: HashSet<String>,
    publishers: HashSet<String>,
    lanes: HashMap<String, Lane>,

    gates: Vec<Gate>,
    scopes: HashSet<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct Gate {
    id: String,
    name: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct Lane {
    id: String,
    members: HashSet<String>,
}

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

    /// Data directory (future use)
    #[arg(long, default_value = "./converge-data")]
    data_dir: PathBuf,

    /// Database URL (future use)
    #[arg(long)]
    db_url: Option<String>,

    /// Development user name
    #[arg(long, default_value = "dev")]
    dev_user: String,

    /// Development bearer token (dev-only)
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

    let state = Arc::new(AppState {
        user: args.dev_user,
        token: args.dev_token,
        data_dir: args.data_dir,
        repos: Arc::new(RwLock::new(HashMap::new())),
    });

    let authed = Router::new()
        .route("/whoami", get(whoami))
        .route("/repos", get(list_repos).post(create_repo))
        .route("/repos/:repo_id", get(get_repo))
        .route("/repos/:repo_id/permissions", get(get_repo_permissions))
        .route("/repos/:repo_id/lanes", get(list_lanes))
        .route("/repos/:repo_id/gates", get(list_gates))
        .route("/repos/:repo_id/scopes", get(list_scopes).post(create_scope))
        .route(
            "/repos/:repo_id/objects/blobs/:blob_id",
            axum::routing::put(put_blob),
        )
        .route(
            "/repos/:repo_id/objects/manifests/:manifest_id",
            axum::routing::put(put_manifest),
        )
        .route(
            "/repos/:repo_id/objects/snaps/:snap_id",
            axum::routing::put(put_snap),
        )
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

    let local_addr = listener
        .local_addr()
        .context("read listener local addr")?;
    eprintln!("converge-server listening on {}", local_addr);

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

    if token != state.token {
        return unauthorized();
    }

    let mut req = req;
    req.extensions_mut()
        .insert(Subject { user: state.user.clone() });
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
    Json(serde_json::json!({"user": subject.user}))
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
    };
    let mut lanes = HashMap::new();
    lanes.insert(default_lane.id.clone(), default_lane);

    let gates = vec![Gate {
        id: "dev-intake".to_string(),
        name: "Dev Intake".to_string(),
    }];

    let mut scopes = HashSet::new();
    scopes.insert("main".to_string());

    let repo = Repo {
        id: payload.id.clone(),
        owner: subject.user.clone(),
        readers,
        publishers,
        lanes,

        gates,
        scopes,
    };
    repos.insert(repo.id.clone(), repo.clone());

    std::fs::create_dir_all(repo_data_dir(&state, &repo.id))
        .map_err(|e| internal_error(anyhow::anyhow!(e)))?;

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

    Ok(Json(repo.gates.clone()))
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

async fn put_manifest(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path((repo_id, manifest_id)): Path<(String, String)>,
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

    let path = repo_data_dir(&state, &repo_id)
        .join("objects/manifests")
        .join(format!("{}.json", manifest_id));
    write_if_absent(&path, &body).map_err(internal_error)?;
    Ok(StatusCode::CREATED)
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

    let computed = converge::model::compute_snap_id(
        &snap.created_at,
        &snap.root_manifest,
        snap.message.as_deref(),
    );
    if computed != snap.id {
        return Err(bad_request(anyhow::anyhow!(
            "snap id failed verification (expected {}, got {})",
            computed,
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
    Ok(StatusCode::CREATED)
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
        return Err(anyhow::anyhow!(
            "repo id must be lowercase alnum or '-'"
        ));
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
