use super::publish_args::parse_publish_args;
use super::*;

impl App {
    pub(in crate::tui_shell) fn cmd_publish(&mut self, args: &[String]) {
        if args.len() == 1 && matches!(args[0].as_str(), "edit" | "prompt" | "custom") {
            self.start_publish_wizard(true);
            return;
        }

        if args.is_empty() {
            self.start_publish_wizard(false);
            return;
        }
        self.cmd_publish_impl(args);
    }

    pub(in crate::tui_shell) fn cmd_publish_impl(&mut self, args: &[String]) {
        let Some(ws) = self.require_workspace() else {
            return;
        };
        let Some(cfg) = self.remote_config() else {
            self.start_login_wizard();
            return;
        };

        let parsed = match parse_publish_args(args) {
            Ok(parsed) => parsed,
            Err(msg) => {
                self.push_error(msg);
                return;
            }
        };

        let snap_id = match parsed.snap_id {
            Some(id) => id,
            None => match ws.list_snaps() {
                Ok(snaps) => match snaps.first() {
                    Some(s) => s.id.clone(),
                    None => {
                        self.push_error("no snaps to publish".to_string());
                        return;
                    }
                },
                Err(err) => {
                    self.push_error(format!("list snaps: {:#}", err));
                    return;
                }
            },
        };

        let snap = match ws.store.get_snap(&snap_id) {
            Ok(s) => s,
            Err(err) => {
                self.push_error(format!("read snap: {:#}", err));
                return;
            }
        };

        let token = match ws.store.get_remote_token(&cfg) {
            Ok(Some(t)) => t,
            Ok(None) => {
                self.push_error(
                    "no remote token configured (run `login <url> <token> <repo>`)".to_string(),
                );
                self.start_login_wizard();
                return;
            }
            Err(err) => {
                self.push_error(format!("read remote token: {:#}", err));
                return;
            }
        };

        let client = match RemoteClient::new(cfg.clone(), token) {
            Ok(c) => c,
            Err(err) => {
                self.push_error(format!("init remote client: {:#}", err));
                return;
            }
        };

        let scope = parsed.scope.unwrap_or_else(|| cfg.scope.clone());
        let gate = parsed.gate.unwrap_or_else(|| cfg.gate.clone());

        let res = if parsed.metadata_only {
            client.publish_snap_metadata_only(&ws.store, &snap, &scope, &gate)
        } else {
            client.publish_snap(&ws.store, &snap, &scope, &gate)
        };

        match res {
            Ok(p) => {
                self.push_output(vec![format!("published {}", p.id)]);

                if let Err(err) = ws.store.set_last_published(&cfg, &scope, &gate, &snap_id) {
                    self.push_error(format!("record publish: {:#}", err));
                }

                self.refresh_root_view();
            }
            Err(err) => {
                self.push_error(format!("publish: {:#}", err));
            }
        }
    }
}
