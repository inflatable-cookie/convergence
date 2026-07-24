//! Doc 17 §4: text-line-merge strategy — clean line merges, true-conflict
//! superpositions, binary fallback, determinism.

use anyhow::Result;

use converge_model::{
    BundleStatus, GateGraph, GateNode, Manifest, ManifestEntry, ManifestEntryKind, ObjectId,
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
            gates: vec![GateNode {
                gate_id: "intake".into(),
                name: "Intake".into(),
                upstreams: vec![],
                required_approvals: 0,
                strategy: "text-line-merge".into(),
            }],
        },
    )?;
    meta.add_grant("alice", "repo", "*", "publish")?;
    Ok(Fixture {
        meta,
        objects,
        _tmp: tmp,
    })
}

fn put_file(fx: &Fixture, name: &str, content: &[u8]) -> Result<ObjectId> {
    let blob = fx.objects.put(ObjectKind::Blob, content)?;
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
    fx.objects
        .put(ObjectKind::Manifest, &serde_json::to_vec(&manifest)?)
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
            snap_id: snap.into(),
            root_manifest: root,
            base_bundle_id: base,
            lane_id: Some(lane.into()),
            notes: None,
        },
    )
}

fn file_bytes(fx: &Fixture, bundle: &StoredBundle, name: &str) -> Result<Vec<u8>> {
    let root = bundle.root_manifest.clone().expect("root");
    let manifest: Manifest = serde_json::from_slice(&fx.objects.get(ObjectKind::Manifest, &root)?)?;
    match &manifest
        .entries
        .iter()
        .find(|e| e.name == name)
        .expect("entry")
        .kind
    {
        ManifestEntryKind::File { blob, .. } => Ok(fx.objects.get(ObjectKind::Blob, blob)?),
        other => anyhow::bail!("expected clean file, got {other:?}"),
    }
}

const BASE: &[u8] = b"line one\nline two\nline three\nline four\nline five\n";

#[test]
fn disjoint_line_edits_merge_cleanly() -> Result<()> {
    let fx = fixture()?;
    let bundle1 = publish(
        &fx,
        "lane-0",
        "snap-0",
        put_file(&fx, "code.txt", BASE)?,
        None,
    )?;

    // B edits line one, C edits line five — disjoint hunks.
    let tree_b = put_file(
        &fx,
        "code.txt",
        b"line one EDITED\nline two\nline three\nline four\nline five\n",
    )?;
    publish(
        &fx,
        "lane-b",
        "snap-b",
        tree_b,
        Some(bundle1.bundle_id.clone()),
    )?;
    let tree_c = put_file(
        &fx,
        "code.txt",
        b"line one\nline two\nline three\nline four\nline five EDITED\n",
    )?;
    let bundle = publish(
        &fx,
        "lane-c",
        "snap-c",
        tree_c,
        Some(bundle1.bundle_id.clone()),
    )?;

    assert_eq!(
        bundle.status,
        BundleStatus::Ready { promotable: true },
        "disjoint text edits must line-merge, not superpose"
    );
    let merged = file_bytes(&fx, &bundle, "code.txt")?;
    assert_eq!(
        merged,
        b"line one EDITED\nline two\nline three\nline four\nline five EDITED\n"
    );
    assert!(
        !merged.windows(7).any(|w| w == b"<<<<<<<"),
        "no conflict markers ever"
    );
    Ok(())
}

#[test]
fn overlapping_edits_superpose_original_variants() -> Result<()> {
    let fx = fixture()?;
    let bundle1 = publish(
        &fx,
        "lane-0",
        "snap-0",
        put_file(&fx, "code.txt", BASE)?,
        None,
    )?;

    let tree_b = put_file(
        &fx,
        "code.txt",
        b"line one B\nline two\nline three\nline four\nline five\n",
    )?;
    publish(
        &fx,
        "lane-b",
        "snap-b",
        tree_b,
        Some(bundle1.bundle_id.clone()),
    )?;
    let tree_c = put_file(
        &fx,
        "code.txt",
        b"line one C\nline two\nline three\nline four\nline five\n",
    )?;
    let bundle = publish(
        &fx,
        "lane-c",
        "snap-c",
        tree_c,
        Some(bundle1.bundle_id.clone()),
    )?;

    assert_eq!(
        bundle.status,
        BundleStatus::Ready { promotable: false },
        "true conflict superposes"
    );
    let root = bundle.root_manifest.clone().expect("root");
    let manifest: Manifest = serde_json::from_slice(&fx.objects.get(ObjectKind::Manifest, &root)?)?;
    match &manifest.entries[0].kind {
        ManifestEntryKind::Superposition { variants } => {
            assert_eq!(variants.len(), 2, "original variants preserved");
        }
        other => panic!("expected superposition, got {other:?}"),
    }
    Ok(())
}

#[test]
fn binary_content_falls_back_to_whole_file() -> Result<()> {
    let fx = fixture()?;
    let base: &[u8] = b"\x00\x01\x02base";
    let bundle1 = publish(
        &fx,
        "lane-0",
        "snap-0",
        put_file(&fx, "asset.bin", base)?,
        None,
    )?;

    let tree_b = put_file(&fx, "asset.bin", b"\x00\x01\x02bbb")?;
    publish(
        &fx,
        "lane-b",
        "snap-b",
        tree_b,
        Some(bundle1.bundle_id.clone()),
    )?;
    let tree_c = put_file(&fx, "asset.bin", b"\x00\x01\x02ccc")?;
    let bundle = publish(
        &fx,
        "lane-c",
        "snap-c",
        tree_c,
        Some(bundle1.bundle_id.clone()),
    )?;

    assert_eq!(
        bundle.status,
        BundleStatus::Ready { promotable: false },
        "binary divergence superposes under text-line-merge"
    );
    Ok(())
}

#[test]
fn text_line_merge_is_deterministic() -> Result<()> {
    let run = || -> Result<(String, ObjectId)> {
        let fx = fixture()?;
        let bundle1 = publish(
            &fx,
            "lane-0",
            "snap-0",
            put_file(&fx, "code.txt", BASE)?,
            None,
        )?;
        let tree_b = put_file(
            &fx,
            "code.txt",
            b"line one EDITED\nline two\nline three\nline four\nline five\n",
        )?;
        publish(
            &fx,
            "lane-b",
            "snap-b",
            tree_b,
            Some(bundle1.bundle_id.clone()),
        )?;
        let tree_c = put_file(
            &fx,
            "code.txt",
            b"line one\nline two\nline three\nline four\nline five EDITED\n",
        )?;
        let bundle = publish(
            &fx,
            "lane-c",
            "snap-c",
            tree_c,
            Some(bundle1.bundle_id.clone()),
        )?;
        Ok((bundle.strategy.clone(), bundle.root_manifest.expect("root")))
    };
    let (strategy1, root1) = run()?;
    let (_strategy2, root2) = run()?;
    assert_eq!(strategy1, "text-line-merge");
    assert_eq!(root1, root2, "same inputs, same merged manifest");
    Ok(())
}
