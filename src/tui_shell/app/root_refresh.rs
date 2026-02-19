use super::*;

impl App {
    fn remote_auth_onboarding_lines(note: &str) -> Vec<String> {
        match note {
            "auth: login" | "auth: unauthorized" => vec![
                "start here:".to_string(),
                "1) login --url <url> --token <token> --repo <id> [--scope <id>] [--gate <id>"
                    .to_string(),
                "2) refresh".to_string(),
                "3) inbox".to_string(),
            ],
            "auth: server unreachable" => vec![
                "start here:".to_string(),
                "1) ping".to_string(),
                "2) if unreachable, verify server URL/network".to_string(),
                "3) login --url <url> --token <token> --repo <id>".to_string(),
            ],
            "auth: server error" => vec![
                "start here:".to_string(),
                "1) ping".to_string(),
                "2) refresh".to_string(),
                "3) retry login after server health recovers".to_string(),
            ],
            _ => vec![
                "start here:".to_string(),
                "1) ping".to_string(),
                "2) login --url <url> --token <token> --repo <id>".to_string(),
                "3) inbox".to_string(),
            ],
        }
    }

    pub(in crate::tui_shell) fn refresh_root_view(&mut self) {
        let ws = self.workspace.clone();
        let ctx = self.root_ctx;
        let ts_mode = self.ts_mode;
        let now = OffsetDateTime::now_utc();
        let rctx = RenderCtx { now, ts_mode };

        let remote_cfg = ws
            .as_ref()
            .and_then(|w| w.store.read_config().ok())
            .and_then(|c| c.remote);

        self.remote_configured = remote_cfg.is_some();

        if let Some(ws) = ws.as_ref() {
            self.refresh_remote_identity(ws, now);
        } else {
            self.remote_identity = None;
            self.remote_identity_note = None;
            self.remote_identity_last_fetch = None;
        }

        // If we don't currently have a valid identity, avoid rendering an error-only dashboard.
        // Instead show a stable "auth required" panel with guidance.
        let remote_auth_block_lines = if self.remote_identity.is_none() {
            if let (Some(ws), Some(remote), Some(note)) = (
                ws.as_ref(),
                remote_cfg.as_ref(),
                self.remote_identity_note.as_deref(),
            ) {
                let token_present = ws.store.get_remote_token(remote).ok().flatten().is_some();

                let mut lines = Vec::new();
                lines.push("Remote".to_string());
                lines.push("".to_string());
                lines.push(format!("remote: {}", remote.base_url));
                lines.push(format!("repo: {}", remote.repo_id));
                lines.push(format!("scope: {}", remote.scope));
                lines.push(format!("gate: {}", remote.gate));
                lines.push(format!(
                    "token: {}",
                    if token_present {
                        "(configured)"
                    } else {
                        "(missing)"
                    }
                ));
                lines.push(note.to_string());
                lines.push("".to_string());
                lines.extend(Self::remote_auth_onboarding_lines(note));
                Some(lines)
            } else {
                None
            }
        } else {
            None
        };

        self.lane_last_synced = ws
            .as_ref()
            .and_then(|w| w.store.read_state().ok())
            .map(|st| {
                st.lane_sync
                    .into_iter()
                    .map(|(k, v)| (k, v.snap_id))
                    .collect()
            })
            .unwrap_or_default();

        self.latest_snap_id = ws
            .as_ref()
            .and_then(|w| w.list_snaps().ok())
            .and_then(|snaps| snaps.first().map(|s| s.id.clone()));

        self.last_published_snap_id = ws.as_ref().zip(remote_cfg.as_ref()).and_then(|(w, r)| {
            w.store
                .get_last_published(r, &r.scope, &r.gate)
                .ok()
                .flatten()
        });

        if let Some(v) = self.current_view_mut::<RootView>() {
            v.ctx = ctx;
            v.remote_auth_block_lines = remote_auth_block_lines;
            v.refresh(ws.as_ref(), &rctx);
        }
    }
}
