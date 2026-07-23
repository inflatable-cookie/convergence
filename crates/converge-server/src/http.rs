use std::collections::HashMap;
use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde_json::json;

use converge_model::{
    ApproveRequest, BundleRecord, NegotiateRequest, NegotiateResponse, ObjectId, ObjectSet,
    PromoteRequest, PublishRequest, WIRE_VERSION,
};

use crate::authz::{Capability, authorize};
use crate::engine::{Engine, PublishInput};
use crate::storage::{MetadataStore, ObjectKind, ObjectStore, StoredBundle};

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
        .route("/api/negotiate", post(negotiate))
        .route("/api/objects/:kind/:id", put(put_object).get(get_object))
        .route("/api/publish", post(publish))
        .route("/api/bundles/:id", get(get_bundle))
        .route("/api/bundles/:id/approve", post(approve))
        .route("/api/bundles/:id/promote", post(promote))
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
        status: bundle.status.clone(),
        created_at: bundle.created_at.clone(),
    }
}

async fn healthz() -> Json<serde_json::Value> {
    Json(json!({"ok": true}))
}

async fn negotiate(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(request): Json<NegotiateRequest>,
) -> Result<Json<NegotiateResponse>, ApiError> {
    subject(&state, &headers)?;
    check_wire_version(request.wire_version)?;
    let missing_of = |kind: ObjectKind, ids: &[ObjectId]| -> Vec<ObjectId> {
        ids.iter()
            .filter(|id| !state.objects.has(kind, id))
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
    Path((kind, id)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<serde_json::Value>, ApiError> {
    subject(&state, &headers)?;
    let kind = parse_kind(&kind)?;
    state
        .objects
        .put_bytes(kind, &ObjectId(id), &body)
        .map_err(|err| bad_request(format!("{err:#}")))?;
    Ok(Json(json!({"ok": true})))
}

async fn get_object(
    State(state): State<SharedState>,
    Path((kind, id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Bytes, ApiError> {
    subject(&state, &headers)?;
    let kind = parse_kind(&kind)?;
    state
        .objects
        .get(kind, &ObjectId(id))
        .map(Bytes::from)
        .map_err(|err| ApiError(StatusCode::NOT_FOUND, format!("{err:#}")))
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
    let engine = Engine {
        meta: state.meta.as_ref(),
        objects: state.objects.as_ref(),
    };
    let bundle = engine
        .publish(
            authz,
            PublishInput {
                gate_id: request.gate_id,
                snap_id: request.snap_id,
                root_manifest: request.root_manifest,
                lane_id: request.lane_id,
                notes: request.notes,
            },
        )
        .map_err(|err| bad_request(format!("{err:#}")))?;
    Ok(Json(bundle_record(&bundle)))
}

async fn get_bundle(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<BundleRecord>, ApiError> {
    subject(&state, &headers)?;
    let bundle = state
        .meta
        .get_bundle(&id)
        .map_err(|err| ApiError(StatusCode::NOT_FOUND, format!("{err:#}")))?;
    Ok(Json(bundle_record(&bundle)))
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
