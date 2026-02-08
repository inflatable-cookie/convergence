use super::*;

impl App {
    pub(super) fn cmd_publish(&mut self, args: &[String]) {
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
            // Treat as a guided "fix it" path.
            self.start_login_wizard();
            return;
        };

        let mut snap_id: Option<String> = None;
        let mut scope: Option<String> = None;
        let mut gate: Option<String> = None;
        let mut metadata_only = false;

        // Flagless UX:
        // - `publish` (defaults to latest snap + configured scope/gate)
        // - `publish <snap> [scope] [gate]`
        // - `publish [snap <id>] [scope <id>] [gate <id>] [meta]`
        if !args.iter().any(|a| a.starts_with("--")) {
            let mut i = 0;
            while i < args.len() {
                match args[i].as_str() {
                    "snap" => {
                        i += 1;
                        let Some(v) = args.get(i) else {
                            self.push_error(
                                "usage: publish [snap <id>] [scope <id>] [gate <id>] [meta]"
                                    .to_string(),
                            );
                            return;
                        };
                        snap_id = Some(v.clone());
                    }
                    "scope" => {
                        i += 1;
                        let Some(v) = args.get(i) else {
                            self.push_error(
                                "usage: publish [snap <id>] [scope <id>] [gate <id>] [meta]"
                                    .to_string(),
                            );
                            return;
                        };
                        scope = Some(v.clone());
                    }
                    "gate" => {
                        i += 1;
                        let Some(v) = args.get(i) else {
                            self.push_error(
                                "usage: publish [snap <id>] [scope <id>] [gate <id>] [meta]"
                                    .to_string(),
                            );
                            return;
                        };
                        gate = Some(v.clone());
                    }
                    "meta" | "metadata" | "metadata-only" => {
                        metadata_only = true;
                    }
                    a => {
                        if snap_id.is_none() {
                            snap_id = Some(a.to_string());
                        } else if scope.is_none() {
                            scope = Some(a.to_string());
                        } else if gate.is_none() {
                            gate = Some(a.to_string());
                        } else {
                            self.push_error(
                                "usage: publish [snap <id>] [scope <id>] [gate <id>] [meta]"
                                    .to_string(),
                            );
                            return;
                        }
                    }
                }
                i += 1;
            }
        } else {
            let mut i = 0;
            while i < args.len() {
                match args[i].as_str() {
                    "--snap-id" => {
                        i += 1;
                        if i >= args.len() {
                            self.push_error("missing value for --snap-id".to_string());
                            return;
                        }
                        snap_id = Some(args[i].clone());
                    }
                    "--scope" => {
                        i += 1;
                        if i >= args.len() {
                            self.push_error("missing value for --scope".to_string());
                            return;
                        }
                        scope = Some(args[i].clone());
                    }
                    "--gate" => {
                        i += 1;
                        if i >= args.len() {
                            self.push_error("missing value for --gate".to_string());
                            return;
                        }
                        gate = Some(args[i].clone());
                    }
                    "--metadata-only" => {
                        metadata_only = true;
                    }
                    a => {
                        self.push_error(format!("unknown arg: {}", a));
                        return;
                    }
                }
                i += 1;
            }
        }

        let snap_id = match snap_id {
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

        let scope = scope.unwrap_or_else(|| cfg.scope.clone());
        let gate = gate.unwrap_or_else(|| cfg.gate.clone());

        let res = if metadata_only {
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

    pub(super) fn cmd_lanes_fetch_mode(&mut self, args: &[String]) {
        if !args.is_empty() {
            self.push_error("usage: fetch".to_string());
            return;
        }

        let Some(v) = self.current_view::<LanesView>() else {
            self.push_error("not in lanes mode".to_string());
            return;
        };
        if v.items.is_empty() {
            self.push_error("(no selection)".to_string());
            return;
        }
        let idx = v.selected.min(v.items.len().saturating_sub(1));
        let it = &v.items[idx];
        let Some(_h) = &it.head else {
            self.push_error("selected member has no head".to_string());
            return;
        };

        self.cmd_fetch(&[
            "--lane".to_string(),
            it.lane_id.clone(),
            "--user".to_string(),
            it.user.clone(),
        ]);
    }

    pub(super) fn cmd_releases_fetch_mode(&mut self, args: &[String]) {
        let Some(v) = self.current_view::<ReleasesView>() else {
            self.push_error("not in releases mode".to_string());
            return;
        };
        if v.items.is_empty() {
            self.push_error("(no selection)".to_string());
            return;
        }
        let idx = v.selected.min(v.items.len().saturating_sub(1));
        let channel = v.items[idx].channel.clone();

        let mut argv = vec!["--release".to_string(), channel];
        argv.extend(args.iter().cloned());
        self.cmd_fetch(&argv);
    }

    pub(super) fn cmd_sync(&mut self, args: &[String]) {
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

        let mut snap_id: Option<String> = None;
        let mut lane: String = "default".to_string();
        let mut client_id: Option<String> = None;

        // Flagless UX:
        // - `sync` (defaults to latest snap + lane=default)
        // - `sync <snap> [lane] [client]`
        // - `sync [snap <id>] [lane <id>] [client <id>]`
        if !args.iter().any(|a| a.starts_with("--")) {
            let mut i = 0;
            while i < args.len() {
                match args[i].as_str() {
                    "snap" => {
                        i += 1;
                        let Some(v) = args.get(i) else {
                            self.push_error(
                                "usage: sync [snap <id>] [lane <id>] [client <id>]".to_string(),
                            );
                            return;
                        };
                        snap_id = Some(v.clone());
                    }
                    "lane" => {
                        i += 1;
                        let Some(v) = args.get(i) else {
                            self.push_error(
                                "usage: sync [snap <id>] [lane <id>] [client <id>]".to_string(),
                            );
                            return;
                        };
                        lane = v.clone();
                    }
                    "client" | "client-id" => {
                        i += 1;
                        let Some(v) = args.get(i) else {
                            self.push_error(
                                "usage: sync [snap <id>] [lane <id>] [client <id>]".to_string(),
                            );
                            return;
                        };
                        client_id = Some(v.clone());
                    }
                    a => {
                        if snap_id.is_none() {
                            snap_id = Some(a.to_string());
                        } else if lane == "default" {
                            lane = a.to_string();
                        } else if client_id.is_none() {
                            client_id = Some(a.to_string());
                        } else {
                            self.push_error(
                                "usage: sync [snap <id>] [lane <id>] [client <id>]".to_string(),
                            );
                            return;
                        }
                    }
                }
                i += 1;
            }
        } else {
            let mut i = 0;
            while i < args.len() {
                match args[i].as_str() {
                    "--snap-id" => {
                        i += 1;
                        if i >= args.len() {
                            self.push_error("missing value for --snap-id".to_string());
                            return;
                        }
                        snap_id = Some(args[i].clone());
                    }
                    "--lane" => {
                        i += 1;
                        if i >= args.len() {
                            self.push_error("missing value for --lane".to_string());
                            return;
                        }
                        lane = args[i].clone();
                    }
                    "--client-id" => {
                        i += 1;
                        if i >= args.len() {
                            self.push_error("missing value for --client-id".to_string());
                            return;
                        }
                        client_id = Some(args[i].clone());
                    }
                    a => {
                        self.push_error(format!("unknown arg: {}", a));
                        return;
                    }
                }
                i += 1;
            }
        }

        let snap_id = match snap_id {
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

        match client.sync_snap(&ws.store, &snap, &lane, client_id) {
            Ok(head) => {
                if let Err(err) = ws.store.set_lane_sync(&lane, &snap.id, &head.updated_at) {
                    self.push_error(format!("record lane sync: {:#}", err));
                }
                let short = head.snap_id.chars().take(8).collect::<String>();
                self.push_output(vec![format!("synced {} to lane {}", short, lane)]);
                self.refresh_root_view();
            }
            Err(err) => {
                self.push_error(format!("sync: {:#}", err));
            }
        }
    }
}
