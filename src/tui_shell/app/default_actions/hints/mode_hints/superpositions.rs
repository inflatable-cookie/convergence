use super::*;

pub(super) fn superpositions_mode_hints(app: &App) -> Vec<String> {
    let Some(v) = app.current_view::<SuperpositionsView>() else {
        return Vec::new();
    };
    let missing = v
        .validation
        .as_ref()
        .map(|x| !x.missing.is_empty())
        .unwrap_or(false);
    if missing {
        vec!["next-missing".to_string(), "pick".to_string()]
    } else {
        vec!["apply".to_string(), "back".to_string()]
    }
}
