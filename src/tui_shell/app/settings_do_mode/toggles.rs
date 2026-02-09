use super::*;

pub(super) fn toggle_timestamps(app: &mut App) {
    app.ts_mode = app.ts_mode.toggle();
    app.refresh_root_view();
    app.refresh_settings_view();
    app.push_output(vec![format!("timestamps: {}", app.ts_mode.label())]);
}
