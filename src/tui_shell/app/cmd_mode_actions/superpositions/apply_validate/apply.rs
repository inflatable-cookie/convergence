use super::super::super::*;
use crate::model::ObjectId;

pub(super) fn cmd_superpositions_apply_mode(app: &mut App, args: &[String]) {
    let mut publish = false;
    for a in args {
        match a.as_str() {
            "--publish" | "publish" => publish = true,
            _ => {
                app.push_error("usage: apply [publish]".to_string());
                return;
            }
        }
    }

    let Some(ws) = app.require_workspace() else {
        return;
    };

    let Some((bundle_id, root_manifest)) = app
        .current_view::<SuperpositionsView>()
        .map(|v| (v.bundle_id.clone(), v.root_manifest.clone()))
    else {
        app.push_error("not in superpositions mode".to_string());
        return;
    };

    let resolution = match ws.store.get_resolution(&bundle_id) {
        Ok(r) => r,
        Err(err) => {
            app.push_error(format!("load resolution: {:#}", err));
            return;
        }
    };
    if resolution.root_manifest != root_manifest {
        app.push_error("resolution root_manifest mismatch".to_string());
        return;
    }

    let resolved_root =
        match crate::resolve::apply_resolution(&ws.store, &root_manifest, &resolution.decisions) {
            Ok(r) => r,
            Err(err) => {
                app.push_error(format!("apply resolution: {:#}", err));
                return;
            }
        };

    let created_at = now_ts();
    let snap_id = crate::model::compute_snap_id(&created_at, &resolved_root);
    let snap = crate::model::SnapRecord {
        version: 1,
        id: snap_id,
        created_at: created_at.clone(),
        root_manifest: resolved_root,
        message: None,
        stats: crate::model::SnapStats::default(),
    };

    if let Err(err) = ws.store.put_snap(&snap) {
        app.push_error(format!("write snap: {:#}", err));
        return;
    }

    let pub_id = if publish {
        match publish_resolved_snap(app, &ws, &bundle_id, &root_manifest, &snap) {
            Ok(pid) => pid,
            Err(()) => return,
        }
    } else {
        None
    };

    if let Some(pid) = pub_id {
        app.push_output(vec![format!(
            "resolved snap {} (published {})",
            snap.id, pid
        )]);
    } else {
        app.push_output(vec![format!("resolved snap {}", snap.id)]);
    }
}

fn publish_resolved_snap(
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
                "no remote token configured (run `login --url ... --token ... --repo ...`)"
                    .to_string(),
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
