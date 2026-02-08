use super::*;

impl App {
    pub(super) fn cmd_help(&mut self, defs: &[CommandDef], args: &[String]) {
        if args.is_empty() {
            let mut lines = Vec::new();
            lines.push("Commands:".to_string());
            let mut defs = defs.to_vec();
            defs.sort_by(|a, b| a.name.cmp(b.name));
            for d in defs {
                lines.push(format!("- {:<10} {}", d.name, d.help));
            }
            lines.push("".to_string());
            lines.push("Notes:".to_string());
            lines.push("- `Esc` goes back (or clears input).".to_string());
            lines.push("- With suggestions open: Up/Down selects; Tab accepts.".to_string());
            lines.push("- History: Ctrl+p / Ctrl+n.".to_string());
            lines.push("- At root: Tab toggles local/remote.".to_string());
            lines.push("- `/` shows available commands in this view.".to_string());
            lines.push("- Root: local shows Status; remote shows Dashboard.".to_string());
            lines.push("- Use `refresh` to recompute the current root view.".to_string());
            lines.push(
                "- `status` opens detailed status (and in local-root acts like refresh)."
                    .to_string(),
            );
            lines.push("- UI: open `settings` to adjust display + retention.".to_string());
            self.open_modal("Help", lines);
            return;
        }

        let q = args[0].to_lowercase();
        let Some(d) = defs
            .iter()
            .find(|d| d.name == q || d.aliases.iter().any(|&a| a == q))
        else {
            self.push_error(format!("unknown command: {}", q));
            return;
        };

        self.open_modal(
            "Help",
            vec![
                format!("{} - {}", d.name, d.help),
                "".to_string(),
                format!("usage: {}", d.usage),
            ],
        );
    }

    pub(in crate::tui_shell) fn remote_config(&mut self) -> Option<RemoteConfig> {
        let ws = self.require_workspace()?;
        let cfg = match ws.store.read_config() {
            Ok(c) => c,
            Err(err) => {
                self.push_error(format!("read config: {:#}", err));
                return None;
            }
        };
        cfg.remote
    }

    pub(in crate::tui_shell) fn remote_client(&mut self) -> Option<RemoteClient> {
        let ws = self.require_workspace()?;

        let cfg = match ws.store.read_config() {
            Ok(c) => c,
            Err(err) => {
                self.push_error(format!("read config: {:#}", err));
                return None;
            }
        };
        let Some(remote) = cfg.remote else {
            self.push_error("no remote configured".to_string());
            return None;
        };

        let token = match ws.store.get_remote_token(&remote) {
            Ok(Some(t)) => t,
            Ok(None) => {
                self.push_error(
                    "no remote token configured (run `login --url ... --token ... --repo ...`)"
                        .to_string(),
                );
                return None;
            }
            Err(err) => {
                self.push_error(format!("read remote token: {:#}", err));
                return None;
            }
        };

        match RemoteClient::new(remote, token) {
            Ok(c) => Some(c),
            Err(err) => {
                self.push_error(format!("init remote client: {:#}", err));
                None
            }
        }
    }

    pub(super) fn cmd_status(&mut self, _args: &[String]) {
        // Local context: status is the root view.
        if self.root_ctx == RootContext::Local && self.mode() == UiMode::Root {
            self.refresh_root_view();
            self.push_output(vec!["refreshed".to_string()]);
            return;
        }

        let Some(ws) = self.require_workspace() else {
            return;
        };

        // Keep dashboard/status view fresh before showing details.
        self.refresh_root_view();

        let ts_mode = self.ts_mode;
        let now = OffsetDateTime::now_utc();
        let rctx = RenderCtx { now, ts_mode };

        let mut lines = Vec::new();
        lines.push("Local".to_string());
        lines.push("".to_string());
        match local_status_lines(&ws, &rctx) {
            Ok(mut l) => lines.append(&mut l),
            Err(err) => lines.push(format!("status: {:#}", err)),
        }

        lines.push("".to_string());
        lines.push("Remote".to_string());
        lines.push("".to_string());
        match remote_status_lines(&ws, &rctx) {
            Ok(mut l) => lines.append(&mut l),
            Err(err) => lines.push(format!("status: {:#}", err)),
        }

        self.open_modal("Status", lines);
    }

    pub(super) fn cmd_init(&mut self, args: &[String]) {
        let mut force = false;
        for a in args {
            match a.as_str() {
                "--force" | "force" => force = true,
                _ => {
                    self.push_error("usage: init [force]".to_string());
                    return;
                }
            }
        }

        let cwd = match std::env::current_dir() {
            Ok(p) => p,
            Err(err) => {
                self.push_error(format!("get current dir: {:#}", err));
                return;
            }
        };

        match Workspace::init(&cwd, force) {
            Ok(ws) => {
                self.workspace = Some(ws);
                self.workspace_err = None;
                self.push_output(vec!["initialized .converge".to_string()]);
                self.refresh_root_view();
            }
            Err(err) => {
                self.push_error(format!("init: {:#}", err));
            }
        }
    }

    pub(super) fn cmd_snap(&mut self, args: &[String]) {
        let Some(ws) = self.require_workspace() else {
            return;
        };

        // Flagless UX: `snap [message...]`.
        if !args.is_empty() && !args[0].starts_with('-') {
            let msg = args.join(" ").trim().to_string();
            let msg = if msg.is_empty() { None } else { Some(msg) };
            match ws.create_snap(msg) {
                Ok(snap) => {
                    self.push_output(vec![format!("snap {}", snap.id)]);
                    self.refresh_root_view();
                }
                Err(err) => {
                    self.push_error(format!("snap: {:#}", err));
                }
            }
            return;
        }

        let message = if args.is_empty() {
            None
        } else if args[0] == "-m" || args[0] == "--message" {
            if args.len() < 2 {
                self.push_error("missing value for -m/--message".to_string());
                return;
            }
            Some(args[1..].join(" "))
        } else {
            self.push_error("usage: snap [message...]".to_string());
            return;
        };

        match ws.create_snap(message) {
            Ok(snap) => {
                self.push_output(vec![format!("snap {}", snap.id)]);
                self.refresh_root_view();
            }
            Err(err) => {
                self.push_error(format!("snap: {:#}", err));
            }
        }
    }

    pub(super) fn cmd_snaps_msg(&mut self, args: &[String]) {
        let Some(ws) = self.require_workspace() else {
            return;
        };
        let Some(v) = self.current_view::<SnapsView>() else {
            self.push_error("not in snaps mode".to_string());
            return;
        };

        let Some(idx) = v.selected_snap_index() else {
            self.push_error("(no snap selected)".to_string());
            return;
        };

        let snap_id = v.items[idx].id.clone();

        if args.is_empty() {
            let initial = v.items[idx].message.clone();
            self.open_snap_message_modal(snap_id, initial);
            return;
        }

        let clear = args.len() == 1 && (args[0] == "--clear" || args[0] == "clear");
        let message = if clear { None } else { Some(args.join(" ")) };

        if let Err(err) = ws.store.update_snap_message(&snap_id, message.as_deref()) {
            self.push_error(format!("set message: {:#}", err));
            return;
        }

        // Refresh the snaps view list so the selected item shows message.
        if let Some(v) = self.current_view_mut::<SnapsView>() {
            match ws.list_snaps() {
                Ok(snaps) => {
                    v.all_items = snaps.clone();
                    v.items = snaps;
                    v.head_id = ws.store.get_head().ok().flatten();
                    v.updated_at = now_ts();
                }
                Err(err) => {
                    self.push_error(format!("list snaps: {:#}", err));
                }
            }
        }
        self.refresh_root_view();
        if clear {
            self.push_output(vec![format!("cleared message for {}", snap_id)]);
        } else {
            self.push_output(vec![format!("updated message for {}", snap_id)]);
        }
    }

    pub(super) fn cmd_snaps(&mut self, args: &[String]) {
        let Some(ws) = self.require_workspace() else {
            return;
        };

        let rctx = RenderCtx {
            now: OffsetDateTime::now_utc(),
            ts_mode: self.ts_mode,
        };

        let mut limit: Option<usize> = None;

        // Flagless UX: `history [N]`.
        if args.len() == 1
            && let Ok(n) = args[0].parse::<usize>()
        {
            limit = Some(n);
        }

        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--limit" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --limit".to_string());
                        return;
                    }
                    limit = match args[i].parse::<usize>() {
                        Ok(n) => Some(n),
                        Err(_) => {
                            self.push_error("invalid --limit".to_string());
                            return;
                        }
                    };
                }
                "limit" if i + 1 < args.len() => {
                    i += 1;
                    limit = match args[i].parse::<usize>() {
                        Ok(n) => Some(n),
                        Err(_) => {
                            self.push_error("invalid limit".to_string());
                            return;
                        }
                    };
                }
                a => {
                    self.push_error(format!("unknown arg: {}", a));
                    return;
                }
            }
            i += 1;
        }

        match ws.list_snaps() {
            Ok(snaps) => {
                let items = if let Some(n) = limit {
                    snaps.into_iter().take(n).collect::<Vec<_>>()
                } else {
                    snaps
                };

                let head_id = ws.store.get_head().ok().flatten();

                let pending_changes = local_status_lines(&ws, &rctx)
                    .ok()
                    .map(|lines| extract_change_summary(lines).0)
                    .and_then(|sum| if sum.total() > 0 { Some(sum) } else { None });

                let has_header =
                    pending_changes.is_some() || (pending_changes.is_none() && head_id.is_some());
                let selected_row = if has_header && !items.is_empty() {
                    1
                } else {
                    0
                };

                self.push_view(SnapsView {
                    updated_at: now_ts(),
                    filter: None,
                    all_items: items.clone(),
                    items,
                    selected_row,
                    head_id,
                    pending_changes,
                });
                self.push_output(vec!["opened snaps".to_string()]);
            }
            Err(err) => {
                self.push_error(format!("snaps: {:#}", err));
            }
        }
    }

    pub(super) fn cmd_snaps_filter(&mut self, args: &[String]) {
        let q = args.join(" ").trim().to_string();

        let out: std::result::Result<String, String> = match self.current_view_mut::<SnapsView>() {
            Some(SnapsView {
                filter,
                all_items,
                items,
                selected_row,
                updated_at,
                pending_changes,
                head_id,
                ..
            }) => {
                if q.is_empty() {
                    let label = filter.clone().unwrap_or_else(|| "(none)".to_string());
                    Ok(format!("filter: {} ({} items)", label, items.len()))
                } else {
                    let q_lc = q.to_lowercase();
                    let mut next = Vec::new();
                    for s in all_items.iter() {
                        let mut ok = s.id.to_lowercase().contains(&q_lc)
                            || s.created_at.to_lowercase().contains(&q_lc);
                        if !ok && let Some(msg) = &s.message {
                            ok = msg.to_lowercase().contains(&q_lc);
                        }
                        if ok {
                            next.push(s.clone());
                        }
                    }

                    *filter = Some(q);
                    *items = next;
                    let has_header = pending_changes.is_some()
                        || (pending_changes.is_none() && head_id.is_some());
                    *selected_row = if has_header && !items.is_empty() {
                        1
                    } else {
                        0
                    };
                    *updated_at = now_ts();
                    Ok(format!("filtered to {} snaps", items.len()))
                }
            }
            _ => Err("not in snaps mode".to_string()),
        };

        match out {
            Ok(line) => self.push_output(vec![line]),
            Err(err) => self.push_error(err),
        }
    }

    pub(super) fn cmd_snaps_clear_filter(&mut self, args: &[String]) {
        if !args.is_empty() {
            self.push_error("usage: clear-filter".to_string());
            return;
        }

        let out: std::result::Result<String, String> = match self.current_view_mut::<SnapsView>() {
            Some(SnapsView {
                filter,
                all_items,
                items,
                selected_row,
                updated_at,
                pending_changes,
                head_id,
                ..
            }) => {
                *filter = None;
                *items = all_items.clone();
                let has_header =
                    pending_changes.is_some() || (pending_changes.is_none() && head_id.is_some());
                *selected_row = if has_header && !items.is_empty() {
                    1
                } else {
                    0
                };
                *updated_at = now_ts();
                Ok(format!("cleared filter ({} snaps)", items.len()))
            }
            _ => Err("not in snaps mode".to_string()),
        };

        match out {
            Ok(line) => self.push_output(vec![line]),
            Err(err) => self.push_error(err),
        }
    }

    pub(super) fn cmd_snaps_snap(&mut self, args: &[String]) {
        let Some(v) = self.current_view::<SnapsView>() else {
            self.push_error("not in snaps mode".to_string());
            return;
        };
        if !v.selected_is_pending() {
            self.push_error("select the pending changes row to snap".to_string());
            return;
        }
        if v.pending_changes.is_none() {
            self.push_error("(no pending changes)".to_string());
            return;
        }

        self.cmd_snap(args);

        let Some(ws) = self.require_workspace() else {
            return;
        };
        let ts_mode = self.ts_mode;
        if let Some(v) = self.current_view_mut::<SnapsView>() {
            match ws.list_snaps() {
                Ok(snaps) => {
                    v.all_items = snaps.clone();
                    v.items = snaps;
                    v.head_id = ws.store.get_head().ok().flatten();

                    let rctx = RenderCtx {
                        now: OffsetDateTime::now_utc(),
                        ts_mode,
                    };
                    v.pending_changes = local_status_lines(&ws, &rctx)
                        .ok()
                        .map(|lines| extract_change_summary(lines).0)
                        .and_then(|sum| if sum.total() > 0 { Some(sum) } else { None });

                    let has_header = v.pending_changes.is_some()
                        || (v.pending_changes.is_none() && v.head_id.is_some());
                    v.selected_row = if has_header && !v.items.is_empty() {
                        1
                    } else {
                        0
                    };
                    v.updated_at = now_ts();
                }
                Err(err) => self.push_error(format!("list snaps: {:#}", err)),
            }
        }
    }

    pub(super) fn cmd_snaps_revert(&mut self, args: &[String]) {
        let mut force = false;
        for a in args {
            if a == "--force" || a == "force" {
                force = true;
                continue;
            }
            self.push_error("usage: revert [force]".to_string());
            return;
        }

        let Some(v) = self.current_view::<SnapsView>() else {
            self.push_error("not in snaps mode".to_string());
            return;
        };
        if !v.selected_is_pending() {
            self.push_error("select the pending changes row to revert".to_string());
            return;
        }
        if v.pending_changes.is_none() {
            self.push_error("(no pending changes)".to_string());
            return;
        }

        let action = PendingAction::Mode {
            mode: UiMode::Snaps,
            cmd: "revert".to_string(),
        };
        if !force && !self.action_is_confirmed(&action) {
            self.open_confirm_modal(action);
            return;
        }

        let Some(ws) = self.require_workspace() else {
            return;
        };
        let Some(head_id) = ws.store.get_head().ok().flatten() else {
            self.push_error("no active snap (head) to revert to".to_string());
            return;
        };

        match ws.restore_snap(&head_id, true) {
            Ok(()) => {
                self.push_output(vec![format!("reverted to {}", head_id)]);

                let ts_mode = self.ts_mode;
                if let Some(v) = self.current_view_mut::<SnapsView>() {
                    v.head_id = Some(head_id.clone());

                    let rctx = RenderCtx {
                        now: OffsetDateTime::now_utc(),
                        ts_mode,
                    };
                    v.pending_changes = local_status_lines(&ws, &rctx)
                        .ok()
                        .map(|lines| extract_change_summary(lines).0)
                        .and_then(|sum| if sum.total() > 0 { Some(sum) } else { None });

                    let has_header = v.pending_changes.is_some()
                        || (v.pending_changes.is_none() && v.head_id.is_some());
                    v.selected_row = if has_header && !v.items.is_empty() {
                        1
                    } else {
                        0
                    };
                    v.updated_at = now_ts();
                }

                self.refresh_root_view();
            }
            Err(err) => self.push_error(format!("revert: {:#}", err)),
        }
    }

    pub(super) fn cmd_snaps_unsnap(&mut self, args: &[String]) {
        if !args.is_empty() {
            self.push_error("usage: unsnap".to_string());
            return;
        }

        let Some(v) = self.current_view::<SnapsView>() else {
            self.push_error("not in snaps mode".to_string());
            return;
        };
        if !v.selected_is_clean() {
            self.push_error("select the clean row to unsnap".to_string());
            return;
        }

        let action = PendingAction::Mode {
            mode: UiMode::Snaps,
            cmd: "unsnap".to_string(),
        };
        if !self.action_is_confirmed(&action) {
            self.open_confirm_modal(action);
            return;
        }

        let Some(ws) = self.require_workspace() else {
            return;
        };
        let Some(head_id) = ws.store.get_head().ok().flatten() else {
            self.push_error("no head snap to unsnap".to_string());
            return;
        };

        let snaps = match ws.list_snaps() {
            Ok(s) => s,
            Err(err) => {
                self.push_error(format!("list snaps: {:#}", err));
                return;
            }
        };
        let head_pos = snaps.iter().position(|s| s.id == head_id);
        let next_head = head_pos
            .and_then(|i| snaps.get(i + 1))
            .map(|s| s.id.clone());

        if let Err(err) = ws.store.delete_snap(&head_id) {
            self.push_error(format!("unsnap: {:#}", err));
            return;
        }
        if let Err(err) = ws.store.set_head(next_head.as_deref()) {
            self.push_error(format!("unsnap: {:#}", err));
            return;
        }

        self.push_output(vec![format!("unsnapped {}", head_id)]);

        let ts_mode = self.ts_mode;
        if let Some(v) = self.current_view_mut::<SnapsView>() {
            let items = match ws.list_snaps() {
                Ok(s) => s,
                Err(err) => {
                    self.push_error(format!("list snaps: {:#}", err));
                    return;
                }
            };

            v.all_items = items.clone();
            v.items = items;
            v.head_id = next_head.clone();

            let rctx = RenderCtx {
                now: OffsetDateTime::now_utc(),
                ts_mode,
            };
            v.pending_changes = local_status_lines(&ws, &rctx)
                .ok()
                .map(|lines| extract_change_summary(lines).0)
                .and_then(|sum| if sum.total() > 0 { Some(sum) } else { None });

            let has_header =
                v.pending_changes.is_some() || (v.pending_changes.is_none() && v.head_id.is_some());
            v.selected_row = if has_header && !v.items.is_empty() {
                1
            } else {
                0
            };
            v.updated_at = now_ts();
        }

        self.refresh_root_view();
    }

    pub(super) fn cmd_snaps_restore(&mut self, args: &[String]) {
        let Some(ws) = self.require_workspace() else {
            return;
        };

        let mut snap_id: Option<String> = None;
        let mut force = false;
        for a in args {
            if a == "--force" || a == "force" {
                force = true;
                continue;
            }
            if snap_id.is_none() {
                snap_id = Some(a.clone());
                continue;
            }
            self.push_error("usage: restore [<snap>] [force]".to_string());
            return;
        }

        if snap_id.is_none()
            && let Some(v) = self.current_view::<SnapsView>()
            && let Some(idx) = v.selected_snap_index()
        {
            snap_id = Some(v.items[idx].id.clone());
        }

        let Some(snap_id) = snap_id else {
            self.push_error("usage: restore [<snap>] [force]".to_string());
            return;
        };

        match ws.restore_snap(&snap_id, force) {
            Ok(()) => {
                self.push_output(vec![format!("restored {}", snap_id)]);

                let ts_mode = self.ts_mode;
                if let Some(v) = self.current_view_mut::<SnapsView>() {
                    v.head_id = Some(snap_id.clone());
                    v.updated_at = now_ts();

                    let rctx = RenderCtx {
                        now: OffsetDateTime::now_utc(),
                        ts_mode,
                    };
                    v.pending_changes = local_status_lines(&ws, &rctx)
                        .ok()
                        .map(|lines| extract_change_summary(lines).0)
                        .and_then(|sum| if sum.total() > 0 { Some(sum) } else { None });
                }

                self.refresh_root_view();
            }
            Err(err) => self.push_error(format!("restore: {:#}", err)),
        }
    }

    pub(super) fn cmd_show(&mut self, args: &[String]) {
        let Some(ws) = self.require_workspace() else {
            return;
        };
        if args.len() != 1 {
            self.push_error("usage: show <snap_id>".to_string());
            return;
        }
        match ws.show_snap(&args[0]) {
            Ok(s) => {
                let mut lines = Vec::new();
                lines.push(format!("id: {}", s.id));
                lines.push(format!("created_at: {}", s.created_at));
                if let Some(msg) = s.message
                    && !msg.is_empty()
                {
                    lines.push(format!("message: {}", msg));
                }
                lines.push(format!("root_manifest: {}", s.root_manifest.as_str()));
                lines.push(format!(
                    "stats: files={} dirs={} symlinks={} bytes={}",
                    s.stats.files, s.stats.dirs, s.stats.symlinks, s.stats.bytes
                ));
                self.push_output(lines);
            }
            Err(err) => {
                self.push_error(format!("show: {:#}", err));
            }
        }
    }

    pub(super) fn cmd_restore(&mut self, args: &[String]) {
        let Some(ws) = self.require_workspace() else {
            return;
        };
        if args.is_empty() {
            self.push_error("usage: restore <snap> [force]".to_string());
            return;
        }

        let mut snap_id = None;
        let mut force = false;
        for a in args {
            if a == "--force" || a == "force" {
                force = true;
                continue;
            }
            if snap_id.is_none() {
                snap_id = Some(a.clone());
                continue;
            }
            self.push_error("usage: restore <snap> [force]".to_string());
            return;
        }

        let Some(snap_id) = snap_id else {
            self.push_error("missing snap_id".to_string());
            return;
        };

        match ws.restore_snap(&snap_id, force) {
            Ok(()) => self.push_output(vec![format!("restored {}", snap_id)]),
            Err(err) => self.push_error(format!("restore: {:#}", err)),
        }
    }

    pub(super) fn cmd_move(&mut self, args: &[String]) {
        if args.is_empty() {
            self.start_move_wizard(None);
            return;
        }
        if args.len() == 1 {
            self.start_move_wizard(Some(args[0].clone()));
            return;
        }

        let Some(ws) = self.require_workspace() else {
            return;
        };
        if args.len() != 2 {
            self.push_error("usage: move [<from>] [<to>]".to_string());
            return;
        }

        let from = &args[0];
        let to = &args[1];
        match ws.move_path(std::path::Path::new(from), std::path::Path::new(to)) {
            Ok(()) => {
                self.push_output(vec![format!("moved {} -> {}", from, to)]);
                self.refresh_root_view();
            }
            Err(err) => self.push_error(format!("move: {:#}", err)),
        }
    }

    pub(super) fn cmd_gc(&mut self, args: &[String]) {
        let Some(ws) = self.require_workspace() else {
            return;
        };

        let mut dry_run = false;
        for a in args {
            match a.as_str() {
                "--dry-run" | "dry" | "dry-run" => dry_run = true,
                _ => {
                    self.push_error("usage: purge [dry]".to_string());
                    return;
                }
            }
        }

        let report = match ws.gc_local(dry_run) {
            Ok(r) => r,
            Err(err) => {
                self.push_error(format!("gc: {:#}", err));
                return;
            }
        };

        self.refresh_root_view();
        self.open_modal(
            if dry_run { "Purge (dry-run)" } else { "Purge" },
            vec![
                format!("kept_snaps: {}", report.kept_snaps),
                format!("pruned_snaps: {}", report.pruned_snaps),
                "".to_string(),
                format!("deleted_blobs: {}", report.deleted_blobs),
                format!("deleted_manifests: {}", report.deleted_manifests),
                format!("deleted_recipes: {}", report.deleted_recipes),
            ],
        );
    }
}
