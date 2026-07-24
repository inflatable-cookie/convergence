use std::collections::HashMap;
use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde_json::json;

use converge_model::{
    AddLaneMemberRequest, ApproveRequest, BundleProvenance, BundleRecord, CreateLaneRequest,
    EventRecord, InboxReport, LaneHead, LaneRecord, NegotiateRequest, NegotiateResponse,
    ObjectFrame, ObjectId, ObjectSet, PromoteRequest, PublishRequest, ReleaseRecord,
    ReleaseRequest, RetentionPolicy, SetLaneHeadRequest, SnapRecord, VerifyReport, WIRE_VERSION,
};

use crate::authz::{AuthzContext, Capability, authorize};
use crate::engine::{Engine, PublishInput};
use crate::storage::{AssociatingObjects, MetadataStore, ObjectKind, ObjectStore, StoredBundle};

pub struct AppState {
    pub meta: Arc<dyn MetadataStore>,
    pub objects: Arc<dyn ObjectStore>,
    /// token -> subject. Slice-grade identity (arch 14 notes MVP tokens);
    /// real identity arrives with a later roadmap.
    pub tokens: HashMap<String, String>,
}

type SharedState = Arc<AppState>;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/healthz", get(healthz))
        .route("/api/repos/:repo/negotiate", post(negotiate))
        .route(
            "/api/repos/:repo/objects/:kind/:id",
            put(put_object).get(get_object),
        )
        .route("/api/repos/:repo/objects/batch", post(put_batch))
        .route("/api/repos/:repo/objects/batch-get", post(get_batch))
        .route("/api/publish", post(publish))
        .route("/api/bundles/:id", get(get_bundle))
        .route("/api/bundles/:id/provenance", get(get_provenance))
        .route("/api/bundles/:id/verify", get(verify_bundle))
        .route("/api/bundles/:id/approve", post(approve))
        .route("/api/bundles/:id/promote", post(promote))
        .route("/api/repos/:repo/lanes", post(create_lane).get(list_lanes))
        .route(
            "/api/repos/:repo/lanes/:lane/members",
            post(add_lane_member),
        )
        .route("/api/repos/:repo/snaps/:id", put(put_snap).get(get_snap))
        .route("/api/repos/:repo/lane-head", post(set_lane_head))
        .route("/api/repos/:repo/lane-head/:lane", get(get_lane_head))
        .route("/api/repos/:repo/inbox", get(inbox))
        .route("/api/repos/:repo/events", get(list_events))
        .route("/api/bundles/:id/release", post(release))
        .route("/api/repos/:repo/releases", get(list_releases))
        .route("/api/repos/:repo/release/:channel", get(channel_head))
        .route(
            "/api/repos/:repo/retention",
            get(get_retention).put(set_retention),
        )
        .route("/api/repos/:repo/gc", post(run_gc))
        .with_state(Arc::new(state))
}

struct ApiError(StatusCode, String);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"ok": false, "error": self.1}))).into_response()
    }
}

fn bad_request(msg: impl Into<String>) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, msg.into())
}

fn forbidden(msg: impl Into<String>) -> ApiError {
    ApiError(StatusCode::FORBIDDEN, msg.into())
}

fn subject(state: &AppState, headers: &HeaderMap) -> Result<String, ApiError> {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| ApiError(StatusCode::UNAUTHORIZED, "missing bearer token".into()))?;
    state
        .tokens
        .get(token)
        .cloned()
        .ok_or_else(|| ApiError(StatusCode::UNAUTHORIZED, "unknown token".into()))
}

fn parse_kind(kind: &str) -> Result<ObjectKind, ApiError> {
    match kind {
        "blobs" => Ok(ObjectKind::Blob),
        "manifests" => Ok(ObjectKind::Manifest),
        "recipes" => Ok(ObjectKind::Recipe),
        other => Err(bad_request(format!("unknown object kind {other}"))),
    }
}

fn check_wire_version(version: u32) -> Result<(), ApiError> {
    if version != WIRE_VERSION {
        return Err(bad_request(format!(
            "unsupported wire version {version} (server speaks {WIRE_VERSION})"
        )));
    }
    Ok(())
}

fn bundle_record(bundle: &StoredBundle) -> BundleRecord {
    BundleRecord {
        bundle_id: bundle.bundle_id.clone(),
        produced_by_gate_id: bundle.gate_id.clone(),
        scope_id: bundle.scope_id.clone(),
        inputs: bundle.inputs.clone(),
        root_manifest: bundle.root_manifest.clone(),
        base_bundle_id: bundle.base_bundle_id.clone(),
        window: bundle.window,
        strategy: bundle.strategy.clone(),
        status: bundle.status.clone(),
        created_at: bundle.created_at.clone(),
    }
}

async fn healthz() -> Json<serde_json::Value> {
    Json(json!({"ok": true}))
}

/// Repo-scoped object view (batch 11.1): writes record the object→repo
/// association; `has` answers for this repo only.
fn scoped_objects<'a>(state: &'a AppState, repo: &str) -> AssociatingObjects<'a> {
    AssociatingObjects {
        inner: state.objects.as_ref(),
        meta: state.meta.as_ref(),
        repo_id: repo.to_string(),
    }
}

fn authorize_repo(
    state: &AppState,
    headers: &HeaderMap,
    repo: &str,
    capability: Capability,
) -> Result<AuthzContext, ApiError> {
    let subject = subject(state, headers)?;
    authorize(state.meta.as_ref(), &subject, repo, "*", capability)
        .map_err(|err| forbidden(format!("{err:#}")))
}

async fn negotiate(
    State(state): State<SharedState>,
    Path(repo): Path<String>,
    headers: HeaderMap,
    Json(request): Json<NegotiateRequest>,
) -> Result<Json<NegotiateResponse>, ApiError> {
    authorize_repo(&state, &headers, &repo, Capability::Publish)?;
    check_wire_version(request.wire_version)?;
    // Present-but-unassociated counts as missing: the client's idempotent
    // re-put is cheap and repairs the association for this repo.
    let scoped = scoped_objects(&state, &repo);
    let missing_of = |kind: ObjectKind, ids: &[ObjectId]| -> Vec<ObjectId> {
        ids.iter()
            .filter(|id| !scoped.has(kind, id))
            .cloned()
            .collect()
    };
    Ok(Json(NegotiateResponse {
        missing: ObjectSet {
            blobs: missing_of(ObjectKind::Blob, &request.objects.blobs),
            manifests: missing_of(ObjectKind::Manifest, &request.objects.manifests),
            recipes: missing_of(ObjectKind::Recipe, &request.objects.recipes),
        },
    }))
}

async fn put_object(
    State(state): State<SharedState>,
    Path((repo, kind, id)): Path<(String, String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<serde_json::Value>, ApiError> {
    authorize_repo(&state, &headers, &repo, Capability::Publish)?;
    let kind = parse_kind(&kind)?;
    scoped_objects(&state, &repo)
        .put_bytes(kind, &ObjectId(id), &body)
        .map_err(|err| bad_request(format!("{err:#}")))?;
    Ok(Json(json!({"ok": true})))
}

async fn get_object(
    State(state): State<SharedState>,
    Path((repo, kind, id)): Path<(String, String, String)>,
    headers: HeaderMap,
) -> Result<Bytes, ApiError> {
    authorize_repo(&state, &headers, &repo, Capability::Read)?;
    let kind = parse_kind(&kind)?;
    let id = ObjectId(id);
    // Membership check before content: an object another repo uploaded is
    // 404 here, indistinguishable from absent.
    if !state
        .meta
        .object_in_repo(&repo, kind, &id)
        .map_err(|err| bad_request(format!("{err:#}")))?
    {
        return Err(not_found_object(kind, &id));
    }
    state
        .objects
        .get(kind, &id)
        .map(Bytes::from)
        .map_err(|_| not_found_object(kind, &id))
}

fn not_found_object(kind: ObjectKind, id: &ObjectId) -> ApiError {
    ApiError(
        StatusCode::NOT_FOUND,
        format!("no {} {}", kind.dir(), id.as_str()),
    )
}

/// Doc 16 §1c: CBOR frame batch upload.
async fn put_batch(
    State(state): State<SharedState>,
    Path(repo): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<serde_json::Value>, ApiError> {
    authorize_repo(&state, &headers, &repo, Capability::Publish)?;
    let frames: Vec<ObjectFrame> = ciborium::from_reader(body.as_ref())
        .map_err(|err| bad_request(format!("decode batch: {err}")))?;
    let scoped = scoped_objects(&state, &repo);
    let mut stored = 0u64;
    for frame in frames {
        let kind = parse_kind(&frame.kind)?;
        scoped
            .put_bytes(kind, &frame.id, &frame.bytes)
            .map_err(|err| bad_request(format!("{err:#}")))?;
        stored += 1;
    }
    Ok(Json(json!({"ok": true, "stored": stored})))
}

/// Doc 16 §1c: batch download as CBOR frames.
async fn get_batch(
    State(state): State<SharedState>,
    Path(repo): Path<String>,
    headers: HeaderMap,
    Json(request): Json<ObjectSet>,
) -> Result<Bytes, ApiError> {
    authorize_repo(&state, &headers, &repo, Capability::Read)?;
    let mut frames: Vec<ObjectFrame> = Vec::new();
    let mut collect = |kind: ObjectKind, name: &str, ids: &[ObjectId]| -> Result<(), ApiError> {
        for id in ids {
            if !state
                .meta
                .object_in_repo(&repo, kind, id)
                .map_err(|err| bad_request(format!("{err:#}")))?
            {
                return Err(not_found_object(kind, id));
            }
            let bytes = state
                .objects
                .get(kind, id)
                .map_err(|_| not_found_object(kind, id))?;
            frames.push(ObjectFrame {
                kind: name.to_string(),
                id: id.clone(),
                bytes,
            });
        }
        Ok(())
    };
    collect(ObjectKind::Blob, "blobs", &request.blobs)?;
    collect(ObjectKind::Manifest, "manifests", &request.manifests)?;
    collect(ObjectKind::Recipe, "recipes", &request.recipes)?;
    let mut out = Vec::new();
    ciborium::into_writer(&frames, &mut out)
        .map_err(|err| bad_request(format!("encode batch: {err}")))?;
    Ok(Bytes::from(out))
}

async fn publish(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(request): Json<PublishRequest>,
) -> Result<Json<BundleRecord>, ApiError> {
    let subject = subject(&state, &headers)?;
    check_wire_version(request.wire_version)?;
    let authz = authorize(
        state.meta.as_ref(),
        &subject,
        &request.repo_id,
        &request.scope_id,
        Capability::Publish,
    )
    .map_err(|err| forbidden(format!("{err:#}")))?;
    let scoped = scoped_objects(&state, &request.repo_id);
    let engine = Engine {
        meta: state.meta.as_ref(),
        objects: &scoped,
    };
    let bundle = engine
        .publish(
            authz,
            PublishInput {
                gate_id: request.gate_id,
                snap: request.snap,
                base_bundle_id: request.base_bundle_id,
                lane_id: request.lane_id,
                notes: request.notes,
            },
        )
        .map_err(|err| bad_request(format!("{err:#}")))?;
    Ok(Json(bundle_record(&bundle)))
}

async fn create_lane(
    State(state): State<SharedState>,
    Path(repo): Path<String>,
    headers: HeaderMap,
    Json(request): Json<CreateLaneRequest>,
) -> Result<Json<LaneRecord>, ApiError> {
    let subject = subject(&state, &headers)?;
    // Creating a lane is a publish-capability act on the repo.
    authorize(
        state.meta.as_ref(),
        &subject,
        &repo,
        "*",
        Capability::Publish,
    )
    .map_err(|err| forbidden(format!("{err:#}")))?;
    if !matches!(request.visibility.as_str(), "private" | "repo") {
        return Err(bad_request(format!(
            "unknown visibility {}",
            request.visibility
        )));
    }
    let lane = LaneRecord {
        lane_id: request.lane_id,
        repo_id: repo,
        owner: subject,
        members: Vec::new(),
        visibility: request.visibility,
        created_at: time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default(),
    };
    state
        .meta
        .create_lane(&lane)
        .map_err(|err| bad_request(format!("{err:#}")))?;
    Ok(Json(lane))
}

async fn list_lanes(
    State(state): State<SharedState>,
    Path(repo): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Vec<LaneRecord>>, ApiError> {
    let subject = subject(&state, &headers)?;
    authorize(state.meta.as_ref(), &subject, &repo, "*", Capability::Read)
        .map_err(|err| forbidden(format!("{err:#}")))?;
    let lanes = state
        .meta
        .list_lanes(&repo)
        .map_err(|err| bad_request(format!("{err:#}")))?;
    Ok(Json(lanes))
}

async fn add_lane_member(
    State(state): State<SharedState>,
    Path((repo, lane)): Path<(String, String)>,
    headers: HeaderMap,
    Json(request): Json<AddLaneMemberRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let subject = subject(&state, &headers)?;
    let record = state
        .meta
        .get_lane(&repo, &lane)
        .map_err(|err| bad_request(format!("{err:#}")))?
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, format!("no lane {lane}")))?;
    // Membership is managed by the owner.
    if record.owner != subject {
        return Err(forbidden(format!("only {} may add members", record.owner)));
    }
    state
        .meta
        .add_lane_member(&repo, &lane, &request.member)
        .map_err(|err| bad_request(format!("{err:#}")))?;
    Ok(Json(json!({"ok": true})))
}

async fn put_snap(
    State(state): State<SharedState>,
    Path((repo, id)): Path<(String, String)>,
    headers: HeaderMap,
    Json(snap): Json<SnapRecord>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let subject = subject(&state, &headers)?;
    let authz = authorize(
        state.meta.as_ref(),
        &subject,
        &repo,
        "*",
        Capability::Publish,
    )
    .map_err(|err| forbidden(format!("{err:#}")))?;
    if snap.id != id {
        return Err(bad_request("snap id mismatch with path"));
    }
    let engine = Engine {
        meta: state.meta.as_ref(),
        objects: state.objects.as_ref(),
    };
    engine
        .upload_snap_record(&authz, &snap)
        .map_err(|err| bad_request(format!("{err:#}")))?;
    Ok(Json(json!({"ok": true})))
}

async fn get_snap(
    State(state): State<SharedState>,
    Path((repo, id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<SnapRecord>, ApiError> {
    let subject = subject(&state, &headers)?;
    authorize(state.meta.as_ref(), &subject, &repo, "*", Capability::Read)
        .map_err(|err| forbidden(format!("{err:#}")))?;
    state
        .meta
        .get_snap_record(&repo, &id)
        .map_err(|err| bad_request(format!("{err:#}")))?
        .map(Json)
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, format!("no snap {id}")))
}

async fn set_lane_head(
    State(state): State<SharedState>,
    Path(repo): Path<String>,
    headers: HeaderMap,
    Json(request): Json<SetLaneHeadRequest>,
) -> Result<Json<LaneHead>, ApiError> {
    let subject = subject(&state, &headers)?;
    let authz = authorize(
        state.meta.as_ref(),
        &subject,
        &repo,
        "*",
        Capability::Publish,
    )
    .map_err(|err| forbidden(format!("{err:#}")))?;
    let engine = Engine {
        meta: state.meta.as_ref(),
        objects: state.objects.as_ref(),
    };
    let head = engine
        .set_lane_head(authz, request.lane_id, &request.snap_id, request.force)
        .map_err(|err| bad_request(format!("{err:#}")))?;
    Ok(Json(head))
}

async fn get_lane_head(
    State(state): State<SharedState>,
    Path((repo, lane)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<LaneHead>, ApiError> {
    let subject = subject(&state, &headers)?;
    let authz = authorize(state.meta.as_ref(), &subject, &repo, "*", Capability::Read)
        .map_err(|err| forbidden(format!("{err:#}")))?;
    let engine = Engine {
        meta: state.meta.as_ref(),
        objects: state.objects.as_ref(),
    };
    engine
        .check_lane_readable(&authz, &lane)
        .map_err(|err| forbidden(format!("{err:#}")))?;
    state
        .meta
        .get_lane_head(&repo, &lane)
        .map_err(|err| bad_request(format!("{err:#}")))?
        .map(Json)
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, format!("lane {lane} has no head")))
}

#[derive(serde::Deserialize)]
struct InboxParams {
    scope: String,
    #[serde(default)]
    since: Option<String>,
}

async fn inbox(
    State(state): State<SharedState>,
    Path(repo): Path<String>,
    Query(params): Query<InboxParams>,
    headers: HeaderMap,
) -> Result<Json<InboxReport>, ApiError> {
    let subject = subject(&state, &headers)?;
    let authz = authorize(
        state.meta.as_ref(),
        &subject,
        &repo,
        &params.scope,
        Capability::Read,
    )
    .map_err(|err| forbidden(format!("{err:#}")))?;
    let engine = Engine {
        meta: state.meta.as_ref(),
        objects: state.objects.as_ref(),
    };
    let report = engine
        .inbox(&authz, params.since.as_deref())
        .map_err(|err| bad_request(format!("{err:#}")))?;
    Ok(Json(report))
}

#[derive(serde::Deserialize)]
struct EventsParams {
    #[serde(default)]
    since: u64,
}

async fn list_events(
    State(state): State<SharedState>,
    Path(repo): Path<String>,
    Query(params): Query<EventsParams>,
    headers: HeaderMap,
) -> Result<Json<Vec<EventRecord>>, ApiError> {
    let subject = subject(&state, &headers)?;
    authorize(state.meta.as_ref(), &subject, &repo, "*", Capability::Read)
        .map_err(|err| forbidden(format!("{err:#}")))?;
    let events = state
        .meta
        .list_events(&repo, params.since)
        .map_err(|err| bad_request(format!("{err:#}")))?;
    Ok(Json(events))
}

/// Resolve a bundle-id-keyed read: the bundle names its repo, the caller
/// must hold `read` there. Unauthorized and absent are both 404 so bundle
/// ids cannot be used as a cross-repo existence oracle.
fn readable_bundle(
    state: &AppState,
    headers: &HeaderMap,
    bundle_id: &str,
) -> Result<StoredBundle, ApiError> {
    let subject = subject(state, headers)?;
    let missing = || ApiError(StatusCode::NOT_FOUND, format!("no bundle {bundle_id}"));
    let bundle = state.meta.get_bundle(bundle_id).map_err(|_| missing())?;
    authorize(
        state.meta.as_ref(),
        &subject,
        &bundle.repo_id,
        &bundle.scope_id,
        Capability::Read,
    )
    .map_err(|_| missing())?;
    Ok(bundle)
}

async fn get_bundle(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<BundleRecord>, ApiError> {
    let bundle = readable_bundle(&state, &headers, &id)?;
    Ok(Json(bundle_record(&bundle)))
}

async fn get_provenance(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<BundleProvenance>, ApiError> {
    let bundle = readable_bundle(&state, &headers, &id)?;
    let mut inputs = Vec::new();
    for publication_id in &bundle.inputs {
        if let Some(publication) = state
            .meta
            .get_publication(publication_id)
            .map_err(|err| bad_request(format!("{err:#}")))?
        {
            inputs.push(publication);
        }
    }
    Ok(Json(BundleProvenance {
        bundle: bundle_record(&bundle),
        inputs,
    }))
}

async fn verify_bundle(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<VerifyReport>, ApiError> {
    let bundle = readable_bundle(&state, &headers, &id)?;
    // Replay writes intermediate objects (until 11.3 gives verify a
    // throwaway store); scope them to the bundle's own repo.
    let scoped = scoped_objects(&state, &bundle.repo_id);
    let engine = Engine {
        meta: state.meta.as_ref(),
        objects: &scoped,
    };
    let report = engine
        .verify(&id)
        .map_err(|err| bad_request(format!("{err:#}")))?;
    Ok(Json(report))
}

async fn approve(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<ApproveRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let subject = subject(&state, &headers)?;
    let authz = authorize(
        state.meta.as_ref(),
        &subject,
        &request.repo_id,
        &request.scope_id,
        Capability::Approve,
    )
    .map_err(|err| forbidden(format!("{err:#}")))?;
    let engine = Engine {
        meta: state.meta.as_ref(),
        objects: state.objects.as_ref(),
    };
    let approvals = engine
        .approve(authz, &id)
        .map_err(|err| bad_request(format!("{err:#}")))?;
    Ok(Json(json!({"ok": true, "approvals": approvals})))
}

async fn release(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<ReleaseRequest>,
) -> Result<Json<ReleaseRecord>, ApiError> {
    let subject = subject(&state, &headers)?;
    let authz = authorize(
        state.meta.as_ref(),
        &subject,
        &request.repo_id,
        &request.scope_id,
        Capability::Release,
    )
    .map_err(|err| forbidden(format!("{err:#}")))?;
    let engine = Engine {
        meta: state.meta.as_ref(),
        objects: state.objects.as_ref(),
    };
    let release = engine
        .release(authz, &id, &request.channel, request.notes)
        .map_err(|err| bad_request(format!("{err:#}")))?;
    Ok(Json(release))
}

async fn list_releases(
    State(state): State<SharedState>,
    Path(repo): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Vec<ReleaseRecord>>, ApiError> {
    let subject = subject(&state, &headers)?;
    authorize(state.meta.as_ref(), &subject, &repo, "*", Capability::Read)
        .map_err(|err| forbidden(format!("{err:#}")))?;
    let releases = state
        .meta
        .list_releases(&repo)
        .map_err(|err| bad_request(format!("{err:#}")))?;
    Ok(Json(releases))
}

async fn channel_head(
    State(state): State<SharedState>,
    Path((repo, channel)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<ReleaseRecord>, ApiError> {
    let subject = subject(&state, &headers)?;
    authorize(state.meta.as_ref(), &subject, &repo, "*", Capability::Read)
        .map_err(|err| forbidden(format!("{err:#}")))?;
    state
        .meta
        .get_channel_head(&repo, &channel)
        .map_err(|err| bad_request(format!("{err:#}")))?
        .map(Json)
        .ok_or_else(|| {
            ApiError(
                StatusCode::NOT_FOUND,
                format!("channel {channel} has no release"),
            )
        })
}

async fn get_retention(
    State(state): State<SharedState>,
    Path(repo): Path<String>,
    headers: HeaderMap,
) -> Result<Json<RetentionPolicy>, ApiError> {
    let subject = subject(&state, &headers)?;
    authorize(state.meta.as_ref(), &subject, &repo, "*", Capability::Read)
        .map_err(|err| forbidden(format!("{err:#}")))?;
    let policy = state
        .meta
        .get_retention(&repo)
        .map_err(|err| bad_request(format!("{err:#}")))?;
    Ok(Json(policy))
}

async fn set_retention(
    State(state): State<SharedState>,
    Path(repo): Path<String>,
    headers: HeaderMap,
    Json(policy): Json<RetentionPolicy>,
) -> Result<Json<RetentionPolicy>, ApiError> {
    let subject = subject(&state, &headers)?;
    // Retention is control-plane config: admin only.
    authorize(state.meta.as_ref(), &subject, &repo, "*", Capability::Admin)
        .map_err(|err| forbidden(format!("{err:#}")))?;
    state
        .meta
        .set_retention(&repo, &policy)
        .map_err(|err| bad_request(format!("{err:#}")))?;
    Ok(Json(policy))
}

#[derive(serde::Deserialize)]
struct GcParams {
    #[serde(default = "default_true")]
    dry_run: bool,
}

fn default_true() -> bool {
    true
}

async fn run_gc(
    State(state): State<SharedState>,
    Path(repo): Path<String>,
    Query(params): Query<GcParams>,
    headers: HeaderMap,
) -> Result<Json<crate::gc::GcReport>, ApiError> {
    let subject = subject(&state, &headers)?;
    let authz = authorize(state.meta.as_ref(), &subject, &repo, "*", Capability::Admin)
        .map_err(|err| forbidden(format!("{err:#}")))?;
    let engine = Engine {
        meta: state.meta.as_ref(),
        objects: state.objects.as_ref(),
    };
    let now = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default();
    let report = engine
        .gc(
            &authz,
            params.dry_run,
            &now,
            std::time::Duration::from_secs(300),
        )
        .map_err(|err| bad_request(format!("{err:#}")))?;
    Ok(Json(report))
}

async fn promote(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<PromoteRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let subject = subject(&state, &headers)?;
    let authz = authorize(
        state.meta.as_ref(),
        &subject,
        &request.repo_id,
        &request.scope_id,
        Capability::Promote,
    )
    .map_err(|err| forbidden(format!("{err:#}")))?;
    let engine = Engine {
        meta: state.meta.as_ref(),
        objects: state.objects.as_ref(),
    };
    engine
        .promote(authz, &id, &request.to_gate)
        .map_err(|err| bad_request(format!("{err:#}")))?;
    Ok(Json(json!({"ok": true})))
}
