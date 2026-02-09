use super::*;

pub(crate) async fn create_scope(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path(repo_id): Path<String>,
    Json(payload): Json<CreateScopeRequest>,
) -> Result<Json<serde_json::Value>, Response> {
    validate_scope_id(&payload.id).map_err(bad_request)?;

    let mut repos = state.repos.write().await;
    let repo = repos.get_mut(&repo_id).ok_or_else(not_found)?;
    if !can_publish(repo, &subject) {
        return Err(forbidden());
    }

    if !repo.scopes.insert(payload.id.clone()) {
        return Err(conflict("scope already exists"));
    }

    persist_repo(state.as_ref(), repo).map_err(internal_error)?;

    Ok(Json(serde_json::json!({"id": payload.id})))
}

pub(crate) async fn list_scopes(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path(repo_id): Path<String>,
) -> Result<Json<Vec<String>>, Response> {
    let repos = state.repos.read().await;
    let repo = repos.get(&repo_id).ok_or_else(not_found)?;
    if !can_read(repo, &subject) {
        return Err(forbidden());
    }

    let mut out: Vec<String> = repo.scopes.iter().cloned().collect();
    out.sort();
    Ok(Json(out))
}
