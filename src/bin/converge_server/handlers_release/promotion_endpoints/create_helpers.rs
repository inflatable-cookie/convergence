use super::*;

use super::create::CreatePromotionRequest;

pub(super) fn validate_create_promotion_request(
    payload: &CreatePromotionRequest,
) -> Result<(), Response> {
    validate_object_id(&payload.bundle_id).map_err(bad_request)?;
    validate_gate_id(&payload.to_gate).map_err(bad_request)?;
    Ok(())
}

pub(super) fn now_rfc3339() -> Result<String, Response> {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|e| internal_error(anyhow::anyhow!(e)))
}

pub(super) fn build_promotion_id(
    repo_id: &str,
    bundle: &Bundle,
    to_gate: &str,
    user: &str,
    promoted_at: &str,
) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(repo_id.as_bytes());
    hasher.update(b"\n");
    hasher.update(bundle.id.as_bytes());
    hasher.update(b"\n");
    hasher.update(bundle.scope.as_bytes());
    hasher.update(b"\n");
    hasher.update(bundle.gate.as_bytes());
    hasher.update(b"\n");
    hasher.update(to_gate.as_bytes());
    hasher.update(b"\n");
    hasher.update(user.as_bytes());
    hasher.update(b"\n");
    hasher.update(promoted_at.as_bytes());
    hasher.finalize().to_hex().to_string()
}
