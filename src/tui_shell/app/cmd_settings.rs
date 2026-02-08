use super::*;

impl App {
    fn load_settings_snapshot(&mut self) -> Option<SettingsSnapshot> {
        let ws = self.workspace.as_ref()?;

        let cfg = match ws.store.read_config() {
            Ok(c) => c,
            Err(err) => {
                self.push_error(format!("read config: {:#}", err));
                return None;
            }
        };

        let (chunk_size, threshold) = cfg
            .chunking
            .as_ref()
            .map(|c| (c.chunk_size, c.threshold))
            .unwrap_or((4 * 1024 * 1024, 8 * 1024 * 1024));

        let r = cfg.retention.unwrap_or_default();
        Some(SettingsSnapshot {
            chunk_size_mib: chunk_size / (1024 * 1024),
            threshold_mib: threshold / (1024 * 1024),

            retention_keep_last: r.keep_last,
            retention_keep_days: r.keep_days,
            retention_prune_snaps: r.prune_snaps,
            retention_pinned: r.pinned.len(),
        })
    }

    pub(super) fn refresh_settings_view(&mut self) {
        let snapshot = self.load_settings_snapshot();
        let Some(v) = self.current_view_mut::<SettingsView>() else {
            return;
        };
        v.snapshot = snapshot;
        v.updated_at = now_ts();
    }

    pub(super) fn cmd_settings(&mut self, args: &[String]) {
        if !args.is_empty() {
            self.push_error("usage: settings".to_string());
            return;
        }

        if self.mode() == UiMode::Settings {
            self.refresh_settings_view();
            self.push_output(vec!["refreshed settings".to_string()]);
            return;
        }

        let snapshot = self.load_settings_snapshot();
        let mut items = vec![SettingsItemKind::ToggleTimestamps];
        if snapshot.is_some() {
            items.extend([
                SettingsItemKind::ChunkingShow,
                SettingsItemKind::ChunkingSet,
                SettingsItemKind::ChunkingReset,
                SettingsItemKind::RetentionShow,
                SettingsItemKind::RetentionKeepLast,
                SettingsItemKind::RetentionKeepDays,
                SettingsItemKind::ToggleRetentionPruneSnaps,
                SettingsItemKind::RetentionReset,
            ]);
        }

        self.push_view(SettingsView {
            updated_at: now_ts(),
            items,
            selected: 0,
            snapshot,
        });
        self.push_output(vec!["opened settings".to_string()]);
    }

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

    pub(super) fn cmd_chunking(&mut self, args: &[String]) {
        let Some(ws) = self.require_workspace() else {
            return;
        };

        let sub = args.first().map(|s| s.as_str()).unwrap_or("show");
        match sub {
            "show" => {
                let cfg = match ws.store.read_config() {
                    Ok(c) => c,
                    Err(err) => {
                        self.push_error(format!("read config: {:#}", err));
                        return;
                    }
                };

                let (chunk_size, threshold) = cfg
                    .chunking
                    .as_ref()
                    .map(|c| (c.chunk_size, c.threshold))
                    .unwrap_or((4 * 1024 * 1024, 8 * 1024 * 1024));
                let lines = vec![
                    format!("chunk_size: {} MiB", chunk_size / (1024 * 1024)),
                    format!("threshold: {} MiB", threshold / (1024 * 1024)),
                    "".to_string(),
                    "Files with size >= threshold are stored as chunked files.".to_string(),
                ];
                self.open_modal("Chunking", lines);
            }
            "set" => {
                let mut chunk_size_mib: Option<u64> = None;
                let mut threshold_mib: Option<u64> = None;

                let mut i = 1;
                while i < args.len() {
                    match args[i].as_str() {
                        "--chunk-size-mib" => {
                            i += 1;
                            let Some(v) = args.get(i) else {
                                self.push_error("missing value for --chunk-size-mib".to_string());
                                return;
                            };
                            chunk_size_mib = v.parse::<u64>().ok();
                        }
                        "--threshold-mib" => {
                            i += 1;
                            let Some(v) = args.get(i) else {
                                self.push_error("missing value for --threshold-mib".to_string());
                                return;
                            };
                            threshold_mib = v.parse::<u64>().ok();
                        }
                        _ => {
                            self.push_error(
                                "usage: settings chunking set --chunk-size-mib N --threshold-mib N"
                                    .to_string(),
                            );
                            return;
                        }
                    }
                    i += 1;
                }

                let Some(chunk_size_mib) = chunk_size_mib else {
                    self.push_error("missing --chunk-size-mib".to_string());
                    return;
                };
                let Some(threshold_mib) = threshold_mib else {
                    self.push_error("missing --threshold-mib".to_string());
                    return;
                };

                let chunk_size = chunk_size_mib * 1024 * 1024;
                let threshold = threshold_mib * 1024 * 1024;

                let mut cfg = match ws.store.read_config() {
                    Ok(c) => c,
                    Err(err) => {
                        self.push_error(format!("read config: {:#}", err));
                        return;
                    }
                };
                cfg.chunking = Some(ChunkingConfig {
                    chunk_size,
                    threshold,
                });
                if let Err(err) = ws.store.write_config(&cfg) {
                    self.push_error(format!("write config: {:#}", err));
                    return;
                }

                self.refresh_root_view();
                self.push_output(vec!["updated chunking config".to_string()]);
            }
            "reset" => {
                let mut cfg = match ws.store.read_config() {
                    Ok(c) => c,
                    Err(err) => {
                        self.push_error(format!("read config: {:#}", err));
                        return;
                    }
                };
                cfg.chunking = None;
                if let Err(err) = ws.store.write_config(&cfg) {
                    self.push_error(format!("write config: {:#}", err));
                    return;
                }
                self.refresh_root_view();
                self.push_output(vec!["reset chunking config".to_string()]);
            }
            _ => {
                self.push_error(
                    "usage: settings chunking show | settings chunking set --chunk-size-mib N --threshold-mib N | settings chunking reset"
                        .to_string(),
                );
            }
        }
    }

    pub(super) fn cmd_retention(&mut self, args: &[String]) {
        let Some(ws) = self.require_workspace() else {
            return;
        };

        let sub = args.first().map(|s| s.as_str()).unwrap_or("show");
        match sub {
            "show" => {
                let cfg = match ws.store.read_config() {
                    Ok(c) => c,
                    Err(err) => {
                        self.push_error(format!("read config: {:#}", err));
                        return;
                    }
                };
                let r = cfg.retention.unwrap_or_default();
                let mut lines = Vec::new();
                lines.push(format!(
                    "keep_last: {}",
                    r.keep_last
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| "(unset)".to_string())
                ));
                lines.push(format!(
                    "keep_days: {}",
                    r.keep_days
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| "(unset)".to_string())
                ));
                lines.push(format!("prune_snaps: {}", r.prune_snaps));
                lines.push(format!("pinned: {}", r.pinned.len()));
                for p in r.pinned {
                    lines.push(format!("  - {}", p));
                }
                self.open_modal("Retention", lines);
            }
            "set" => {
                let mut keep_last: Option<u64> = None;
                let mut keep_days: Option<u64> = None;
                let mut prune_snaps: Option<bool> = None;

                let mut i = 1;
                while i < args.len() {
                    match args[i].as_str() {
                        "--keep-last" => {
                            i += 1;
                            let Some(v) = args.get(i) else {
                                self.push_error("missing value for --keep-last".to_string());
                                return;
                            };
                            keep_last = v.parse::<u64>().ok();
                        }
                        "--keep-days" => {
                            i += 1;
                            let Some(v) = args.get(i) else {
                                self.push_error("missing value for --keep-days".to_string());
                                return;
                            };
                            keep_days = v.parse::<u64>().ok();
                        }
                        "--prune-snaps" => {
                            i += 1;
                            let Some(v) = args.get(i) else {
                                self.push_error("missing value for --prune-snaps".to_string());
                                return;
                            };
                            prune_snaps = match v.as_str() {
                                "true" => Some(true),
                                "false" => Some(false),
                                _ => None,
                            };
                        }
                        _ => {
                            self.push_error(
                                "usage: settings retention set [--keep-last N] [--keep-days N] [--prune-snaps true|false]"
                                    .to_string(),
                            );
                            return;
                        }
                    }
                    i += 1;
                }

                let mut cfg = match ws.store.read_config() {
                    Ok(c) => c,
                    Err(err) => {
                        self.push_error(format!("read config: {:#}", err));
                        return;
                    }
                };
                let mut r = cfg.retention.unwrap_or_default();
                if keep_last.is_some() {
                    r.keep_last = keep_last;
                }
                if keep_days.is_some() {
                    r.keep_days = keep_days;
                }
                if let Some(v) = prune_snaps {
                    r.prune_snaps = v;
                }
                cfg.retention = Some(r);
                if let Err(err) = ws.store.write_config(&cfg) {
                    self.push_error(format!("write config: {:#}", err));
                    return;
                }
                self.refresh_root_view();
                self.push_output(vec!["updated retention config".to_string()]);
            }
            "reset" => {
                let mut cfg = match ws.store.read_config() {
                    Ok(c) => c,
                    Err(err) => {
                        self.push_error(format!("read config: {:#}", err));
                        return;
                    }
                };
                cfg.retention = None;
                if let Err(err) = ws.store.write_config(&cfg) {
                    self.push_error(format!("write config: {:#}", err));
                    return;
                }
                self.refresh_root_view();
                self.push_output(vec!["reset retention config".to_string()]);
            }
            "pin" | "unpin" => {
                if args.len() != 2 {
                    self.push_error(format!("usage: retention {} <snap_id_prefix>", sub));
                    return;
                }
                let prefix = &args[1];
                let snaps = match ws.list_snaps() {
                    Ok(s) => s,
                    Err(err) => {
                        self.push_error(format!("list snaps: {:#}", err));
                        return;
                    }
                };
                let matches = snaps
                    .iter()
                    .filter(|s| s.id.starts_with(prefix))
                    .map(|s| s.id.clone())
                    .collect::<Vec<_>>();
                if matches.is_empty() {
                    self.push_error(format!("no snap matches {}", prefix));
                    return;
                }
                if matches.len() > 1 {
                    self.push_error(format!("ambiguous snap prefix {}", prefix));
                    return;
                }
                let snap_id = matches[0].clone();

                let mut cfg = match ws.store.read_config() {
                    Ok(c) => c,
                    Err(err) => {
                        self.push_error(format!("read config: {:#}", err));
                        return;
                    }
                };
                let mut r = cfg.retention.unwrap_or_default();
                if sub == "pin" {
                    if !r.pinned.iter().any(|p| p == &snap_id) {
                        r.pinned.push(snap_id.clone());
                    }
                } else {
                    r.pinned.retain(|p| p != &snap_id);
                }
                cfg.retention = Some(r);
                if let Err(err) = ws.store.write_config(&cfg) {
                    self.push_error(format!("write config: {:#}", err));
                    return;
                }
                self.refresh_root_view();
                self.push_output(vec![format!("{} {}", sub, snap_id)]);
            }
            _ => {
                self.push_error(
                    "usage: settings retention show | settings retention set [--keep-last N] [--keep-days N] [--prune-snaps true|false] | settings retention pin <snap> | settings retention unpin <snap> | settings retention reset"
                        .to_string(),
                );
            }
        }
    }
}
