//! The publish-to-promote flow: candidates, inbox, events.

use axum::Json;
use axum::extract::Path;
use axum::extract::Query;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use serde_json::json;

use converge_model::{
    ApproveRequest, CandidateProvenance, CandidateRecord, InboxReport, PromoteRequest,
    PublishRequest, VerifyReport,
};

use crate::authz::Capability;

use crate::engine::{Engine, PublishInput};

use crate::storage::StoredCandidate;

use super::{
    ApiError, AppState, SharedState, authorize_repo, authorize_scoped, bad_request,
    check_wire_version, internal_error, scoped_objects,
};

fn candidate_record(candidate: &StoredCandidate) -> CandidateRecord {
    CandidateRecord {
        candidate_id: candidate.candidate_id.clone(),
        produced_by_gate_id: candidate.gate_id.clone(),
        scope_id: candidate.scope_id.clone(),
        inputs: candidate.inputs.clone(),
        root_manifest: candidate.root_manifest.clone(),
        base_candidate_id: candidate.base_candidate_id.clone(),
        window: candidate.window,
        strategy: candidate.strategy.clone(),
        status: candidate.status.clone(),
        created_at: candidate.created_at.clone(),
    }
}

pub(crate) async fn publish(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(request): Json<PublishRequest>,
) -> Result<Json<CandidateRecord>, ApiError> {
    check_wire_version(request.wire_version)?;
    let authz = authorize_scoped(
        &state,
        &headers,
        &request.repo_id,
        &request.scope_id,
        Capability::Publish,
    )?;
    let scoped = scoped_objects(&state, &request.repo_id);
    let engine = Engine {
        meta: state.meta.as_ref(),
        objects: &scoped,
    };
    let candidate = engine
        .publish(
            authz,
            PublishInput {
                gate_id: request.gate_id,
                snap: request.snap,
                base_candidate_id: request.base_candidate_id,
                lane_id: request.lane_id,
                notes: request.notes,
            },
        )
        .map_err(|err| bad_request(format!("{err:#}")))?;
    Ok(Json(candidate_record(&candidate)))
}

#[derive(serde::Deserialize)]
pub(crate) struct InboxParams {
    scope: String,
    #[serde(default)]
    since: Option<String>,
}

pub(crate) async fn inbox(
    State(state): State<SharedState>,
    Path(repo): Path<String>,
    Query(params): Query<InboxParams>,
    headers: HeaderMap,
) -> Result<Json<InboxReport>, ApiError> {
    let authz = authorize_scoped(&state, &headers, &repo, &params.scope, Capability::Read)?;
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
pub(crate) struct EventsParams {
    #[serde(default)]
    since: u64,
}

pub(crate) async fn list_events(
    State(state): State<SharedState>,
    Path(repo): Path<String>,
    Query(params): Query<EventsParams>,
    headers: HeaderMap,
) -> Result<Json<converge_model::EventPage>, ApiError> {
    authorize_repo(&state, &headers, &repo, Capability::Read)?;
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

/// Resolve a candidate-id-keyed read: the candidate names its repo, the caller
/// must hold `read` there. Unauthorized and absent are both 404 so candidate
/// ids cannot be used as a cross-repo existence oracle.
fn readable_candidate(
    state: &AppState,
    headers: &HeaderMap,
    candidate_id: &str,
) -> Result<StoredCandidate, ApiError> {
    let missing = || {
        ApiError(
            StatusCode::NOT_FOUND,
            format!("no candidate {candidate_id}"),
        )
    };
    let candidate = state
        .meta
        .get_candidate(candidate_id)
        .map_err(|_| missing())?;
    // A refusal here is a 404 on purpose (batch 11.3): whether a candidate
    // exists is itself readable-only information.
    authorize_scoped(
        state,
        headers,
        &candidate.repo_id,
        &candidate.scope_id,
        Capability::Read,
    )
    .map_err(|_| missing())?;
    Ok(candidate)
}

pub(crate) async fn get_candidate(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<CandidateRecord>, ApiError> {
    let candidate = readable_candidate(&state, &headers, &id)?;
    Ok(Json(candidate_record(&candidate)))
}

pub(crate) async fn get_provenance(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<CandidateProvenance>, ApiError> {
    let candidate = readable_candidate(&state, &headers, &id)?;
    let mut inputs = Vec::new();
    for publication_id in &candidate.inputs {
        if let Some(publication) = state
            .meta
            .get_publication(publication_id)
            .map_err(internal_error)?
        {
            inputs.push(publication);
        }
    }
    Ok(Json(CandidateProvenance {
        candidate: candidate_record(&candidate),
        inputs,
    }))
}

pub(crate) async fn verify_candidate(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<VerifyReport>, ApiError> {
    readable_candidate(&state, &headers, &id)?;
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

pub(crate) async fn approve(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<ApproveRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let authz = authorize_scoped(
        &state,
        &headers,
        &request.repo_id,
        &request.scope_id,
        Capability::Approve,
    )?;
    let engine = Engine {
        meta: state.meta.as_ref(),
        objects: state.objects.as_ref(),
    };
    let approvals = engine
        .approve(authz, &id)
        .map_err(|err| bad_request(format!("{err:#}")))?;
    Ok(Json(json!({"ok": true, "approvals": approvals})))
}

pub(crate) async fn promote(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<PromoteRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let authz = authorize_scoped(
        &state,
        &headers,
        &request.repo_id,
        &request.scope_id,
        Capability::Promote,
    )?;
    let engine = Engine {
        meta: state.meta.as_ref(),
        objects: state.objects.as_ref(),
    };
    engine
        .promote(authz, &id, &request.to_gate)
        .map_err(|err| bad_request(format!("{err:#}")))?;
    Ok(Json(json!({"ok": true})))
}
