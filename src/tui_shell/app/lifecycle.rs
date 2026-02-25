use super::*;

impl App {
    pub(super) fn load(opts: crate::tui::TuiRunOptions) -> Self {
        let mut app = App::default();
        let cwd = match std::env::current_dir() {
            Ok(p) => p,
            Err(err) => {
                app.workspace_err = Some(format!("get current dir: {:#}", err));
                return app;
            }
        };

        match Workspace::discover(&cwd) {
            Ok(ws) => {
                app.workspace = Some(ws);
            }
            Err(err) => {
                app.workspace_err = Some(format!("{}", err));
            }
        }

        app.refresh_root_view();

        app.push_output(vec![
            "Type `help` for commands.".to_string(),
            "(Use `Esc` to go back; use `/` to show available commands.)".to_string(),
        ]);
        app.enable_agent_trace(opts.agent_trace);
        app
    }

    pub(in crate::tui_shell) fn require_workspace(&mut self) -> Option<Workspace> {
        match self.workspace.clone() {
            Some(ws) => Some(ws),
            None => {
                let msg = self
                    .workspace_err
                    .clone()
                    .unwrap_or_else(|| "not in a converge workspace".to_string());
                self.push_error(msg);
                None
            }
        }
    }
}
