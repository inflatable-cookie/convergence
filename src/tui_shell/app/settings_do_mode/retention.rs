use super::*;

pub(super) fn show(app: &mut App) {
    app.cmd_retention(&["show".to_string()]);
    app.refresh_settings_view();
}

pub(super) fn keep_last(app: &mut App) {
    let initial = app
        .current_view::<SettingsView>()
        .and_then(|v| v.snapshot)
        .and_then(|s| s.retention_keep_last)
        .map(|n| n.to_string());
    app.open_text_input_modal(
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

pub(super) fn keep_days(app: &mut App) {
    let initial = app
        .current_view::<SettingsView>()
        .and_then(|v| v.snapshot)
        .and_then(|s| s.retention_keep_days)
        .map(|n| n.to_string());
    app.open_text_input_modal(
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

pub(super) fn toggle_prune_snaps(app: &mut App) {
    let Some(ws) = app.require_workspace() else {
        return;
    };

    let mut cfg = match ws.store.read_config() {
        Ok(c) => c,
        Err(err) => {
            app.push_error(format!("read config: {:#}", err));
            return;
        }
    };
    let mut r = cfg.retention.unwrap_or_default();
    r.prune_snaps = !r.prune_snaps;
    let prune = r.prune_snaps;
    cfg.retention = Some(r);
    if let Err(err) = ws.store.write_config(&cfg) {
        app.push_error(format!("write config: {:#}", err));
        return;
    }

    app.refresh_root_view();
    app.refresh_settings_view();
    app.push_output(vec![format!("retention.prune_snaps: {}", prune)]);
}

pub(super) fn reset(app: &mut App) {
    app.cmd_retention(&["reset".to_string()]);
    app.refresh_root_view();
    app.refresh_settings_view();
}
