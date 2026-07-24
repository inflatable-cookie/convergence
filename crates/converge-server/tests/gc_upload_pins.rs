//! Batch 12.2 (audit C2): an uploaded-but-unpublished object survives GC
//! regardless of clock time, because the upload pins it. Exercised at the
//! engine level so GC can run with zero grace (HTTP hardcodes 300s).

use std::time::Duration;

use anyhow::Result;

use converge_client::model::ManifestEntryKind;
use converge_client::workspace::Workspace;
use converge_model::{GateGraph, GateNode, ObjectId};
use converge_server::{
    AuthzContext, Capability, Engine, FsObjectStore, MetadataStore, ObjectKind, ObjectStore,
    SqliteMetadataStore, authorize, storage::AssociatingObjects,
};

fn admin(meta: &dyn MetadataStore) -> Result<AuthzContext> {
    authorize(meta, "alice", "repo", "scope", Capability::Admin)
}

/// Copy a locally-snapped single-file tree into the server store through
/// the associating (pinning) write path, as a client upload would.
fn upload_tree_pinned(
    scoped: &AssociatingObjects,
    ws: &Workspace,
    root: &ObjectId,
) -> Result<Vec<(ObjectKind, ObjectId)>> {
    let mut uploaded = Vec::new();
    let manifest = ws.store.get_manifest(root)?;
    for entry in &manifest.entries {
        if let ManifestEntryKind::File { blob, .. } = &entry.kind {
            let bytes = ws.store.get_blob(blob)?;
            scoped.put_bytes(ObjectKind::Blob, blob, &bytes)?;
            uploaded.push((ObjectKind::Blob, blob.clone()));
        }
    }
    let manifest_bytes = ws.store.get_manifest_bytes(root)?;
    scoped.put_bytes(ObjectKind::Manifest, root, &manifest_bytes)?;
    uploaded.push((ObjectKind::Manifest, root.clone()));
    Ok(uploaded)
}

#[test]
fn uploaded_objects_survive_gc_before_publish_then_publish_succeeds() -> Result<()> {
    let data = tempfile::tempdir()?;
    let meta = SqliteMetadataStore::open(&data.path().join("meta.sqlite"))?;
    meta.create_repo("repo")?;
    meta.set_gate_graph(
        "repo",
        &GateGraph {
            gates: vec![GateNode {
                gate_id: "intake".into(),
                name: "Intake".into(),
                upstreams: vec![],
                required_approvals: 0,
                strategy: "whole-file".into(),
                may_release: false,
            }],
        },
    )?;
    meta.upsert_user("alice")?;
    for capability in ["read", "publish", "admin"] {
        meta.add_grant("alice", "repo", "*", capability)?;
    }
    let objects = FsObjectStore::new(data.path());

    // Produce a tree locally, then "upload" it (pins each object).
    let ws_dir = tempfile::tempdir()?;
    let ws = Workspace::init(ws_dir.path(), false)?;
    std::fs::write(ws_dir.path().join("f.txt"), "important bytes")?;
    let snap = ws.create_snap(Some("s".into()))?;
    let scoped = AssociatingObjects {
        inner: &objects,
        meta: &meta,
        repo_id: "repo".into(),
    };
    let uploaded = upload_tree_pinned(&scoped, &ws, &snap.root_manifest)?;

    // An orphan blob with no pin, to prove GC is actually sweeping.
    let orphan = objects.put(ObjectKind::Blob, b"orphan junk")?;

    // GC with zero grace: mtime offers no protection, nothing references
    // the uploaded tree yet — only the pins can save it.
    let gc_engine = Engine {
        meta: &meta,
        objects: &objects,
    };
    let report = gc_engine.gc(
        &admin(&meta)?,
        false,
        "2026-07-24T00:00:00Z",
        Duration::ZERO,
    )?;
    assert!(report.swept_objects >= 1, "orphan should be swept");
    assert!(
        !objects.has(ObjectKind::Blob, &orphan),
        "unpinned orphan swept"
    );
    for (kind, id) in &uploaded {
        assert!(objects.has(*kind, id), "pinned upload survived GC");
    }

    // Publish references the surviving tree, releasing its pins.
    let publish_authz = authorize(&meta, "alice", "repo", "scope", Capability::Publish)?;
    let engine = Engine {
        meta: &meta,
        objects: &scoped,
    };
    engine.publish(
        publish_authz,
        converge_server::PublishInput {
            gate_id: "intake".into(),
            snap: snap.clone(),
            base_bundle_id: None,
            lane_id: None,
            notes: None,
        },
    )?;

    // Still reachable through the bundle after another zero-grace GC.
    gc_engine.gc(
        &admin(&meta)?,
        false,
        "2026-07-24T00:00:00Z",
        Duration::ZERO,
    )?;
    for (kind, id) in &uploaded {
        assert!(objects.has(*kind, id), "published tree still reachable");
    }
    Ok(())
}
