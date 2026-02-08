use super::super::*;

#[derive(Debug, serde::Deserialize)]
pub(in super::super) struct CreatePublicationRequest {
    snap_id: String,
    scope: String,
    gate: String,

    #[serde(default)]
    metadata_only: bool,

    #[serde(default)]
    resolution: Option<PublicationResolution>,
}

pub(super) async fn create_publication(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path(repo_id): Path<String>,
    Json(payload): Json<CreatePublicationRequest>,
) -> Result<Json<Publication>, Response> {
    validate_object_id(&payload.snap_id).map_err(bad_request)?;
    validate_scope_id(&payload.scope).map_err(bad_request)?;
    validate_gate_id(&payload.gate).map_err(bad_request)?;

    let created_at = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|e| internal_error(anyhow::anyhow!(e)))?;

    let id = {
        let mut hasher = blake3::Hasher::new();
        hasher.update(repo_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(payload.snap_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(payload.scope.as_bytes());
        hasher.update(b"\n");
        hasher.update(payload.gate.as_bytes());
        hasher.update(b"\n");
        hasher.update(subject.user.as_bytes());
        hasher.update(b"\n");
        hasher.update(created_at.as_bytes());
        hasher.finalize().to_hex().to_string()
    };

    let mut repos = state.repos.write().await;
    let repo = repos.get_mut(&repo_id).ok_or_else(not_found)?;
    if !can_publish(repo, &subject) {
        return Err(forbidden());
    }
    if !repo.scopes.contains(&payload.scope) {
        return Err(bad_request(anyhow::anyhow!("unknown scope")));
    }
    if !repo.gate_graph.gates.iter().any(|g| g.id == payload.gate) {
        return Err(bad_request(anyhow::anyhow!("unknown gate")));
    }

    // Enforce at-most-once publication for a given snap+scope+gate.
    // If you need to publish again, create a new snap.
    if repo
        .publications
        .iter()
        .any(|p| p.snap_id == payload.snap_id && p.scope == payload.scope && p.gate == payload.gate)
    {
        return Err(conflict("snap already published to this scope/gate"));
    }

    let gate_def = repo
        .gate_graph
        .gates
        .iter()
        .find(|g| g.id == payload.gate)
        .ok_or_else(|| bad_request(anyhow::anyhow!("unknown gate")))?;
    if payload.metadata_only && !gate_def.allow_metadata_only_publications {
        return Err(bad_request(anyhow::anyhow!(
            "metadata-only publications not allowed in this gate"
        )));
    }
    if !repo.snaps.contains(&payload.snap_id) {
        return Err(bad_request(anyhow::anyhow!(
            "unknown snap (upload snap first)"
        )));
    }

    // For non-metadata-only publications, require full availability of referenced objects.
    // For metadata-only publications, we still require the manifest structure to be present
    // (snaps/manifests/recipes), but allow blob bytes to be pending.
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

pub(super) async fn list_publications(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path(repo_id): Path<String>,
) -> Result<Json<Vec<Publication>>, Response> {
    let repos = state.repos.read().await;
    let repo = repos.get(&repo_id).ok_or_else(not_found)?;
    if !can_read(repo, &subject) {
        return Err(forbidden());
    }
    Ok(Json(repo.publications.clone()))
}
