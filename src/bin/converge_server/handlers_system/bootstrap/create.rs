use super::*;

pub(super) fn create_admin_user(
    payload: &BootstrapRequest,
    created_at: &str,
) -> Result<User, Response> {
    let user_id = generate_token_secret().map_err(internal_error)?;
    Ok(User {
        id: user_id,
        handle: payload.handle.clone(),
        display_name: payload.display_name.clone(),
        admin: true,
        created_at: created_at.to_string(),
    })
}

pub(super) async fn assert_bootstrap_window_open(state: &Arc<AppState>) -> Result<(), Response> {
    // Enforce one-time semantics per data_dir: only allow bootstrapping if no admin exists.
    {
        let users = state.users.read().await;
        if users.values().any(|u| u.admin) {
            return Err(conflict("already bootstrapped"));
        }
    }
    Ok(())
}

pub(super) async fn insert_user(state: &Arc<AppState>, user: User) -> Result<(), Response> {
    let user_id = user.id.clone();
    let mut users = state.users.write().await;
    if users.values().any(|u| u.handle == user.handle) {
        return Err(conflict("user handle already exists"));
    }
    // Re-check under write lock.
    if users.values().any(|u| u.admin) {
        return Err(conflict("already bootstrapped"));
    }
    users.insert(user_id, user);
    Ok(())
}

pub(super) async fn create_bootstrap_token(
    state: &Arc<AppState>,
    user: &User,
    created_at: &str,
) -> Result<(String, String), Response> {
    let token_secret = generate_token_secret().map_err(internal_error)?;
    let token_hash = hash_token(&token_secret);
    let token_id = {
        let mut h = blake3::Hasher::new();
        h.update(user.id.as_bytes());
        h.update(b"\n");
        h.update(token_hash.as_bytes());
        h.update(b"\n");
        h.update(created_at.as_bytes());
        h.finalize().to_hex().to_string()
    };

    {
        let mut tokens = state.tokens.write().await;
        tokens.insert(
            token_id.clone(),
            AccessToken {
                id: token_id.clone(),
                user_id: user.id.clone(),
                token_hash: token_hash.clone(),
                label: Some("bootstrap".to_string()),
                created_at: created_at.to_string(),
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
    Ok((token_id, token_secret))
}
