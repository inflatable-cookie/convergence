use super::*;

impl App {
    pub(in crate::tui_shell) fn finish_sync_wizard(&mut self) {
        let Some(w) = self.sync_wizard.clone() else {
            self.push_error("sync wizard not active".to_string());
            return;
        };
        self.sync_wizard = None;

        let mut argv: Vec<String> = Vec::new();
        if let Some(s) = w.snap {
            argv.extend(["--snap-id".to_string(), s]);
        }
        if !w.lane.trim().is_empty() {
            argv.extend(["--lane".to_string(), w.lane]);
        }
        if let Some(c) = w.client {
            argv.extend(["--client-id".to_string(), c]);
        }
        self.cmd_sync_impl(&argv);
    }
}
