use super::*;

pub(super) fn apply_retention_update(
    app: &mut App,
    ws: &Workspace,
    action: TextInputAction,
    value: String,
) {
    let v = value.trim();
    let v_lc = v.to_lowercase();
    let parsed = if v_lc == "unset" || v_lc == "none" {
        None
    } else {
        match v.parse::<u64>() {
            Ok(n) if n > 0 => Some(n),
            _ => {
                app.push_error("expected a positive number (or 'unset')".to_string());
                return;
            }
        }
    };

    let mut cfg = match ws.store.read_config() {
        Ok(c) => c,
        Err(err) => {
            app.push_error(format!("read config: {:#}", err));
            return;
        }
    };
    let mut retention = cfg.retention.unwrap_or_default();
    match action {
        TextInputAction::RetentionKeepLast => retention.keep_last = parsed,
        TextInputAction::RetentionKeepDays => retention.keep_days = parsed,
        _ => {}
    }
    cfg.retention = Some(retention);
    if let Err(err) = ws.store.write_config(&cfg) {
        app.push_error(format!("write config: {:#}", err));
        return;
    }

    app.refresh_root_view();
    app.refresh_settings_view();
    match action {
        TextInputAction::RetentionKeepLast => {
            app.push_output(vec!["updated retention keep_last".to_string()]);
        }
        TextInputAction::RetentionKeepDays => {
            app.push_output(vec!["updated retention keep_days".to_string()]);
        }
        _ => {}
    }
}
