use super::*;

impl crate::tui_shell::App {
    pub(in crate::tui_shell) fn start_fetch_wizard(&mut self) {
        let Some(_) = self.require_workspace() else {
            return;
        };

        if self.remote_client().is_none() {
            // If fetch can't run, it's almost always because we need login.
            self.start_login_wizard();
            return;
        }

        self.fetch_wizard = Some(super::super::types::FetchWizard {
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
}
