use crate::model::RemoteConfig;

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use super::TextInputAction;
use super::views::{BundlesView, InboxView};

#[derive(Clone, Debug)]
pub(super) struct LoginWizard {
    pub(super) url: Option<String>,
    pub(super) token: Option<String>,
    pub(super) repo: Option<String>,
    pub(super) scope: String,
    pub(super) gate: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FetchKind {
    Snap,
    Bundle,
    Release,
    Lane,
}

#[derive(Clone, Debug)]
pub(super) struct FetchWizard {
    pub(super) kind: Option<FetchKind>,
    pub(super) id: Option<String>,
    pub(super) user: Option<String>,
    pub(super) options: Option<String>,
}

#[derive(Clone, Debug)]
pub(super) struct PublishWizard {
    pub(super) snap: Option<String>,
    pub(super) scope: Option<String>,
    pub(super) gate: Option<String>,
    pub(super) meta: bool,
}

#[derive(Clone, Debug)]
pub(super) struct SyncWizard {
    pub(super) snap: Option<String>,
    pub(super) lane: String,
    pub(super) client: Option<String>,
}

#[derive(Clone, Debug)]
pub(super) struct ReleaseWizard {
    pub(super) bundle_id: String,
    pub(super) channel: String,
    pub(super) notes: Option<String>,
}

#[derive(Clone, Debug)]
pub(super) struct PinWizard {
    pub(super) bundle_id: Option<String>,
}

#[derive(Clone, Debug)]
pub(super) struct PromoteWizard {
    pub(super) bundle_id: String,
    pub(super) candidates: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MemberAction {
    Add,
    Remove,
}

#[derive(Clone, Debug)]
pub(super) struct MemberWizard {
    pub(super) action: Option<MemberAction>,
    pub(super) handle: Option<String>,
    pub(super) role: String,
}

#[derive(Clone, Debug)]
pub(super) struct LaneMemberWizard {
    pub(super) action: Option<MemberAction>,
    pub(super) lane: Option<String>,
    pub(super) handle: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BrowseTarget {
    Inbox,
    Bundles,
}

#[derive(Clone, Debug)]
pub(super) struct BrowseWizard {
    pub(super) target: BrowseTarget,
    pub(super) scope: String,
    pub(super) gate: String,
    pub(super) filter: Option<String>,
    pub(super) limit: Option<usize>,
}

#[derive(Clone, Debug)]
pub(super) struct MoveWizard {
    pub(super) query: Option<String>,
    pub(super) candidates: Vec<String>,
    pub(super) from: Option<String>,
}

impl super::App {
    pub(super) fn cancel_wizards(&mut self) {
        self.login_wizard = None;
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

    pub(super) fn start_fetch_wizard(&mut self) {
        let Some(_) = self.require_workspace() else {
            return;
        };

        if self.remote_client().is_none() {
            // If fetch can't run, it's almost always because we need login.
            self.start_login_wizard();
            return;
        }

        self.fetch_wizard = Some(FetchWizard {
            kind: None,
            id: None,
            user: None,
            options: None,
        });

        self.open_text_input_modal(
            "Fetch",
            "what> ",
            TextInputAction::FetchKind,
            Some("snap".to_string()),
            vec!["What to fetch? snap | bundle | release | lane".to_string()],
        );
    }

    pub(super) fn continue_fetch_wizard(&mut self, action: TextInputAction, value: String) {
        if self.fetch_wizard.is_none() {
            self.push_error("fetch wizard not active".to_string());
            return;
        }

        match action {
            TextInputAction::FetchKind => {
                let v = value.trim().to_lowercase();
                let v = if v.is_empty() { "snap".to_string() } else { v };
                let kind = match v.as_str() {
                    "snap" | "snaps" => Some(FetchKind::Snap),
                    "bundle" | "bundles" => Some(FetchKind::Bundle),
                    "release" | "releases" => Some(FetchKind::Release),
                    "lane" | "lanes" => Some(FetchKind::Lane),
                    _ => None,
                };

                let Some(kind) = kind else {
                    self.open_text_input_modal(
                        "Fetch",
                        "what> ",
                        TextInputAction::FetchKind,
                        Some("snap".to_string()),
                        vec![
                            "error: choose snap | bundle | release | lane".to_string(),
                            "".to_string(),
                            "What to fetch?".to_string(),
                        ],
                    );
                    return;
                };

                if let Some(w) = self.fetch_wizard.as_mut() {
                    w.kind = Some(kind);
                }

                let (prompt, initial, lines) = match kind {
                    FetchKind::Snap => (
                        "snap id (blank=all)> ",
                        None,
                        vec!["Optional: leave blank to fetch all publications.".to_string()],
                    ),
                    FetchKind::Bundle => ("bundle id> ", None, vec!["Paste bundle id".to_string()]),
                    FetchKind::Release => (
                        "channel> ",
                        None,
                        vec!["Release channel name (example: main)".to_string()],
                    ),
                    FetchKind::Lane => (
                        "lane id> ",
                        Some("default".to_string()),
                        vec!["Lane id (example: default)".to_string()],
                    ),
                };

                self.open_text_input_modal(
                    "Fetch",
                    prompt,
                    TextInputAction::FetchId,
                    initial,
                    lines,
                );
            }

            TextInputAction::FetchId => {
                let kind = self.fetch_wizard.as_ref().and_then(|w| w.kind);
                let Some(kind) = kind else {
                    self.start_fetch_wizard();
                    return;
                };

                let id = value.trim().to_string();
                if id.is_empty() && kind != FetchKind::Snap {
                    let prompt = match kind {
                        FetchKind::Bundle => "bundle id> ",
                        FetchKind::Release => "channel> ",
                        FetchKind::Lane => "lane id> ",
                        FetchKind::Snap => "snap id (blank=all)> ",
                    };
                    self.open_text_input_modal(
                        "Fetch",
                        prompt,
                        TextInputAction::FetchId,
                        None,
                        vec!["error: value required".to_string()],
                    );
                    return;
                }

                if let Some(w) = self.fetch_wizard.as_mut() {
                    w.id = if id.is_empty() { None } else { Some(id) };
                }

                match kind {
                    FetchKind::Lane => {
                        self.open_text_input_modal(
                            "Fetch",
                            "user (blank=all)> ",
                            TextInputAction::FetchUser,
                            None,
                            vec!["Optional: filter by user handle".to_string()],
                        );
                    }
                    FetchKind::Bundle | FetchKind::Release => {
                        self.open_text_input_modal(
                            "Fetch",
                            "options> ",
                            TextInputAction::FetchOptions,
                            None,
                            vec![
                                "Optional:".to_string(),
                                "- empty: fetch only".to_string(),
                                "- restore: also materialize into a directory".to_string(),
                                "- into <dir>: choose directory (implies restore)".to_string(),
                                "- force: overwrite files when restoring".to_string(),
                            ],
                        );
                    }
                    FetchKind::Snap => {
                        self.finish_fetch_wizard();
                    }
                }
            }

            TextInputAction::FetchUser => {
                let v = value.trim().to_string();
                if let Some(w) = self.fetch_wizard.as_mut() {
                    w.user = if v.is_empty() { None } else { Some(v) };
                }
                self.finish_fetch_wizard();
            }

            TextInputAction::FetchOptions => {
                if let Some(w) = self.fetch_wizard.as_mut() {
                    let v = value.trim().to_string();
                    w.options = if v.is_empty() { None } else { Some(v) };
                }
                self.finish_fetch_wizard();
            }

            _ => {
                self.push_error("unexpected fetch wizard input".to_string());
            }
        }
    }

    pub(super) fn finish_fetch_wizard(&mut self) {
        let Some(w) = self.fetch_wizard.clone() else {
            self.push_error("fetch wizard not active".to_string());
            return;
        };
        self.fetch_wizard = None;

        let Some(kind) = w.kind else {
            self.push_error("fetch: missing kind".to_string());
            return;
        };

        let mut argv: Vec<String> = Vec::new();
        match kind {
            FetchKind::Snap => {
                if let Some(id) = w.id {
                    argv.extend(["--snap-id".to_string(), id]);
                }
            }
            FetchKind::Bundle => {
                let Some(id) = w.id else {
                    self.push_error("fetch: missing bundle id".to_string());
                    return;
                };
                argv.extend(["--bundle-id".to_string(), id]);
            }
            FetchKind::Release => {
                let Some(id) = w.id else {
                    self.push_error("fetch: missing channel".to_string());
                    return;
                };
                argv.extend(["--release".to_string(), id]);
            }
            FetchKind::Lane => {
                let Some(id) = w.id else {
                    self.push_error("fetch: missing lane".to_string());
                    return;
                };
                argv.extend(["--lane".to_string(), id]);
                if let Some(u) = w.user {
                    argv.extend(["--user".to_string(), u]);
                }
            }
        }

        if matches!(kind, FetchKind::Bundle | FetchKind::Release) {
            let mut restore = false;
            let mut into: Option<String> = None;
            let mut force = false;

            if let Some(s) = w.options {
                let parts = s.split_whitespace().collect::<Vec<_>>();
                let mut i = 0;
                while i < parts.len() {
                    match parts[i].to_lowercase().as_str() {
                        "restore" => restore = true,
                        "force" => force = true,
                        "into" => {
                            i += 1;
                            if i < parts.len() {
                                into = Some(parts[i].to_string());
                            }
                        }
                        _ => {}
                    }
                    i += 1;
                }

                if into.is_some() || force {
                    restore = true;
                }
            }

            if restore {
                argv.push("--restore".to_string());
            }
            if let Some(p) = into {
                argv.extend(["--into".to_string(), p]);
            }
            if force {
                argv.push("--force".to_string());
            }
        }

        self.cmd_fetch_impl(&argv);
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

    pub(super) fn start_member_wizard(&mut self, action: Option<MemberAction>) {
        if self.remote_client().is_none() {
            self.start_login_wizard();
            return;
        }

        self.member_wizard = Some(MemberWizard {
            action,
            handle: None,
            role: "read".to_string(),
        });

        match action {
            None => {
                self.open_text_input_modal(
                    "Member",
                    "action> ",
                    TextInputAction::MemberAction,
                    Some("add".to_string()),
                    vec!["add | remove".to_string()],
                );
            }
            Some(_) => {
                self.open_text_input_modal(
                    "Member",
                    "handle> ",
                    TextInputAction::MemberHandle,
                    None,
                    vec!["GitHub handle / user handle".to_string()],
                );
            }
        }
    }

    pub(super) fn continue_member_wizard(&mut self, action: TextInputAction, value: String) {
        if self.member_wizard.is_none() {
            self.push_error("member wizard not active".to_string());
            return;
        }

        match action {
            TextInputAction::MemberAction => {
                let v = value.trim().to_lowercase();
                let act = match v.as_str() {
                    "add" => Some(MemberAction::Add),
                    "remove" | "rm" | "del" => Some(MemberAction::Remove),
                    _ => None,
                };
                let Some(act) = act else {
                    self.open_text_input_modal(
                        "Member",
                        "action> ",
                        TextInputAction::MemberAction,
                        Some("add".to_string()),
                        vec!["error: choose add | remove".to_string()],
                    );
                    return;
                };
                if let Some(w) = self.member_wizard.as_mut() {
                    w.action = Some(act);
                }
                self.open_text_input_modal(
                    "Member",
                    "handle> ",
                    TextInputAction::MemberHandle,
                    None,
                    vec!["GitHub handle / user handle".to_string()],
                );
            }
            TextInputAction::MemberHandle => {
                let handle = value.trim().to_string();
                if handle.is_empty() {
                    self.open_text_input_modal(
                        "Member",
                        "handle> ",
                        TextInputAction::MemberHandle,
                        None,
                        vec!["error: value required".to_string()],
                    );
                    return;
                }
                let act = self.member_wizard.as_ref().and_then(|w| w.action);
                if let Some(w) = self.member_wizard.as_mut() {
                    w.handle = Some(handle);
                }
                match act {
                    Some(MemberAction::Add) => {
                        self.open_text_input_modal(
                            "Member",
                            "role (read/publish)> ",
                            TextInputAction::MemberRole,
                            Some("read".to_string()),
                            vec!["Default: read".to_string()],
                        );
                    }
                    Some(MemberAction::Remove) => {
                        self.finish_member_wizard();
                    }
                    None => {
                        self.start_member_wizard(None);
                    }
                }
            }
            TextInputAction::MemberRole => {
                let role = value.trim().to_lowercase();
                let role = if role.is_empty() {
                    "read".to_string()
                } else {
                    role
                };
                if role != "read" && role != "publish" {
                    self.open_text_input_modal(
                        "Member",
                        "role (read/publish)> ",
                        TextInputAction::MemberRole,
                        Some(role),
                        vec!["error: role must be read or publish".to_string()],
                    );
                    return;
                }
                if let Some(w) = self.member_wizard.as_mut() {
                    w.role = role;
                }
                self.finish_member_wizard();
            }
            _ => {
                self.push_error("unexpected member wizard input".to_string());
            }
        }
    }

    pub(super) fn finish_member_wizard(&mut self) {
        let Some(w) = self.member_wizard.clone() else {
            self.push_error("member wizard not active".to_string());
            return;
        };
        self.member_wizard = None;

        let client = match self.remote_client() {
            Some(c) => c,
            None => return,
        };
        let Some(action) = w.action else {
            self.push_error("member: missing action".to_string());
            return;
        };
        let Some(handle) = w.handle else {
            self.push_error("member: missing handle".to_string());
            return;
        };

        match action {
            MemberAction::Add => match client.add_repo_member(&handle, &w.role) {
                Ok(()) => {
                    self.push_output(vec![format!("added {} ({})", handle, w.role)]);
                    self.refresh_root_view();
                }
                Err(err) => self.push_error(format!("member add: {:#}", err)),
            },
            MemberAction::Remove => match client.remove_repo_member(&handle) {
                Ok(()) => {
                    self.push_output(vec![format!("removed {}", handle)]);
                    self.refresh_root_view();
                }
                Err(err) => self.push_error(format!("member remove: {:#}", err)),
            },
        }
    }

    pub(super) fn start_lane_member_wizard(&mut self, action: Option<MemberAction>) {
        if self.remote_client().is_none() {
            self.start_login_wizard();
            return;
        }

        self.lane_member_wizard = Some(LaneMemberWizard {
            action,
            lane: None,
            handle: None,
        });

        match action {
            None => {
                self.open_text_input_modal(
                    "Lane Member",
                    "action> ",
                    TextInputAction::LaneMemberAction,
                    Some("add".to_string()),
                    vec!["add | remove".to_string()],
                );
            }
            Some(_) => {
                self.open_text_input_modal(
                    "Lane Member",
                    "lane> ",
                    TextInputAction::LaneMemberLane,
                    Some("default".to_string()),
                    vec!["Lane id".to_string()],
                );
            }
        }
    }

    pub(super) fn continue_lane_member_wizard(&mut self, action: TextInputAction, value: String) {
        if self.lane_member_wizard.is_none() {
            self.push_error("lane-member wizard not active".to_string());
            return;
        }

        match action {
            TextInputAction::LaneMemberAction => {
                let v = value.trim().to_lowercase();
                let act = match v.as_str() {
                    "add" => Some(MemberAction::Add),
                    "remove" | "rm" | "del" => Some(MemberAction::Remove),
                    _ => None,
                };
                let Some(act) = act else {
                    self.open_text_input_modal(
                        "Lane Member",
                        "action> ",
                        TextInputAction::LaneMemberAction,
                        Some("add".to_string()),
                        vec!["error: choose add | remove".to_string()],
                    );
                    return;
                };
                if let Some(w) = self.lane_member_wizard.as_mut() {
                    w.action = Some(act);
                }
                self.open_text_input_modal(
                    "Lane Member",
                    "lane> ",
                    TextInputAction::LaneMemberLane,
                    Some("default".to_string()),
                    vec!["Lane id".to_string()],
                );
            }
            TextInputAction::LaneMemberLane => {
                let lane = value.trim().to_string();
                if lane.is_empty() {
                    self.open_text_input_modal(
                        "Lane Member",
                        "lane> ",
                        TextInputAction::LaneMemberLane,
                        Some("default".to_string()),
                        vec!["error: value required".to_string()],
                    );
                    return;
                }
                if let Some(w) = self.lane_member_wizard.as_mut() {
                    w.lane = Some(lane);
                }
                self.open_text_input_modal(
                    "Lane Member",
                    "handle> ",
                    TextInputAction::LaneMemberHandle,
                    None,
                    vec!["User handle".to_string()],
                );
            }
            TextInputAction::LaneMemberHandle => {
                let handle = value.trim().to_string();
                if handle.is_empty() {
                    self.open_text_input_modal(
                        "Lane Member",
                        "handle> ",
                        TextInputAction::LaneMemberHandle,
                        None,
                        vec!["error: value required".to_string()],
                    );
                    return;
                }
                if let Some(w) = self.lane_member_wizard.as_mut() {
                    w.handle = Some(handle);
                }
                self.finish_lane_member_wizard();
            }
            _ => {
                self.push_error("unexpected lane-member wizard input".to_string());
            }
        }
    }

    pub(super) fn finish_lane_member_wizard(&mut self) {
        let Some(w) = self.lane_member_wizard.clone() else {
            self.push_error("lane-member wizard not active".to_string());
            return;
        };
        self.lane_member_wizard = None;

        let client = match self.remote_client() {
            Some(c) => c,
            None => return,
        };
        let Some(action) = w.action else {
            self.push_error("lane-member: missing action".to_string());
            return;
        };
        let Some(lane) = w.lane else {
            self.push_error("lane-member: missing lane".to_string());
            return;
        };
        let Some(handle) = w.handle else {
            self.push_error("lane-member: missing handle".to_string());
            return;
        };

        match action {
            MemberAction::Add => match client.add_lane_member(&lane, &handle) {
                Ok(()) => {
                    self.push_output(vec![format!("added {} to lane {}", handle, lane)]);
                    self.refresh_root_view();
                }
                Err(err) => self.push_error(format!("lane-member add: {:#}", err)),
            },
            MemberAction::Remove => match client.remove_lane_member(&lane, &handle) {
                Ok(()) => {
                    self.push_output(vec![format!("removed {} from lane {}", handle, lane)]);
                    self.refresh_root_view();
                }
                Err(err) => self.push_error(format!("lane-member remove: {:#}", err)),
            },
        }
    }

    pub(super) fn start_browse_wizard(&mut self, target: BrowseTarget) {
        let cfg = match self.remote_config() {
            Some(c) => c,
            None => {
                self.start_login_wizard();
                return;
            }
        };

        let (scope, gate, filter, limit) = match target {
            BrowseTarget::Inbox => self
                .current_view::<InboxView>()
                .map(|v| (v.scope.clone(), v.gate.clone(), v.filter.clone(), v.limit))
                .unwrap_or((cfg.scope.clone(), cfg.gate.clone(), None, None)),
            BrowseTarget::Bundles => self
                .current_view::<BundlesView>()
                .map(|v| (v.scope.clone(), v.gate.clone(), v.filter.clone(), v.limit))
                .unwrap_or((cfg.scope.clone(), cfg.gate.clone(), None, None)),
        };

        self.browse_wizard = Some(BrowseWizard {
            target,
            scope,
            gate,
            filter,
            limit,
        });

        let initial = self.browse_wizard.as_ref().map(|w| w.scope.clone());
        self.open_text_input_modal(
            "Browse",
            "scope> ",
            TextInputAction::BrowseScope,
            initial,
            vec!["Scope id (Enter keeps current).".to_string()],
        );
    }

    pub(super) fn continue_browse_wizard(&mut self, action: TextInputAction, value: String) {
        if self.browse_wizard.is_none() {
            self.push_error("browse wizard not active".to_string());
            return;
        }

        match action {
            TextInputAction::BrowseScope => {
                let v = value.trim().to_string();
                if let Some(w) = self.browse_wizard.as_mut()
                    && !v.is_empty()
                {
                    w.scope = v;
                }
                let initial = self.browse_wizard.as_ref().map(|w| w.gate.clone());
                self.open_text_input_modal(
                    "Browse",
                    "gate> ",
                    TextInputAction::BrowseGate,
                    initial,
                    vec!["Gate id (Enter keeps current).".to_string()],
                );
            }
            TextInputAction::BrowseGate => {
                let v = value.trim().to_string();
                if let Some(w) = self.browse_wizard.as_mut()
                    && !v.is_empty()
                {
                    w.gate = v;
                }
                let initial = self.browse_wizard.as_ref().and_then(|w| w.filter.clone());
                self.open_text_input_modal(
                    "Browse",
                    "filter (blank=none)> ",
                    TextInputAction::BrowseFilter,
                    initial,
                    vec!["Optional filter query".to_string()],
                );
            }
            TextInputAction::BrowseFilter => {
                let v = value.trim().to_string();
                if let Some(w) = self.browse_wizard.as_mut() {
                    w.filter = if v.is_empty() { None } else { Some(v) };
                }
                let initial = self
                    .browse_wizard
                    .as_ref()
                    .and_then(|w| w.limit)
                    .map(|n| n.to_string());
                self.open_text_input_modal(
                    "Browse",
                    "limit (blank=none)> ",
                    TextInputAction::BrowseLimit,
                    initial,
                    vec!["Optional limit".to_string()],
                );
            }
            TextInputAction::BrowseLimit => {
                let v = value.trim().to_string();
                let limit = if v.is_empty() {
                    None
                } else {
                    match v.parse::<usize>() {
                        Ok(n) => Some(n),
                        Err(_) => {
                            self.open_text_input_modal(
                                "Browse",
                                "limit (blank=none)> ",
                                TextInputAction::BrowseLimit,
                                Some(v),
                                vec!["error: invalid number".to_string()],
                            );
                            return;
                        }
                    }
                };
                if let Some(w) = self.browse_wizard.as_mut() {
                    w.limit = limit;
                }
                self.finish_browse_wizard();
            }
            _ => {
                self.push_error("unexpected browse wizard input".to_string());
            }
        }
    }

    pub(super) fn finish_browse_wizard(&mut self) {
        let Some(w) = self.browse_wizard.clone() else {
            self.push_error("browse wizard not active".to_string());
            return;
        };
        self.browse_wizard = None;

        match w.target {
            BrowseTarget::Inbox => self.open_inbox_view(w.scope, w.gate, w.filter, w.limit),
            BrowseTarget::Bundles => self.open_bundles_view(w.scope, w.gate, w.filter, w.limit),
        }
    }

    fn move_wizard_from(&mut self, value: String) {
        let Some(ws) = self.require_workspace() else {
            return;
        };
        let Some(w) = self.move_wizard.as_mut() else {
            self.push_error("move wizard not active".to_string());
            return;
        };

        let raw = value.trim().to_string();
        if raw.is_empty() {
            self.push_error("missing from glob".to_string());
            return;
        }

        if !w.candidates.is_empty()
            && raw.chars().all(|c| c.is_ascii_digit())
            && let Ok(n) = raw.parse::<usize>()
            && n >= 1
            && n <= w.candidates.len()
        {
            let from = w.candidates[n - 1].clone();
            w.candidates.clear();
            w.query = Some(raw);
            w.from = Some(from.clone());
            self.open_text_input_modal(
                "Move",
                "to> ",
                TextInputAction::MoveTo,
                Some(from.clone()),
                vec![
                    format!("from: {}", from),
                    "Edit the destination path (relative to workspace root).".to_string(),
                ],
            );
            return;
        }

        w.query = Some(raw.clone());
        let matches = match glob_search(&ws.root, &raw) {
            Ok(m) => m,
            Err(err) => {
                w.candidates.clear();
                self.open_text_input_modal(
                    "Move",
                    "from (glob)> ",
                    TextInputAction::MoveFrom,
                    Some(raw),
                    vec![
                        format!("error: {:#}", err),
                        "".to_string(),
                        "Enter a glob to find the source path.".to_string(),
                    ],
                );
                return;
            }
        };

        if matches.is_empty() {
            w.candidates.clear();
            self.open_text_input_modal(
                "Move",
                "from (glob)> ",
                TextInputAction::MoveFrom,
                Some(raw),
                vec![
                    "error: no matches".to_string(),
                    "".to_string(),
                    "Try a more specific glob.".to_string(),
                ],
            );
            return;
        }

        if matches.len() == 1 {
            let from = matches[0].clone();
            w.candidates.clear();
            w.from = Some(from.clone());
            self.open_text_input_modal(
                "Move",
                "to> ",
                TextInputAction::MoveTo,
                Some(from.clone()),
                vec![
                    format!("from: {}", from),
                    "Edit the destination path (relative to workspace root).".to_string(),
                ],
            );
            return;
        }

        w.candidates = matches.clone();
        let mut lines = Vec::new();
        lines.push(format!("matches: {}", matches.len()));
        lines.push("Enter a number to pick, or refine the glob.".to_string());
        lines.push("".to_string());

        let limit = 20usize;
        for (i, p) in matches.iter().take(limit).enumerate() {
            lines.push(format!("{:>2}. {}", i + 1, p));
        }
        if matches.len() > limit {
            lines.push(format!("… and {} more", matches.len() - limit));
        }

        self.open_text_input_modal(
            "Move",
            "from (glob or #)> ",
            TextInputAction::MoveFrom,
            Some(raw),
            lines,
        );
    }

    fn move_wizard_to(&mut self, value: String) {
        let Some(ws) = self.require_workspace() else {
            return;
        };
        let Some(w) = self.move_wizard.as_mut() else {
            self.push_error("move wizard not active".to_string());
            return;
        };

        let Some(from) = w.from.clone() else {
            self.push_error("missing from".to_string());
            self.move_wizard = None;
            return;
        };

        let to = value.trim().trim_start_matches("./").to_string();
        if to.is_empty() {
            self.push_error("missing destination".to_string());
            return;
        }
        if to == from {
            self.push_error("destination must differ from source".to_string());
            return;
        }

        match ws.move_path(Path::new(&from), Path::new(&to)) {
            Ok(()) => {
                self.move_wizard = None;
                self.push_output(vec![format!("moved {} -> {}", from, to)]);
                self.refresh_root_view();
            }
            Err(err) => {
                self.open_text_input_modal(
                    "Move",
                    "to> ",
                    TextInputAction::MoveTo,
                    Some(to),
                    vec![
                        format!("from: {}", from),
                        format!("error: {:#}", err),
                        "Edit destination and try again.".to_string(),
                    ],
                );
            }
        }
    }
}

fn is_glob_query(s: &str) -> bool {
    s.contains('*') || s.contains('?') || s.contains('[')
}

fn normalize_glob_query(q: &str) -> String {
    let q = q.trim().trim_start_matches("./");
    if q.is_empty() {
        return q.to_string();
    }
    if is_glob_query(q) {
        q.to_string()
    } else {
        format!("**/*{}*", q)
    }
}

fn collect_paths(root: &Path) -> Result<Vec<String>> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) -> Result<()> {
        for entry in fs::read_dir(dir).with_context(|| format!("read dir {}", dir.display()))? {
            let entry = entry.context("read dir entry")?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.as_ref() == ".converge" || name.as_ref() == ".git" {
                continue;
            }
            if name.as_ref().starts_with(".converge_tmp_") {
                continue;
            }

            let path = entry.path();
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            out.push(rel);

            let ft = entry.file_type().context("read file type")?;
            if ft.is_dir() {
                walk(root, &path, out)?;
            }
        }
        Ok(())
    }

    let mut out = Vec::new();
    walk(root, root, &mut out)?;
    out.sort();
    Ok(out)
}

fn glob_search(root: &Path, query: &str) -> Result<Vec<String>> {
    let query = normalize_glob_query(query);
    let matcher = globset::Glob::new(&query)
        .with_context(|| format!("invalid glob: {}", query))?
        .compile_matcher();

    let all = collect_paths(root)?;
    let mut out = Vec::new();
    for p in all {
        if matcher.is_match(&p) {
            out.push(p);
        }
    }
    Ok(out)
}
