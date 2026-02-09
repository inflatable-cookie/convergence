use super::*;

impl App {
    pub(in crate::tui_shell) fn cmd_bootstrap(&mut self, args: &[String]) {
        let Some(_) = self.require_workspace() else {
            return;
        };
        if !args.is_empty() {
            self.push_error("usage: bootstrap".to_string());
            return;
        }
        self.start_bootstrap_wizard();
    }

    pub(in crate::tui_shell) fn cmd_login(&mut self, args: &[String]) {
        let Some(_) = self.require_workspace() else {
            return;
        };

        if args.is_empty() {
            self.start_login_wizard();
            return;
        }

        // Flagless UX: `login <url> <token> <repo> [scope] [gate]`.
        if args.len() >= 3 && !args.iter().any(|a| a.starts_with("--")) {
            if args.len() > 5 {
                self.push_error("usage: login <url> <token> <repo> [scope] [gate]".to_string());
                return;
            }

            let base_url = args[0].clone();
            let token = args[1].clone();
            let repo_id = args[2].clone();
            let scope = args.get(3).cloned().unwrap_or_else(|| "main".to_string());
            let gate = args
                .get(4)
                .cloned()
                .unwrap_or_else(|| "dev-intake".to_string());

            self.apply_login_config(base_url, token, repo_id, scope, gate);
            return;
        }

        let mut url: Option<String> = None;
        let mut token: Option<String> = None;
        let mut repo: Option<String> = None;
        let mut scope: Option<String> = None;
        let mut gate: Option<String> = None;

        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--url" => {
                    i += 1;
                    url = args.get(i).cloned();
                }
                "--token" => {
                    i += 1;
                    token = args.get(i).cloned();
                }
                "--repo" => {
                    i += 1;
                    repo = args.get(i).cloned();
                }
                "--scope" => {
                    i += 1;
                    scope = args.get(i).cloned();
                }
                "--gate" => {
                    i += 1;
                    gate = args.get(i).cloned();
                }
                a => {
                    self.push_error(format!("unknown arg: {}", a));
                    return;
                }
            }
            if i >= args.len() {
                self.push_error("missing value for flag".to_string());
                return;
            }
            i += 1;
        }

        let (Some(base_url), Some(token), Some(repo_id)) = (url, token, repo) else {
            self.push_error("usage: login <url> <token> <repo> [scope] [gate]".to_string());
            return;
        };

        let scope = scope.unwrap_or_else(|| "main".to_string());
        let gate = gate.unwrap_or_else(|| "dev-intake".to_string());

        self.apply_login_config(base_url, token, repo_id, scope, gate);
    }

    pub(in crate::tui_shell) fn cmd_logout(&mut self, _args: &[String]) {
        let Some(ws) = self.require_workspace() else {
            return;
        };

        let cfg = match ws.store.read_config() {
            Ok(c) => c,
            Err(err) => {
                self.push_error(format!("read config: {:#}", err));
                return;
            }
        };

        let Some(remote) = cfg.remote else {
            self.push_error("no remote configured".to_string());
            return;
        };

        if let Err(err) = ws.store.clear_remote_token(&remote) {
            self.push_error(format!("clear remote token: {:#}", err));
            return;
        }

        self.push_output(vec!["logged out".to_string()]);
        self.refresh_root_view();
    }
}
