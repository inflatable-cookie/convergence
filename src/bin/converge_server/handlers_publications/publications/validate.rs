use super::*;

pub(super) fn validate_publication_request(
    payload: &CreatePublicationRequest,
) -> Result<(), Response> {
    validate_object_id(&payload.snap_id).map_err(bad_request)?;
    validate_scope_id(&payload.scope).map_err(bad_request)?;
    validate_gate_id(&payload.gate).map_err(bad_request)?;
    Ok(())
}

pub(super) fn created_at() -> Result<String, Response> {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|e| internal_error(anyhow::anyhow!(e)))
}

pub(super) fn publication_id(
    repo_id: &str,
    payload: &CreatePublicationRequest,
    user: &str,
    created_at: &str,
) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(repo_id.as_bytes());
    hasher.update(b"\n");
    hasher.update(payload.snap_id.as_bytes());
    hasher.update(b"\n");
    hasher.update(payload.scope.as_bytes());
    hasher.update(b"\n");
    hasher.update(payload.gate.as_bytes());
    hasher.update(b"\n");
    hasher.update(user.as_bytes());
    hasher.update(b"\n");
    hasher.update(created_at.as_bytes());
    hasher.finalize().to_hex().to_string()
}

pub(super) fn enforce_publication_constraints(
    repo: &Repo,
    payload: &CreatePublicationRequest,
    subject: &Subject,
) -> Result<(), Response> {
    if !can_publish(repo, subject) {
        return Err(forbidden());
    }
    if !repo.scopes.contains(&payload.scope) {
        return Err(bad_request(anyhow::anyhow!("unknown scope")));
    }
    if !repo.gate_graph.gates.iter().any(|g| g.id == payload.gate) {
        return Err(bad_request(anyhow::anyhow!("unknown gate")));
    }

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
    Ok(())
}
