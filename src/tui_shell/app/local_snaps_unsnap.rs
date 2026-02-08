use super::*;

impl App {
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
}
