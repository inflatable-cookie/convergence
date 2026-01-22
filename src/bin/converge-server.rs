use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::{collections::HashMap, collections::HashSet};

use anyhow::{Context, Result};
use axum::extract::{Extension, Query, State};
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

    gate_graph: GateGraph,
    scopes: HashSet<String>,

    snaps: HashSet<String>,
    publications: Vec<Publication>,

    bundles: Vec<Bundle>,
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
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct Publication {
    id: String,
    snap_id: String,
    scope: String,
    gate: String,
    publisher: String,
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
        .route(
            "/repos/:repo_id/gate-graph",
            get(get_gate_graph).put(put_gate_graph),
        )
        .route("/repos/:repo_id/scopes", get(list_scopes).post(create_scope))
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
            "/repos/:repo_id/objects/blobs/:blob_id",
            axum::routing::put(put_blob).get(get_blob),
        )
        .route(
            "/repos/:repo_id/objects/manifests/:manifest_id",
            axum::routing::put(put_manifest).get(get_manifest),
        )
        .route(
            "/repos/:repo_id/objects/snaps/:snap_id",
            axum::routing::put(put_snap).get(get_snap),
        )
        .route(
            "/repos/:repo_id/objects/missing",
            axum::routing::post(find_missing_objects),
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

    let gate_graph = GateGraph {
        version: 1,
        terminal_gate: "dev-intake".to_string(),
        gates: vec![GateDef {
            id: "dev-intake".to_string(),
            name: "Dev Intake".to_string(),
            upstream: vec![],
        }],
    };

    let mut scopes = HashSet::new();
    scopes.insert("main".to_string());

    let snaps = HashSet::new();
    let publications = Vec::new();
    let bundles = Vec::new();

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
    };
    repos.insert(repo.id.clone(), repo.clone());

    std::fs::create_dir_all(repo_data_dir(&state, &repo.id))
        .map_err(|e| internal_error(anyhow::anyhow!(e)))?;

    std::fs::create_dir_all(repo_data_dir(&state, &repo.id).join("bundles"))
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

    // Phase 2: require referenced objects to exist. This simplifies client behavior and
    // ensures that a manifest is never persisted with dangling references.
    for entry in &manifest.entries {
        match &entry.kind {
            converge::model::ManifestEntryKind::File { blob, .. } => {
                let p = repo_data_dir(&state, &repo_id)
                    .join("objects/blobs")
                    .join(blob.as_str());
                if !p.exists() {
                    return Err(bad_request(anyhow::anyhow!(
                        "missing referenced blob {}",
                        blob.as_str()
                    )));
                }
            }
            converge::model::ManifestEntryKind::Dir { manifest } => {
                let p = repo_data_dir(&state, &repo_id)
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
        }
    }

    let path = repo_data_dir(&state, &repo_id)
        .join("objects/manifests")
        .join(format!("{}.json", manifest_id));
    write_if_absent(&path, &body).map_err(internal_error)?;
    Ok(StatusCode::CREATED)
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

    // Record snap existence for later publication validation.
    {
        let mut repos = state.repos.write().await;
        if let Some(repo) = repos.get_mut(&repo_id) {
            repo.snaps.insert(snap_id);
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
    let bytes = std::fs::read(&path)
        .with_context(|| format!("read {}", path.display()))
        .map_err(|e| internal_error(anyhow::anyhow!(e)))?;
    let snap: converge::model::SnapRecord =
        serde_json::from_slice(&bytes).map_err(|e| internal_error(anyhow::anyhow!(e)))?;
    let computed = converge::model::compute_snap_id(
        &snap.created_at,
        &snap.root_manifest,
        snap.message.as_deref(),
    );
    if computed != snap.id {
        return Err(internal_error(anyhow::anyhow!(
            "snap integrity check failed"
        )));
    }
    Ok(json_bytes(bytes))
}

fn json_bytes(bytes: Vec<u8>) -> Response {
    (
        [(header::CONTENT_TYPE, "application/json")],
        axum::body::Bytes::from(bytes),
    )
        .into_response()
}

#[derive(Debug, serde::Deserialize)]
struct MissingObjectsRequest {
    blobs: Vec<String>,
    manifests: Vec<String>,
    snaps: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
struct MissingObjectsResponse {
    missing_blobs: Vec<String>,
    missing_manifests: Vec<String>,
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
        missing_snaps,
    }))
}

#[derive(Debug, serde::Deserialize)]
struct CreatePublicationRequest {
    snap_id: String,
    scope: String,
    gate: String,
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
    if !repo.snaps.contains(&payload.snap_id) {
        return Err(bad_request(anyhow::anyhow!(
            "unknown snap (upload snap first)"
        )));
    }

    let pubrec = Publication {
        id,
        snap_id: payload.snap_id,
        scope: payload.scope,
        gate: payload.gate,
        publisher: subject.user,
        created_at,
    };
    repo.publications.push(pubrec.clone());

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
    root_manifest: String,
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
    validate_object_id(&payload.root_manifest).map_err(bad_request)?;
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

    let id = {
        let mut hasher = blake3::Hasher::new();
        hasher.update(repo_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(payload.scope.as_bytes());
        hasher.update(b"\n");
        hasher.update(payload.gate.as_bytes());
        hasher.update(b"\n");
        hasher.update(payload.root_manifest.as_bytes());
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

    // Validate the root manifest exists in storage.
    let root_manifest_path = repo_data_dir(&state, &repo_id)
        .join("objects/manifests")
        .join(format!("{}.json", payload.root_manifest));
    if !root_manifest_path.exists() {
        return Err(bad_request(anyhow::anyhow!(
            "unknown root_manifest (upload manifest first)"
        )));
    }

    // Validate publication ids exist and match scope/gate.
    for pid in &input_publications {
        let Some(p) = repo.publications.iter().find(|p| &p.id == pid) else {
            return Err(bad_request(anyhow::anyhow!(
                "unknown publication {}",
                pid
            )));
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
    }

    let bundle = Bundle {
        id: id.clone(),
        scope: payload.scope,
        gate: payload.gate,
        root_manifest: payload.root_manifest,
        input_publications,
        created_by: subject.user,
        created_at,
    };

    let bytes = serde_json::to_vec_pretty(&bundle).map_err(|e| internal_error(anyhow::anyhow!(e)))?;
    let path = repo_data_dir(&state, &repo_id)
        .join("bundles")
        .join(format!("{}.json", id));
    write_if_absent(&path, &bytes).map_err(internal_error)?;

    repo.bundles.push(bundle.clone());
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
        if let Some(scope) = &q.scope {
            if &b.scope != scope {
                continue;
            }
        }
        if let Some(gate) = &q.gate {
            if &b.gate != gate {
                continue;
            }
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
    let bundle: Bundle = serde_json::from_slice(&bytes)
        .map_err(|e| internal_error(anyhow::anyhow!(e)))?;
    Ok(Json(bundle))
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
