use crate::model::RemoteConfig;

use super::TextInputAction;

mod browse_flow;
mod fetch_flow;
mod member_flow;
mod move_flow;
mod move_glob;
mod types;
pub(in crate::tui_shell) use self::types::{
    BootstrapWizard, BrowseTarget, BrowseWizard, FetchWizard, LaneMemberWizard, LoginWizard,
    MemberAction, MemberWizard, MoveWizard, PinWizard, PromoteWizard, PublishWizard, ReleaseWizard,
    SyncWizard,
};

impl super::App {
    pub(super) fn cancel_wizards(&mut self) {
        self.login_wizard = None;
        self.bootstrap_wizard = None;
        self.fetch_wizard = None;
        self.publish_wizard = None;
        self.sync_wizard = None;
        self.release_wizard = None;
        self.pin_wizard = None;
        self.promote_wizard = None;
        self.member_wizard = None;
        self.lane_member_wizard = None;
        self.browse_wizard = None;
        self.move_wizard = None;
    }

    pub(super) fn start_move_wizard(&mut self, initial_query: Option<String>) {
        let Some(_ws) = self.require_workspace() else {
            return;
        };

        self.move_wizard = Some(MoveWizard {
            query: initial_query.clone(),
            candidates: Vec::new(),
            from: None,
        });

        self.open_text_input_modal(
            "Move",
            "from (glob)> ",
            TextInputAction::MoveFrom,
            initial_query,
            vec![
                "Enter a glob to find the source path.".to_string(),
                "Tip: a plain token searches as **/*<token>*.".to_string(),
                "Examples: src/**/*.rs   docs/*.md   README.md".to_string(),
            ],
        );
    }

    pub(super) fn continue_move_wizard(&mut self, action: TextInputAction, value: String) {
        match action {
            TextInputAction::MoveFrom => self.move_wizard_from(value),
            TextInputAction::MoveTo => self.move_wizard_to(value),
            _ => self.push_error("unexpected move wizard input".to_string()),
        }
    }

    pub(super) fn start_bootstrap_wizard(&mut self) {
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

    pub(super) fn continue_bootstrap_wizard(&mut self, action: TextInputAction, value: String) {
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

    pub(super) fn start_login_wizard(&mut self) {
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

    pub(super) fn continue_login_wizard(&mut self, action: TextInputAction, value: String) {
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

                if base_url.trim().is_empty() {
                    self.push_error("login: missing url".to_string());
                    self.login_wizard = None;
                    return;
                }
                if token.trim().is_empty() {
                    self.push_error("login: missing token".to_string());
                    self.login_wizard = None;
                    return;
                }
                if repo_id.trim().is_empty() {
                    self.push_error("login: missing repo".to_string());
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

    pub(super) fn apply_login_config(
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

    pub(super) fn finish_bootstrap_wizard(&mut self) {
        let Some(w) = self.bootstrap_wizard.clone() else {
            self.push_error("bootstrap wizard not active".to_string());
            return;
        };
        self.bootstrap_wizard = None;

        let Some(base_url) = w.url.clone() else {
            self.push_error("bootstrap: missing url".to_string());
            return;
        };
        let Some(bootstrap_token) = w.bootstrap_token.clone() else {
            self.push_error("bootstrap: missing token".to_string());
            return;
        };
        let handle = w.handle.trim().to_string();
        if handle.is_empty() {
            self.push_error("bootstrap: missing handle".to_string());
            return;
        }
        let Some(repo_id) = w.repo.clone() else {
            self.push_error("bootstrap: missing repo".to_string());
            return;
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

    pub(super) fn start_publish_wizard(&mut self, edit: bool) {
        let Some(_) = self.require_workspace() else {
            return;
        };
        let Some(cfg) = self.remote_config() else {
            self.start_login_wizard();
            return;
        };

        self.publish_wizard = Some(PublishWizard {
            snap: None,
            scope: Some(cfg.scope.clone()),
            gate: Some(cfg.gate.clone()),
            meta: false,
        });

        if edit {
            self.open_text_input_modal(
                "Publish",
                "snap (blank=latest)> ",
                TextInputAction::PublishSnap,
                None,
                vec![
                    "Optional: snap id (leave blank to publish latest).".to_string(),
                    "Esc cancels.".to_string(),
                ],
            );
        } else {
            self.open_text_input_modal(
                "Publish",
                "publish> ",
                TextInputAction::PublishStart,
                None,
                vec![
                    format!("Default: latest snap -> {}/{}", cfg.scope, cfg.gate),
                    "Enter: publish now".to_string(),
                    "Type `edit` to customize (snap/scope/gate/meta).".to_string(),
                ],
            );
        }
    }

    pub(super) fn continue_publish_wizard(&mut self, action: TextInputAction, value: String) {
        if self.publish_wizard.is_none() {
            self.push_error("publish wizard not active".to_string());
            return;
        }

        match action {
            TextInputAction::PublishStart => {
                let v = value.trim().to_string();
                if v.is_empty() {
                    self.publish_wizard = None;
                    self.cmd_publish_impl(&[]);
                    return;
                }

                let v_lc = v.to_lowercase();
                if matches!(v_lc.as_str(), "edit" | "prompt" | "custom") {
                    // Jump into the snap prompt (blank=latest).
                    self.open_text_input_modal(
                        "Publish",
                        "snap (blank=latest)> ",
                        TextInputAction::PublishSnap,
                        None,
                        vec!["Optional: snap id".to_string()],
                    );
                    return;
                }

                // Treat any other input as a snap id override.
                if let Some(w) = self.publish_wizard.as_mut() {
                    w.snap = Some(v);
                }

                let initial = self.publish_wizard.as_ref().and_then(|w| w.scope.clone());
                self.open_text_input_modal(
                    "Publish",
                    "scope> ",
                    TextInputAction::PublishScope,
                    initial,
                    vec!["Scope id (Enter keeps default).".to_string()],
                );
            }
            TextInputAction::PublishSnap => {
                let v = value.trim().to_string();
                if let Some(w) = self.publish_wizard.as_mut() {
                    w.snap = if v.is_empty() { None } else { Some(v) };
                }

                let initial = self.publish_wizard.as_ref().and_then(|w| w.scope.clone());
                self.open_text_input_modal(
                    "Publish",
                    "scope> ",
                    TextInputAction::PublishScope,
                    initial,
                    vec!["Scope id (Enter keeps default).".to_string()],
                );
            }
            TextInputAction::PublishScope => {
                let v = value.trim().to_string();
                if let Some(w) = self.publish_wizard.as_mut() {
                    w.scope = if v.is_empty() { None } else { Some(v) };
                }

                let initial = self.publish_wizard.as_ref().and_then(|w| w.gate.clone());
                self.open_text_input_modal(
                    "Publish",
                    "gate> ",
                    TextInputAction::PublishGate,
                    initial,
                    vec!["Gate id (Enter keeps default).".to_string()],
                );
            }
            TextInputAction::PublishGate => {
                let v = value.trim().to_string();
                if let Some(w) = self.publish_wizard.as_mut() {
                    w.gate = if v.is_empty() { None } else { Some(v) };
                }

                self.open_text_input_modal(
                    "Publish",
                    "metadata-only? (y/N)> ",
                    TextInputAction::PublishMeta,
                    Some("n".to_string()),
                    vec![
                        "If yes, publish metadata only (objects may be missing until later)."
                            .to_string(),
                    ],
                );
            }
            TextInputAction::PublishMeta => {
                let v = value.trim().to_lowercase();
                let meta = matches!(v.as_str(), "y" | "yes" | "true" | "1");
                if let Some(w) = self.publish_wizard.as_mut() {
                    w.meta = meta;
                }
                self.finish_publish_wizard();
            }
            _ => {
                self.push_error("unexpected publish wizard input".to_string());
            }
        }
    }

    pub(super) fn finish_publish_wizard(&mut self) {
        let Some(w) = self.publish_wizard.clone() else {
            self.push_error("publish wizard not active".to_string());
            return;
        };
        self.publish_wizard = None;

        let mut argv: Vec<String> = Vec::new();
        if let Some(s) = w.snap {
            argv.extend(["--snap-id".to_string(), s]);
        }
        if let Some(s) = w.scope {
            argv.extend(["--scope".to_string(), s]);
        }
        if let Some(g) = w.gate {
            argv.extend(["--gate".to_string(), g]);
        }
        if w.meta {
            argv.push("--metadata-only".to_string());
        }

        self.cmd_publish_impl(&argv);
    }

    pub(super) fn start_sync_wizard(&mut self, edit: bool) {
        let Some(_) = self.require_workspace() else {
            return;
        };
        if self.remote_config().is_none() {
            self.start_login_wizard();
            return;
        }

        self.sync_wizard = Some(SyncWizard {
            snap: None,
            lane: "default".to_string(),
            client: None,
        });

        if edit {
            self.open_text_input_modal(
                "Sync",
                "lane> ",
                TextInputAction::SyncLane,
                Some("default".to_string()),
                vec!["Lane id (Enter keeps default).".to_string()],
            );
        } else {
            self.open_text_input_modal(
                "Sync",
                "sync> ",
                TextInputAction::SyncStart,
                None,
                vec![
                    "Default: latest snap -> lane=default".to_string(),
                    "Enter: sync now".to_string(),
                    "Type a lane id, or `edit` to customize (lane/client/snap).".to_string(),
                ],
            );
        }
    }

    pub(super) fn continue_sync_wizard(&mut self, action: TextInputAction, value: String) {
        if self.sync_wizard.is_none() {
            self.push_error("sync wizard not active".to_string());
            return;
        }

        match action {
            TextInputAction::SyncStart => {
                let v = value.trim().to_string();
                if v.is_empty() {
                    self.sync_wizard = None;
                    self.cmd_sync_impl(&[]);
                    return;
                }

                let v_lc = v.to_lowercase();
                if matches!(v_lc.as_str(), "edit" | "prompt" | "custom") {
                    self.open_text_input_modal(
                        "Sync",
                        "lane> ",
                        TextInputAction::SyncLane,
                        Some("default".to_string()),
                        vec!["Lane id (Enter keeps default).".to_string()],
                    );
                    return;
                }

                if let Some(w) = self.sync_wizard.as_mut() {
                    w.lane = v;
                }
                self.open_text_input_modal(
                    "Sync",
                    "client (blank=auto)> ",
                    TextInputAction::SyncClient,
                    None,
                    vec!["Optional: client id (rarely needed).".to_string()],
                );
            }

            TextInputAction::SyncLane => {
                let v = value.trim().to_string();
                if let Some(w) = self.sync_wizard.as_mut()
                    && !v.is_empty()
                {
                    w.lane = v;
                }
                self.open_text_input_modal(
                    "Sync",
                    "client (blank=auto)> ",
                    TextInputAction::SyncClient,
                    None,
                    vec!["Optional: client id (rarely needed).".to_string()],
                );
            }

            TextInputAction::SyncClient => {
                let v = value.trim().to_string();
                if let Some(w) = self.sync_wizard.as_mut() {
                    w.client = if v.is_empty() { None } else { Some(v) };
                }
                self.open_text_input_modal(
                    "Sync",
                    "snap (blank=latest)> ",
                    TextInputAction::SyncSnap,
                    None,
                    vec!["Optional: snap id (leave blank for latest).".to_string()],
                );
            }

            TextInputAction::SyncSnap => {
                let v = value.trim().to_string();
                if let Some(w) = self.sync_wizard.as_mut() {
                    w.snap = if v.is_empty() { None } else { Some(v) };
                }
                self.finish_sync_wizard();
            }

            _ => {
                self.push_error("unexpected sync wizard input".to_string());
            }
        }
    }

    pub(super) fn finish_sync_wizard(&mut self) {
        let Some(w) = self.sync_wizard.clone() else {
            self.push_error("sync wizard not active".to_string());
            return;
        };
        self.sync_wizard = None;

        let mut argv: Vec<String> = Vec::new();
        if let Some(s) = w.snap {
            argv.extend(["--snap-id".to_string(), s]);
        }
        if !w.lane.trim().is_empty() {
            argv.extend(["--lane".to_string(), w.lane]);
        }
        if let Some(c) = w.client {
            argv.extend(["--client-id".to_string(), c]);
        }
        self.cmd_sync_impl(&argv);
    }

    pub(super) fn start_release_wizard(&mut self, bundle_id: String) {
        self.release_wizard = Some(ReleaseWizard {
            bundle_id,
            channel: "main".to_string(),
            notes: None,
        });

        self.open_text_input_modal(
            "Release",
            "channel> ",
            TextInputAction::ReleaseChannel,
            Some("main".to_string()),
            vec![
                "Release channel name (example: main).".to_string(),
                "Esc cancels.".to_string(),
            ],
        );
    }

    pub(super) fn continue_release_wizard(&mut self, action: TextInputAction, value: String) {
        if self.release_wizard.is_none() {
            self.push_error("release wizard not active".to_string());
            return;
        }

        match action {
            TextInputAction::ReleaseChannel => {
                let v = value.trim().to_string();
                if let Some(w) = self.release_wizard.as_mut()
                    && !v.is_empty()
                {
                    w.channel = v;
                }

                self.open_text_input_modal(
                    "Release",
                    "notes (blank=none)> ",
                    TextInputAction::ReleaseNotes,
                    None,
                    vec!["Optional release notes.".to_string()],
                );
            }

            TextInputAction::ReleaseNotes => {
                let v = value.trim().to_string();
                if let Some(w) = self.release_wizard.as_mut() {
                    w.notes = if v.is_empty() { None } else { Some(v) };
                }
                self.finish_release_wizard();
            }

            _ => {
                self.push_error("unexpected release wizard input".to_string());
            }
        }
    }

    pub(super) fn finish_release_wizard(&mut self) {
        let Some(w) = self.release_wizard.clone() else {
            self.push_error("release wizard not active".to_string());
            return;
        };
        self.release_wizard = None;

        let mut argv = vec![
            "--channel".to_string(),
            w.channel,
            "--bundle-id".to_string(),
            w.bundle_id,
        ];
        if let Some(n) = w.notes {
            argv.extend(["--notes".to_string(), n]);
        }
        self.cmd_release(&argv);
    }

    pub(super) fn start_pin_wizard(&mut self) {
        if self.remote_client().is_none() {
            self.start_login_wizard();
            return;
        }

        self.pin_wizard = Some(PinWizard { bundle_id: None });
        self.open_text_input_modal(
            "Pin",
            "bundle id> ",
            TextInputAction::PinBundleId,
            None,
            vec!["Bundle id".to_string()],
        );
    }

    pub(super) fn finish_pin_wizard(&mut self, value: String) {
        let Some(w) = self.pin_wizard.clone() else {
            self.push_error("pin wizard not active".to_string());
            return;
        };

        let bundle_id = match w.bundle_id {
            Some(id) if !id.trim().is_empty() => id,
            _ => {
                self.pin_wizard = None;
                self.push_error("pin: missing bundle id".to_string());
                return;
            }
        };

        let v = value.trim().to_lowercase();
        let unpin = matches!(v.as_str(), "unpin" | "u" | "rm" | "remove");

        self.pin_wizard = None;

        let client = match self.remote_client() {
            Some(c) => c,
            None => return,
        };
        let res = if unpin {
            client.unpin_bundle(&bundle_id)
        } else {
            client.pin_bundle(&bundle_id)
        };
        match res {
            Ok(()) => {
                if unpin {
                    self.push_output(vec![format!("unpinned {}", bundle_id)]);
                } else {
                    self.push_output(vec![format!("pinned {}", bundle_id)]);
                }
                self.refresh_root_view();
            }
            Err(err) => {
                self.push_error(format!("pin: {:#}", err));
            }
        }
    }

    pub(super) fn start_promote_wizard(
        &mut self,
        bundle_id: String,
        candidates: Vec<String>,
        initial: Option<String>,
    ) {
        let initial = initial.or_else(|| candidates.first().cloned());
        let preview = candidates
            .iter()
            .take(12)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");

        self.promote_wizard = Some(PromoteWizard {
            bundle_id,
            candidates,
        });

        self.open_text_input_modal(
            "Promote",
            "to gate> ",
            TextInputAction::PromoteToGate,
            initial,
            vec![
                "Choose a destination gate.".to_string(),
                format!("candidates: {}", preview),
            ],
        );
    }

    pub(super) fn continue_promote_wizard(&mut self, value: String) {
        let Some(w) = self.promote_wizard.clone() else {
            self.push_error("promote wizard not active".to_string());
            return;
        };
        let gate = value.trim().to_string();
        if gate.is_empty() {
            self.start_promote_wizard(w.bundle_id, w.candidates, None);
            self.push_error("missing to gate".to_string());
            return;
        }
        if !w.candidates.iter().any(|g| g == &gate) {
            self.start_promote_wizard(w.bundle_id, w.candidates, Some(gate));
            self.push_error("invalid gate (not a candidate)".to_string());
            return;
        }

        self.promote_wizard = None;
        let client = match self.remote_client() {
            Some(c) => c,
            None => return,
        };
        match client.promote_bundle(&w.bundle_id, &gate) {
            Ok(_) => {
                self.push_output(vec![format!("promoted {} -> {}", w.bundle_id, gate)]);
                self.refresh_root_view();
            }
            Err(err) => self.push_error(format!("promote: {:#}", err)),
        }
    }
}
