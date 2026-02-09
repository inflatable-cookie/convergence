use super::*;

impl crate::tui_shell::App {
    pub(in crate::tui_shell) fn finish_fetch_wizard(&mut self) {
        let Some(w) = self.fetch_wizard.clone() else {
            self.push_error("fetch wizard not active".to_string());
            return;
        };
        self.fetch_wizard = None;

        let Some(kind) = w.kind else {
            self.push_error("fetch: missing kind".to_string());
            return;
        };

        let mut argv: Vec<String> = Vec::new();
        match kind {
            FetchKind::Snap => {
                if let Some(id) = w.id {
                    argv.extend(["--snap-id".to_string(), id]);
                }
            }
            FetchKind::Bundle => {
                let Some(id) = w.id else {
                    self.push_error("fetch: missing bundle id".to_string());
                    return;
                };
                argv.extend(["--bundle-id".to_string(), id]);
            }
            FetchKind::Release => {
                let Some(id) = w.id else {
                    self.push_error("fetch: missing channel".to_string());
                    return;
                };
                argv.extend(["--release".to_string(), id]);
            }
            FetchKind::Lane => {
                let Some(id) = w.id else {
                    self.push_error("fetch: missing lane".to_string());
                    return;
                };
                argv.extend(["--lane".to_string(), id]);
                if let Some(u) = w.user {
                    argv.extend(["--user".to_string(), u]);
                }
            }
        }

        if matches!(kind, FetchKind::Bundle | FetchKind::Release) {
            let mut restore = false;
            let mut into: Option<String> = None;
            let mut force = false;

            if let Some(s) = w.options {
                let parts = s.split_whitespace().collect::<Vec<_>>();
                let mut i = 0;
                while i < parts.len() {
                    match parts[i].to_lowercase().as_str() {
                        "restore" => restore = true,
                        "force" => force = true,
                        "into" => {
                            i += 1;
                            if i < parts.len() {
                                into = Some(parts[i].to_string());
                            }
                        }
                        _ => {}
                    }
                    i += 1;
                }

                if into.is_some() || force {
                    restore = true;
                }
            }

            if restore {
                argv.push("--restore".to_string());
            }
            if let Some(p) = into {
                argv.extend(["--into".to_string(), p]);
            }
            if force {
                argv.push("--force".to_string());
            }
        }

        self.cmd_fetch_impl(&argv);
    }
}
