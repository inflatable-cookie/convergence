use super::*;

impl App {
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
