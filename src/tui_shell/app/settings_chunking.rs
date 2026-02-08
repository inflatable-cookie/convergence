use super::*;

impl App {
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
}
