use std::path::PathBuf;

use tempfile::tempdir;

use super::*;

fn args_with_data_dir(data_dir: PathBuf) -> Args {
    Args {
        addr: "127.0.0.1:0".parse().expect("parse socket addr"),
        addr_file: None,
        data_dir,
        db_url: None,
        bootstrap_token: None,
        dev_user: "dev".to_string(),
        dev_token: "dev-token".to_string(),
    }
}

#[test]
fn load_or_bootstrap_identity_seeds_dev_identity_without_bootstrap_token() {
    let temp = tempdir().expect("create temp dir");
    let args = args_with_data_dir(temp.path().to_path_buf());

    let (users, tokens) = load_or_bootstrap_identity(&args).expect("load identity");

    assert_eq!(users.len(), 1);
    assert_eq!(tokens.len(), 1);
    assert!(users.values().any(|u| u.handle == "dev"));

    let users_path = crate::identity_store::identity_users_path(temp.path());
    let tokens_path = crate::identity_store::identity_tokens_path(temp.path());
    assert!(users_path.exists(), "users file should be persisted");
    assert!(tokens_path.exists(), "tokens file should be persisted");
}

#[test]
fn load_or_bootstrap_identity_keeps_store_empty_with_bootstrap_token() {
    let temp = tempdir().expect("create temp dir");
    let mut args = args_with_data_dir(temp.path().to_path_buf());
    args.bootstrap_token = Some("bootstrap-secret".to_string());

    let (users, tokens) = load_or_bootstrap_identity(&args).expect("load identity");

    assert!(users.is_empty());
    assert!(tokens.is_empty());
}
