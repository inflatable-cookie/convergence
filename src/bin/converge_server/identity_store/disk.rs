use super::*;

pub(crate) fn load_identity_from_disk(
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

pub(crate) fn persist_identity_to_disk(
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
