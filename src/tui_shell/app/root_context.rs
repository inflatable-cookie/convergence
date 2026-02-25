use time::OffsetDateTime;

use crate::remote::RemoteClient;

use super::parse_utils::server_label;
use super::*;

impl App {
    fn classify_remote_auth_error(err: &str) -> &'static str {
        let lower = err.to_lowercase();
        if lower.contains("unauthorized")
            || lower.contains("401")
            || lower.contains("forbidden")
            || lower.contains("403")
        {
            return "auth: unauthorized";
        }
        if lower.contains("connection refused")
            || lower.contains("connection reset")
            || lower.contains("dns")
            || lower.contains("timed out")
            || lower.contains("timeout")
            || lower.contains("unreachable")
            || lower.contains("refused")
        {
            return "auth: server unreachable";
        }
        if lower.contains("500")
            || lower.contains("502")
            || lower.contains("503")
            || lower.contains("504")
        {
            return "auth: server error";
        }
        "auth: error"
    }

    pub(super) fn switch_to_local_root(&mut self) {
        let from = self.root_ctx.label().to_string();
        self.root_ctx = RootContext::Local;
        self.frames = vec![ViewFrame {
            view: Box::new(RootView::new(RootContext::Local)),
        }];
        self.refresh_root_view();
        self.trace_state_change("root_context", &from, self.root_ctx.label());
    }

    pub(super) fn switch_to_remote_root(&mut self) {
        let from = self.root_ctx.label().to_string();
        self.root_ctx = RootContext::Remote;
        self.frames = vec![ViewFrame {
            view: Box::new(RootView::new(RootContext::Remote)),
        }];
        self.refresh_root_view();
        self.trace_state_change("root_context", &from, self.root_ctx.label());
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
                let note = Self::classify_remote_auth_error(&err.to_string());
                self.remote_identity_note = Some(note.to_string());
                self.remote_identity = None;
            }
        }

        self.remote_identity_last_fetch = Some(now);
    }
}
