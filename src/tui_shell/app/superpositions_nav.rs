use super::*;

pub(super) fn superpositions_clear_decision(app: &mut App) {
    let Some(ws) = app.require_workspace() else {
        return;
    };

    let (bundle_id, root_manifest, path) = match app.current_view::<SuperpositionsView>() {
        Some(v) => {
            if v.items.is_empty() {
                app.push_error("no selected superposition".to_string());
                return;
            }
            let idx = v.selected.min(v.items.len().saturating_sub(1));
            let path = v.items[idx].0.clone();
            (v.bundle_id.clone(), v.root_manifest.clone(), path)
        }
        None => return,
    };

    // Load or init resolution.
    let mut res = if ws.store.has_resolution(&bundle_id) {
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

    if res.root_manifest != root_manifest {
        app.push_error("resolution root_manifest mismatch".to_string());
        return;
    }
    if res.version == 1 {
        res.version = 2;
    }

    res.decisions.remove(&path);
    if let Err(err) = ws.store.put_resolution(&res) {
        app.push_error(format!("write resolution: {:#}", err));
        return;
    }

    if let Some(v) = app.current_view_mut::<SuperpositionsView>() {
        v.decisions.remove(&path);
        v.validation = validate_resolution(&ws.store, &v.root_manifest, &v.decisions).ok();
        v.updated_at = now_ts();
    }

    app.push_output(vec![format!("cleared decision for {}", path)]);
}

pub(super) fn superpositions_pick_variant(app: &mut App, variant_index: usize) {
    let Some(ws) = app.require_workspace() else {
        return;
    };

    let (bundle_id, root_manifest, path, key, variants_len) =
        match app.current_view::<SuperpositionsView>() {
            Some(v) => {
                if v.items.is_empty() {
                    app.push_error("no selected superposition".to_string());
                    return;
                }
                let idx = v.selected.min(v.items.len().saturating_sub(1));
                let path = v.items[idx].0.clone();
                let Some(vs) = v.variants.get(&path) else {
                    app.push_error("variants not loaded".to_string());
                    return;
                };
                let variants_len = vs.len();
                let Some(vr) = vs.get(variant_index) else {
                    app.push_error(format!("variant out of range (variants: {})", variants_len));
                    return;
                };
                (
                    v.bundle_id.clone(),
                    v.root_manifest.clone(),
                    path,
                    vr.key(),
                    variants_len,
                )
            }
            None => return,
        };

    // Load or init resolution.
    let mut res = if ws.store.has_resolution(&bundle_id) {
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

    if res.root_manifest != root_manifest {
        app.push_error("resolution root_manifest mismatch".to_string());
        return;
    }
    if res.version == 1 {
        res.version = 2;
    }

    let decision = ResolutionDecision::Key(key);
    res.decisions.insert(path.clone(), decision.clone());
    if let Err(err) = ws.store.put_resolution(&res) {
        app.push_error(format!("write resolution: {:#}", err));
        return;
    }

    if let Some(v) = app.current_view_mut::<SuperpositionsView>() {
        v.decisions.insert(path.clone(), decision);
        v.validation = validate_resolution(&ws.store, &v.root_manifest, &v.decisions).ok();
        v.updated_at = now_ts();
    }

    app.push_output(vec![format!(
        "picked variant #{} for {} (variants: {})",
        variant_index + 1,
        path,
        variants_len
    )]);
}

pub(super) fn superpositions_jump_next_missing(app: &mut App) {
    let next = match app.current_view::<SuperpositionsView>() {
        Some(v) => {
            if v.items.is_empty() {
                return;
            }
            let start = v.selected.min(v.items.len().saturating_sub(1));
            (1..=v.items.len()).find_map(|off| {
                let idx = (start + off) % v.items.len();
                let path = &v.items[idx].0;
                if !v.decisions.contains_key(path) {
                    Some(idx)
                } else {
                    None
                }
            })
        }
        None => return,
    };

    if let Some(idx) = next {
        if let Some(v) = app.current_view_mut::<SuperpositionsView>() {
            v.selected = idx;
            v.updated_at = now_ts();
        }
        app.push_output(vec!["jumped to missing".to_string()]);
    } else {
        app.push_output(vec!["no missing decisions".to_string()]);
    }
}

pub(super) fn superpositions_jump_next_invalid(app: &mut App) {
    let next = match app.current_view::<SuperpositionsView>() {
        Some(v) => {
            if v.items.is_empty() {
                return;
            }

            let Some(vr) = v.validation.as_ref() else {
                return;
            };

            let mut invalid = std::collections::HashSet::new();
            for d in &vr.invalid_keys {
                invalid.insert(d.path.as_str());
            }
            for d in &vr.out_of_range {
                invalid.insert(d.path.as_str());
            }

            let start = v.selected.min(v.items.len().saturating_sub(1));
            (1..=v.items.len()).find_map(|off| {
                let idx = (start + off) % v.items.len();
                let path = v.items[idx].0.as_str();
                if invalid.contains(path) {
                    Some(idx)
                } else {
                    None
                }
            })
        }
        None => return,
    };

    if let Some(idx) = next {
        if let Some(v) = app.current_view_mut::<SuperpositionsView>() {
            v.selected = idx;
            v.updated_at = now_ts();
        }
        app.push_output(vec!["jumped to invalid".to_string()]);
    } else {
        app.push_output(vec!["no invalid decisions".to_string()]);
    }
}
