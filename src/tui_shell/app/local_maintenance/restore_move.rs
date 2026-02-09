use super::*;

impl App {
    pub(in crate::tui_shell::app) fn cmd_restore(&mut self, args: &[String]) {
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

    pub(in crate::tui_shell::app) fn cmd_move(&mut self, args: &[String]) {
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
}
