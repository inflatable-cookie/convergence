//! Batch 13.4: resolution/apply parity, recapture rules, locked state,
//! canonical snap identity (audit C1, C3, C2-client, L3).

use std::sync::Arc;

use anyhow::Result;

use converge_client::model::{
    Manifest, ManifestEntry, ManifestEntryKind, ObjectId, ResolutionDecision, SuperpositionVariant,
    SuperpositionVariantKind, compute_snap_id,
};
use converge_client::resolve::{apply_resolution, validate_resolution};
use converge_client::store::LocalStore;
use converge_client::workspace::Workspace;

fn put_manifest(store: &LocalStore, entries: Vec<ManifestEntry>) -> Result<ObjectId> {
    store.put_manifest(&Manifest {
        version: 1,
        entries,
    })
}

/// Audit C1: a superposition whose chosen variant is a directory can
/// contain further superpositions. Validate must require those too —
/// otherwise validate says ok and apply blows up.
#[test]
fn nested_superposition_under_dir_variant_is_required_by_validate() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let ws = Workspace::init(tmp.path(), false)?;
    let store = &ws.store;

    // inner/ holds a superposition at inner/leaf.txt
    let leaf_a = store.put_blob(b"leaf a")?;
    let leaf_b = store.put_blob(b"leaf b")?;
    let inner = put_manifest(
        store,
        vec![ManifestEntry {
            name: "leaf.txt".into(),
            kind: ManifestEntryKind::Superposition {
                variants: vec![
                    SuperpositionVariant {
                        source: "lane-a".into(),
                        kind: SuperpositionVariantKind::File {
                            blob: leaf_a,
                            mode: 0o644,
                            size: 6,
                        },
                    },
                    SuperpositionVariant {
                        source: "lane-b".into(),
                        kind: SuperpositionVariantKind::File {
                            blob: leaf_b,
                            mode: 0o644,
                            size: 6,
                        },
                    },
                ],
            },
        }],
    )?;
    // The root superposes "sub" between that dir and a plain file.
    let flat = store.put_blob(b"flat")?;
    let root = put_manifest(
        store,
        vec![ManifestEntry {
            name: "sub".into(),
            kind: ManifestEntryKind::Superposition {
                variants: vec![
                    SuperpositionVariant {
                        source: "lane-dir".into(),
                        kind: SuperpositionVariantKind::Dir { manifest: inner },
                    },
                    SuperpositionVariant {
                        source: "lane-flat".into(),
                        kind: SuperpositionVariantKind::File {
                            blob: flat,
                            mode: 0o644,
                            size: 4,
                        },
                    },
                ],
            },
        }],
    )?;

    // Deciding only the outer path, in favour of the directory, is not a
    // complete resolution: the nested path must be reported missing.
    let decisions =
        std::collections::BTreeMap::from([("sub".to_string(), ResolutionDecision::Index(0))]);
    let report = validate_resolution(store, &root, &decisions)?;
    assert!(!report.ok, "nested superposition must fail validation");
    assert_eq!(report.missing, vec!["sub/leaf.txt".to_string()]);
    assert!(
        apply_resolution(store, &root, &decisions).is_err(),
        "apply agrees with validate"
    );

    // Deciding both paths validates and applies.
    let decisions = std::collections::BTreeMap::from([
        ("sub".to_string(), ResolutionDecision::Index(0)),
        ("sub/leaf.txt".to_string(), ResolutionDecision::Index(1)),
    ]);
    let report = validate_resolution(store, &root, &decisions)?;
    assert!(report.ok, "complete resolution: {report:?}");
    let resolved = apply_resolution(store, &root, &decisions)?;
    let manifest = store.get_manifest(&resolved)?;
    match &manifest.entries[0].kind {
        ManifestEntryKind::Dir { manifest } => {
            let inner = store.get_manifest(manifest)?;
            match &inner.entries[0].kind {
                ManifestEntryKind::File { blob, .. } => {
                    assert_eq!(store.get_blob(blob)?, b"leaf b");
                }
                other => panic!("expected resolved file, got {other:?}"),
            }
        }
        other => panic!("expected dir, got {other:?}"),
    }

    // Choosing the flat variant instead makes the nested path irrelevant.
    let decisions =
        std::collections::BTreeMap::from([("sub".to_string(), ResolutionDecision::Index(1))]);
    let report = validate_resolution(store, &root, &decisions)?;
    assert!(
        report.ok,
        "flat variant needs no nested decision: {report:?}"
    );
    apply_resolution(store, &root, &decisions)?;
    Ok(())
}

/// Audit C3: an explicit message on an unchanged tree is intent, not a
/// no-op — it lands on the head record instead of being dropped, and
/// creates no phantom lineage node (doc 17 §1: messages are metadata).
#[test]
fn message_bearing_recapture_keeps_the_message() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let ws = Workspace::init(tmp.path(), false)?;
    std::fs::write(tmp.path().join("a.txt"), "one")?;

    let first = ws.create_snap(None)?;
    assert_eq!(first.message, None);

    let marked = ws.create_snap(Some("release candidate".into()))?;
    assert_eq!(marked.id, first.id, "no new lineage node");
    assert_eq!(marked.message.as_deref(), Some("release candidate"));
    assert_eq!(
        ws.store.get_snap(&first.id)?.message.as_deref(),
        Some("release candidate"),
        "message persisted, not just returned"
    );
    assert_eq!(ws.store.list_snaps()?.len(), 1);

    // A bare recapture afterwards leaves the message alone.
    let bare = ws.create_snap(None)?;
    assert_eq!(bare.message.as_deref(), Some("release candidate"));
    Ok(())
}

/// Audit C3: dedup also applies with no HEAD — repeated captures of an
/// identical parentless tree must not pile up records.
#[test]
fn recapture_dedups_without_head() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let ws = Workspace::init(tmp.path(), false)?;
    std::fs::write(tmp.path().join("a.txt"), "one")?;

    let first = ws.create_snap(None)?;
    ws.store.set_head(None)?;
    let second = ws.create_snap(None)?;

    assert_eq!(second.id, first.id, "same parentless tree dedups");
    assert_eq!(ws.store.list_snaps()?.len(), 1, "no duplicate record");
    assert_eq!(ws.store.get_head()?.as_deref(), Some(first.id.as_str()));
    Ok(())
}

/// Audit C3: two records can share an id while carrying different
/// metadata; the stored one wins rather than being clobbered.
#[test]
fn put_snap_preserves_the_first_writer() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let ws = Workspace::init(tmp.path(), false)?;
    std::fs::write(tmp.path().join("a.txt"), "one")?;
    let snap = ws.create_snap(Some("original".into()))?;

    let mut impostor = snap.clone();
    impostor.message = Some("overwritten".into());
    impostor.trigger = "auto".into();
    ws.store.put_snap(&impostor)?;
    assert_eq!(
        ws.store.get_snap(&snap.id)?.message.as_deref(),
        Some("original"),
        "put_snap must not discard the stored record"
    );

    // Deliberate edits still work.
    ws.store.update_snap_message(&snap.id, Some("edited"))?;
    assert_eq!(
        ws.store.get_snap(&snap.id)?.message.as_deref(),
        Some("edited")
    );
    Ok(())
}

/// Audit C2 (client): concurrent read-modify-write of state.json must
/// not lose updates.
#[test]
fn concurrent_state_mutations_all_land() -> Result<()> {
    const WRITERS: usize = 8;
    let tmp = tempfile::tempdir()?;
    let ws = Workspace::init(tmp.path(), false)?;
    let store = Arc::new(ws.store);

    let mut handles = Vec::new();
    for i in 0..WRITERS {
        let store = store.clone();
        handles.push(std::thread::spawn(move || -> Result<()> {
            store.set_lane_sync(
                &format!("lane-{i}"),
                &format!("snap-{i}"),
                "2026-07-25T00:00:00Z",
            )
        }));
    }
    for handle in handles {
        handle.join().expect("writer thread")?;
    }

    let state = store.read_state()?;
    assert_eq!(
        state.lane_sync.len(),
        WRITERS,
        "every concurrent update survived"
    );
    Ok(())
}

/// Audit L3: parent lists must not be able to collide across different
/// boundaries once joined.
#[test]
fn snap_id_parents_are_length_prefixed() -> Result<()> {
    let root = ObjectId("aa".repeat(32));
    let split = compute_snap_id(&root, &["ab".to_string(), "cd".to_string()], None);
    let joined = compute_snap_id(&root, &["ab,cd".to_string()], None);
    assert_ne!(split, joined, "parent boundaries must be canonical");

    // Empty-parent shapes stay distinct too.
    let none = compute_snap_id(&root, &[], None);
    let one_empty = compute_snap_id(&root, &[String::new()], None);
    assert_ne!(none, one_empty);
    Ok(())
}
