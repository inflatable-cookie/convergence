use super::*;

impl App {
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
