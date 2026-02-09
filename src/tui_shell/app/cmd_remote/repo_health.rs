use super::*;

impl App {
    pub(in crate::tui_shell) fn cmd_create_repo(&mut self, args: &[String]) {
        if !args.is_empty() {
            self.push_error("usage: create-repo".to_string());
            return;
        }

        let client = match self.remote_client() {
            Some(c) => c,
            None => {
                // This typically means we need login first.
                self.start_login_wizard();
                return;
            }
        };

        let repo_id = client.remote().repo_id.clone();
        match client.create_repo(&repo_id) {
            Ok(_) => {
                self.push_output(vec![format!("created repo {}", repo_id)]);
                self.refresh_root_view();
            }
            Err(err) => {
                self.push_error(format!("create-repo: {:#}", err));
            }
        }
    }

    pub(in crate::tui_shell) fn cmd_ping(&mut self, _args: &[String]) {
        let Some(cfg) = self.remote_config() else {
            self.push_error("no remote configured".to_string());
            return;
        };

        let url = format!("{}/healthz", cfg.base_url.trim_end_matches('/'));
        let start = std::time::Instant::now();
        let resp = reqwest::blocking::get(&url);
        match resp {
            Ok(r) => {
                let ms = start.elapsed().as_millis();
                self.push_output(vec![format!("{} {}ms", r.status(), ms)]);
            }
            Err(err) => {
                self.push_error(format!("ping failed: {:#}", err));
            }
        }
    }
}
