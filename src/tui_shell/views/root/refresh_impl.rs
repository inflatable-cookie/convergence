use super::*;

impl RootView {
    pub(in crate::tui_shell) fn refresh(&mut self, ws: Option<&Workspace>, ctx: &RenderCtx) {
        let prev_lines_len = self.lines.len();
        let prev_baseline = self.baseline_compact.clone();
        let prev_keys = self.change_keys.clone();

        let lines = match (self.ctx, ws) {
            (_, None) => vec!["No workspace".to_string()],
            (RootContext::Local, Some(ws)) => {
                local_status_lines(ws, ctx).unwrap_or_else(|e| vec![format!("status: {:#}", e)])
            }
            (RootContext::Remote, Some(ws)) => self.refresh_remote_lines(ws, ctx),
        };

        if self.ctx == RootContext::Local {
            refresh_local_state(self, lines, prev_lines_len, prev_baseline, prev_keys);
        } else {
            clear_local_tracking_for_remote(self, lines);
        }
        self.updated_at = now_ts();
    }

    pub(in crate::tui_shell) fn remote_repo_missing(&self) -> bool {
        self.ctx == RootContext::Remote
            && self
                .lines
                .iter()
                .any(|l| l.to_lowercase().contains("remote repo not found"))
    }

    fn refresh_remote_lines(&mut self, ws: &Workspace, ctx: &RenderCtx) -> Vec<String> {
        if let Some(lines) = self.remote_auth_block_lines.clone() {
            return lines;
        }
        match dashboard_data(ws, ctx) {
            Ok(dashboard) => {
                self.remote_dashboard = Some(dashboard);
                self.remote_err = None;
                Vec::new()
            }
            Err(err) => {
                self.remote_dashboard = None;
                let s = sanitize_dashboard_err(&format!("{:#}", err));
                self.remote_err = Some(s.clone());
                vec![s]
            }
        }
    }
}

fn sanitize_dashboard_err(msg: &str) -> String {
    const REPO_NOT_FOUND_SUFFIX: &str =
        " (create it with `converge remote create-repo` or POST /repos)";

    let msg = msg.trim();
    let msg = msg.strip_suffix(REPO_NOT_FOUND_SUFFIX).unwrap_or(msg);
    format!("dashboard: {}", msg)
}
