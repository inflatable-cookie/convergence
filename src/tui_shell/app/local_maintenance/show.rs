use super::*;

impl App {
    pub(in crate::tui_shell::app) fn cmd_show(&mut self, args: &[String]) {
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
}
