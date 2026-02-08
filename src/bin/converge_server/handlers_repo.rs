use super::*;

#[derive(Debug, serde::Deserialize)]
pub(super) struct CreateRepoRequest {
    id: String,
}

pub(super) async fn create_repo(
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
    let mut reader_user_ids = HashSet::new();
    reader_user_ids.insert(subject.user_id.clone());

    let mut publishers = HashSet::new();
    publishers.insert(subject.user.clone());
    let mut publisher_user_ids = HashSet::new();
    publisher_user_ids.insert(subject.user_id.clone());

    let mut members = HashSet::new();
    members.insert(subject.user.clone());
    let mut member_user_ids = HashSet::new();
    member_user_ids.insert(subject.user_id.clone());
    let default_lane = Lane {
        id: "default".to_string(),
        members,
        member_user_ids,
        heads: HashMap::new(),
        head_history: HashMap::new(),
    };
    let mut lanes = HashMap::new();
    lanes.insert(default_lane.id.clone(), default_lane);

    let gate_graph = GateGraph {
        version: 1,
        gates: vec![GateDef {
            id: "dev-intake".to_string(),
            name: "Dev Intake".to_string(),
            upstream: vec![],
            allow_releases: true,
            allow_superpositions: false,
            allow_metadata_only_publications: false,
            required_approvals: 0,
        }],
    };

    let mut scopes = HashSet::new();
    scopes.insert("main".to_string());

    let snaps = HashSet::new();
    let publications = Vec::new();
    let bundles = Vec::new();
    let pinned_bundles = HashSet::new();
    let promotions = Vec::new();
    let promotion_state = HashMap::new();
    let releases = Vec::new();

    let repo = Repo {
        id: payload.id.clone(),
        owner: subject.user.clone(),
        owner_user_id: Some(subject.user_id.clone()),
        readers,
        reader_user_ids,
        publishers,
        publisher_user_ids,
        lanes,

        gate_graph,
        scopes,

        snaps,
        publications,

        bundles,

        pinned_bundles,

        promotions,
        promotion_state,

        releases,
    };
    repos.insert(repo.id.clone(), repo.clone());

    std::fs::create_dir_all(repo_data_dir(&state, &repo.id))
        .map_err(|e| internal_error(anyhow::anyhow!(e)))?;

    std::fs::create_dir_all(repo_data_dir(&state, &repo.id).join("bundles"))
        .map_err(|e| internal_error(anyhow::anyhow!(e)))?;

    std::fs::create_dir_all(repo_data_dir(&state, &repo.id).join("promotions"))
        .map_err(|e| internal_error(anyhow::anyhow!(e)))?;

    std::fs::create_dir_all(repo_data_dir(&state, &repo.id).join("releases"))
        .map_err(|e| internal_error(anyhow::anyhow!(e)))?;

    persist_repo(state.as_ref(), &repo).map_err(internal_error)?;

    Ok(Json(repo))
}

pub(super) async fn list_repos(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
) -> Result<Json<Vec<Repo>>, Response> {
    let repos = state.repos.read().await;
    let mut out = Vec::new();
    for repo in repos.values() {
        if can_read(repo, &subject) {
            out.push(repo.clone());
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(Json(out))
}

pub(super) async fn get_repo(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path(repo_id): Path<String>,
) -> Result<Json<Repo>, Response> {
    let repos = state.repos.read().await;
    let repo = repos.get(&repo_id).ok_or_else(not_found)?;
    if !can_read(repo, &subject) {
        return Err(forbidden());
    }
    Ok(Json(repo.clone()))
}

pub(super) async fn get_repo_permissions(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path(repo_id): Path<String>,
) -> Result<Json<serde_json::Value>, Response> {
    let repos = state.repos.read().await;
    let repo = repos.get(&repo_id).ok_or_else(not_found)?;
    Ok(Json(serde_json::json!({
        "read": can_read(repo, &subject),
        "publish": can_publish(repo, &subject)
    })))
}

#[derive(Debug, serde::Deserialize)]
pub(super) struct MemberHandleRequest {
    handle: String,

    #[serde(default)]
    role: Option<String>,
}

pub(super) async fn list_repo_members(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path(repo_id): Path<String>,
) -> Result<Json<serde_json::Value>, Response> {
    let repos = state.repos.read().await;
    let repo = repos.get(&repo_id).ok_or_else(not_found)?;
    if !subject.admin && repo.owner_user_id.as_ref() != Some(&subject.user_id) {
        return Err(forbidden());
    }
    Ok(Json(serde_json::json!({
        "owner": repo.owner,
        "readers": repo.readers,
        "publishers": repo.publishers,
        "owner_user_id": repo.owner_user_id,
        "reader_user_ids": repo.reader_user_ids,
        "publisher_user_ids": repo.publisher_user_ids,
    })))
}

pub(super) async fn add_repo_member(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path(repo_id): Path<String>,
    Json(payload): Json<MemberHandleRequest>,
) -> Result<Json<serde_json::Value>, Response> {
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

    let role = payload.role.unwrap_or_else(|| "read".to_string());
    match role.as_str() {
        "read" => {
            repo.readers.insert(handle);
            repo.reader_user_ids.insert(user_id);
        }
        "publish" => {
            repo.readers.insert(handle.clone());
            repo.reader_user_ids.insert(user_id.clone());
            repo.publishers.insert(handle);
            repo.publisher_user_ids.insert(user_id);
        }
        _ => return Err(bad_request(anyhow::anyhow!("unknown role"))),
    }

    persist_repo(state.as_ref(), repo).map_err(internal_error)?;
    Ok(Json(serde_json::json!({"ok": true})))
}

pub(super) async fn remove_repo_member(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path((repo_id, handle)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, Response> {
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

    repo.readers.remove(&handle);
    repo.publishers.remove(&handle);
    if let Some(uid) = uid {
        repo.reader_user_ids.remove(&uid);
        repo.publisher_user_ids.remove(&uid);
    }

    persist_repo(state.as_ref(), repo).map_err(internal_error)?;
    Ok(Json(serde_json::json!({"ok": true})))
}

pub(super) async fn list_lane_members(
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

pub(super) async fn add_lane_member(
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

pub(super) async fn remove_lane_member(
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

pub(super) async fn list_lanes(
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

#[derive(Debug, serde::Deserialize)]
pub(super) struct UpdateLaneHeadRequest {
    snap_id: String,

    #[serde(default)]
    client_id: Option<String>,
}

pub(super) async fn update_lane_head_me(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path((repo_id, lane_id)): Path<(String, String)>,
    Json(payload): Json<UpdateLaneHeadRequest>,
) -> Result<Json<LaneHead>, Response> {
    validate_lane_id(&lane_id).map_err(bad_request)?;
    validate_object_id(&payload.snap_id).map_err(bad_request)?;

    let mut repos = state.repos.write().await;
    let repo = repos.get_mut(&repo_id).ok_or_else(not_found)?;
    if !can_publish(repo, &subject) {
        return Err(forbidden());
    }

    let lane = repo.lanes.get_mut(&lane_id).ok_or_else(not_found)?;
    if !lane.members.contains(&subject.user) && !lane.member_user_ids.contains(&subject.user_id) {
        return Err(forbidden());
    }

    if !repo.snaps.contains(&payload.snap_id) {
        return Err(bad_request(anyhow::anyhow!(
            "unknown snap (upload snap first)"
        )));
    }

    let updated_at = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|e| internal_error(anyhow::anyhow!(e)))?;

    let head = LaneHead {
        snap_id: payload.snap_id,
        updated_at,
        client_id: payload.client_id,
    };
    lane.heads.insert(subject.user.clone(), head.clone());

    let hist = lane.head_history.entry(subject.user.clone()).or_default();
    // Keep newest first.
    hist.insert(0, head.clone());
    if hist.len() > LANE_HEAD_HISTORY_KEEP_LAST {
        hist.truncate(LANE_HEAD_HISTORY_KEEP_LAST);
    }
    persist_repo(state.as_ref(), repo).map_err(internal_error)?;
    Ok(Json(head))
}

pub(super) async fn get_lane_head(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path((repo_id, lane_id, user)): Path<(String, String, String)>,
) -> Result<Json<LaneHead>, Response> {
    validate_lane_id(&lane_id).map_err(bad_request)?;

    let repos = state.repos.read().await;
    let repo = repos.get(&repo_id).ok_or_else(not_found)?;
    if !can_read(repo, &subject) {
        return Err(forbidden());
    }
    let lane = repo.lanes.get(&lane_id).ok_or_else(not_found)?;
    if !lane.members.contains(&subject.user) && !lane.member_user_ids.contains(&subject.user_id) {
        return Err(forbidden());
    }

    let head = lane.heads.get(&user).ok_or_else(not_found)?;
    Ok(Json(head.clone()))
}
