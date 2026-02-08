use super::types::BootstrapWizard;

pub(super) struct BootstrapInputs {
    pub(super) base_url: String,
    pub(super) bootstrap_token: String,
    pub(super) handle: String,
    pub(super) repo_id: String,
}

pub(super) fn parse_bootstrap_inputs(w: &BootstrapWizard) -> Result<BootstrapInputs, String> {
    let Some(base_url) = w.url.clone() else {
        return Err("bootstrap: missing url".to_string());
    };
    let Some(bootstrap_token) = w.bootstrap_token.clone() else {
        return Err("bootstrap: missing token".to_string());
    };
    let handle = w.handle.trim().to_string();
    if handle.is_empty() {
        return Err("bootstrap: missing handle".to_string());
    }
    let Some(repo_id) = w.repo.clone() else {
        return Err("bootstrap: missing repo".to_string());
    };

    Ok(BootstrapInputs {
        base_url,
        bootstrap_token,
        handle,
        repo_id,
    })
}

pub(super) fn validate_login_inputs(
    base_url: &str,
    token: &str,
    repo_id: &str,
) -> Result<(), String> {
    if base_url.trim().is_empty() {
        return Err("login: missing url".to_string());
    }
    if token.trim().is_empty() {
        return Err("login: missing token".to_string());
    }
    if repo_id.trim().is_empty() {
        return Err("login: missing repo".to_string());
    }
    Ok(())
}
