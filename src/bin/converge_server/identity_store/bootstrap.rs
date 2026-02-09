use super::*;

pub(crate) fn bootstrap_identity(handle: &str, token_secret: &str) -> (User, AccessToken) {
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

pub(crate) fn generate_token_secret() -> Result<String> {
    // 32 bytes of entropy, hex-encoded.
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes).map_err(|e| anyhow::anyhow!("getrandom: {:?}", e))?;
    let mut out = String::with_capacity(64);
    for b in &bytes {
        out.push_str(&format!("{:02x}", b));
    }
    Ok(out)
}
