use super::*;

pub(super) fn root_mode_hints(app: &App) -> Vec<String> {
    match app.root_ctx {
        RootContext::Local => {
            if app.workspace.is_none() {
                if app
                    .workspace_err
                    .as_deref()
                    .is_some_and(|e| e.contains("No .converge directory found"))
                {
                    return vec!["init".to_string()];
                }
                return Vec::new();
            }

            let mut changes = 0usize;
            if let Some(v) = app.current_view::<RootView>() {
                changes = v.change_summary.added
                    + v.change_summary.modified
                    + v.change_summary.deleted
                    + v.change_summary.renamed;
            }
            if changes > 0 {
                return vec!["snap".to_string(), "history".to_string()];
            }

            if app.remote_configured {
                let latest = app.latest_snap_id.clone();
                let synced = app.lane_last_synced.get("default").cloned();
                if latest.is_some() && latest != synced {
                    return vec!["sync".to_string(), "history".to_string()];
                }
                if latest.is_some() && latest != app.last_published_snap_id {
                    return vec!["publish".to_string(), "history".to_string()];
                }
            }

            vec!["history".to_string()]
        }
        RootContext::Remote => {
            if !app.remote_configured || app.remote_identity.is_none() {
                vec!["login".to_string(), "bootstrap".to_string()]
            } else if app.remote_repo_missing() {
                vec!["create-repo".to_string()]
            } else {
                vec!["inbox".to_string(), "releases".to_string()]
            }
        }
    }
}
