use super::*;

impl App {
    pub(in crate::tui_shell::app) fn run_current_input(&mut self) {
        let line = self.input.buf.trim().to_string();
        if line.is_empty() {
            return;
        }

        self.input.push_history(&line);
        self.push_command(format!("{} {}", self.prompt(), line));
        self.input.clear();
        self.suggestions.clear();
        self.suggestion_selected = 0;

        let line = line.trim_start().strip_prefix('/').unwrap_or(&line).trim();
        let tokens = match tokenize(line) {
            Ok(t) => t,
            Err(err) => {
                self.push_error(format!("parse error: {}", err));
                return;
            }
        };
        if tokens.is_empty() {
            return;
        }

        let mut cmd = tokens[0].to_lowercase();
        let args = &tokens[1..];

        let mode = self.mode();
        let mut defs = self.available_command_defs();
        defs.sort_by(|a, b| a.name.cmp(b.name));

        if defs.iter().any(|d| d.name == cmd) {
            // already canonical
        } else if let Some(d) = defs.iter().find(|d| d.aliases.iter().any(|&a| a == cmd)) {
            cmd = d.name.to_string();
        } else {
            let matches = defs
                .iter()
                .filter(|d| d.name.starts_with(&cmd))
                .collect::<Vec<_>>();
            if matches.len() == 1 {
                cmd = matches[0].name.to_string();
            }
        }

        if cmd == "help" {
            self.trace_command_submitted(line, "help");
            self.cmd_help(&defs, args);
            return;
        }

        self.trace_command_submitted(line, &cmd);
        if mode == UiMode::Root {
            self.dispatch_root(cmd.as_str(), args);
        } else {
            self.dispatch_mode(mode, cmd.as_str(), args);
        }
    }
}
