use super::*;

pub(super) fn load_or_init_resolution(
    app: &mut App,
    bundle_id: &str,
    root_manifest: &crate::model::ObjectId,
) -> Option<Resolution> {
    let ws = app.require_workspace()?;

    let mut resolution = if ws.store.has_resolution(bundle_id) {
        match ws.store.get_resolution(bundle_id) {
            Ok(r) => r,
            Err(err) => {
                app.push_error(format!("load resolution: {:#}", err));
                return None;
            }
        }
    } else {
        Resolution {
            version: 2,
            bundle_id: bundle_id.to_string(),
            root_manifest: root_manifest.clone(),
            created_at: now_ts(),
            decisions: std::collections::BTreeMap::new(),
        }
    };

    if resolution.root_manifest != *root_manifest {
        app.push_error("resolution root_manifest mismatch".to_string());
        return None;
    }
    if resolution.version == 1 {
        resolution.version = 2;
    }

    Some(resolution)
}
