use super::*;

impl App {
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
}
