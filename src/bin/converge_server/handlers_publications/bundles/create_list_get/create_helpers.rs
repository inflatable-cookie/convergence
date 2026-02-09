use super::super::super::super::*;

use super::types::CreateBundleRequest;

pub(super) fn validate_bundle_create_input(payload: &CreateBundleRequest) -> Result<(), Response> {
    validate_scope_id(&payload.scope).map_err(bad_request)?;
    validate_gate_id(&payload.gate).map_err(bad_request)?;
    if payload.input_publications.is_empty() {
        return Err(bad_request(anyhow::anyhow!(
            "bundle must include at least one input publication"
        )));
    }
    for pid in &payload.input_publications {
        validate_object_id(pid).map_err(bad_request)?;
    }
    Ok(())
}

pub(super) fn now_rfc3339() -> Result<String, Response> {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|e| internal_error(anyhow::anyhow!(e)))
}

pub(super) fn normalize_input_publications(mut input_publications: Vec<String>) -> Vec<String> {
    input_publications.sort();
    input_publications.dedup();
    input_publications
}

pub(super) fn build_bundle_id(
    repo_id: &str,
    scope: &str,
    gate: &str,
    root_manifest: &str,
    input_publications: &[String],
    user: &str,
    created_at: &str,
) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(repo_id.as_bytes());
    hasher.update(b"\n");
    hasher.update(scope.as_bytes());
    hasher.update(b"\n");
    hasher.update(gate.as_bytes());
    hasher.update(b"\n");
    hasher.update(root_manifest.as_bytes());
    hasher.update(b"\n");
    for pid in input_publications {
        hasher.update(pid.as_bytes());
        hasher.update(b"\n");
    }
    hasher.update(user.as_bytes());
    hasher.update(b"\n");
    hasher.update(created_at.as_bytes());
    hasher.finalize().to_hex().to_string()
}
