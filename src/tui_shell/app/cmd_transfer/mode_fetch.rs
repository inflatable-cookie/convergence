use super::*;

impl App {
    pub(in crate::tui_shell) fn cmd_lanes_fetch_mode(&mut self, args: &[String]) {
        if !args.is_empty() {
            self.push_error("usage: fetch".to_string());
            return;
        }

        let Some(v) = self.current_view::<LanesView>() else {
            self.push_error("not in lanes mode".to_string());
            return;
        };
        if v.items.is_empty() {
            self.push_error("(no selection)".to_string());
            return;
        }
        let idx = v.selected.min(v.items.len().saturating_sub(1));
        let it = &v.items[idx];
        let Some(_h) = &it.head else {
            self.push_error("selected member has no head".to_string());
            return;
        };

        self.cmd_fetch(&[
            "--lane".to_string(),
            it.lane_id.clone(),
            "--user".to_string(),
            it.user.clone(),
        ]);
    }

    pub(in crate::tui_shell) fn cmd_releases_fetch_mode(&mut self, args: &[String]) {
        let Some(v) = self.current_view::<ReleasesView>() else {
            self.push_error("not in releases mode".to_string());
            return;
        };
        if v.items.is_empty() {
            self.push_error("(no selection)".to_string());
            return;
        }
        let idx = v.selected.min(v.items.len().saturating_sub(1));
        let channel = v.items[idx].channel.clone();

        let mut argv = vec!["--release".to_string(), channel];
        argv.extend(args.iter().cloned());
        self.cmd_fetch(&argv);
    }
}
