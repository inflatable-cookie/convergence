use super::*;

impl App {
    pub(super) fn recompute_suggestions(&mut self) {
        let show = self.input.buf.trim_start().starts_with('/');
        let q = self.input.buf.trim_start_matches('/').trim().to_lowercase();
        if q.is_empty() {
            if show {
                let mut defs = self.available_command_defs();
                defs.sort_by(|a, b| a.name.cmp(b.name));
                self.suggestions = defs;
                self.suggestion_selected = 0;
            } else {
                self.suggestions.clear();
                self.suggestion_selected = 0;
            }
            return;
        }

        // Only match the first token for palette.
        let first = q.split_whitespace().next().unwrap_or("");
        if first.is_empty() {
            self.suggestions.clear();
            self.suggestion_selected = 0;
            return;
        }

        let mut defs = self.available_command_defs();
        defs.sort_by(|a, b| a.name.cmp(b.name));

        let mut scored = Vec::new();
        for d in defs {
            let mut best = score_match(first, d.name);
            for &a in d.aliases {
                best = best.max(score_match(first, a));
            }
            if best > 0 {
                scored.push((best, d));
            }
        }

        // If a command is visible in the input hints, prioritize it in suggestions.
        // This makes the "type the first letter then Enter" flow match what the UI is already nudging.
        let hint_order = self.primary_hint_commands();
        sort_scored_suggestions(&mut scored, &hint_order);
        self.suggestions = scored.into_iter().map(|(_, d)| d).collect();
        self.suggestion_selected = self.suggestion_selected.min(self.suggestions.len());
    }

    pub(super) fn apply_selected_suggestion(&mut self) {
        if self.suggestions.is_empty() {
            return;
        }
        let show = self.input.buf.trim_start().starts_with('/');
        let sel = self
            .suggestion_selected
            .min(self.suggestions.len().saturating_sub(1));
        let cmd = self.suggestions[sel].name;

        // If the user opened suggestions with `/`, keep it so the list stays visible.
        let prefix = if show { "/" } else { "" };
        let raw = self.input.buf.trim_start_matches('/');
        let trimmed = raw.trim_start();
        let mut iter = trimmed.splitn(2, char::is_whitespace);
        let first = iter.next().unwrap_or("");
        let rest = iter.next().unwrap_or("");

        if first.is_empty() {
            self.input.set(format!("{}{} ", prefix, cmd));
        } else {
            // Replace first token.
            if rest.is_empty() {
                self.input.set(format!("{}{} ", prefix, cmd));
            } else {
                self.input
                    .set(format!("{}{} {}", prefix, cmd, rest.trim_start()));
            }
        }
        self.recompute_suggestions();
    }

    pub(super) fn run_current_input(&mut self) {
        let line = self.input.buf.trim().to_string();
        if line.is_empty() {
            return;
        }

        self.input.push_history(&line);
        self.push_command(format!("{} {}", self.prompt(), line));
        self.input.clear();
        self.suggestions.clear();
        self.suggestion_selected = 0;

        let line = line.trim_start().strip_prefix('/').unwrap_or(&line).trim();
        let tokens = match tokenize(line) {
            Ok(t) => t,
            Err(err) => {
                self.push_error(format!("parse error: {}", err));
                return;
            }
        };
        if tokens.is_empty() {
            return;
        }

        let mut cmd = tokens[0].to_lowercase();
        let args = &tokens[1..];

        let mode = self.mode();
        let mut defs = self.available_command_defs();
        defs.sort_by(|a, b| a.name.cmp(b.name));

        // Resolve aliases.
        if let Some(d) = defs.iter().find(|d| d.name == cmd) {
            let _ = d;
        } else if let Some(d) = defs.iter().find(|d| d.aliases.iter().any(|&a| a == cmd)) {
            cmd = d.name.to_string();
        } else {
            // Try prefix match if unambiguous.
            let matches = defs
                .iter()
                .filter(|d| d.name.starts_with(&cmd))
                .collect::<Vec<_>>();
            if matches.len() == 1 {
                cmd = matches[0].name.to_string();
            }
        }

        if cmd == "help" {
            self.cmd_help(&defs, args);
            return;
        }

        if mode == UiMode::Root {
            self.dispatch_root(cmd.as_str(), args);
        } else {
            self.dispatch_mode(mode, cmd.as_str(), args);
        }
    }

    pub(super) fn dispatch_root(&mut self, cmd: &str, args: &[String]) {
        match self.root_ctx {
            RootContext::Local => match cmd {
                "status" => self.cmd_status(args),
                "refresh" | "r" => {
                    let _ = args;
                    self.refresh_root_view();
                    self.push_output(vec!["refreshed".to_string()]);
                }
                "init" => self.cmd_init(args),
                "snap" => self.cmd_snap(args),
                "publish" => self.cmd_publish(args),
                "sync" => self.cmd_sync(args),
                "history" => self.cmd_snaps(args),
                "show" => self.cmd_show(args),
                "restore" => self.cmd_restore(args),
                "move" => self.cmd_move(args),
                "purge" => self.cmd_gc(args),

                "clear" => {
                    self.log.clear();
                    self.last_command = None;
                    self.last_result = None;
                }
                "quit" => {
                    self.quit = true;
                }

                "bootstrap" | "remote" | "ping" | "fetch" | "lanes" | "members" | "member"
                | "lane-member" | "inbox" | "bundles" | "bundle" | "pins" | "pin" | "approve"
                | "promote" | "release" | "superpositions" | "supers" => {
                    self.push_error("remote command; press Tab to switch to remote".to_string());
                }

                _ => {
                    if !self.dispatch_global(cmd, args) {
                        self.push_error(format!("unknown command: {}", cmd));
                    }
                }
            },
            RootContext::Remote => match cmd {
                "status" => self.cmd_status(args),
                "bootstrap" => self.cmd_bootstrap(args),
                "create-repo" => self.cmd_create_repo(args),
                "gates" => self.cmd_gate_graph(args),
                "refresh" | "r" => {
                    let _ = args;
                    self.refresh_root_view();
                    self.push_output(vec!["refreshed".to_string()]);
                }
                "remote" => self.cmd_remote(args),
                "ping" => self.cmd_ping(args),
                "fetch" => self.cmd_fetch(args),
                "lanes" => self.cmd_lanes(args),
                "releases" => self.cmd_releases(args),
                "members" => self.cmd_members(args),
                "member" => self.cmd_member(args),
                "lane-member" => self.cmd_lane_member(args),
                "inbox" => self.cmd_inbox(args),
                "bundles" => self.cmd_bundles(args),
                "bundle" => self.cmd_bundle(args),
                "pins" => self.cmd_pins(args),
                "pin" => self.cmd_pin(args),
                "approve" => self.cmd_approve(args),
                "promote" => self.cmd_promote(args),
                "release" => self.cmd_release(args),
                "superpositions" => self.cmd_superpositions(args),
                "supers" => self.cmd_superpositions(args),

                "clear" => {
                    self.log.clear();
                    self.last_command = None;
                    self.last_result = None;
                }
                "quit" => {
                    self.quit = true;
                }

                "init" | "snap" | "publish" | "history" | "show" | "restore" | "move" | "mv" => {
                    self.push_error("local command; press Tab to switch to local".to_string());
                }

                _ => {
                    if !self.dispatch_global(cmd, args) {
                        self.push_error(format!("unknown command: {}", cmd));
                    }
                }
            },
        }
    }

    pub(super) fn dispatch_global(&mut self, cmd: &str, args: &[String]) -> bool {
        match cmd {
            "quit" => {
                self.quit = true;
                true
            }
            "settings" => {
                self.cmd_settings(args);
                true
            }
            "login" => {
                if self.mode() != UiMode::Root {
                    self.push_error("login is only available at root".to_string());
                } else {
                    self.cmd_login(args);
                }
                true
            }
            "logout" => {
                if self.mode() != UiMode::Root {
                    self.push_error("logout is only available at root".to_string());
                } else {
                    self.cmd_logout(args);
                }
                true
            }
            _ => false,
        }
    }

    pub(super) fn dispatch_mode(&mut self, mode: UiMode, cmd: &str, args: &[String]) {
        match mode {
            UiMode::Snaps => match cmd {
                "back" => {
                    self.pop_mode();
                    self.push_output(vec!["back".to_string()]);
                }
                "filter" => self.cmd_snaps_filter(args),
                "clear-filter" => self.cmd_snaps_clear_filter(args),
                "snap" => self.cmd_snaps_snap(args),
                "msg" => self.cmd_snaps_msg(args),
                "revert" => self.cmd_snaps_revert(args),
                "unsnap" => self.cmd_snaps_unsnap(args),
                "restore" => self.cmd_snaps_restore(args),
                _ => {
                    if !self.dispatch_global(cmd, args) {
                        self.push_error(format!(
                            "unknown command in {:?} mode: {} (try /help)",
                            mode, cmd
                        ));
                    }
                }
            },
            UiMode::Inbox => match cmd {
                "back" => {
                    self.pop_mode();
                    self.push_output(vec!["back".to_string()]);
                }
                "edit" => {
                    if !args.is_empty() {
                        self.push_error("usage: edit".to_string());
                        return;
                    }
                    self.start_browse_wizard(BrowseTarget::Inbox);
                }
                "bundle" => self.cmd_inbox_bundle_mode(args),
                "fetch" => self.cmd_inbox_fetch_mode(args),
                _ => {
                    if !self.dispatch_global(cmd, args) {
                        self.push_error(format!(
                            "unknown command in {:?} mode: {} (try /help)",
                            mode, cmd
                        ));
                    }
                }
            },
            UiMode::Bundles => match cmd {
                "back" => {
                    self.pop_mode();
                    self.push_output(vec!["back".to_string()]);
                }
                "edit" => {
                    if !args.is_empty() {
                        self.push_error("usage: edit".to_string());
                        return;
                    }
                    self.start_browse_wizard(BrowseTarget::Bundles);
                }
                "approve" => self.cmd_bundles_approve_mode(args),
                "pin" => self.cmd_bundles_pin_mode(args),
                "promote" => self.cmd_bundles_promote_mode(args),
                "release" => self.cmd_bundles_release_mode(args),
                "superpositions" | "supers" => self.cmd_bundles_superpositions_mode(args),
                _ => {
                    if !self.dispatch_global(cmd, args) {
                        self.push_error(format!(
                            "unknown command in {:?} mode: {} (try /help)",
                            mode, cmd
                        ));
                    }
                }
            },
            UiMode::Releases => match cmd {
                "back" => {
                    self.pop_mode();
                    self.push_output(vec!["back".to_string()]);
                }
                "fetch" => self.cmd_releases_fetch_mode(args),
                _ => {
                    if !self.dispatch_global(cmd, args) {
                        self.push_error(format!(
                            "unknown command in {:?} mode: {} (try /help)",
                            mode, cmd
                        ));
                    }
                }
            },
            UiMode::Lanes => match cmd {
                "back" => {
                    self.pop_mode();
                    self.push_output(vec!["back".to_string()]);
                }
                "fetch" => self.cmd_lanes_fetch_mode(args),
                _ => {
                    if !self.dispatch_global(cmd, args) {
                        self.push_error(format!(
                            "unknown command in {:?} mode: {} (try /help)",
                            mode, cmd
                        ));
                    }
                }
            },
            UiMode::Superpositions => match cmd {
                "back" => {
                    self.pop_mode();
                    self.push_output(vec!["back".to_string()]);
                }
                "pick" => self.cmd_superpositions_pick_mode(args),
                "clear" => self.cmd_superpositions_clear_mode(args),
                "next-missing" => self.cmd_superpositions_next_missing_mode(args),
                "next-invalid" => self.cmd_superpositions_next_invalid_mode(args),
                "validate" => self.cmd_superpositions_validate_mode(args),
                "apply" => self.cmd_superpositions_apply_mode(args),
                _ => {
                    if !self.dispatch_global(cmd, args) {
                        self.push_error(format!(
                            "unknown command in {:?} mode: {} (try /help)",
                            mode, cmd
                        ));
                    }
                }
            },
            UiMode::GateGraph => match cmd {
                "back" => {
                    self.pop_mode();
                    self.push_output(vec!["back".to_string()]);
                }
                "refresh" | "r" => {
                    let _ = args;
                    self.open_gate_graph_view();
                }
                "add-gate" => {
                    let _ = args;
                    self.cmd_gate_graph_add_gate();
                }
                "remove-gate" => {
                    let _ = args;
                    self.cmd_gate_graph_remove_gate();
                }
                "edit-upstream" => {
                    let _ = args;
                    self.cmd_gate_graph_edit_upstream();
                }
                "set-approvals" => {
                    let _ = args;
                    self.cmd_gate_graph_set_approvals();
                }
                "toggle-releases" => {
                    let _ = args;
                    self.cmd_gate_graph_toggle_releases();
                }
                "toggle-superpositions" => {
                    let _ = args;
                    self.cmd_gate_graph_toggle_superpositions();
                }
                "toggle-metadata-only" => {
                    let _ = args;
                    self.cmd_gate_graph_toggle_metadata_only();
                }
                _ => {
                    if !self.dispatch_global(cmd, args) {
                        self.push_error(format!(
                            "unknown command in {:?} mode: {} (try /help)",
                            mode, cmd
                        ));
                    }
                }
            },
            UiMode::Settings => match cmd {
                "back" => {
                    self.pop_mode();
                    self.push_output(vec!["back".to_string()]);
                }
                "do" => {
                    if !args.is_empty() {
                        self.push_error("usage: do".to_string());
                        return;
                    }
                    self.cmd_settings_do_mode();
                }
                _ => {
                    if !self.dispatch_global(cmd, args) {
                        self.push_error(format!(
                            "unknown command in {:?} mode: {} (try /help)",
                            mode, cmd
                        ));
                    }
                }
            },
            UiMode::Root => {
                self.dispatch_root(cmd, args);
            }
        }
    }
}
