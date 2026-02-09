use super::super::remote_action_parse::parse_approve_args;
use super::*;

impl App {
    pub(in crate::tui_shell::app) fn cmd_approve(&mut self, args: &[String]) {
        if args.is_empty() {
            self.open_text_input_modal(
                "Approve",
                "bundle id> ",
                TextInputAction::ApproveBundleId,
                None,
                vec!["Bundle id".to_string()],
            );
            return;
        }

        let client = match self.remote_client() {
            Some(c) => c,
            None => {
                self.start_login_wizard();
                return;
            }
        };
        let parsed = match parse_approve_args(args) {
            Ok(p) => p,
            Err(msg) => {
                self.push_error(msg);
                return;
            }
        };
        let Some(bundle_id) = parsed.bundle_id else {
            self.push_error("usage: approve <bundle_id>".to_string());
            return;
        };

        match client.approve_bundle(&bundle_id) {
            Ok(_) => self.push_output(vec![format!("approved {}", bundle_id)]),
            Err(err) => self.push_error(format!("approve: {:#}", err)),
        }
    }
}
