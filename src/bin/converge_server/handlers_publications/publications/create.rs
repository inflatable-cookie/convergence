use super::*;

pub(super) async fn create_publication(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path(repo_id): Path<String>,
    Json(payload): Json<CreatePublicationRequest>,
) -> Result<Json<Publication>, Response> {
    validate::validate_publication_request(&payload)?;
    let created_at = validate::created_at()?;
    let id = validate::publication_id(&repo_id, &payload, &subject.user, &created_at);

    let mut repos = state.repos.write().await;
    let repo = repos.get_mut(&repo_id).ok_or_else(not_found)?;
    validate::enforce_publication_constraints(repo, &payload, &subject)?;

    let snap = read_snap(state.as_ref(), &repo_id, &payload.snap_id)?;
    validate_manifest_tree_availability(
        state.as_ref(),
        &repo_id,
        snap.root_manifest.as_str(),
        !payload.metadata_only,
    )?;

    let pubrec = Publication {
        id,
        snap_id: payload.snap_id,
        scope: payload.scope,
        gate: payload.gate,
        publisher: subject.user,
        publisher_user_id: Some(subject.user_id),
        created_at,
        resolution: payload.resolution,
    };
    repo.publications.push(pubrec.clone());

    persist_repo(state.as_ref(), repo).map_err(internal_error)?;
    Ok(Json(pubrec))
}
