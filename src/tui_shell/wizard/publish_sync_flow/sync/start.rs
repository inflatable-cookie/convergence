use super::*;

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
}
