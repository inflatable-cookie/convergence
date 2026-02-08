use time::OffsetDateTime;

use crate::remote::RemoteClient;

use super::parse_utils::server_label;
use super::*;

impl App {
    pub(super) fn switch_to_local_root(&mut self) {
        self.root_ctx = RootContext::Local;
        self.frames = vec![ViewFrame {
            view: Box::new(RootView::new(RootContext::Local)),
        }];
        self.refresh_root_view();
    }

    pub(super) fn switch_to_remote_root(&mut self) {
        self.root_ctx = RootContext::Remote;
        self.frames = vec![ViewFrame {
            view: Box::new(RootView::new(RootContext::Remote)),
        }];
        self.refresh_root_view();
    }

    pub(super) fn remote_repo_missing(&self) -> bool {
        if self.mode() != UiMode::Root || self.root_ctx != RootContext::Remote {
            return false;
        }
        self.current_view::<RootView>()
            .is_some_and(|v| v.remote_repo_missing())
    }

    pub(super) fn refresh_remote_identity(&mut self, ws: &Workspace, now: OffsetDateTime) {
        // Avoid spamming whoami calls.
        if let Some(last) = self.remote_identity_last_fetch
            && now - last < time::Duration::seconds(10)
        {
            return;
        }

        let cfg = match ws.store.read_config() {
            Ok(c) => c,
            Err(err) => {
                self.remote_identity = None;
                self.remote_identity_note = Some(format!("auth: {}", err));
                self.remote_identity_last_fetch = Some(now);
                return;
            }
        };

        let Some(remote) = cfg.remote else {
            self.remote_identity = None;
            self.remote_identity_note = None;
            self.remote_identity_last_fetch = None;
            return;
        };

        let token = match ws.store.get_remote_token(&remote) {
            Ok(Some(t)) => t,
            Ok(None) => {
                self.remote_identity = None;
                self.remote_identity_note = Some("auth: login".to_string());
                self.remote_identity_last_fetch = Some(now);
                return;
            }
            Err(err) => {
                self.remote_identity = None;
                self.remote_identity_note = Some(format!("auth: {}", err));
                self.remote_identity_last_fetch = Some(now);
                return;
            }
        };

        let client = match RemoteClient::new(remote.clone(), token) {
            Ok(c) => c,
            Err(err) => {
                self.remote_identity = None;
                self.remote_identity_note = Some(format!("auth: {}", err));
                self.remote_identity_last_fetch = Some(now);
                return;
            }
        };

        match client.whoami() {
            Ok(w) => {
                self.remote_identity =
                    Some(format!("{}@{}", w.user, server_label(&remote.base_url)));
                self.remote_identity_note = None;
            }
            Err(err) => {
                let s = err.to_string();
                if s.contains("unauthorized") {
                    self.remote_identity_note = Some("auth: unauthorized".to_string());
                } else {
                    self.remote_identity_note = Some("auth: error".to_string());
                }
                self.remote_identity = None;
            }
        }

        self.remote_identity_last_fetch = Some(now);
    }
}
