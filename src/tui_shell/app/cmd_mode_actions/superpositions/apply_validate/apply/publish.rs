use super::*;
use crate::model::ObjectId;

pub(super) fn publish_resolved_snap(
    app: &mut App,
    ws: &Workspace,
    bundle_id: &str,
    root_manifest: &ObjectId,
    snap: &crate::model::SnapRecord,
) -> std::result::Result<Option<String>, ()> {
    let remote = match app.remote_config() {
        Some(r) => r,
        None => {
            app.push_error("no remote configured".to_string());
            return Err(());
        }
    };

    let token = match ws.store.get_remote_token(&remote) {
        Ok(Some(t)) => t,
        Ok(None) => {
            app.push_error(
                "no remote token configured (run `login --url ... --token .....`)".to_string(),
            );
            return Err(());
        }
        Err(err) => {
            app.push_error(format!("read remote token: {:#}", err));
            return Err(());
        }
    };

    let client = match RemoteClient::new(remote.clone(), token) {
        Ok(c) => c,
        Err(err) => {
            app.push_error(format!("init remote client: {:#}", err));
            return Err(());
        }
    };

    let res_meta = crate::remote::PublicationResolution {
        bundle_id: bundle_id.to_string(),
        root_manifest: root_manifest.as_str().to_string(),
        resolved_root_manifest: snap.root_manifest.as_str().to_string(),
        created_at: snap.created_at.clone(),
    };

    match client.publish_snap_with_resolution(
        &ws.store,
        snap,
        &remote.scope,
        &remote.gate,
        Some(res_meta),
    ) {
        Ok(p) => Ok(Some(p.id)),
        Err(err) => {
            app.push_error(format!("publish: {:#}", err));
            Err(())
        }
    }
}
