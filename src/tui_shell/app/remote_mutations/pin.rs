use super::super::remote_action_parse::parse_pin_args;
use super::*;

impl App {
    pub(in crate::tui_shell::app) fn cmd_pin(&mut self, args: &[String]) {
        if args.is_empty() {
            self.start_pin_wizard();
            return;
        }

        let client = match self.remote_client() {
            Some(c) => c,
            None => {
                self.start_login_wizard();
                return;
            }
        };

        let parsed = match parse_pin_args(args) {
            Ok(p) => p,
            Err(msg) => {
                self.push_error(msg);
                return;
            }
        };
        let Some(bundle_id) = parsed.bundle_id else {
            self.push_error("usage: pin <bundle_id> [unpin]".to_string());
            return;
        };

        let res = if parsed.unpin {
            client.unpin_bundle(&bundle_id)
        } else {
            client.pin_bundle(&bundle_id)
        };
        match res {
            Ok(()) => {
                if parsed.unpin {
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
}
