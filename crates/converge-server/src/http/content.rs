//! Object-store transport: negotiate, blobs, and batches.

use axum::Json;
use axum::body::Bytes;
use axum::extract::Path;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use serde_json::json;

use converge_model::{NegotiateRequest, NegotiateResponse, ObjectFrame, ObjectId, ObjectSet};

use crate::authz::Capability;

use crate::storage::{ObjectKind, ObjectStore};

use super::{
    ApiError, AppState, SharedState, authorize_repo, bad_request, check_wire_version,
    internal_error, scoped_objects,
};

/// Wire-contract caps (doc 16 §1c, batch 11.4).
const MAX_BATCH_FRAMES: usize = 4096;

const MAX_BATCH_IDS: usize = 4096;

fn parse_kind(kind: &str) -> Result<ObjectKind, ApiError> {
    match kind {
        "blobs" => Ok(ObjectKind::Blob),
        "manifests" => Ok(ObjectKind::Manifest),
        "recipes" => Ok(ObjectKind::Recipe),
        other => Err(bad_request(format!("unknown object kind {other}"))),
    }
}

pub(crate) async fn negotiate(
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

pub(crate) async fn put_object(
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

pub(crate) async fn get_object(
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
pub(crate) async fn put_batch(
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
pub(crate) async fn get_batch(
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
