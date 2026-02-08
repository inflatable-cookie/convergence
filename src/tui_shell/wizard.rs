use super::TextInputAction;

mod browse_flow;
mod fetch_flow;
mod login_bootstrap_flow;
mod member_flow;
mod move_flow;
mod move_glob;
mod publish_sync_flow;
mod release_ops_flow;
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
}
