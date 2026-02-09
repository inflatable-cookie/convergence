use super::*;

pub(super) fn superpositions_clear_decision(app: &mut App) {
    let Some(ws) = app.require_workspace() else {
        return;
    };

    let (bundle_id, root_manifest, path) = match app.current_view::<SuperpositionsView>() {
        Some(view) => {
            if view.items.is_empty() {
                app.push_error("no selected superposition".to_string());
                return;
            }
            let idx = view.selected.min(view.items.len().saturating_sub(1));
            let path = view.items[idx].0.clone();
            (view.bundle_id.clone(), view.root_manifest.clone(), path)
        }
        None => return,
    };

    let Some(mut resolution) = resolution::load_or_init_resolution(app, &bundle_id, &root_manifest)
    else {
        return;
    };

    resolution.decisions.remove(&path);
    if let Err(err) = ws.store.put_resolution(&resolution) {
        app.push_error(format!("write resolution: {:#}", err));
        return;
    }

    if let Some(view) = app.current_view_mut::<SuperpositionsView>() {
        view.decisions.remove(&path);
        view.validation = validate_resolution(&ws.store, &view.root_manifest, &view.decisions).ok();
        view.updated_at = now_ts();
    }

    app.push_output(vec![format!("cleared decision for {}", path)]);
}
