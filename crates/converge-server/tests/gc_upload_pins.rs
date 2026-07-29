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
    // Scopes are declared repo state (batch 14.3).
    for scope in ["scope", "resolved-scope"] {
        meta.create_scope("repo", scope, "2026-07-25T00:00:00Z")?;
    }
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
            base_candidate_id: None,
            lane_id: None,
            notes: None,
        },
    )?;

    // Still reachable through the candidate after another zero-grace GC.
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

/// The other half, found in batch 22.4: a pin is released by publishing
/// the tree it belongs to, so an upload that is never published kept its
/// pin forever. GC reported the object unreachable and declined to sweep
/// it on every run, for the life of the deployment.
#[test]
fn an_upload_that_is_never_published_stops_being_pinned() -> Result<()> {
    let data = tempfile::tempdir()?;
    let meta = SqliteMetadataStore::open(&data.path().join("meta.sqlite"))?;
    meta.create_repo("repo")?;
    meta.create_scope("repo", "scope", "2026-07-25T00:00:00Z")?;
    meta.upsert_user("alice")?;
    for capability in ["read", "publish", "admin"] {
        meta.add_grant("alice", "repo", "*", capability)?;
    }
    let objects = FsObjectStore::new(data.path());

    let ws_dir = tempfile::tempdir()?;
    let ws = Workspace::init(ws_dir.path(), false)?;
    std::fs::write(ws_dir.path().join("f.txt"), "abandoned bytes")?;
    let snap = ws.create_snap(Some("s".into()))?;
    let scoped = AssociatingObjects {
        inner: &objects,
        meta: &meta,
        repo_id: "repo".into(),
    };
    let uploaded = upload_tree_pinned(&scoped, &ws, &snap.root_manifest)?;

    let gc_engine = Engine {
        meta: &meta,
        objects: &objects,
    };
    let fresh = gc_engine.gc(
        &admin(&meta)?,
        false,
        "2026-07-24T00:00:00Z",
        Duration::ZERO,
    )?;
    assert_eq!(fresh.expired_pins, 0, "a pin made moments ago is not stale");
    for (kind, id) in &uploaded {
        assert!(objects.has(*kind, id), "a fresh upload must survive");
    }

    // Age every pin past the grace. A cutoff in the future makes them
    // all older than it, which is what a day of wall clock would do.
    let expired = meta.sweep_stale_pins(converge_server::gc::unix_now() + 1)?;
    assert!(expired >= 1, "stale pins were not cleared");

    // Now nothing protects the objects, and GC reclaims what it could
    // previously only keep reporting as unreachable.
    let after = gc_engine.gc(
        &admin(&meta)?,
        false,
        "2026-07-24T00:00:00Z",
        Duration::ZERO,
    )?;
    assert!(after.swept_objects >= 1, "abandoned upload still not swept");
    for (kind, id) in &uploaded {
        assert!(
            !objects.has(*kind, id),
            "an abandoned upload is still leaking storage"
        );
    }
    Ok(())
}

/// A dry run must leave the deployment exactly as it found it, including
/// the pin table -- otherwise "show me what you would do" would itself
/// change what happens next.
#[test]
fn a_dry_run_does_not_clear_pins() -> Result<()> {
    let data = tempfile::tempdir()?;
    let meta = SqliteMetadataStore::open(&data.path().join("meta.sqlite"))?;
    meta.create_repo("repo")?;
    meta.create_scope("repo", "scope", "2026-07-25T00:00:00Z")?;
    meta.upsert_user("alice")?;
    for capability in ["read", "publish", "admin"] {
        meta.add_grant("alice", "repo", "*", capability)?;
    }
    let objects = FsObjectStore::new(data.path());
    let id = objects.put(ObjectKind::Blob, b"pinned bytes")?;
    meta.pin_object("repo", ObjectKind::Blob, &id)?;

    let gc_engine = Engine {
        meta: &meta,
        objects: &objects,
    };
    let report = gc_engine.gc(&admin(&meta)?, true, "2026-07-24T00:00:00Z", Duration::ZERO)?;
    assert_eq!(report.expired_pins, 0, "a dry run cleared pins");
    assert!(
        meta.is_object_pinned(ObjectKind::Blob, &id, 0)?,
        "the pin did not survive a dry run"
    );
    Ok(())
}
