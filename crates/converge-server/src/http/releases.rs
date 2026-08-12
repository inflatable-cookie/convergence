//! Releases, retention, and GC.

use axum::Json;
use axum::extract::Path;
use axum::extract::Query;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::StatusCode;

use converge_model::{Page, ReleaseRecord, ReleaseRequest, RetentionPolicy};

use crate::authz::Capability;

use crate::engine::Engine;

use super::{
    ApiError, PageParams, SharedState, authorize_repo, authorize_scoped, bad_request,
    internal_error,
};

pub(crate) async fn release(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<ReleaseRequest>,
) -> Result<Json<ReleaseRecord>, ApiError> {
    let authz = authorize_scoped(
        &state,
        &headers,
        &request.repo_id,
        &request.scope_id,
        Capability::Release,
    )?;
    let engine = Engine {
        meta: state.meta.as_ref(),
        objects: state.objects.as_ref(),
    };
    let release = engine
        .release(authz, &id, &request.channel, request.notes)
        .map_err(|err| bad_request(format!("{err:#}")))?;
    Ok(Json(release))
}

pub(crate) async fn list_releases(
    State(state): State<SharedState>,
    Path(repo): Path<String>,
    Query(params): Query<PageParams>,
    headers: HeaderMap,
) -> Result<Json<Page<ReleaseRecord>>, ApiError> {
    authorize_repo(&state, &headers, &repo, Capability::Read)?;
    let limit = params.limit();
    let after_seq = params.after.as_deref().and_then(|c| c.parse::<u64>().ok());
    let rows = state
        .meta
        .list_releases_page(&repo, after_seq, limit)
        .map_err(internal_error)?;
    let next_cursor = (rows.len() == limit).then(|| rows.last().map(|(seq, _)| seq.to_string()));
    Ok(Json(Page {
        items: rows.into_iter().map(|(_, record)| record).collect(),
        next_cursor: next_cursor.flatten(),
    }))
}

/// Resolve `latest`, an exact version, or a range (g02.028). The
/// resolution rules live in `converge_model::releases`, so the CLI and
/// any future front-end cannot disagree with the server about what
/// `latest` means.
pub(crate) async fn release_lookup(
    State(state): State<SharedState>,
    Path((repo, request)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<ReleaseRecord>, ApiError> {
    authorize_repo(&state, &headers, &repo, Capability::Read)?;
    let releases = state.meta.list_releases(&repo).map_err(internal_error)?;
    let parsed: Vec<(semver::Version, bool)> = releases
        .iter()
        .filter_map(|r| {
            converge_model::releases::parse_version(&r.version)
                .ok()
                .map(|v| (v, r.yanked))
        })
        .collect();
    let version = converge_model::releases::resolve(&request, &parsed)
        .map_err(|err| ApiError(StatusCode::NOT_FOUND, err))?
        .to_string();
    releases
        .into_iter()
        .find(|r| r.version == version)
        .map(Json)
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, format!("no release {version}")))
}

pub(crate) async fn yank_release(
    State(state): State<SharedState>,
    Path((repo, version)): Path<(String, String)>,
    headers: HeaderMap,
    Json(request): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let authz = authorize_repo(&state, &headers, &repo, Capability::Release)?;
    let engine = crate::Engine {
        meta: &*state.meta,
        objects: &*state.objects,
    };
    let reason = request["reason"].as_str().unwrap_or("").to_string();
    engine
        .yank(authz, &version, &reason)
        .map_err(|err| ApiError(StatusCode::BAD_REQUEST, format!("{err:#}")))?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub(crate) async fn get_retention(
    State(state): State<SharedState>,
    Path(repo): Path<String>,
    headers: HeaderMap,
) -> Result<Json<RetentionPolicy>, ApiError> {
    authorize_repo(&state, &headers, &repo, Capability::Read)?;
    let policy = state.meta.get_retention(&repo).map_err(internal_error)?;
    Ok(Json(policy))
}

pub(crate) async fn set_retention(
    State(state): State<SharedState>,
    Path(repo): Path<String>,
    headers: HeaderMap,
    Json(policy): Json<RetentionPolicy>,
) -> Result<Json<RetentionPolicy>, ApiError> {
    // Retention is control-plane config: admin only.
    authorize_repo(&state, &headers, &repo, Capability::Admin)?;
    state
        .meta
        .set_retention(&repo, &policy)
        .map_err(|err| bad_request(format!("{err:#}")))?;
    Ok(Json(policy))
}

#[derive(serde::Deserialize)]
pub(crate) struct GcParams {
    #[serde(default = "default_true")]
    dry_run: bool,
}

fn default_true() -> bool {
    true
}

pub(crate) async fn run_gc(
    State(state): State<SharedState>,
    Path(repo): Path<String>,
    Query(params): Query<GcParams>,
    headers: HeaderMap,
) -> Result<Json<crate::gc::GcReport>, ApiError> {
    let authz = authorize_repo(&state, &headers, &repo, Capability::Admin)?;
    // Single-flight (batch 14.4): a second concurrent GC would repeat the
    // whole-store walk for nothing.
    let _running = state
        .gc_running
        .try_lock()
        .map_err(|_| bad_request("a garbage collection is already running for this server"))?;
    let now = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default();

    // GC walks the whole object store; running it inline blocks a runtime
    // worker and stalls every other request's futures on that thread.
    let meta = state.meta.clone();
    let objects = state.objects.clone();
    let dry_run = params.dry_run;
    let report = tokio::task::spawn_blocking(move || {
        let engine = Engine {
            meta: meta.as_ref(),
            objects: objects.as_ref(),
        };
        engine.gc(&authz, dry_run, &now, std::time::Duration::from_secs(300))
    })
    .await
    .map_err(|err| internal_error(anyhow::anyhow!("gc task: {err}")))?
    .map_err(|err| bad_request(format!("{err:#}")))?;
    Ok(Json(report))
}
