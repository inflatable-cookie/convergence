use super::*;

impl App {
    pub(super) fn cmd_settings_do_mode(&mut self) {
        let Some(kind) = self
            .current_view::<SettingsView>()
            .and_then(|v| v.selected_kind())
        else {
            self.push_error("no selected setting".to_string());
            return;
        };

        match kind {
            SettingsItemKind::ToggleTimestamps => {
                self.ts_mode = self.ts_mode.toggle();
                self.refresh_root_view();
                self.refresh_settings_view();
                self.push_output(vec![format!("timestamps: {}", self.ts_mode.label())]);
            }
            SettingsItemKind::ChunkingShow => {
                self.cmd_chunking(&["show".to_string()]);
                self.refresh_settings_view();
            }
            SettingsItemKind::ChunkingSet => {
                let (chunk, threshold) = self
                    .current_view::<SettingsView>()
                    .and_then(|v| v.snapshot)
                    .map(|s| (s.chunk_size_mib, s.threshold_mib))
                    .unwrap_or((4, 8));
                self.open_text_input_modal(
                    "Chunking",
                    "chunking> ",
                    TextInputAction::ChunkingSet,
                    Some(format!("{} {}", chunk, threshold)),
                    vec![
                        "Set chunking config (MiB).".to_string(),
                        "Format: <chunk_size_mib> <threshold_mib>".to_string(),
                    ],
                );
            }
            SettingsItemKind::ChunkingReset => {
                self.cmd_chunking(&["reset".to_string()]);
                self.refresh_settings_view();
            }
            SettingsItemKind::RetentionShow => {
                self.cmd_retention(&["show".to_string()]);
                self.refresh_settings_view();
            }
            SettingsItemKind::RetentionKeepLast => {
                let initial = self
                    .current_view::<SettingsView>()
                    .and_then(|v| v.snapshot)
                    .and_then(|s| s.retention_keep_last)
                    .map(|n| n.to_string());
                self.open_text_input_modal(
                    "Retention",
                    "keep_last> ",
                    TextInputAction::RetentionKeepLast,
                    initial,
                    vec![
                        "Set retention keep_last.".to_string(),
                        "Enter a number of snaps, or 'unset'.".to_string(),
                    ],
                );
            }
            SettingsItemKind::RetentionKeepDays => {
                let initial = self
                    .current_view::<SettingsView>()
                    .and_then(|v| v.snapshot)
                    .and_then(|s| s.retention_keep_days)
                    .map(|n| n.to_string());
                self.open_text_input_modal(
                    "Retention",
                    "keep_days> ",
                    TextInputAction::RetentionKeepDays,
                    initial,
                    vec![
                        "Set retention keep_days.".to_string(),
                        "Enter a number of days, or 'unset'.".to_string(),
                    ],
                );
            }
            SettingsItemKind::ToggleRetentionPruneSnaps => {
                let Some(ws) = self.require_workspace() else {
                    return;
                };

                let mut cfg = match ws.store.read_config() {
                    Ok(c) => c,
                    Err(err) => {
                        self.push_error(format!("read config: {:#}", err));
                        return;
                    }
                };
                let mut r = cfg.retention.unwrap_or_default();
                r.prune_snaps = !r.prune_snaps;
                let prune = r.prune_snaps;
                cfg.retention = Some(r);
                if let Err(err) = ws.store.write_config(&cfg) {
                    self.push_error(format!("write config: {:#}", err));
                    return;
                }

                self.refresh_root_view();
                self.refresh_settings_view();
                self.push_output(vec![format!("retention.prune_snaps: {}", prune)]);
            }
            SettingsItemKind::RetentionReset => {
                self.cmd_retention(&["reset".to_string()]);
                self.refresh_root_view();
                self.refresh_settings_view();
            }
        }
    }
}
