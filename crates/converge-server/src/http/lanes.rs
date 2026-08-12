//! Lanes, lane heads, and snap upload.

use axum::Json;
use axum::extract::Path;
use axum::extract::Query;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use serde_json::json;

use converge_model::{
    AddLaneMemberRequest, CreateLaneRequest, LaneHead, LaneRecord, Page, SetLaneHeadRequest,
    SnapRecord,
};

use crate::authz::Capability;

use crate::engine::Engine;

use super::{
    ApiError, PageParams, SharedState, authorize_repo, bad_request, forbidden, internal_error,
    page_of,
};

pub(crate) async fn create_lane(
    State(state): State<SharedState>,
    Path(repo): Path<String>,
    headers: HeaderMap,
    Json(request): Json<CreateLaneRequest>,
) -> Result<Json<LaneRecord>, ApiError> {
    // Creating a lane is a publish-capability act on the repo.
    let authz = authorize_repo(&state, &headers, &repo, Capability::Publish)?;
    let subject = authz.subject().to_string();
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

pub(crate) async fn list_lanes(
    State(state): State<SharedState>,
    Path(repo): Path<String>,
    Query(params): Query<PageParams>,
    headers: HeaderMap,
) -> Result<Json<Page<LaneRecord>>, ApiError> {
    authorize_repo(&state, &headers, &repo, Capability::Read)?;
    let limit = params.limit();
    let lanes = state
        .meta
        .list_lanes_page(&repo, params.after.as_deref(), limit)
        .map_err(internal_error)?;
    Ok(Json(page_of(lanes, limit, |l| l.lane_id.clone())))
}

pub(crate) async fn add_lane_member(
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

pub(crate) async fn put_snap(
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

pub(crate) async fn get_snap(
    State(state): State<SharedState>,
    Path((repo, id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<SnapRecord>, ApiError> {
    authorize_repo(&state, &headers, &repo, Capability::Read)?;
    state
        .meta
        .get_snap_record(&repo, &id)
        .map_err(internal_error)?
        .map(Json)
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, format!("no snap {id}")))
}

pub(crate) async fn set_lane_head(
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

pub(crate) async fn get_lane_head(
    State(state): State<SharedState>,
    Path((repo, lane)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<LaneHead>, ApiError> {
    let authz = authorize_repo(&state, &headers, &repo, Capability::Read)?;
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
