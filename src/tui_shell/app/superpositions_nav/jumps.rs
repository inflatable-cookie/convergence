use super::*;

pub(in crate::tui_shell::app) fn superpositions_jump_next_missing(app: &mut App) {
    let next = match app.current_view::<SuperpositionsView>() {
        Some(view) => {
            if view.items.is_empty() {
                return;
            }
            let start = view.selected.min(view.items.len().saturating_sub(1));
            (1..=view.items.len()).find_map(|offset| {
                let idx = (start + offset) % view.items.len();
                let path = &view.items[idx].0;
                if !view.decisions.contains_key(path) {
                    Some(idx)
                } else {
                    None
                }
            })
        }
        None => return,
    };

    if let Some(idx) = next {
        if let Some(view) = app.current_view_mut::<SuperpositionsView>() {
            view.selected = idx;
            view.updated_at = now_ts();
        }
        app.push_output(vec!["jumped to missing".to_string()]);
    } else {
        app.push_output(vec!["no missing decisions".to_string()]);
    }
}

pub(in crate::tui_shell::app) fn superpositions_jump_next_invalid(app: &mut App) {
    let next = match app.current_view::<SuperpositionsView>() {
        Some(view) => {
            if view.items.is_empty() {
                return;
            }
            let Some(validation) = view.validation.as_ref() else {
                return;
            };

            let mut invalid_paths = std::collections::HashSet::new();
            for detail in &validation.invalid_keys {
                invalid_paths.insert(detail.path.as_str());
            }
            for detail in &validation.out_of_range {
                invalid_paths.insert(detail.path.as_str());
            }

            let start = view.selected.min(view.items.len().saturating_sub(1));
            (1..=view.items.len()).find_map(|offset| {
                let idx = (start + offset) % view.items.len();
                let path = view.items[idx].0.as_str();
                if invalid_paths.contains(path) {
                    Some(idx)
                } else {
                    None
                }
            })
        }
        None => return,
    };

    if let Some(idx) = next {
        if let Some(view) = app.current_view_mut::<SuperpositionsView>() {
            view.selected = idx;
            view.updated_at = now_ts();
        }
        app.push_output(vec!["jumped to invalid".to_string()]);
    } else {
        app.push_output(vec!["no invalid decisions".to_string()]);
    }
}
