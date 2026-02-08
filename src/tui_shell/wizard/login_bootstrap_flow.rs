use crate::model::RemoteConfig;

use super::super::TextInputAction;
use super::login_bootstrap_validate::{parse_bootstrap_inputs, validate_login_inputs};
use super::types::{BootstrapWizard, LoginWizard};

impl super::super::App {
    pub(in crate::tui_shell) fn start_bootstrap_wizard(&mut self) {
        let Some(ws) = self.require_workspace() else {
            return;
        };

        let remote = ws.store.read_config().ok().and_then(|c| c.remote);

        let default_url = remote
            .as_ref()
            .map(|r| r.base_url.clone())
            .unwrap_or_else(|| "http://127.0.0.1:8080".to_string());
        let default_repo = remote
            .as_ref()
            .map(|r| r.repo_id.clone())
            .unwrap_or_else(|| "test".to_string());
        let default_scope = remote
            .as_ref()
            .map(|r| r.scope.clone())
            .unwrap_or_else(|| "main".to_string());
        let default_gate = remote
            .as_ref()
            .map(|r| r.gate.clone())
            .unwrap_or_else(|| "dev-intake".to_string());

        self.bootstrap_wizard = Some(BootstrapWizard {
            url: Some(default_url.clone()),
            bootstrap_token: None,
            handle: "admin".to_string(),
            display_name: None,
            repo: Some(default_repo),
            scope: default_scope,
            gate: default_gate,
        });

        self.open_text_input_modal(
            "Bootstrap",
            "url> ",
            TextInputAction::BootstrapUrl,
            Some(default_url),
            vec![
                "Server base URL (example: http://127.0.0.1:8080)".to_string(),
                "Start converge-server with --bootstrap-token first.".to_string(),
            ],
        );
    }

    pub(in crate::tui_shell) fn continue_bootstrap_wizard(
        &mut self,
        action: TextInputAction,
        value: String,
    ) {
        if self.bootstrap_wizard.is_none() {
            self.push_error("bootstrap wizard not active".to_string());
            return;
        }

        match action {
            TextInputAction::BootstrapUrl => {
                let v = value.trim().to_string();
                if v.is_empty() {
                    self.push_error("bootstrap: missing url".to_string());
                    self.bootstrap_wizard = None;
                    return;
                }
                if let Some(w) = self.bootstrap_wizard.as_mut() {
                    w.url = Some(v);
                }
                let default = self
                    .bootstrap_wizard
                    .as_ref()
                    .and_then(|w| w.repo.clone())
                    .unwrap_or_else(|| "test".to_string());
                self.open_text_input_modal(
                    "Bootstrap",
                    "repo> ",
                    TextInputAction::BootstrapRepo,
                    Some(default),
                    vec![
                        "Repo id to use for the client config.".to_string(),
                        "If it doesn't exist, the wizard will create it.".to_string(),
                    ],
                );
            }

            TextInputAction::BootstrapRepo => {
                let v = value.trim().to_string();
                if v.is_empty() {
                    self.push_error("bootstrap: missing repo".to_string());
                    self.bootstrap_wizard = None;
                    return;
                }
                if let Some(w) = self.bootstrap_wizard.as_mut() {
                    w.repo = Some(v);
                }
                let default = self
                    .bootstrap_wizard
                    .as_ref()
                    .map(|w| w.scope.clone())
                    .unwrap_or_else(|| "main".to_string());
                self.open_text_input_modal(
                    "Bootstrap",
                    "scope> ",
                    TextInputAction::BootstrapScope,
                    Some(default),
                    vec!["Default scope for remote operations.".to_string()],
                );
            }

            TextInputAction::BootstrapScope => {
                let v = value.trim().to_string();
                if v.is_empty() {
                    self.push_error("bootstrap: missing scope".to_string());
                    self.bootstrap_wizard = None;
                    return;
                }
                if let Some(w) = self.bootstrap_wizard.as_mut() {
                    w.scope = v;
                }
                let default = self
                    .bootstrap_wizard
                    .as_ref()
                    .map(|w| w.gate.clone())
                    .unwrap_or_else(|| "dev-intake".to_string());
                self.open_text_input_modal(
                    "Bootstrap",
                    "gate> ",
                    TextInputAction::BootstrapGate,
                    Some(default),
                    vec!["Default gate for remote operations.".to_string()],
                );
            }

            TextInputAction::BootstrapGate => {
                let v = value.trim().to_string();
                if v.is_empty() {
                    self.push_error("bootstrap: missing gate".to_string());
                    self.bootstrap_wizard = None;
                    return;
                }
                if let Some(w) = self.bootstrap_wizard.as_mut() {
                    w.gate = v;
                }

                self.open_text_input_modal(
                    "Bootstrap",
                    "bootstrap token> ",
                    TextInputAction::BootstrapToken,
                    None,
                    vec![
                        "One-time bootstrap token (the same value passed to converge-server --bootstrap-token)."
                            .to_string(),
                        "Generate one: openssl rand -hex 32".to_string(),
                    ],
                );
            }

            TextInputAction::BootstrapToken => {
                let v = value.trim().to_string();
                if v.is_empty() {
                    self.push_error("bootstrap: missing token".to_string());
                    self.bootstrap_wizard = None;
                    return;
                }
                if let Some(w) = self.bootstrap_wizard.as_mut() {
                    w.bootstrap_token = Some(v);
                }
                self.open_text_input_modal(
                    "Bootstrap",
                    "admin handle> ",
                    TextInputAction::BootstrapHandle,
                    Some("admin".to_string()),
                    vec![
                        "Admin handle to create (one-time).".to_string(),
                        "Response includes a plaintext admin token; it will be stored in .converge/state.json".to_string(),
                    ],
                );
            }

            TextInputAction::BootstrapHandle => {
                let v = value.trim().to_string();
                if v.is_empty() {
                    self.push_error("bootstrap: missing handle".to_string());
                    self.bootstrap_wizard = None;
                    return;
                }
                if let Some(w) = self.bootstrap_wizard.as_mut() {
                    w.handle = v;
                }
                self.open_text_input_modal(
                    "Bootstrap",
                    "display name (optional)> ",
                    TextInputAction::BootstrapDisplayName,
                    None,
                    vec!["Optional display name (leave blank to skip).".to_string()],
                );
            }

            TextInputAction::BootstrapDisplayName => {
                if let Some(w) = self.bootstrap_wizard.as_mut() {
                    let v = value.trim().to_string();
                    w.display_name = if v.is_empty() { None } else { Some(v) };
                }
                self.finish_bootstrap_wizard();
            }

            _ => {
                self.push_error("unexpected bootstrap wizard input".to_string());
            }
        }
    }

    pub(in crate::tui_shell) fn start_login_wizard(&mut self) {
        let Some(ws) = self.require_workspace() else {
            return;
        };

        let remote = ws.store.read_config().ok().and_then(|c| c.remote);

        let default_url = remote.as_ref().map(|r| r.base_url.clone());
        let default_repo = remote.as_ref().map(|r| r.repo_id.clone());
        let default_scope = remote
            .as_ref()
            .map(|r| r.scope.clone())
            .unwrap_or_else(|| "main".to_string());
        let default_gate = remote
            .as_ref()
            .map(|r| r.gate.clone())
            .unwrap_or_else(|| "dev-intake".to_string());

        self.login_wizard = Some(LoginWizard {
            url: default_url.clone(),
            token: None,
            repo: default_repo,
            scope: default_scope,
            gate: default_gate,
        });

        self.open_text_input_modal(
            "Login",
            "url> ",
            TextInputAction::LoginUrl,
            default_url,
            vec![
                "Remote base URL (example: https://example.com)".to_string(),
                "Esc cancels; Enter continues.".to_string(),
            ],
        );
    }

    pub(in crate::tui_shell) fn continue_login_wizard(
        &mut self,
        action: TextInputAction,
        value: String,
    ) {
        if self.login_wizard.is_none() {
            self.push_error("login wizard not active".to_string());
            return;
        }

        match action {
            TextInputAction::LoginUrl => {
                if let Some(w) = self.login_wizard.as_mut() {
                    w.url = Some(value);
                }
                self.open_text_input_modal(
                    "Login",
                    "token> ",
                    TextInputAction::LoginToken,
                    None,
                    vec![
                        "Access token (will be stored locally).".to_string(),
                        "Tip: paste it, then Enter.".to_string(),
                    ],
                );
            }
            TextInputAction::LoginToken => {
                if let Some(w) = self.login_wizard.as_mut() {
                    w.token = Some(value);
                }
                let repo_initial = self.login_wizard.as_ref().and_then(|w| w.repo.clone());
                self.open_text_input_modal(
                    "Login",
                    "repo> ",
                    TextInputAction::LoginRepo,
                    repo_initial,
                    vec!["Repo id".to_string()],
                );
            }
            TextInputAction::LoginRepo => {
                if let Some(w) = self.login_wizard.as_mut() {
                    w.repo = Some(value);
                }
                let scope_initial = self.login_wizard.as_ref().map(|w| w.scope.clone());
                self.open_text_input_modal(
                    "Login",
                    "scope> ",
                    TextInputAction::LoginScope,
                    scope_initial,
                    vec!["Scope id".to_string()],
                );
            }
            TextInputAction::LoginScope => {
                if let Some(w) = self.login_wizard.as_mut()
                    && !value.is_empty()
                {
                    w.scope = value;
                }
                let gate_initial = self.login_wizard.as_ref().map(|w| w.gate.clone());
                self.open_text_input_modal(
                    "Login",
                    "gate> ",
                    TextInputAction::LoginGate,
                    gate_initial,
                    vec!["Gate id".to_string()],
                );
            }
            TextInputAction::LoginGate => {
                if let Some(w) = self.login_wizard.as_mut()
                    && !value.is_empty()
                {
                    w.gate = value;
                }

                let (base_url, token, repo_id, scope, gate) = match self.login_wizard.as_ref() {
                    Some(w) => {
                        let base_url = w.url.clone().unwrap_or_default();
                        let token = w.token.clone().unwrap_or_default();
                        let repo_id = w.repo.clone().unwrap_or_default();
                        let scope = w.scope.clone();
                        let gate = w.gate.clone();
                        (base_url, token, repo_id, scope, gate)
                    }
                    None => {
                        self.push_error("login wizard not active".to_string());
                        return;
                    }
                };

                if let Err(err) = validate_login_inputs(&base_url, &token, &repo_id) {
                    self.push_error(err);
                    self.login_wizard = None;
                    return;
                }

                self.login_wizard = None;
                self.apply_login_config(base_url, token, repo_id, scope, gate);
            }

            _ => {
                self.push_error("unexpected login wizard input".to_string());
            }
        }
    }

    pub(in crate::tui_shell) fn apply_login_config(
        &mut self,
        base_url: String,
        token: String,
        repo_id: String,
        scope: String,
        gate: String,
    ) {
        let Some(ws) = self.require_workspace() else {
            return;
        };

        let mut cfg = match ws.store.read_config() {
            Ok(c) => c,
            Err(err) => {
                self.push_error(format!("read config: {:#}", err));
                return;
            }
        };

        let remote = RemoteConfig {
            base_url: base_url.clone(),
            token: None,
            repo_id,
            scope,
            gate,
        };

        if let Err(err) = ws.store.set_remote_token(&remote, &token) {
            self.push_error(format!("store remote token: {:#}", err));
            return;
        }

        cfg.remote = Some(remote);
        if let Err(err) = ws.store.write_config(&cfg) {
            self.push_error(format!("write config: {:#}", err));
            return;
        }

        self.push_output(vec![format!("logged in to {}", base_url)]);
        self.refresh_root_view();
    }

    pub(in crate::tui_shell) fn finish_bootstrap_wizard(&mut self) {
        let Some(w) = self.bootstrap_wizard.clone() else {
            self.push_error("bootstrap wizard not active".to_string());
            return;
        };
        self.bootstrap_wizard = None;

        let (base_url, bootstrap_token, handle, repo_id) = match parse_bootstrap_inputs(&w) {
            Ok(inputs) => (
                inputs.base_url,
                inputs.bootstrap_token,
                inputs.handle,
                inputs.repo_id,
            ),
            Err(err) => {
                self.push_error(err);
                return;
            }
        };

        let remote = RemoteConfig {
            base_url: base_url.clone(),
            token: None,
            repo_id: repo_id.clone(),
            scope: w.scope.clone(),
            gate: w.gate.clone(),
        };

        let client = match crate::remote::RemoteClient::new(remote.clone(), bootstrap_token) {
            Ok(c) => c,
            Err(err) => {
                self.push_error(format!("bootstrap: {:#}", err));
                return;
            }
        };

        let bootstrap = match client.bootstrap_first_admin(&handle, w.display_name.clone()) {
            Ok(r) => r,
            Err(err) => {
                self.push_error(format!("bootstrap: {:#}", err));
                return;
            }
        };

        self.apply_login_config(
            base_url.clone(),
            bootstrap.token.token.clone(),
            repo_id.clone(),
            w.scope.clone(),
            w.gate.clone(),
        );

        // Ensure the repo exists for the configured remote (best-effort).
        if let Some(client) = self.remote_client() {
            match client.get_repo(&repo_id) {
                Ok(_) => {
                    self.push_output(vec![format!("repo {} exists", repo_id)]);
                }
                Err(err) if err.to_string().contains("remote repo not found") => {
                    match client.create_repo(&repo_id) {
                        Ok(_) => self.push_output(vec![format!("created repo {}", repo_id)]),
                        Err(err) => self.push_error(format!("create repo: {:#}", err)),
                    }
                }
                Err(err) => {
                    self.push_error(format!("get repo: {:#}", err));
                }
            }
        }

        self.push_output(vec![
            format!("bootstrapped admin {}", bootstrap.user.handle),
            "Restart the server without --bootstrap-token.".to_string(),
        ]);
        self.refresh_root_view();
    }
}
