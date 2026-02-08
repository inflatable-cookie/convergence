use super::*;

pub(super) fn now_ts() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "<time>".to_string())
}

pub(super) fn hash_token(secret: &str) -> String {
    blake3::hash(secret.as_bytes()).to_hex().to_string()
}

pub(super) fn identity_users_path(data_dir: &std::path::Path) -> std::path::PathBuf {
    data_dir.join("users.json")
}

pub(super) fn identity_tokens_path(data_dir: &std::path::Path) -> std::path::PathBuf {
    data_dir.join("tokens.json")
}

pub(super) fn load_identity_from_disk(
    data_dir: &std::path::Path,
) -> Result<(HashMap<String, User>, HashMap<String, AccessToken>)> {
    let users: HashMap<String, User> = if identity_users_path(data_dir).exists() {
        let bytes = std::fs::read(identity_users_path(data_dir)).context("read users.json")?;
        let list: Vec<User> = serde_json::from_slice(&bytes).context("parse users.json")?;
        list.into_iter().map(|u| (u.id.clone(), u)).collect()
    } else {
        HashMap::new()
    };

    let tokens: HashMap<String, AccessToken> = if identity_tokens_path(data_dir).exists() {
        let bytes = std::fs::read(identity_tokens_path(data_dir)).context("read tokens.json")?;
        let list: Vec<AccessToken> = serde_json::from_slice(&bytes).context("parse tokens.json")?;
        list.into_iter().map(|t| (t.id.clone(), t)).collect()
    } else {
        HashMap::new()
    };

    Ok((users, tokens))
}

pub(super) fn persist_identity_to_disk(
    data_dir: &std::path::Path,
    users: &HashMap<String, User>,
    tokens: &HashMap<String, AccessToken>,
) -> Result<()> {
    let mut user_list: Vec<User> = users.values().cloned().collect();
    user_list.sort_by(|a, b| a.handle.cmp(&b.handle));
    let users_bytes = serde_json::to_vec_pretty(&user_list).context("serialize users")?;
    write_atomic_overwrite(&identity_users_path(data_dir), &users_bytes)
        .context("write users.json")?;

    let mut token_list: Vec<AccessToken> = tokens.values().cloned().collect();
    token_list.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    let tokens_bytes = serde_json::to_vec_pretty(&token_list).context("serialize tokens")?;
    write_atomic_overwrite(&identity_tokens_path(data_dir), &tokens_bytes)
        .context("write tokens.json")?;

    Ok(())
}

pub(super) fn bootstrap_identity(handle: &str, token_secret: &str) -> (User, AccessToken) {
    let created_at = now_ts();
    let user_id = {
        let mut h = blake3::Hasher::new();
        h.update(handle.as_bytes());
        h.update(b"\n");
        h.update(created_at.as_bytes());
        h.finalize().to_hex().to_string()
    };
    let user = User {
        id: user_id.clone(),
        handle: handle.to_string(),
        display_name: None,
        admin: true,
        created_at: created_at.clone(),
    };

    let token_hash = hash_token(token_secret);
    let token_id = {
        let mut h = blake3::Hasher::new();
        h.update(user_id.as_bytes());
        h.update(b"\n");
        h.update(token_hash.as_bytes());
        h.finalize().to_hex().to_string()
    };
    let token = AccessToken {
        id: token_id,
        user_id,
        token_hash,
        label: Some("bootstrap".to_string()),
        created_at,
        last_used_at: None,
        revoked_at: None,
        expires_at: None,
    };

    (user, token)
}

pub(super) fn generate_token_secret() -> Result<String> {
    // 32 bytes of entropy, hex-encoded.
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes).map_err(|e| anyhow::anyhow!("getrandom: {:?}", e))?;
    let mut out = String::with_capacity(64);
    for b in &bytes {
        out.push_str(&format!("{:02x}", b));
    }
    Ok(out)
}
