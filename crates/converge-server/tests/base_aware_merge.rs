//! Doc 17 §2-3 decision-table coverage: base-aware fold, supersession,
//! deletions, tombstone superpositions, window reset on promotion.

use anyhow::Result;

use converge_model::{
    BundleStatus, GateGraph, GateNode, Manifest, ManifestEntry, ManifestEntryKind, ObjectId,
    SuperpositionVariantKind,
};
use converge_server::{
    Capability, Engine, FsObjectStore, MetadataStore, ObjectKind, ObjectStore, PublishInput,
    SqliteMetadataStore, StoredBundle, authorize,
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
    meta.create_repo("repo")?;
    meta.set_gate_graph(
        "repo",
        &GateGraph {
            gates: vec![
                GateNode {
                    gate_id: "intake".into(),
                    name: "Intake".into(),
                    upstreams: vec![],
                    required_approvals: 0,
                    strategy: "whole-file".into(),
                },
                GateNode {
                    gate_id: "main".into(),
                    name: "Main".into(),
                    upstreams: vec!["intake".into()],
                    required_approvals: 0,
                    strategy: "whole-file".into(),
                },
            ],
        },
    )?;
    for capability in ["publish", "promote"] {
        meta.add_grant("alice", "repo", "*", capability)?;
    }
    Ok(Fixture {
        meta,
        objects,
        _tmp: tmp,
    })
}

fn put_tree(fx: &Fixture, files: &[(&str, &[u8])]) -> Result<ObjectId> {
    let mut entries = Vec::new();
    for (name, content) in files {
        let blob = fx.objects.put(ObjectKind::Blob, content)?;
        entries.push(ManifestEntry {
            name: name.to_string(),
            kind: ManifestEntryKind::File {
                blob,
                mode: 0o644,
                size: content.len() as u64,
            },
        });
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    let manifest = Manifest {
        version: 1,
        entries,
    };
    fx.objects
        .put(ObjectKind::Manifest, &serde_json::to_vec(&manifest)?)
}

fn test_snap_record(tag: &str, root: converge_model::ObjectId) -> converge_model::SnapRecord {
    let _ = tag;
    converge_model::SnapRecord {
        version: 2,
        id: converge_model::compute_snap_id(&root, &[], None),
        created_at: "2026-07-24T00:00:00Z".into(),
        root_manifest: root,
        parents: Vec::new(),
        derived_from_bundle: None,
        message: None,
        trigger: "explicit".into(),
        stats: converge_model::SnapStats::default(),
    }
}

fn ensure_lane(fx: &Fixture, lane: &str) {
    if fx
        .meta
        .get_lane("repo", lane)
        .expect("lane query")
        .is_none()
    {
        fx.meta
            .create_lane(&converge_model::LaneRecord {
                lane_id: lane.into(),
                repo_id: "repo".into(),
                owner: "alice".into(),
                members: vec!["bob".into()],
                visibility: "repo".into(),
                created_at: "2026-07-24T00:00:00Z".into(),
            })
            .expect("create lane");
    }
}

fn publish(
    fx: &Fixture,
    lane: &str,
    snap: &str,
    root: ObjectId,
    base: Option<String>,
) -> Result<StoredBundle> {
    let engine = Engine {
        meta: &fx.meta,
        objects: &fx.objects,
    };
    ensure_lane(fx, lane);
    let authz = authorize(&fx.meta, "alice", "repo", "scope", Capability::Publish)?;
    engine.publish(
        authz,
        PublishInput {
            gate_id: "intake".into(),
            snap: test_snap_record(snap, root),
            base_bundle_id: base,
            lane_id: Some(lane.into()),
            notes: None,
        },
    )
}

fn promote(fx: &Fixture, bundle_id: &str) -> Result<()> {
    let engine = Engine {
        meta: &fx.meta,
        objects: &fx.objects,
    };
    let authz = authorize(&fx.meta, "alice", "repo", "scope", Capability::Promote)?;
    engine.promote(authz, bundle_id, "main")
}

fn manifest_of(fx: &Fixture, bundle: &StoredBundle) -> Result<Manifest> {
    let root = bundle.root_manifest.clone().expect("root");
    Ok(serde_json::from_slice(
        &fx.objects.get(ObjectKind::Manifest, &root)?,
    )?)
}

fn entry<'m>(manifest: &'m Manifest, name: &str) -> Option<&'m ManifestEntryKind> {
    manifest
        .entries
        .iter()
        .find(|e| e.name == name)
        .map(|e| &e.kind)
}

#[test]
fn sequential_edit_supersedes_instead_of_superposing() -> Result<()> {
    let fx = fixture()?;
    let tree_a = put_tree(&fx, &[("config.txt", b"X"), ("other.txt", b"Y")])?;
    let bundle1 = publish(&fx, "lane-a", "snap-a", tree_a, None)?;
    assert_eq!(bundle1.status, BundleStatus::Ready { promotable: true });

    // B builds on bundle1 (its base contains A's X) and modifies config.
    let tree_b = put_tree(&fx, &[("config.txt", b"Z"), ("other.txt", b"Y")])?;
    let bundle2 = publish(
        &fx,
        "lane-b",
        "snap-b",
        tree_b,
        Some(bundle1.bundle_id.clone()),
    )?;

    assert_eq!(
        bundle2.status,
        BundleStatus::Ready { promotable: true },
        "sequential edit must not superpose"
    );
    let manifest = manifest_of(&fx, &bundle2)?;
    match entry(&manifest, "config.txt").expect("config present") {
        ManifestEntryKind::File { blob, .. } => {
            assert_eq!(fx.objects.get(ObjectKind::Blob, blob)?, b"Z");
        }
        other => panic!("expected clean file, got {other:?}"),
    }
    Ok(())
}

#[test]
fn untouched_publisher_never_collides() -> Result<()> {
    let fx = fixture()?;
    let tree_a = put_tree(&fx, &[("app.txt", b"code"), ("doc.txt", b"v1")])?;
    let bundle1 = publish(&fx, "lane-a", "snap-a", tree_a, None)?;

    // B publishes with base=bundle1, touching only doc.txt.
    let tree_b = put_tree(&fx, &[("app.txt", b"code"), ("doc.txt", b"v2")])?;
    let bundle2 = publish(
        &fx,
        "lane-b",
        "snap-b",
        tree_b,
        Some(bundle1.bundle_id.clone()),
    )?;

    let manifest = manifest_of(&fx, &bundle2)?;
    match entry(&manifest, "app.txt").expect("app present") {
        ManifestEntryKind::File { .. } => {}
        other => panic!("untouched path superposed: {other:?}"),
    }
    match entry(&manifest, "doc.txt").expect("doc present") {
        ManifestEntryKind::File { blob, .. } => {
            assert_eq!(fx.objects.get(ObjectKind::Blob, blob)?, b"v2");
        }
        other => panic!("expected clean file, got {other:?}"),
    }
    assert_eq!(bundle2.status, BundleStatus::Ready { promotable: true });
    Ok(())
}

#[test]
fn clean_deletion_removes_path() -> Result<()> {
    let fx = fixture()?;
    let tree_a = put_tree(&fx, &[("keep.txt", b"k"), ("gone.txt", b"g")])?;
    let bundle1 = publish(&fx, "lane-a", "snap-a", tree_a, None)?;

    let tree_b = put_tree(&fx, &[("keep.txt", b"k")])?;
    let bundle2 = publish(
        &fx,
        "lane-b",
        "snap-b",
        tree_b,
        Some(bundle1.bundle_id.clone()),
    )?;

    let manifest = manifest_of(&fx, &bundle2)?;
    assert!(
        entry(&manifest, "gone.txt").is_none(),
        "deletion propagated"
    );
    assert!(entry(&manifest, "keep.txt").is_some());
    assert_eq!(bundle2.status, BundleStatus::Ready { promotable: true });
    Ok(())
}

#[test]
fn delete_vs_modify_superposes_with_tombstone() -> Result<()> {
    let fx = fixture()?;
    let tree = put_tree(&fx, &[("contested.txt", b"original")])?;
    let bundle1 = publish(&fx, "lane-a", "snap-a", tree, None)?;
    promote(&fx, &bundle1.bundle_id)?;

    // Parallel over bundle1: B modifies, C deletes.
    let tree_b = put_tree(&fx, &[("contested.txt", b"modified")])?;
    publish(
        &fx,
        "lane-b",
        "snap-b",
        tree_b,
        Some(bundle1.bundle_id.clone()),
    )?;
    let tree_c = put_tree(&fx, &[])?;
    let bundle3 = publish(
        &fx,
        "lane-c",
        "snap-c",
        tree_c,
        Some(bundle1.bundle_id.clone()),
    )?;

    assert_eq!(bundle3.status, BundleStatus::Ready { promotable: false });
    let manifest = manifest_of(&fx, &bundle3)?;
    match entry(&manifest, "contested.txt").expect("contested present") {
        ManifestEntryKind::Superposition { variants } => {
            assert_eq!(variants.len(), 2);
            assert!(
                variants
                    .iter()
                    .any(|v| v.kind == SuperpositionVariantKind::Tombstone),
                "tombstone variant present"
            );
            assert!(
                variants
                    .iter()
                    .any(|v| matches!(v.kind, SuperpositionVariantKind::File { .. })),
                "modified variant present"
            );
        }
        other => panic!("expected superposition, got {other:?}"),
    }
    Ok(())
}

#[test]
fn promotion_resets_window_and_sets_w() -> Result<()> {
    let fx = fixture()?;
    let tree_a = put_tree(&fx, &[("a.txt", b"1")])?;
    let tree_b = put_tree(&fx, &[("a.txt", b"1"), ("b.txt", b"2")])?;
    publish(&fx, "lane-a", "snap-a", tree_a, None)?;
    let bundle2 = publish(&fx, "lane-a", "snap-b", tree_b, None)?;
    assert_eq!(bundle2.window, (1, 2), "window spans both publications");

    promote(&fx, &bundle2.bundle_id)?;

    // Post-promotion publish: window contains only the new publication and
    // folds onto the promoted bundle as W.
    let tree_c = put_tree(&fx, &[("a.txt", b"1"), ("b.txt", b"2"), ("c.txt", b"3")])?;
    let bundle3 = publish(
        &fx,
        "lane-c",
        "snap-c",
        tree_c,
        Some(bundle2.bundle_id.clone()),
    )?;

    assert_eq!(
        bundle3.inputs.len(),
        1,
        "earlier publications left the pool"
    );
    assert_eq!(bundle3.window, (3, 3));
    assert_eq!(
        bundle3.base_bundle_id.as_deref(),
        Some(bundle2.bundle_id.as_str()),
        "promoted bundle is W"
    );
    let manifest = manifest_of(&fx, &bundle3)?;
    assert!(entry(&manifest, "a.txt").is_some(), "W carried through");
    assert!(entry(&manifest, "c.txt").is_some());
    assert_eq!(bundle3.status, BundleStatus::Ready { promotable: true });
    Ok(())
}
