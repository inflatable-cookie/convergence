use std::path::Path;

use super::move_glob::glob_search;

impl super::super::App {
    pub(super) fn move_wizard_from(&mut self, value: String) {
        let Some(ws) = self.require_workspace() else {
            return;
        };
        let Some(w) = self.move_wizard.as_mut() else {
            self.push_error("move wizard not active".to_string());
            return;
        };

        let raw = value.trim().to_string();
        if raw.is_empty() {
            self.push_error("missing from glob".to_string());
            return;
        }

        if !w.candidates.is_empty()
            && raw.chars().all(|c| c.is_ascii_digit())
            && let Ok(n) = raw.parse::<usize>()
            && n >= 1
            && n <= w.candidates.len()
        {
            let from = w.candidates[n - 1].clone();
            w.candidates.clear();
            w.query = Some(raw);
            w.from = Some(from.clone());
            self.open_text_input_modal(
                "Move",
                "to> ",
                super::super::TextInputAction::MoveTo,
                Some(from.clone()),
                vec![
                    format!("from: {}", from),
                    "Edit the destination path (relative to workspace root).".to_string(),
                ],
            );
            return;
        }

        w.query = Some(raw.clone());
        let matches = match glob_search(&ws.root, &raw) {
            Ok(m) => m,
            Err(err) => {
                w.candidates.clear();
                self.open_text_input_modal(
                    "Move",
                    "from (glob)> ",
                    super::super::TextInputAction::MoveFrom,
                    Some(raw),
                    vec![
                        format!("error: {:#}", err),
                        "".to_string(),
                        "Enter a glob to find the source path.".to_string(),
                    ],
                );
                return;
            }
        };

        if matches.is_empty() {
            w.candidates.clear();
            self.open_text_input_modal(
                "Move",
                "from (glob)> ",
                super::super::TextInputAction::MoveFrom,
                Some(raw),
                vec![
                    "error: no matches".to_string(),
                    "".to_string(),
                    "Try a more specific glob.".to_string(),
                ],
            );
            return;
        }

        if matches.len() == 1 {
            let from = matches[0].clone();
            w.candidates.clear();
            w.from = Some(from.clone());
            self.open_text_input_modal(
                "Move",
                "to> ",
                super::super::TextInputAction::MoveTo,
                Some(from.clone()),
                vec![
                    format!("from: {}", from),
                    "Edit the destination path (relative to workspace root).".to_string(),
                ],
            );
            return;
        }

        w.candidates = matches.clone();
        let mut lines = Vec::new();
        lines.push(format!("matches: {}", matches.len()));
        lines.push("Enter a number to pick, or refine the glob.".to_string());
        lines.push("".to_string());

        let limit = 20usize;
        for (i, p) in matches.iter().take(limit).enumerate() {
            lines.push(format!("{:>2}. {}", i + 1, p));
        }
        if matches.len() > limit {
            lines.push(format!("… and {} more", matches.len() - limit));
        }

        self.open_text_input_modal(
            "Move",
            "from (glob or #)> ",
            super::super::TextInputAction::MoveFrom,
            Some(raw),
            lines,
        );
    }

    pub(super) fn move_wizard_to(&mut self, value: String) {
        let Some(ws) = self.require_workspace() else {
            return;
        };
        let Some(w) = self.move_wizard.as_mut() else {
            self.push_error("move wizard not active".to_string());
            return;
        };

        let Some(from) = w.from.clone() else {
            self.push_error("missing from".to_string());
            self.move_wizard = None;
            return;
        };

        let to = value.trim().trim_start_matches("./").to_string();
        if to.is_empty() {
            self.push_error("missing destination".to_string());
            return;
        }
        if to == from {
            self.push_error("destination must differ from source".to_string());
            return;
        }

        match ws.move_path(Path::new(&from), Path::new(&to)) {
            Ok(()) => {
                self.move_wizard = None;
                self.push_output(vec![format!("moved {} -> {}", from, to)]);
                self.refresh_root_view();
            }
            Err(err) => {
                self.open_text_input_modal(
                    "Move",
                    "to> ",
                    super::super::TextInputAction::MoveTo,
                    Some(to),
                    vec![
                        format!("from: {}", from),
                        format!("error: {:#}", err),
                        "Edit destination and try again.".to_string(),
                    ],
                );
            }
        }
    }
}
