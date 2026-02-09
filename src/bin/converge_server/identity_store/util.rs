pub(crate) fn now_ts() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "<time>".to_string())
}

pub(crate) fn hash_token(secret: &str) -> String {
    blake3::hash(secret.as_bytes()).to_hex().to_string()
}

pub(crate) fn identity_users_path(data_dir: &std::path::Path) -> std::path::PathBuf {
    data_dir.join("users.json")
}

pub(crate) fn identity_tokens_path(data_dir: &std::path::Path) -> std::path::PathBuf {
    data_dir.join("tokens.json")
}
