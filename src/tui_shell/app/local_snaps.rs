use super::*;

impl App {
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
}
