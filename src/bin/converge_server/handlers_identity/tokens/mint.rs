use super::types::CreateTokenResponse;
use super::*;

pub(super) async fn mint_token(
    state: &Arc<AppState>,
    user_id: &str,
    label: Option<String>,
) -> Result<CreateTokenResponse, Response> {
    let created_at = now_ts();

    let token = generate_token_secret().map_err(internal_error)?;
    let token_hash = hash_token(&token);
    let token_id = {
        let mut hasher = blake3::Hasher::new();
        hasher.update(user_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(token_hash.as_bytes());
        hasher.update(b"\n");
        hasher.update(created_at.as_bytes());
        hasher.finalize().to_hex().to_string()
    };

    {
        let mut tokens = state.tokens.write().await;
        tokens.insert(
            token_id.clone(),
            AccessToken {
                id: token_id.clone(),
                user_id: user_id.to_string(),
                token_hash: token_hash.clone(),
                label,
                created_at: created_at.clone(),
                last_used_at: None,
                revoked_at: None,
                expires_at: None,
            },
        );
    }
    {
        let mut idx = state.token_hash_index.write().await;
        idx.insert(token_hash, token_id.clone());
    }

    persist_identity(state).await?;

    Ok(CreateTokenResponse {
        id: token_id,
        token,
        created_at,
    })
}

async fn persist_identity(state: &Arc<AppState>) -> Result<(), Response> {
    let users = state.users.read().await;
    let tokens = state.tokens.read().await;
    persist_identity_to_disk(&state.data_dir, &users, &tokens).map_err(internal_error)?;
    Ok(())
}
