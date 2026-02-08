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
}
