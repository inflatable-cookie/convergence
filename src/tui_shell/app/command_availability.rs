use super::mode_commands::mode_command_defs;
use super::*;

impl App {
    pub(super) fn available_command_defs(&self) -> Vec<CommandDef> {
        let mode = self.mode();
        let root_ctx = self.root_ctx;
        let mut defs = mode_command_defs(mode, root_ctx);

        // If the workspace isn't initialized, only offer init + global navigation.
        if mode == UiMode::Root && root_ctx == RootContext::Local {
            if self.workspace.is_none() {
                let can_init = self
                    .workspace_err
                    .as_deref()
                    .is_some_and(|e| e.contains("No .converge directory found"));

                defs.retain(|d| {
                    d.name == "help"
                        || d.name == "quit"
                        || d.name == "clear"
                        || (can_init && d.name == "init")
                });
            } else {
                // Already initialized; hide init from the command surface.
                defs.retain(|d| d.name != "init");
            }
        }

        // If remote isn't ready, only offer login + global navigation.
        if mode == UiMode::Root
            && root_ctx == RootContext::Remote
            && (!self.remote_configured || self.remote_identity.is_none())
        {
            defs.retain(|d| {
                d.name == "login"
                    || d.name == "bootstrap"
                    || d.name == "help"
                    || d.name == "quit"
                    || d.name == "clear"
            });
        }

        // If the remote repo doesn't exist yet, only offer repo setup + safe navigation.
        if mode == UiMode::Root && root_ctx == RootContext::Remote && self.remote_repo_missing() {
            defs.retain(|d| {
                d.name == "create-repo"
                    || d.name == "remote"
                    || d.name == "ping"
                    || d.name == "login"
                    || d.name == "bootstrap"
                    || d.name == "help"
                    || d.name == "quit"
                    || d.name == "clear"
                    || d.name == "refresh"
            });
        }

        defs
    }
}
