use super::*;

pub(in crate::tui_shell::app) fn superpositions_clear_decision(app: &mut App) {
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

    // Load or init resolution.
    let mut resolution = if ws.store.has_resolution(&bundle_id) {
        match ws.store.get_resolution(&bundle_id) {
            Ok(r) => r,
            Err(err) => {
                app.push_error(format!("load resolution: {:#}", err));
                return;
            }
        }
    } else {
        Resolution {
            version: 2,
            bundle_id: bundle_id.clone(),
            root_manifest: root_manifest.clone(),
            created_at: now_ts(),
            decisions: std::collections::BTreeMap::new(),
        }
    };

    if resolution.root_manifest != root_manifest {
        app.push_error("resolution root_manifest mismatch".to_string());
        return;
    }
    if resolution.version == 1 {
        resolution.version = 2;
    }

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

pub(in crate::tui_shell::app) fn superpositions_pick_variant(app: &mut App, variant_index: usize) {
    let Some(ws) = app.require_workspace() else {
        return;
    };

    let (bundle_id, root_manifest, path, key, variants_len) =
        match app.current_view::<SuperpositionsView>() {
            Some(view) => {
                if view.items.is_empty() {
                    app.push_error("no selected superposition".to_string());
                    return;
                }
                let idx = view.selected.min(view.items.len().saturating_sub(1));
                let path = view.items[idx].0.clone();
                let Some(variants) = view.variants.get(&path) else {
                    app.push_error("variants not loaded".to_string());
                    return;
                };
                let variants_len = variants.len();
                let Some(variant) = variants.get(variant_index) else {
                    app.push_error(format!("variant out of range (variants: {})", variants_len));
                    return;
                };
                (
                    view.bundle_id.clone(),
                    view.root_manifest.clone(),
                    path,
                    variant.key(),
                    variants_len,
                )
            }
            None => return,
        };

    // Load or init resolution.
    let mut resolution = if ws.store.has_resolution(&bundle_id) {
        match ws.store.get_resolution(&bundle_id) {
            Ok(r) => r,
            Err(err) => {
                app.push_error(format!("load resolution: {:#}", err));
                return;
            }
        }
    } else {
        Resolution {
            version: 2,
            bundle_id: bundle_id.clone(),
            root_manifest: root_manifest.clone(),
            created_at: now_ts(),
            decisions: std::collections::BTreeMap::new(),
        }
    };

    if resolution.root_manifest != root_manifest {
        app.push_error("resolution root_manifest mismatch".to_string());
        return;
    }
    if resolution.version == 1 {
        resolution.version = 2;
    }

    let decision = ResolutionDecision::Key(key);
    resolution.decisions.insert(path.clone(), decision.clone());
    if let Err(err) = ws.store.put_resolution(&resolution) {
        app.push_error(format!("write resolution: {:#}", err));
        return;
    }

    if let Some(view) = app.current_view_mut::<SuperpositionsView>() {
        view.decisions.insert(path.clone(), decision);
        view.validation = validate_resolution(&ws.store, &view.root_manifest, &view.decisions).ok();
        view.updated_at = now_ts();
    }

    app.push_output(vec![format!(
        "picked variant #{} for {} (variants: {})",
        variant_index + 1,
        path,
        variants_len
    )]);
}
