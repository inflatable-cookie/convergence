use super::*;

pub(super) async fn revoke_token(
    state: &Arc<AppState>,
    subject: &Subject,
    token_id: &str,
) -> Result<Json<serde_json::Value>, Response> {
    let revoked_at = now_ts();

    {
        let mut tokens = state.tokens.write().await;
        let Some(token) = tokens.get_mut(token_id) else {
            return Err(not_found());
        };
        if token.user_id != subject.user_id && !subject.admin {
            return Err(forbidden());
        }
        token.revoked_at = Some(revoked_at.clone());
    }

    {
        let users = state.users.read().await;
        let tokens = state.tokens.read().await;
        if let Err(err) = persist_identity_to_disk(&state.data_dir, &users, &tokens) {
            return Err(internal_error(err));
        }
    }

    Ok(Json(serde_json::json!({
        "revoked": true,
        "token_id": token_id,
        "revoked_at": revoked_at
    })))
}
