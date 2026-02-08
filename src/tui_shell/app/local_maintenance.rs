use super::*;

impl App {
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
