use super::*;

impl App {
    pub(in crate::tui_shell::app) fn run_default_action(&mut self) {
        self.run_default_action_with_confirm(true);
    }

    pub(in crate::tui_shell::app) fn run_default_action_with_confirm(
        &mut self,
        confirm_destructive: bool,
    ) {
        let cmds = self.primary_hint_commands();
        if cmds.is_empty() {
            return;
        }

        let cmd = cmds[0].clone();
        self.write_trace_event(
            "user_action",
            serde_json::json!({
                "source": "default_action",
                "action": "run_primary_hint",
                "command": cmd.clone(),
                "mode": format!("{:?}", self.mode()).to_lowercase(),
                "root_context": self.root_ctx.label(),
            }),
        );
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

    pub(in crate::tui_shell::app) fn action_is_confirmed(&self, action: &PendingAction) -> bool {
        self.confirmed_action.as_ref() == Some(action)
    }
}
