use super::*;

pub(super) fn settings_mode_hints(app: &App) -> Vec<String> {
    let Some(v) = app.current_view::<SettingsView>() else {
        return vec!["back".to_string()];
    };
    match v.selected_kind() {
        None => vec!["back".to_string()],
        Some(_) => vec!["do".to_string(), "back".to_string()],
    }
}
