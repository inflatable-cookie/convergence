use super::*;

pub(super) fn snaps_mode_hints(app: &App) -> Vec<String> {
    let Some(v) = app.current_view::<SnapsView>() else {
        return Vec::new();
    };
    if v.selected_is_pending() {
        vec!["snap".to_string(), "revert".to_string()]
    } else if v.selected_is_clean() {
        vec!["unsnap".to_string()]
    } else {
        vec!["restore".to_string(), "msg".to_string()]
    }
}
