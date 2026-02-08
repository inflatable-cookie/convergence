use super::types::FetchKind;

impl super::super::App {
    pub(in crate::tui_shell) fn start_fetch_wizard(&mut self) {
        let Some(_) = self.require_workspace() else {
            return;
        };

        if self.remote_client().is_none() {
            // If fetch can't run, it's almost always because we need login.
            self.start_login_wizard();
            return;
        }

        self.fetch_wizard = Some(super::types::FetchWizard {
            kind: None,
            id: None,
            user: None,
            options: None,
        });

        self.open_text_input_modal(
            "Fetch",
            "what> ",
            super::super::TextInputAction::FetchKind,
            Some("snap".to_string()),
            vec!["What to fetch? snap | bundle | release | lane".to_string()],
        );
    }

    pub(in crate::tui_shell) fn continue_fetch_wizard(
        &mut self,
        action: super::super::TextInputAction,
        value: String,
    ) {
        if self.fetch_wizard.is_none() {
            self.push_error("fetch wizard not active".to_string());
            return;
        }

        match action {
            super::super::TextInputAction::FetchKind => {
                let v = value.trim().to_lowercase();
                let v = if v.is_empty() { "snap".to_string() } else { v };
                let kind = match v.as_str() {
                    "snap" | "snaps" => Some(FetchKind::Snap),
                    "bundle" | "bundles" => Some(FetchKind::Bundle),
                    "release" | "releases" => Some(FetchKind::Release),
                    "lane" | "lanes" => Some(FetchKind::Lane),
                    _ => None,
                };

                let Some(kind) = kind else {
                    self.open_text_input_modal(
                        "Fetch",
                        "what> ",
                        super::super::TextInputAction::FetchKind,
                        Some("snap".to_string()),
                        vec![
                            "error: choose snap | bundle | release | lane".to_string(),
                            "".to_string(),
                            "What to fetch?".to_string(),
                        ],
                    );
                    return;
                };

                if let Some(w) = self.fetch_wizard.as_mut() {
                    w.kind = Some(kind);
                }

                let (prompt, initial, lines) = match kind {
                    FetchKind::Snap => (
                        "snap id (blank=all)> ",
                        None,
                        vec!["Optional: leave blank to fetch all publications.".to_string()],
                    ),
                    FetchKind::Bundle => ("bundle id> ", None, vec!["Paste bundle id".to_string()]),
                    FetchKind::Release => (
                        "channel> ",
                        None,
                        vec!["Release channel name (example: main)".to_string()],
                    ),
                    FetchKind::Lane => (
                        "lane id> ",
                        Some("default".to_string()),
                        vec!["Lane id (example: default)".to_string()],
                    ),
                };

                self.open_text_input_modal(
                    "Fetch",
                    prompt,
                    super::super::TextInputAction::FetchId,
                    initial,
                    lines,
                );
            }

            super::super::TextInputAction::FetchId => {
                let kind = self.fetch_wizard.as_ref().and_then(|w| w.kind);
                let Some(kind) = kind else {
                    self.start_fetch_wizard();
                    return;
                };

                let id = value.trim().to_string();
                if id.is_empty() && kind != FetchKind::Snap {
                    let prompt = match kind {
                        FetchKind::Bundle => "bundle id> ",
                        FetchKind::Release => "channel> ",
                        FetchKind::Lane => "lane id> ",
                        FetchKind::Snap => "snap id (blank=all)> ",
                    };
                    self.open_text_input_modal(
                        "Fetch",
                        prompt,
                        super::super::TextInputAction::FetchId,
                        None,
                        vec!["error: value required".to_string()],
                    );
                    return;
                }

                if let Some(w) = self.fetch_wizard.as_mut() {
                    w.id = if id.is_empty() { None } else { Some(id) };
                }

                match kind {
                    FetchKind::Lane => {
                        self.open_text_input_modal(
                            "Fetch",
                            "user (blank=all)> ",
                            super::super::TextInputAction::FetchUser,
                            None,
                            vec!["Optional: filter by user handle".to_string()],
                        );
                    }
                    FetchKind::Bundle | FetchKind::Release => {
                        self.open_text_input_modal(
                            "Fetch",
                            "options> ",
                            super::super::TextInputAction::FetchOptions,
                            None,
                            vec![
                                "Optional:".to_string(),
                                "- empty: fetch only".to_string(),
                                "- restore: also materialize into a directory".to_string(),
                                "- into <dir>: choose directory (implies restore)".to_string(),
                                "- force: overwrite files when restoring".to_string(),
                            ],
                        );
                    }
                    FetchKind::Snap => {
                        self.finish_fetch_wizard();
                    }
                }
            }

            super::super::TextInputAction::FetchUser => {
                let v = value.trim().to_string();
                if let Some(w) = self.fetch_wizard.as_mut() {
                    w.user = if v.is_empty() { None } else { Some(v) };
                }
                self.finish_fetch_wizard();
            }

            super::super::TextInputAction::FetchOptions => {
                if let Some(w) = self.fetch_wizard.as_mut() {
                    let v = value.trim().to_string();
                    w.options = if v.is_empty() { None } else { Some(v) };
                }
                self.finish_fetch_wizard();
            }

            _ => {
                self.push_error("unexpected fetch wizard input".to_string());
            }
        }
    }

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
