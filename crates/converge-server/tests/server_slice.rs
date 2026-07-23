use anyhow::Result;

use converge_model::{
    BundleStatus, GateGraph, GateNode, Manifest, ManifestEntry, ManifestEntryKind, ObjectId,
};
use converge_server::{
    Capability, Engine, FsObjectStore, MetadataStore, ObjectKind, ObjectStore, PublishInput,
    SqliteMetadataStore, authorize,
};

struct Fixture {
    meta: SqliteMetadataStore,
    objects: FsObjectStore,
    _tmp: tempfile::TempDir,
}

fn fixture() -> Result<Fixture> {
    let tmp = tempfile::tempdir()?;
    let meta = SqliteMetadataStore::open_in_memory()?;
    let objects = FsObjectStore::new(tmp.path());

    meta.upsert_user("alice")?;
    meta.upsert_user("bob")?;
    meta.create_repo("repo")?;
    meta.set_gate_graph(
        "repo",
        &GateGraph {
            gates: vec![
                GateNode {
                    gate_id: "intake".into(),
                    name: "Intake".into(),
                    upstreams: vec![],
                    required_approvals: 1,
                },
                GateNode {
                    gate_id: "main".into(),
                    name: "Main".into(),
                    upstreams: vec!["intake".into()],
                    required_approvals: 0,
                },
            ],
        },
    )?;
    meta.add_grant("alice", "repo", "*", "publish")?;
    meta.add_grant("alice", "repo", "*", "approve")?;
    meta.add_grant("alice", "repo", "*", "promote")?;
    Ok(Fixture {
        meta,
        objects,
        _tmp: tmp,
    })
}

fn put_file_manifest(objects: &FsObjectStore, name: &str, content: &[u8]) -> Result<ObjectId> {
    let blob = objects.put(ObjectKind::Blob, content)?;
    let manifest = Manifest {
        version: 1,
        entries: vec![ManifestEntry {
            name: name.into(),
            kind: ManifestEntryKind::File {
                blob,
                mode: 0o644,
                size: content.len() as u64,
            },
        }],
    };
    objects.put(ObjectKind::Manifest, &serde_json::to_vec(&manifest)?)
}

fn publish(
    fx: &Fixture,
    subject: &str,
    lane: &str,
    snap: &str,
    root: ObjectId,
) -> Result<converge_server::StoredBundle> {
    let engine = Engine {
        meta: &fx.meta,
        objects: &fx.objects,
    };
    let authz = authorize(&fx.meta, subject, "repo", "scope", Capability::Publish)?;
    engine.publish(
        authz,
        PublishInput {
            gate_id: "intake".into(),
            snap_id: snap.into(),
            root_manifest: root,
            lane_id: lane.into(),
            notes: None,
        },
    )
}

#[test]
fn authz_denied_without_grant() -> Result<()> {
    let fx = fixture()?;
    let err = authorize(&fx.meta, "bob", "repo", "scope", Capability::Publish).unwrap_err();
    assert!(err.to_string().contains("authorization denied"));
    Ok(())
}

#[test]
fn divergent_publishes_produce_superposition_bundle() -> Result<()> {
    let fx = fixture()?;
    let root_a = put_file_manifest(&fx.objects, "config.txt", b"lane a version")?;
    let root_b = put_file_manifest(&fx.objects, "config.txt", b"lane b version")?;

    publish(&fx, "alice", "lane-a", "snap-a", root_a)?;
    let bundle = publish(&fx, "alice", "lane-b", "snap-b", root_b)?;

    assert_eq!(
        bundle.status,
        BundleStatus::Ready { promotable: false },
        "superposed bundle must not be promotable"
    );
    let root = bundle.root_manifest.expect("merged root");
    let manifest: Manifest = serde_json::from_slice(&fx.objects.get(ObjectKind::Manifest, &root)?)?;
    match &manifest.entries[0].kind {
        ManifestEntryKind::Superposition { variants } => {
            assert_eq!(variants.len(), 2);
            let sources: Vec<&str> = variants.iter().map(|v| v.source.as_str()).collect();
            assert_eq!(sources, vec!["lane-a", "lane-b"]);
        }
        other => panic!("expected superposition, got {other:?}"),
    }
    Ok(())
}

#[test]
fn bundle_build_is_deterministic() -> Result<()> {
    let build = |fx: &Fixture| -> Result<(ObjectId, String)> {
        let root_a = put_file_manifest(&fx.objects, "config.txt", b"aaa")?;
        let root_b = put_file_manifest(&fx.objects, "config.txt", b"bbb")?;
        publish(fx, "alice", "lane-a", "snap-a", root_a)?;
        let bundle = publish(fx, "alice", "lane-b", "snap-b", root_b)?;
        Ok((bundle.root_manifest.expect("root"), bundle.gate_id))
    };
    let (root1, _) = build(&fixture()?)?;
    let (root2, _) = build(&fixture()?)?;
    assert_eq!(root1, root2, "same inputs, same merged manifest");
    Ok(())
}

#[test]
fn identical_publishes_pass_through_and_promote_with_approval() -> Result<()> {
    let fx = fixture()?;
    let root = put_file_manifest(&fx.objects, "app.txt", b"same content")?;
    publish(&fx, "alice", "lane-a", "snap-a", root.clone())?;
    let bundle = publish(&fx, "alice", "lane-b", "snap-b", root.clone())?;

    assert_eq!(bundle.status, BundleStatus::Ready { promotable: true });
    assert_eq!(bundle.root_manifest.as_ref(), Some(&root), "pass-through");

    let engine = Engine {
        meta: &fx.meta,
        objects: &fx.objects,
    };

    // Blocked: intake requires 1 approval.
    let authz = authorize(&fx.meta, "alice", "repo", "scope", Capability::Promote)?;
    let err = engine
        .promote(authz, &bundle.bundle_id, "main")
        .unwrap_err();
    assert!(err.to_string().contains("required approvals"));

    let authz = authorize(&fx.meta, "alice", "repo", "scope", Capability::Approve)?;
    engine.approve(authz, &bundle.bundle_id)?;

    let authz = authorize(&fx.meta, "alice", "repo", "scope", Capability::Promote)?;
    engine.promote(authz, &bundle.bundle_id, "main")?;
    assert_eq!(fx.meta.count_approvals(&bundle.bundle_id)?, 1);

    let promotions = fx.meta.list_promotions(&bundle.bundle_id)?;
    assert_eq!(promotions.len(), 1);
    assert_eq!(promotions[0].0, "intake");
    assert_eq!(promotions[0].1, "main");
    Ok(())
}

#[test]
fn superposed_bundle_cannot_promote() -> Result<()> {
    let fx = fixture()?;
    let root_a = put_file_manifest(&fx.objects, "x", b"1")?;
    let root_b = put_file_manifest(&fx.objects, "x", b"2")?;
    publish(&fx, "alice", "lane-a", "snap-a", root_a)?;
    let bundle = publish(&fx, "alice", "lane-b", "snap-b", root_b)?;

    let engine = Engine {
        meta: &fx.meta,
        objects: &fx.objects,
    };
    let authz = authorize(&fx.meta, "alice", "repo", "scope", Capability::Promote)?;
    let err = engine
        .promote(authz, &bundle.bundle_id, "main")
        .unwrap_err();
    assert!(err.to_string().contains("unresolved superpositions"));
    Ok(())
}

#[test]
fn capability_mismatch_rejected_even_with_other_grants() -> Result<()> {
    let fx = fixture()?;
    let root = put_file_manifest(&fx.objects, "y", b"z")?;
    publish(&fx, "alice", "lane-a", "snap-a", root)?;

    // A publish-capability context cannot drive promote.
    let engine = Engine {
        meta: &fx.meta,
        objects: &fx.objects,
    };
    let authz = authorize(&fx.meta, "alice", "repo", "scope", Capability::Publish)?;
    let err = engine.promote(authz, "whatever", "main").unwrap_err();
    assert!(err.to_string().contains("operation needs promote"));
    Ok(())
}
