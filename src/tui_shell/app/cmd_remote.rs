use super::*;

impl App {
    pub(super) fn cmd_remote(&mut self, args: &[String]) {
        let sub = args.first().map(|s| s.as_str()).unwrap_or("show");
        match sub {
            "show" => {
                let Some(cfg) = self.remote_config() else {
                    self.push_error("no remote configured".to_string());
                    return;
                };
                self.push_output(vec![
                    format!("url: {}", cfg.base_url),
                    format!("repo: {}", cfg.repo_id),
                    format!("scope: {}", cfg.scope),
                    format!("gate: {}", cfg.gate),
                ]);
            }
            "ping" => {
                self.cmd_ping(&[]);
            }
            "set" => {
                self.cmd_remote_set(&args[1..]);
            }
            "unset" => {
                self.cmd_remote_unset(&args[1..]);
            }
            _ => {
                self.push_error("usage: remote show|ping|set|unset".to_string());
            }
        }
    }

    pub(super) fn cmd_remote_set(&mut self, args: &[String]) {
        let Some(ws) = self.require_workspace() else {
            return;
        };

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

        let (Some(base_url), Some(token), Some(repo_id), Some(scope), Some(gate)) =
            (url, token, repo, scope, gate)
        else {
            self.push_error(
                "usage: remote set --url <url> --token <token> --repo <id> --scope <id> --gate <id> (tip: use `login ...`)"
                    .to_string(),
            );
            return;
        };

        let mut cfg = match ws.store.read_config() {
            Ok(c) => c,
            Err(err) => {
                self.push_error(format!("read config: {:#}", err));
                return;
            }
        };

        cfg.remote = Some(RemoteConfig {
            base_url,
            token: None,
            repo_id,
            scope,
            gate,
        });

        let remote = cfg.remote.clone().expect("remote config just set above");
        if let Err(err) = ws.store.set_remote_token(&remote, &token) {
            self.push_error(format!("store remote token: {:#}", err));
            return;
        }

        if let Err(err) = ws.store.write_config(&cfg) {
            self.push_error(format!("write config: {:#}", err));
            return;
        }

        self.push_output(vec!["remote configured".to_string()]);
        self.refresh_root_view();
    }

    pub(super) fn cmd_remote_unset(&mut self, args: &[String]) {
        let _ = args;
        let Some(ws) = self.require_workspace() else {
            return;
        };

        let mut cfg = match ws.store.read_config() {
            Ok(c) => c,
            Err(err) => {
                self.push_error(format!("read config: {:#}", err));
                return;
            }
        };

        if let Some(remote) = cfg.remote.take()
            && let Err(err) = ws.store.clear_remote_token(&remote)
        {
            self.push_error(format!("clear remote token: {:#}", err));
            return;
        }

        cfg.remote = None;
        if let Err(err) = ws.store.write_config(&cfg) {
            self.push_error(format!("write config: {:#}", err));
            return;
        }
        self.push_output(vec!["remote unset".to_string()]);
        self.refresh_root_view();
    }

    pub(super) fn cmd_create_repo(&mut self, args: &[String]) {
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

    pub(super) fn cmd_bootstrap(&mut self, args: &[String]) {
        let Some(_) = self.require_workspace() else {
            return;
        };
        if !args.is_empty() {
            self.push_error("usage: bootstrap".to_string());
            return;
        }
        self.start_bootstrap_wizard();
    }

    pub(super) fn cmd_login(&mut self, args: &[String]) {
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

    pub(super) fn cmd_logout(&mut self, _args: &[String]) {
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

    pub(super) fn cmd_ping(&mut self, _args: &[String]) {
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
