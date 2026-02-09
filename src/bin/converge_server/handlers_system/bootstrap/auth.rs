use super::*;

pub(super) fn verify_bootstrap_bearer(
    state: &Arc<AppState>,
    headers: &axum::http::HeaderMap,
) -> Result<(), Response> {
    let Some(expected_hash) = state.bootstrap_token_hash.as_deref() else {
        return Err(not_found());
    };

    let Some(value) = headers.get(header::AUTHORIZATION) else {
        return Err(unauthorized());
    };
    let Ok(value) = value.to_str() else {
        return Err(unauthorized());
    };
    let Some(token) = value.strip_prefix("Bearer ") else {
        return Err(unauthorized());
    };
    if hash_token(token) != expected_hash {
        return Err(unauthorized());
    }
    Ok(())
}
