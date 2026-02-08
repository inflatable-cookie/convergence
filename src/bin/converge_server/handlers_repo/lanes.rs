use super::super::*;

use super::members::MemberHandleRequest;

pub(crate) async fn list_lane_members(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path((repo_id, lane_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, Response> {
    validate_lane_id(&lane_id).map_err(bad_request)?;
    let repos = state.repos.read().await;
    let repo = repos.get(&repo_id).ok_or_else(not_found)?;
    if !subject.admin && repo.owner_user_id.as_ref() != Some(&subject.user_id) {
        return Err(forbidden());
    }
    let lane = repo.lanes.get(&lane_id).ok_or_else(not_found)?;
    Ok(Json(serde_json::json!({
        "lane": lane.id,
        "members": lane.members,
        "member_user_ids": lane.member_user_ids,
    })))
}

pub(crate) async fn add_lane_member(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path((repo_id, lane_id)): Path<(String, String)>,
    Json(payload): Json<MemberHandleRequest>,
) -> Result<Json<serde_json::Value>, Response> {
    validate_lane_id(&lane_id).map_err(bad_request)?;
    validate_user_handle(&payload.handle).map_err(bad_request)?;

    let mut repos = state.repos.write().await;
    let repo = repos.get_mut(&repo_id).ok_or_else(not_found)?;
    if !subject.admin && repo.owner_user_id.as_ref() != Some(&subject.user_id) {
        return Err(forbidden());
    }

    let users = state.users.read().await;
    let (user_id, handle) = users
        .values()
        .find(|u| u.handle == payload.handle)
        .map(|u| (u.id.clone(), u.handle.clone()))
        .ok_or_else(|| bad_request(anyhow::anyhow!("unknown user handle")))?;
    drop(users);

    let lane = repo.lanes.get_mut(&lane_id).ok_or_else(not_found)?;
    lane.members.insert(handle);
    lane.member_user_ids.insert(user_id);
    persist_repo(state.as_ref(), repo).map_err(internal_error)?;
    Ok(Json(serde_json::json!({"ok": true})))
}

pub(crate) async fn remove_lane_member(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path((repo_id, lane_id, handle)): Path<(String, String, String)>,
) -> Result<Json<serde_json::Value>, Response> {
    validate_lane_id(&lane_id).map_err(bad_request)?;
    validate_user_handle(&handle).map_err(bad_request)?;

    let mut repos = state.repos.write().await;
    let repo = repos.get_mut(&repo_id).ok_or_else(not_found)?;
    if !subject.admin && repo.owner_user_id.as_ref() != Some(&subject.user_id) {
        return Err(forbidden());
    }

    let users = state.users.read().await;
    let uid = users
        .values()
        .find(|u| u.handle == handle)
        .map(|u| u.id.clone());
    drop(users);

    let lane = repo.lanes.get_mut(&lane_id).ok_or_else(not_found)?;
    lane.members.remove(&handle);
    if let Some(uid) = uid {
        lane.member_user_ids.remove(&uid);
    }

    persist_repo(state.as_ref(), repo).map_err(internal_error)?;
    Ok(Json(serde_json::json!({"ok": true})))
}

pub(crate) async fn list_lanes(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path(repo_id): Path<String>,
) -> Result<Json<Vec<Lane>>, Response> {
    let repos = state.repos.read().await;
    let repo = repos.get(&repo_id).ok_or_else(not_found)?;
    if !can_read(repo, &subject) {
        return Err(forbidden());
    }

    let mut out: Vec<Lane> = repo.lanes.values().cloned().collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(Json(out))
}
