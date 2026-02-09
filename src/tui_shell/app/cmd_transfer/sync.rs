use super::sync_args::parse_sync_args;
use super::*;

impl App {
    pub(in crate::tui_shell) fn cmd_sync(&mut self, args: &[String]) {
        if args.len() == 1 && matches!(args[0].as_str(), "edit" | "prompt" | "custom") {
            self.start_sync_wizard(true);
            return;
        }

        if args.is_empty() {
            self.start_sync_wizard(false);
            return;
        }

        self.cmd_sync_impl(args);
    }

    pub(in crate::tui_shell) fn cmd_sync_impl(&mut self, args: &[String]) {
        let Some(ws) = self.require_workspace() else {
            return;
        };
        let Some(cfg) = self.remote_config() else {
            self.start_login_wizard();
            return;
        };

        let parsed = match parse_sync_args(args) {
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
                        self.push_error("no snaps to sync".to_string());
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

        match client.sync_snap(&ws.store, &snap, &parsed.lane, parsed.client_id) {
            Ok(head) => {
                if let Err(err) = ws
                    .store
                    .set_lane_sync(&parsed.lane, &snap.id, &head.updated_at)
                {
                    self.push_error(format!("record lane sync: {:#}", err));
                }
                let short = head.snap_id.chars().take(8).collect::<String>();
                self.push_output(vec![format!("synced {} to lane {}", short, parsed.lane)]);
                self.refresh_root_view();
            }
            Err(err) => {
                self.push_error(format!("sync: {:#}", err));
            }
        }
    }
}
