use super::*;

impl crate::tui_shell::App {
    pub(in crate::tui_shell) fn continue_fetch_wizard(
        &mut self,
        action: TextInputAction,
        value: String,
    ) {
        if self.fetch_wizard.is_none() {
            self.push_error("fetch wizard not active".to_string());
            return;
        }

        match action {
            TextInputAction::FetchKind => {
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
                        TextInputAction::FetchKind,
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
                    TextInputAction::FetchId,
                    initial,
                    lines,
                );
            }

            TextInputAction::FetchId => {
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
                        TextInputAction::FetchId,
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
                            TextInputAction::FetchUser,
                            None,
                            vec!["Optional: filter by user handle".to_string()],
                        );
                    }
                    FetchKind::Bundle | FetchKind::Release => {
                        self.open_text_input_modal(
                            "Fetch",
                            "options> ",
                            TextInputAction::FetchOptions,
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

            TextInputAction::FetchUser => {
                let v = value.trim().to_string();
                if let Some(w) = self.fetch_wizard.as_mut() {
                    w.user = if v.is_empty() { None } else { Some(v) };
                }
                self.finish_fetch_wizard();
            }

            TextInputAction::FetchOptions => {
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
}
