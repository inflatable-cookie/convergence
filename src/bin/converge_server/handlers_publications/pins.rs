use super::super::*;

pub(super) async fn list_pins(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path(repo_id): Path<String>,
) -> Result<Json<serde_json::Value>, Response> {
    let repos = state.repos.read().await;
    let repo = repos.get(&repo_id).ok_or_else(not_found)?;
    if !can_read(repo, &subject) {
        return Err(forbidden());
    }

    let mut bundles: Vec<String> = repo.pinned_bundles.iter().cloned().collect();
    bundles.sort();
    Ok(Json(serde_json::json!({"bundles": bundles})))
}

pub(super) async fn pin_bundle(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path((repo_id, bundle_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, Response> {
    validate_object_id(&bundle_id).map_err(bad_request)?;

    let mut repos = state.repos.write().await;
    let repo = repos.get_mut(&repo_id).ok_or_else(not_found)?;
    if !can_publish(repo, &subject) {
        return Err(forbidden());
    }

    // Ensure bundle exists (in memory or on disk).
    let _ = if repo.bundles.iter().any(|b| b.id == bundle_id) {
        None
    } else {
        Some(load_bundle_from_disk(state.as_ref(), &repo_id, &bundle_id)?)
    };

    repo.pinned_bundles.insert(bundle_id.clone());
    persist_repo(state.as_ref(), repo).map_err(internal_error)?;

    Ok(Json(
        serde_json::json!({"bundle_id": bundle_id, "pinned": true}),
    ))
}

pub(super) async fn unpin_bundle(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path((repo_id, bundle_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, Response> {
    validate_object_id(&bundle_id).map_err(bad_request)?;

    let mut repos = state.repos.write().await;
    let repo = repos.get_mut(&repo_id).ok_or_else(not_found)?;
    if !can_publish(repo, &subject) {
        return Err(forbidden());
    }

    repo.pinned_bundles.remove(&bundle_id);
    persist_repo(state.as_ref(), repo).map_err(internal_error)?;
    Ok(Json(
        serde_json::json!({"bundle_id": bundle_id, "pinned": false}),
    ))
}
