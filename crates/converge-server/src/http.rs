use std::collections::HashMap;
use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use serde_json::json;

use converge_model::{
    AddLaneMemberRequest, ApproveRequest, BundleProvenance, BundleRecord, CreateLaneRequest,
    InboxReport, LaneHead, LaneRecord, NegotiateRequest, NegotiateResponse, ObjectFrame, ObjectId,
    ObjectSet, Page, PromoteRequest, PublishRequest, ReleaseRecord, ReleaseRequest,
    RetentionPolicy, SetLaneHeadRequest, SnapRecord, VerifyReport, WIRE_VERSION,
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
    /// Held for the duration of a GC run so a second one is refused
    /// rather than repeating the walk (batch 14.4).
    pub gc_running: Arc<tokio::sync::Mutex<()>>,
}

type SharedState = Arc<AppState>;

/// Wire-contract caps (doc 16 §1c, batch 11.4).
const MAX_BATCH_FRAMES: usize = 4096;
const MAX_BATCH_IDS: usize = 4096;
const MAX_BODY_BYTES: usize = 64 * 1024 * 1024;
/// Page cap for cursor listings (batch 15.2), matching the event feed's.
const MAX_PAGE_ITEMS: usize = 1000;

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
        .route("/api/repos", post(create_repo))
        .route(
            "/api/repos/:repo/members",
            post(add_member).get(list_members),
        )
        .route("/api/repos/:repo/members/:subject", delete(remove_member))
        .route("/api/repos/:repo/keys", post(register_key).get(list_keys))
        .route("/api/repos/:repo/secrets", get(list_secrets))
        .route(
            "/api/repos/:repo/secrets/:name",
            put(set_secret).get(get_secret).delete(delete_secret),
        )
        .route(
            "/api/repos/:repo/scopes",
            post(create_scope).get(list_scopes),
        )
        .route("/api/repos/:repo/lanes", post(create_lane).get(list_lanes))
        .route(
            "/api/repos/:repo/lanes/:lane/members",
            post(add_lane_member),
        )
        .route("/api/repos/:repo/snaps/:id", put(put_snap).get(get_snap))
        .route("/api/repos/:repo/lane-head", post(set_lane_head))
        .route("/api/repos/:repo/lane-head/:lane", get(get_lane_head))
        .route("/api/repos/:repo/gates", get(get_gates))
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
        .layer(axum::extract::DefaultBodyLimit::max(MAX_BODY_BYTES))
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

/// Storage-layer failure (batch 11.3): the chain is logged server-side;
/// only a stable public message crosses the wire.
fn internal_error(err: anyhow::Error) -> ApiError {
    eprintln!("internal error: {err:#}");
    ApiError(StatusCode::INTERNAL_SERVER_ERROR, "internal error".into())
}

fn subject(state: &AppState, headers: &HeaderMap) -> Result<String, ApiError> {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| ApiError(StatusCode::UNAUTHORIZED, "missing bearer token".into()))?;
    // Startup-flag tokens first (dev), then tokens issued at runtime
    // (batch 16.3). Issued tokens are stored hashed, so the comparison is
    // over the hash and the database never holds a usable credential.
    if let Some(subject) = state.tokens.get(token) {
        return Ok(subject.clone());
    }
    state
        .meta
        .subject_for_token_hash(&token_hash(token))
        .map_err(internal_error)?
        .ok_or_else(|| ApiError(StatusCode::UNAUTHORIZED, "unknown token".into()))
}

/// Tokens are recognised by hash, never stored raw.
pub fn token_hash(token: &str) -> String {
    blake3::hash(token.as_bytes()).to_hex().to_string()
}

/// Server-wide admin check for operations that name no repo yet.
fn site_admin(state: &AppState, headers: &HeaderMap) -> Result<String, ApiError> {
    let subject = subject(state, headers)?;
    if state.meta.is_site_admin(&subject).map_err(internal_error)? {
        return Ok(subject);
    }
    Err(forbidden(format!("{subject} is not a server admin")))
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
    authorize_repo(&state, &headers, &repo, Capability::SnapSync)?;
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
    authorize_repo(&state, &headers, &repo, Capability::SnapSync)?;
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
        .map_err(internal_error)?
    {
        return Err(not_found_object(kind, &id));
    }
    state
        .objects
        .get(kind, &id)
        .map(Bytes::from)
        .map_err(|err| read_failure(&state, kind, &id, err))
}

/// Distinguish "we do not have it" from "we have it and it is rotten"
/// (batch 18.2).
///
/// Both used to be 404, which is a lie in the second case and a trap:
/// negotiate answers from `has`, so the client is told the server holds
/// the object and then told it does not exist when fetching — a loop
/// with no exit and no mention of corruption. A stored object failing
/// its hash is a server fault, and now says so.
fn read_failure(state: &AppState, kind: ObjectKind, id: &ObjectId, err: anyhow::Error) -> ApiError {
    if state.objects.has(kind, id) {
        eprintln!("corrupt object {} {}: {err:#}", kind.dir(), id.as_str());
        return ApiError(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!(
                "stored {} {} failed its integrity check — the copy on this server                  is corrupt and must be restored or re-uploaded",
                kind.dir(),
                id.as_str()
            ),
        );
    }
    not_found_object(kind, id)
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
    authorize_repo(&state, &headers, &repo, Capability::SnapSync)?;
    let frames: Vec<ObjectFrame> = ciborium::from_reader(body.as_ref())
        .map_err(|err| bad_request(format!("decode batch: {err}")))?;
    if frames.len() > MAX_BATCH_FRAMES {
        return Err(bad_request(format!(
            "batch of {} frames exceeds the {MAX_BATCH_FRAMES}-frame cap; split the upload",
            frames.len()
        )));
    }
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
    let requested = request.blobs.len() + request.manifests.len() + request.recipes.len();
    if requested > MAX_BATCH_IDS {
        return Err(bad_request(format!(
            "batch-get of {requested} ids exceeds the {MAX_BATCH_IDS}-id cap; split the request"
        )));
    }
    let mut frames: Vec<ObjectFrame> = Vec::new();
    let mut collect = |kind: ObjectKind, name: &str, ids: &[ObjectId]| -> Result<(), ApiError> {
        for id in ids {
            if !state
                .meta
                .object_in_repo(&repo, kind, id)
                .map_err(internal_error)?
            {
                return Err(not_found_object(kind, id));
            }
            let bytes = state
                .objects
                .get(kind, id)
                .map_err(|err| read_failure(&state, kind, id, err))?;
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
    // `personal/<subject>` is reserved (arch 14 §4): creating another
    // subject's personal lane would capture their default publishes.
    if request.lane_id.starts_with("personal/") && request.lane_id != format!("personal/{subject}")
    {
        return Err(forbidden(format!(
            "lane id {} is in the reserved personal/ namespace",
            request.lane_id
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

/// Register a scope (batch 14.3). Admin-only: scopes define the
/// partitioning of a repo, so minting them is a policy act.
/// Create a repo with its `default` scope and an `intake` gate (batch
/// 16.3). Server admins only: this runs before the repo exists, so there
/// is no repo-scoped grant to check.
async fn create_repo(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(request): Json<converge_model::CreateRepoRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let admin = site_admin(&state, &headers)?;
    if request.repo_id.is_empty() || request.repo_id == "*" {
        return Err(bad_request(format!(
            "invalid repo id {:?}",
            request.repo_id
        )));
    }
    if state
        .meta
        .repo_exists(&request.repo_id)
        .map_err(internal_error)?
    {
        return Err(bad_request(format!("repo {} exists", request.repo_id)));
    }
    let created_at = now_rfc3339()?;
    state
        .meta
        .create_repo(&request.repo_id)
        .map_err(internal_error)?;
    state
        .meta
        .create_scope(&request.repo_id, "default", &created_at)
        .map_err(internal_error)?;
    // A repo with no gate cannot accept a publish, so an empty one would
    // be a second setup step nobody is told about.
    state
        .meta
        .set_gate_graph(
            &request.repo_id,
            &converge_model::GateGraph {
                gates: vec![converge_model::GateNode {
                    gate_id: "intake".into(),
                    name: "Intake".into(),
                    upstreams: vec![],
                    required_approvals: 0,
                    strategy: "whole-file".into(),
                    may_release: true,
                }],
            },
        )
        .map_err(internal_error)?;
    // The creator can work in it immediately.
    for capability in [
        Capability::Read,
        Capability::Publish,
        Capability::Resolve,
        Capability::Approve,
        Capability::Promote,
        Capability::Release,
        Capability::Admin,
    ] {
        state
            .meta
            .add_grant(&admin, &request.repo_id, "*", capability.as_str())
            .map_err(internal_error)?;
    }
    Ok(Json(serde_json::json!({
        "repo_id": request.repo_id,
        "scope": "default",
        "gate": "intake",
    })))
}

/// Add a teammate: upsert, grant, and optionally issue a token (batch
/// 16.3, audit P1.6). Repo admins only.
async fn add_member(
    State(state): State<SharedState>,
    Path(repo): Path<String>,
    headers: HeaderMap,
    Json(request): Json<converge_model::AddMemberRequest>,
) -> Result<Json<converge_model::MemberAdded>, ApiError> {
    let subject = subject(&state, &headers)?;
    authorize(state.meta.as_ref(), &subject, &repo, "*", Capability::Admin)
        .map_err(|err| forbidden(format!("{err:#}")))?;
    if request.subject.is_empty() {
        return Err(bad_request("member subject is required"));
    }
    if request.capabilities.is_empty() {
        return Err(bad_request("at least one capability is required"));
    }
    // Unknown capability strings would sit in the table forever granting
    // nothing, so they are refused rather than stored.
    let known = [
        Capability::Read,
        Capability::SnapSync,
        Capability::Publish,
        Capability::Resolve,
        Capability::Approve,
        Capability::Promote,
        Capability::Release,
        Capability::Admin,
    ];
    for capability in &request.capabilities {
        if !known.iter().any(|c| c.as_str() == capability) {
            return Err(bad_request(format!(
                "unknown capability {capability}; known: {}",
                known
                    .iter()
                    .map(Capability::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
    }
    let scope_pattern = if request.scope_pattern.is_empty() {
        "*".to_string()
    } else {
        request.scope_pattern.clone()
    };

    state
        .meta
        .upsert_user(&request.subject)
        .map_err(internal_error)?;
    for capability in &request.capabilities {
        state
            .meta
            .add_grant(&request.subject, &repo, &scope_pattern, capability)
            .map_err(internal_error)?;
    }

    let token = if request.issue_token {
        let token = mint_token()?;
        state
            .meta
            .create_token(&token_hash(&token), &request.subject)
            .map_err(internal_error)?;
        Some(token)
    } else {
        None
    };

    Ok(Json(converge_model::MemberAdded {
        subject: request.subject,
        granted: request.capabilities,
        token,
    }))
}

/// Store an encrypted secret (batch 19.2).
///
/// The server is an envelope service: it validates the *shape* of the
/// request and never the ciphertext, which it stores and returns
/// byte-exact.
async fn set_secret(
    State(state): State<SharedState>,
    Path((repo, name)): Path<(String, String)>,
    headers: HeaderMap,
    Json(request): Json<converge_model::SetSecretRequest>,
) -> Result<Json<converge_model::SecretSummary>, ApiError> {
    let authz = authorize_repo(&state, &headers, &repo, Capability::Secret)?;
    let subject = authz.subject().to_string();
    validate_secret_name(&name)?;
    if request.ciphertext.is_empty() {
        return Err(bad_request("ciphertext is empty"));
    }
    if request.recipients.is_empty() {
        return Err(bad_request(
            "a secret with no recipients could never be read again",
        ));
    }

    let now = now_rfc3339()?;
    // A re-share keeps the previous value version and timestamp, so an
    // audit can still answer "when was this credential last changed?"
    // after any number of membership edits.
    let previous = state
        .meta
        .get_secret(&repo, &subject, &name)
        .map_err(internal_error)?;
    let (value_version, value_updated_at) = match (&previous, request.value_changed) {
        (_, true) => (
            previous.as_ref().map(|p| p.value_version).unwrap_or(0) + 1,
            now.clone(),
        ),
        (Some(previous), false) => (previous.value_version, previous.value_updated_at.clone()),
        (None, false) => (1, now.clone()),
    };

    let record = converge_model::SecretRecord {
        name: name.clone(),
        owner: subject.clone(),
        recipients: request.recipients,
        ciphertext: request.ciphertext,
        version: request.expected_version + 1,
        value_version,
        value_updated_at,
        updated_at: now,
        updated_by: subject.clone(),
    };
    // Guarded like publish and promote (doc 14 §3): a stale write fails
    // the batch instead of quietly winning, so two people rotating the
    // same credential cannot lose one of the rotations.
    state
        .meta
        .apply_batch(&[
            crate::storage::MetaOp::AssertSecretVersion {
                repo_id: repo.clone(),
                owner: subject.clone(),
                name: name.clone(),
                expected: request.expected_version,
            },
            crate::storage::MetaOp::PutSecret {
                repo_id: repo.clone(),
                record: record.clone(),
            },
            crate::storage::MetaOp::AddEvent {
                repo_id: repo.clone(),
                kind: "secret.changed".into(),
                subject_id: format!("{subject}/{name}@{}", record.version),
                created_at: record.updated_at.clone(),
            },
        ])
        .map_err(|err| {
            if err.is::<crate::storage::BatchConflict>() {
                ApiError(StatusCode::CONFLICT, format!("{err}"))
            } else {
                internal_error(err)
            }
        })?;
    Ok(Json(summarize_secret(&record)))
}

/// Fetch ciphertext. Only a recipient may, which the grant check alone
/// does not give us: `admin` subsumes every capability (doc 14 §4), so
/// without this a repo admin could pull every envelope in the repo.
async fn get_secret(
    State(state): State<SharedState>,
    Path((repo, name)): Path<(String, String)>,
    Query(params): Query<SecretQuery>,
    headers: HeaderMap,
) -> Result<Json<converge_model::SecretRecord>, ApiError> {
    let authz = authorize_repo(&state, &headers, &repo, Capability::Secret)?;
    let subject = authz.subject().to_string();
    let record = find_secret(&state, &repo, &name, &subject, params.owner.as_deref())?;

    let keys = state.meta.list_public_keys(&repo).map_err(internal_error)?;
    if !is_recipient(&record, &subject, &keys) {
        // Same shape as "no such secret": whether a secret exists is
        // itself information a non-recipient has no claim to.
        return Err(not_found_secret(&name));
    }

    // Every fetch that enables a decryption is on the record (doc 19
    // §10c). A file on disk cannot tell you it was read; this can, and
    // that is what turns a leaked credential into a bounded incident.
    // The trade is deliberate: the server learns when each person uses
    // each secret, which §10c chooses over read-privacy.
    let at = now_rfc3339()?;
    state
        .meta
        .add_event(
            &repo,
            "secret.read",
            &format!("{subject}/{name}@{}", record.version),
            &at,
        )
        .map_err(internal_error)?;
    Ok(Json(record))
}

/// Names and versions, never ciphertext. Deliberately readable by any
/// member: knowing that a secret exists is what lets someone ask to be
/// added to it.
async fn list_secrets(
    State(state): State<SharedState>,
    Path(repo): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Vec<converge_model::SecretSummary>>, ApiError> {
    authorize_repo(&state, &headers, &repo, Capability::Read)?;
    Ok(Json(
        state
            .meta
            .list_secrets(&repo)
            .map_err(internal_error)?
            .iter()
            .map(summarize_secret)
            .collect(),
    ))
}

/// Only the owner may delete: a recipient can read a secret, not
/// destroy someone else's copy of it.
async fn delete_secret(
    State(state): State<SharedState>,
    Path((repo, name)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let authz = authorize_repo(&state, &headers, &repo, Capability::Secret)?;
    let subject = authz.subject().to_string();
    let record = find_secret(&state, &repo, &name, &subject, Some(&subject))?;
    if record.owner != subject {
        return Err(forbidden(format!("{name} belongs to {}", record.owner)));
    }
    state
        .meta
        .delete_secret(&repo, &subject, &name)
        .map_err(internal_error)?;
    Ok(Json(serde_json::json!({ "deleted": name })))
}

/// `?owner=` disambiguates when two people hold the same name.
#[derive(serde::Deserialize)]
struct SecretQuery {
    owner: Option<String>,
}

fn summarize_secret(record: &converge_model::SecretRecord) -> converge_model::SecretSummary {
    converge_model::SecretSummary {
        name: record.name.clone(),
        owner: record.owner.clone(),
        recipients: record.recipients.clone(),
        version: record.version,
        value_version: record.value_version,
        value_updated_at: record.value_updated_at.clone(),
        updated_at: record.updated_at.clone(),
        updated_by: record.updated_by.clone(),
    }
}

fn not_found_secret(name: &str) -> ApiError {
    ApiError(StatusCode::NOT_FOUND, format!("no secret {name}"))
}

/// Resolve a secret by name for a caller (batch 20.1).
///
/// Records are keyed `(repo, owner, name)`, so a name alone is not an
/// address once more than one person can hold a secret. Resolution
/// prefers the caller's own, falls back to the single one they can
/// read, and refuses rather than guessing when several match — the
/// previous code took the first by owner order, which meant two people
/// with a `db-password` silently served whoever sorted first.
fn find_secret(
    state: &AppState,
    repo: &str,
    name: &str,
    subject: &str,
    owner: Option<&str>,
) -> Result<converge_model::SecretRecord, ApiError> {
    let all = state.meta.list_secrets(repo).map_err(internal_error)?;
    let matching: Vec<converge_model::SecretRecord> = all
        .into_iter()
        .filter(|record| record.name == name)
        .filter(|record| owner.is_none_or(|owner| record.owner == owner))
        .collect();

    if let Some(mine) = matching.iter().find(|record| record.owner == subject) {
        return Ok(mine.clone());
    }
    let keys = state.meta.list_public_keys(repo).map_err(internal_error)?;
    let readable: Vec<&converge_model::SecretRecord> = matching
        .iter()
        .filter(|record| is_recipient(record, subject, &keys))
        .collect();
    match readable.as_slice() {
        [only] => Ok((*only).clone()),
        [] => Err(not_found_secret(name)),
        several => {
            let owners: Vec<&str> = several.iter().map(|r| r.owner.as_str()).collect();
            Err(bad_request(format!(
                "{name} is ambiguous: {} hold one; name whose with --owner",
                owners.join(", ")
            )))
        }
    }
}

/// Can `subject` decrypt this record, by owning it or holding one of
/// its recipient keys?
fn is_recipient(
    record: &converge_model::SecretRecord,
    subject: &str,
    keys: &[converge_model::PublicKeyRecord],
) -> bool {
    record.owner == subject
        || record.recipients.iter().any(|key_id| {
            keys.iter()
                .any(|key| &key.key_id == key_id && key.subject == subject)
        })
}

/// Names travel in a URL path and land in a database key, so the
/// grammar is narrow on purpose.
fn validate_secret_name(name: &str) -> Result<(), ApiError> {
    if name.is_empty() || name.len() > 128 {
        return Err(bad_request("secret name must be 1-128 characters"));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        return Err(bad_request(
            "secret name may use letters, digits, dash, underscore and dot",
        ));
    }
    Ok(())
}

/// Register a public key for the *calling* subject (batch 19.1).
///
/// The subject comes from the token, never the body. Letting a caller
/// name someone else would let them register a key that future secrets
/// get sealed to — the whole guarantee, given away in one field.
async fn register_key(
    State(state): State<SharedState>,
    Path(repo): Path<String>,
    headers: HeaderMap,
    Json(request): Json<converge_model::RegisterKeyRequest>,
) -> Result<Json<converge_model::PublicKeyRecord>, ApiError> {
    let authz = authorize_repo(&state, &headers, &repo, Capability::Read)?;
    // Parsed, not trusted: a malformed recipient would be stored and
    // then fail at encryption time, somewhere much less obvious.
    let recipient: age::x25519::Recipient = request
        .public_key
        .trim()
        .parse()
        .map_err(|err| bad_request(format!("not an age recipient: {err}")))?;
    let public_key = recipient.to_string();

    let record = converge_model::PublicKeyRecord {
        key_id: blake3::hash(public_key.as_bytes())
            .to_hex()
            .chars()
            .take(16)
            .collect(),
        subject: authz.subject().to_string(),
        public_key,
        label: request.label,
        created_at: now_rfc3339()?,
    };
    state
        .meta
        .add_public_key(&repo, &record)
        .map_err(internal_error)?;
    Ok(Json(record))
}

/// Every registered key in the repo. Public data: members need each
/// other's keys to share a secret, and hiding them would only mean an
/// out-of-band exchange that is easier to get wrong.
async fn list_keys(
    State(state): State<SharedState>,
    Path(repo): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Vec<converge_model::PublicKeyRecord>>, ApiError> {
    authorize_repo(&state, &headers, &repo, Capability::Read)?;
    Ok(Json(
        state.meta.list_public_keys(&repo).map_err(internal_error)?,
    ))
}

/// Remove every grant a subject holds in this repo (batch 20.2).
///
/// What this does *not* do is re-encrypt anything: the server holds no
/// key that opens a secret (doc 19 §7), so the response names the
/// secrets still sealed to them and leaves the work with the owners who
/// can actually do it. Reporting it is the honest alternative to
/// pretending access was withdrawn.
async fn remove_member(
    State(state): State<SharedState>,
    Path((repo, subject)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<converge_model::MemberRemoved>, ApiError> {
    let authz = authorize_repo(&state, &headers, &repo, Capability::Admin)?;
    if authz.subject() == subject {
        return Err(bad_request(
            "removing yourself would leave the repo without the admin doing it",
        ));
    }

    let grants = state.meta.list_grants(&repo).map_err(internal_error)?;
    let admins: Vec<&String> = grants
        .iter()
        .filter(|(_, capability, _)| capability == Capability::Admin.as_str())
        .map(|(subject, _, _)| subject)
        .collect();
    if admins.len() == 1 && admins[0] == &subject {
        return Err(bad_request(
            "that is the repo's only admin; grant admin to someone else first",
        ));
    }

    let removed = state
        .meta
        .remove_grants(&repo, &subject)
        .map_err(internal_error)?;

    // Which secrets are still sealed to them, so somebody can act.
    let keys = state.meta.list_public_keys(&repo).map_err(internal_error)?;
    let their_keys: Vec<&str> = keys
        .iter()
        .filter(|key| key.subject == subject)
        .map(|key| key.key_id.as_str())
        .collect();
    let still_sealed: Vec<converge_model::SecretSummary> = state
        .meta
        .list_secrets(&repo)
        .map_err(internal_error)?
        .iter()
        .filter(|record| {
            record.owner == subject
                || record
                    .recipients
                    .iter()
                    .any(|key_id| their_keys.contains(&key_id.as_str()))
        })
        .map(summarize_secret)
        .collect();

    Ok(Json(converge_model::MemberRemoved {
        subject,
        grants_removed: removed,
        still_sealed,
    }))
}

async fn list_members(
    State(state): State<SharedState>,
    Path(repo): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Vec<converge_model::MemberRecord>>, ApiError> {
    let subject = subject(&state, &headers)?;
    authorize(state.meta.as_ref(), &subject, &repo, "*", Capability::Read)
        .map_err(|err| forbidden(format!("{err:#}")))?;
    let mut members: Vec<converge_model::MemberRecord> = Vec::new();
    for (subject, capability, scope_pattern) in
        state.meta.list_grants(&repo).map_err(internal_error)?
    {
        match members.last_mut() {
            Some(member) if member.subject == subject => {
                member.grants.push((capability, scope_pattern))
            }
            _ => members.push(converge_model::MemberRecord {
                subject,
                grants: vec![(capability, scope_pattern)],
            }),
        }
    }
    Ok(Json(members))
}

fn now_rfc3339() -> Result<String, ApiError> {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|err| internal_error(anyhow::anyhow!("format timestamp: {err}")))
}

/// 256 bits from the OS CSPRNG, hex encoded. The server keeps only the
/// hash, so this string exists exactly once: in the response that issued
/// it.
fn mint_token() -> Result<String, ApiError> {
    mint_admin_token().map_err(internal_error)
}

/// The same minting the API uses, for the bootstrap path in `main`.
pub fn mint_admin_token() -> anyhow::Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|err| anyhow::anyhow!("read system randomness: {err}"))?;
    Ok(blake3::Hasher::new()
        .update(&bytes)
        .finalize()
        .to_hex()
        .to_string())
}

/// The repo's gate graph (batch 17.1). Reading the shape of the pipeline
/// you publish into needed a server round trip that did not exist.
async fn get_gates(
    State(state): State<SharedState>,
    Path(repo): Path<String>,
    headers: HeaderMap,
) -> Result<Json<converge_model::GateGraph>, ApiError> {
    let subject = subject(&state, &headers)?;
    authorize(state.meta.as_ref(), &subject, &repo, "*", Capability::Read)
        .map_err(|err| forbidden(format!("{err:#}")))?;
    Ok(Json(
        state.meta.get_gate_graph(&repo).map_err(internal_error)?,
    ))
}

async fn create_scope(
    State(state): State<SharedState>,
    Path(repo): Path<String>,
    headers: HeaderMap,
    Json(request): Json<converge_model::CreateScopeRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let subject = subject(&state, &headers)?;
    authorize(state.meta.as_ref(), &subject, &repo, "*", Capability::Admin)
        .map_err(|err| forbidden(format!("{err:#}")))?;
    if request.scope_id.is_empty() || request.scope_id == "*" {
        return Err(bad_request(format!(
            "invalid scope id {:?}",
            request.scope_id
        )));
    }
    let created_at = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|err| internal_error(anyhow::anyhow!("format timestamp: {err}")))?;
    state
        .meta
        .create_scope(&repo, &request.scope_id, &created_at)
        .map_err(internal_error)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn list_scopes(
    State(state): State<SharedState>,
    Path(repo): Path<String>,
    Query(params): Query<PageParams>,
    headers: HeaderMap,
) -> Result<Json<Page<String>>, ApiError> {
    let subject = subject(&state, &headers)?;
    authorize(state.meta.as_ref(), &subject, &repo, "*", Capability::Read)
        .map_err(|err| forbidden(format!("{err:#}")))?;
    let limit = params.limit();
    let scopes = state
        .meta
        .list_scopes_page(&repo, params.after.as_deref(), limit)
        .map_err(internal_error)?;
    Ok(Json(page_of(scopes, limit, |s| s.clone())))
}

/// Cursor paging params (batch 15.2). `limit` is clamped to the page cap
/// whether or not the client sends one.
#[derive(serde::Deserialize)]
struct PageParams {
    #[serde(default)]
    after: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
}

impl PageParams {
    fn limit(&self) -> usize {
        self.limit
            .unwrap_or(MAX_PAGE_ITEMS)
            .clamp(1, MAX_PAGE_ITEMS)
    }
}

/// Build a page, reporting a cursor only when the page filled — a short
/// page means the listing is exhausted.
fn page_of<T>(items: Vec<T>, limit: usize, cursor: impl Fn(&T) -> String) -> Page<T> {
    let next_cursor = (items.len() == limit)
        .then(|| items.last().map(&cursor))
        .flatten();
    Page { items, next_cursor }
}

async fn list_lanes(
    State(state): State<SharedState>,
    Path(repo): Path<String>,
    Query(params): Query<PageParams>,
    headers: HeaderMap,
) -> Result<Json<Page<LaneRecord>>, ApiError> {
    let subject = subject(&state, &headers)?;
    authorize(state.meta.as_ref(), &subject, &repo, "*", Capability::Read)
        .map_err(|err| forbidden(format!("{err:#}")))?;
    let limit = params.limit();
    let lanes = state
        .meta
        .list_lanes_page(&repo, params.after.as_deref(), limit)
        .map_err(internal_error)?;
    Ok(Json(page_of(lanes, limit, |l| l.lane_id.clone())))
}

async fn add_lane_member(
    State(state): State<SharedState>,
    Path((repo, lane)): Path<(String, String)>,
    headers: HeaderMap,
    Json(request): Json<AddLaneMemberRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // Inside the authorize discipline like every other write; the owner
    // check below is the finer gate.
    let authz = authorize_repo(&state, &headers, &repo, Capability::Read)?;
    let subject = authz.subject().to_string();
    let record = state
        .meta
        .get_lane(&repo, &lane)
        .map_err(internal_error)?
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
    let authz = authorize_repo(&state, &headers, &repo, Capability::SnapSync)?;
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
        .map_err(internal_error)?
        .map(Json)
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, format!("no snap {id}")))
}

async fn set_lane_head(
    State(state): State<SharedState>,
    Path(repo): Path<String>,
    headers: HeaderMap,
    Json(request): Json<SetLaneHeadRequest>,
) -> Result<Json<LaneHead>, ApiError> {
    let authz = authorize_repo(&state, &headers, &repo, Capability::SnapSync)?;
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
        .map_err(internal_error)?
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
) -> Result<Json<converge_model::EventPage>, ApiError> {
    let subject = subject(&state, &headers)?;
    authorize(state.meta.as_ref(), &subject, &repo, "*", Capability::Read)
        .map_err(|err| forbidden(format!("{err:#}")))?;
    let floor = state.meta.event_floor(&repo).map_err(internal_error)?;
    let events = state
        .meta
        .list_events(&repo, params.since)
        .map_err(internal_error)?;
    Ok(Json(converge_model::EventPage {
        events,
        floor,
        // A cursor at or below the floor missed pruned events. Hints, so
        // this costs freshness — but the client must be told (batch 14.4).
        gap: params.since < floor,
    }))
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
            .map_err(internal_error)?
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
    readable_bundle(&state, &headers, &id)?;
    // Read-only means read-only (batch 11.3): the replay merges into a
    // scratch overlay; the shared store is untouched by this GET.
    let scratch = crate::storage::ScratchObjects::over(state.objects.as_ref());
    let engine = Engine {
        meta: state.meta.as_ref(),
        objects: &scratch,
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
    Query(params): Query<PageParams>,
    headers: HeaderMap,
) -> Result<Json<Page<ReleaseRecord>>, ApiError> {
    let subject = subject(&state, &headers)?;
    authorize(state.meta.as_ref(), &subject, &repo, "*", Capability::Read)
        .map_err(|err| forbidden(format!("{err:#}")))?;
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
        .map_err(internal_error)?
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
    let policy = state.meta.get_retention(&repo).map_err(internal_error)?;
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
