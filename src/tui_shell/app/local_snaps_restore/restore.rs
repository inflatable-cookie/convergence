use super::*;

impl App {
    pub(in crate::tui_shell::app) fn cmd_snaps_restore(&mut self, args: &[String]) {
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
