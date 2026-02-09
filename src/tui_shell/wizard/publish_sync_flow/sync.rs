use super::*;
use crate::tui_shell::App;

impl App {
    pub(in crate::tui_shell) fn start_sync_wizard(&mut self, edit: bool) {
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

    pub(in crate::tui_shell) fn continue_sync_wizard(
        &mut self,
        action: TextInputAction,
        value: String,
    ) {
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

    pub(in crate::tui_shell) fn finish_sync_wizard(&mut self) {
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
}
