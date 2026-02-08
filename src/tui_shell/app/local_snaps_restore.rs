use super::*;

impl App {
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
