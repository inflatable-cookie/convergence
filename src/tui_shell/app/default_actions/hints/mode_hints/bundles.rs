use super::*;

pub(super) fn bundles_mode_hints(app: &App) -> Vec<String> {
    let Some(v) = app.current_view::<BundlesView>() else {
        return Vec::new();
    };
    if v.items.is_empty() {
        return vec!["back".to_string()];
    }
    let idx = v.selected.min(v.items.len().saturating_sub(1));
    let b = &v.items[idx];

    if b.reasons.iter().any(|r| r == "superpositions_present") {
        return vec!["superpositions".to_string(), "back".to_string()];
    }
    if b.reasons.iter().any(|r| r == "approvals_missing") {
        return vec!["approve".to_string(), "back".to_string()];
    }
    if b.promotable {
        return vec!["promote".to_string(), "back".to_string()];
    }

    vec!["back".to_string()]
}
