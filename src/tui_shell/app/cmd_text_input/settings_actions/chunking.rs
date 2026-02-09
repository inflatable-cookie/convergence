use super::*;

pub(super) fn apply_chunking_set(app: &mut App, ws: &Workspace, value: String) {
    let norm = value.replace(',', " ");
    let parts = norm.split_whitespace().collect::<Vec<_>>();
    if parts.len() != 2 {
        app.push_error("format: <chunk_size_mib> <threshold_mib>".to_string());
        return;
    }
    let chunk_size_mib = match parts[0].parse::<u64>() {
        Ok(n) if n > 0 => n,
        _ => {
            app.push_error("invalid chunk_size_mib".to_string());
            return;
        }
    };
    let threshold_mib = match parts[1].parse::<u64>() {
        Ok(n) if n > 0 => n,
        _ => {
            app.push_error("invalid threshold_mib".to_string());
            return;
        }
    };
    if threshold_mib < chunk_size_mib {
        app.push_error("threshold must be >= chunk_size".to_string());
        return;
    }

    let mut cfg = match ws.store.read_config() {
        Ok(c) => c,
        Err(err) => {
            app.push_error(format!("read config: {:#}", err));
            return;
        }
    };
    cfg.chunking = Some(ChunkingConfig {
        chunk_size: chunk_size_mib * 1024 * 1024,
        threshold: threshold_mib * 1024 * 1024,
    });
    if let Err(err) = ws.store.write_config(&cfg) {
        app.push_error(format!("write config: {:#}", err));
        return;
    }

    app.refresh_root_view();
    app.refresh_settings_view();
    app.push_output(vec!["updated chunking config".to_string()]);
}
