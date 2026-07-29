//! Batch 12.1 (audit D1, D2): restore never destroys the workspace
//! before the target fully materializes, and hostile manifest names /
//! symlink targets are refused.

use anyhow::Result;

use converge_client::model::{
    Manifest, ManifestEntry, ManifestEntryKind, SnapRecord, SuperpositionVariant,
    SuperpositionVariantKind,
};
use converge_client::workspace::Workspace;
use converge_model::{ObjectId, compute_snap_id};

/// Store a hand-built manifest as a snap and return its id.
fn snap_for_root(ws: &Workspace, root: ObjectId) -> Result<String> {
    let id = compute_snap_id(&root, &[], None);
    ws.store.put_snap(&SnapRecord {
        version: 2,
        id: id.clone(),
        created_at: "2026-07-24T00:00:00Z".into(),
        root_manifest: root,
        parents: vec![],
        derived_from_candidate: None,
        message: Some("hostile".into()),
        trigger: "explicit".into(),
        stats: Default::default(),
    })?;
    Ok(id)
}

#[test]
fn restore_to_superposed_snap_preserves_the_workspace() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let ws = Workspace::init(dir.path(), false)?;
    std::fs::write(dir.path().join("keep.txt"), "precious work")?;
    ws.create_snap(Some("base".into()))?;

    // A snap whose tree carries a superposition cannot materialize.
    let blob = ws.store.put_blob(b"variant\n")?;
    let superposed = ws.store.put_manifest(&Manifest {
        version: 1,
        entries: vec![ManifestEntry {
            name: "conflict.txt".into(),
            kind: ManifestEntryKind::Superposition {
                variants: vec![
                    SuperpositionVariant {
                        source: "a".into(),
                        kind: SuperpositionVariantKind::File {
                            blob: blob.clone(),
                            mode: 0o100644,
                            size: 8,
                        },
                    },
                    SuperpositionVariant {
                        source: "b".into(),
                        kind: SuperpositionVariantKind::Tombstone,
                    },
                ],
            },
        }],
    })?;
    let snap_id = snap_for_root(&ws, superposed)?;

    let err = ws.restore_snap(&snap_id, true).unwrap_err();
    assert!(format!("{err:#}").contains("superposition"));
    // The workspace is untouched: precious work survives.
    assert_eq!(
        std::fs::read_to_string(dir.path().join("keep.txt"))?,
        "precious work"
    );
    assert!(!dir.path().join("conflict.txt").exists());
    // No temp debris left behind.
    for entry in std::fs::read_dir(dir.path())? {
        let name = entry?.file_name();
        assert!(
            !name.to_string_lossy().starts_with(".converge-materialize"),
            "temp dir leaked: {name:?}"
        );
    }
    Ok(())
}

#[test]
fn restore_to_snap_with_missing_blob_preserves_the_workspace() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let ws = Workspace::init(dir.path(), false)?;
    std::fs::write(dir.path().join("keep.txt"), "precious work")?;
    ws.create_snap(Some("base".into()))?;

    // Manifest references a blob that was never stored.
    let phantom = ObjectId("ab".repeat(32));
    let broken = ws.store.put_manifest(&Manifest {
        version: 1,
        entries: vec![ManifestEntry {
            name: "ghost.txt".into(),
            kind: ManifestEntryKind::File {
                blob: phantom,
                mode: 0o100644,
                size: 3,
            },
        }],
    })?;
    let snap_id = snap_for_root(&ws, broken)?;

    assert!(ws.restore_snap(&snap_id, true).is_err());
    assert_eq!(
        std::fs::read_to_string(dir.path().join("keep.txt"))?,
        "precious work"
    );
    Ok(())
}

#[test]
fn traversal_entry_name_is_refused() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let ws = Workspace::init(dir.path(), false)?;
    ws.create_snap(Some("base".into()))?;

    let blob = ws.store.put_blob(b"pwn\n")?;
    let evil = ws.store.put_manifest(&Manifest {
        version: 1,
        entries: vec![ManifestEntry {
            name: "../escape.txt".into(),
            kind: ManifestEntryKind::File {
                blob,
                mode: 0o100644,
                size: 4,
            },
        }],
    })?;
    let snap_id = snap_for_root(&ws, evil)?;

    let err = ws.restore_snap(&snap_id, true).unwrap_err();
    assert!(format!("{err:#}").contains("single path component"));
    assert!(!dir.path().parent().unwrap().join("escape.txt").exists());
    Ok(())
}

#[test]
fn escaping_symlink_target_is_refused() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let ws = Workspace::init(dir.path(), false)?;
    ws.create_snap(Some("base".into()))?;

    // Symlink at depth 0 pointing above the root.
    let evil = ws.store.put_manifest(&Manifest {
        version: 1,
        entries: vec![ManifestEntry {
            name: "link".into(),
            kind: ManifestEntryKind::Symlink {
                target: "../../etc/passwd".into(),
            },
        }],
    })?;
    let snap_id = snap_for_root(&ws, evil)?;

    let err = ws.restore_snap(&snap_id, true).unwrap_err();
    assert!(format!("{err:#}").contains("escapes"));
    Ok(())
}

#[test]
fn duplicate_entry_name_is_refused() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let ws = Workspace::init(dir.path(), false)?;
    ws.create_snap(Some("base".into()))?;

    let blob = ws.store.put_blob(b"x\n")?;
    let dup = ws.store.put_manifest(&Manifest {
        version: 1,
        entries: vec![
            ManifestEntry {
                name: "same".into(),
                kind: ManifestEntryKind::File {
                    blob: blob.clone(),
                    mode: 0o100644,
                    size: 2,
                },
            },
            ManifestEntry {
                name: "same".into(),
                kind: ManifestEntryKind::File {
                    blob,
                    mode: 0o100644,
                    size: 2,
                },
            },
        ],
    })?;
    let snap_id = snap_for_root(&ws, dup)?;

    let err = ws.restore_snap(&snap_id, true).unwrap_err();
    assert!(format!("{err:#}").contains("twice"));
    Ok(())
}

#[test]
fn happy_path_restore_still_works() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let ws = Workspace::init(dir.path(), false)?;
    std::fs::write(dir.path().join("a.txt"), "first")?;
    let first = ws.create_snap(Some("first".into()))?;
    std::fs::write(dir.path().join("a.txt"), "second")?;
    std::fs::write(dir.path().join("b.txt"), "new")?;
    ws.create_snap(Some("second".into()))?;

    ws.restore_snap(&first.id, true)?;
    assert_eq!(std::fs::read_to_string(dir.path().join("a.txt"))?, "first");
    assert!(!dir.path().join("b.txt").exists());
    Ok(())
}
