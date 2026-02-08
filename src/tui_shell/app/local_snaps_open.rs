use super::*;

impl App {
    pub(super) fn cmd_snaps(&mut self, args: &[String]) {
        let Some(ws) = self.require_workspace() else {
            return;
        };

        let rctx = RenderCtx {
            now: OffsetDateTime::now_utc(),
            ts_mode: self.ts_mode,
        };

        let mut limit: Option<usize> = None;

        if args.len() == 1
            && let Ok(n) = args[0].parse::<usize>()
        {
            limit = Some(n);
        }

        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--limit" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --limit".to_string());
                        return;
                    }
                    limit = match args[i].parse::<usize>() {
                        Ok(n) => Some(n),
                        Err(_) => {
                            self.push_error("invalid --limit".to_string());
                            return;
                        }
                    };
                }
                "limit" if i + 1 < args.len() => {
                    i += 1;
                    limit = match args[i].parse::<usize>() {
                        Ok(n) => Some(n),
                        Err(_) => {
                            self.push_error("invalid limit".to_string());
                            return;
                        }
                    };
                }
                a => {
                    self.push_error(format!("unknown arg: {}", a));
                    return;
                }
            }
            i += 1;
        }

        match ws.list_snaps() {
            Ok(snaps) => {
                let items = if let Some(n) = limit {
                    snaps.into_iter().take(n).collect::<Vec<_>>()
                } else {
                    snaps
                };

                let head_id = ws.store.get_head().ok().flatten();

                let pending_changes = local_status_lines(&ws, &rctx)
                    .ok()
                    .map(|lines| extract_change_summary(lines).0)
                    .and_then(|sum| if sum.total() > 0 { Some(sum) } else { None });

                let has_header =
                    pending_changes.is_some() || (pending_changes.is_none() && head_id.is_some());
                let selected_row = if has_header && !items.is_empty() {
                    1
                } else {
                    0
                };

                self.push_view(SnapsView {
                    updated_at: now_ts(),
                    filter: None,
                    all_items: items.clone(),
                    items,
                    selected_row,
                    head_id,
                    pending_changes,
                });
                self.push_output(vec!["opened snaps".to_string()]);
            }
            Err(err) => {
                self.push_error(format!("snaps: {:#}", err));
            }
        }
    }
}
