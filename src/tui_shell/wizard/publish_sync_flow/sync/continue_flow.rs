use super::*;

fn open_sync_lane_modal(app: &mut App) {
    app.open_text_input_modal(
        "Sync",
        "lane> ",
        TextInputAction::SyncLane,
        Some("default".to_string()),
        vec!["Lane id (Enter keeps default).".to_string()],
    );
}

fn open_sync_client_modal(app: &mut App) {
    app.open_text_input_modal(
        "Sync",
        "client (blank=auto)> ",
        TextInputAction::SyncClient,
        None,
        vec!["Optional: client id (rarely needed).".to_string()],
    );
}

fn open_sync_snap_modal(app: &mut App) {
    app.open_text_input_modal(
        "Sync",
        "snap (blank=latest)> ",
        TextInputAction::SyncSnap,
        None,
        vec!["Optional: snap id (leave blank for latest).".to_string()],
    );
}

impl App {
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
                    open_sync_lane_modal(self);
                    return;
                }

                if let Some(w) = self.sync_wizard.as_mut() {
                    w.lane = v;
                }
                open_sync_client_modal(self);
            }

            TextInputAction::SyncLane => {
                let v = value.trim().to_string();
                if let Some(w) = self.sync_wizard.as_mut()
                    && !v.is_empty()
                {
                    w.lane = v;
                }
                open_sync_client_modal(self);
            }

            TextInputAction::SyncClient => {
                let v = value.trim().to_string();
                if let Some(w) = self.sync_wizard.as_mut() {
                    w.client = if v.is_empty() { None } else { Some(v) };
                }
                open_sync_snap_modal(self);
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
}
