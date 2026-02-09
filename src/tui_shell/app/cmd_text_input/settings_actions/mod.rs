use super::*;

mod chunking;
mod retention;

pub(super) fn is_settings_action(action: &TextInputAction) -> bool {
    matches!(
        action,
        TextInputAction::ChunkingSet
            | TextInputAction::RetentionKeepLast
            | TextInputAction::RetentionKeepDays
    )
}

pub(super) fn apply_settings_text_input(app: &mut App, action: TextInputAction, value: String) {
    let Some(ws) = app.require_workspace() else {
        return;
    };

    match action {
        TextInputAction::ChunkingSet => chunking::apply_chunking_set(app, &ws, value),
        TextInputAction::RetentionKeepLast | TextInputAction::RetentionKeepDays => {
            retention::apply_retention_update(app, &ws, action, value);
        }
        _ => app.push_error("unexpected settings text input action".to_string()),
    }
}
