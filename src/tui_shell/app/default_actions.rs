use super::*;

impl App {
    fn hint_key(&self) -> usize {
        match (self.mode(), self.root_ctx) {
            (UiMode::Root, RootContext::Local) => 0,
            (UiMode::Root, RootContext::Remote) => 1,
            (UiMode::Snaps, _) => 2,
            (UiMode::Inbox, _) => 3,
            (UiMode::Bundles, _) => 4,
            (UiMode::Releases, _) => 5,
            (UiMode::Lanes, _) => 6,
            (UiMode::Superpositions, _) => 7,
            (UiMode::GateGraph, _) => 8,
            (UiMode::Settings, _) => 9,
        }
    }

    pub(super) fn rotate_hint(&mut self, dir: i32) {
        if !self.input.buf.is_empty() || self.modal.is_some() {
            return;
        }

        let n = self.hint_commands_raw().len();
        if n <= 1 {
            self.hint_rotation[self.hint_key()] = 0;
            return;
        }

        let key = self.hint_key();

        if dir > 0 {
            self.hint_rotation[key] = (self.hint_rotation[key] + 1) % n;
        } else if dir < 0 {
            self.hint_rotation[key] = (self.hint_rotation[key] + n - 1) % n;
        }
    }

    fn hint_commands_raw(&self) -> Vec<String> {
        match self.mode() {
            UiMode::Root => match self.root_ctx {
                RootContext::Local => {
                    if self.workspace.is_none() {
                        // Only suggest init if we're truly uninitialized.
                        if self
                            .workspace_err
                            .as_deref()
                            .is_some_and(|e| e.contains("No .converge directory found"))
                        {
                            return vec!["init".to_string()];
                        }
                        return Vec::new();
                    }

                    let mut changes = 0usize;
                    if let Some(v) = self.current_view::<RootView>() {
                        changes = v.change_summary.added
                            + v.change_summary.modified
                            + v.change_summary.deleted
                            + v.change_summary.renamed;
                    }
                    if changes > 0 {
                        return vec!["snap".to_string(), "history".to_string()];
                    }

                    if self.remote_configured {
                        let latest = self.latest_snap_id.clone();
                        let synced = self.lane_last_synced.get("default").cloned();
                        if latest.is_some() && latest != synced {
                            return vec!["sync".to_string(), "history".to_string()];
                        }
                        if latest.is_some() && latest != self.last_published_snap_id {
                            return vec!["publish".to_string(), "history".to_string()];
                        }
                    }

                    vec!["history".to_string()]
                }
                RootContext::Remote => {
                    if !self.remote_configured || self.remote_identity.is_none() {
                        vec!["login".to_string(), "bootstrap".to_string()]
                    } else if self.remote_repo_missing() {
                        vec!["create-repo".to_string()]
                    } else {
                        vec!["inbox".to_string(), "releases".to_string()]
                    }
                }
            },
            UiMode::Snaps => {
                let Some(v) = self.current_view::<SnapsView>() else {
                    return Vec::new();
                };
                if v.selected_is_pending() {
                    vec!["snap".to_string(), "revert".to_string()]
                } else if v.selected_is_clean() {
                    vec!["unsnap".to_string()]
                } else {
                    vec!["restore".to_string(), "msg".to_string()]
                }
            }
            UiMode::Inbox => vec!["bundle".to_string(), "fetch".to_string()],
            UiMode::Releases => vec!["fetch".to_string(), "back".to_string()],
            UiMode::Lanes => vec!["fetch".to_string(), "back".to_string()],
            UiMode::Bundles => {
                let Some(v) = self.current_view::<BundlesView>() else {
                    return Vec::new();
                };
                if v.items.is_empty() {
                    return vec!["back".to_string()];
                }
                let idx = v.selected.min(v.items.len().saturating_sub(1));
                let b = &v.items[idx];

                if b.reasons.iter().any(|r| r == "superpositions_present") {
                    return vec!["superpositions".to_string(), "back".to_string()];
                }
                if b.reasons.iter().any(|r| r == "approvals_missing") {
                    return vec!["approve".to_string(), "back".to_string()];
                }
                if b.promotable {
                    return vec!["promote".to_string(), "back".to_string()];
                }

                vec!["back".to_string()]
            }
            UiMode::Superpositions => {
                let Some(v) = self.current_view::<SuperpositionsView>() else {
                    return Vec::new();
                };
                let missing = v
                    .validation
                    .as_ref()
                    .map(|x| !x.missing.is_empty())
                    .unwrap_or(false);
                if missing {
                    vec!["next-missing".to_string(), "pick".to_string()]
                } else {
                    vec!["apply".to_string(), "back".to_string()]
                }
            }

            UiMode::GateGraph => Vec::new(),

            UiMode::Settings => {
                let Some(v) = self.current_view::<SettingsView>() else {
                    return vec!["back".to_string()];
                };
                match v.selected_kind() {
                    None => vec!["back".to_string()],
                    Some(_) => vec!["do".to_string(), "back".to_string()],
                }
            }
        }
    }

    pub(super) fn primary_hint_commands(&self) -> Vec<String> {
        let raw = self.hint_commands_raw();
        if raw.is_empty() {
            return raw;
        }
        let n = raw.len();
        let rot = self.hint_rotation[self.hint_key()] % n;
        if rot == 0 {
            return raw;
        }
        raw.into_iter().cycle().skip(rot).take(n).collect()
    }

    pub(super) fn run_default_action(&mut self) {
        self.run_default_action_with_confirm(true);
    }

    pub(super) fn run_default_action_with_confirm(&mut self, confirm_destructive: bool) {
        let cmds = self.primary_hint_commands();
        if cmds.is_empty() {
            return;
        }

        let cmd = cmds[0].clone();
        let action = if self.mode() == UiMode::Root {
            PendingAction::Root {
                root_ctx: self.root_ctx,
                cmd: cmd.clone(),
            }
        } else {
            PendingAction::Mode {
                mode: self.mode(),
                cmd: cmd.clone(),
            }
        };

        if confirm_destructive && self.is_destructive_default_action(&cmd) {
            self.open_confirm_modal(action);
            return;
        }

        self.execute_action(action);
    }

    fn is_destructive_default_action(&self, cmd: &str) -> bool {
        match (self.mode(), self.root_ctx, cmd) {
            // Local filesystem destructive.
            (UiMode::Snaps, _, "restore") => true,
            (UiMode::Snaps, _, "revert") => true,
            (UiMode::Snaps, _, "unsnap") => true,

            // Remote state mutations that are hard to "undo".
            (UiMode::Bundles, _, "promote") => true,
            (UiMode::Bundles, _, "release") => true,

            // Anything explicitly about GC/retention.
            (UiMode::Root, RootContext::Local, "purge") => true,

            // Settings resets.
            (UiMode::Settings, _, "do") => {
                let Some(v) = self.current_view::<SettingsView>() else {
                    return false;
                };
                matches!(
                    v.selected_kind(),
                    Some(SettingsItemKind::ChunkingReset | SettingsItemKind::RetentionReset)
                )
            }

            _ => false,
        }
    }

    pub(super) fn open_confirm_modal(&mut self, action: PendingAction) {
        let (cmd, context) = match &action {
            PendingAction::Root { root_ctx, cmd } => (cmd.as_str(), root_ctx.label()),
            PendingAction::Mode { mode, cmd } => (cmd.as_str(), mode.prompt()),
        };

        let cmd_display = match &action {
            PendingAction::Mode { mode, cmd }
                if *mode == UiMode::Settings && cmd.as_str() == "do" =>
            {
                match self
                    .current_view::<SettingsView>()
                    .and_then(|v| v.selected_kind())
                {
                    Some(SettingsItemKind::ChunkingReset) => "reset chunking".to_string(),
                    Some(SettingsItemKind::RetentionReset) => "reset retention".to_string(),
                    _ => "settings action".to_string(),
                }
            }
            _ => cmd.to_string(),
        };

        let mut lines = Vec::new();
        lines.push(format!("Run: {}", cmd_display));
        lines.push(format!("Where: {}", context));
        lines.push("".to_string());
        lines.push("This action changes data.".to_string());
        lines.push("Enter: confirm    Esc: cancel".to_string());

        self.modal = Some(Modal {
            title: "Confirm".to_string(),
            lines,
            scroll: 0,
            kind: ModalKind::ConfirmAction { action },
            input: Input::default(),
        });
    }

    pub(in crate::tui_shell) fn execute_action(&mut self, action: PendingAction) {
        match action {
            PendingAction::Root { root_ctx: _, cmd } => self.dispatch_root(cmd.as_str(), &[]),
            PendingAction::Mode { mode, cmd } => self.dispatch_mode(mode, cmd.as_str(), &[]),
        }
    }

    pub(in crate::tui_shell) fn execute_action_confirmed(&mut self, action: PendingAction) {
        self.confirmed_action = Some(action.clone());
        self.execute_action(action);
        self.confirmed_action = None;
    }

    pub(super) fn action_is_confirmed(&self, action: &PendingAction) -> bool {
        self.confirmed_action.as_ref() == Some(action)
    }
}
