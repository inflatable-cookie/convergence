//! Batch 13.3 (audit H4): every cell of the doc 17 §2 merge decision
//! table, exercised directly against `merge_window`. Root-level single
//! files keep the trees trivial; the semantics under test are per-path.

use anyhow::Result;

use converge_model::{
    Manifest, ManifestEntry, ManifestEntryKind, ObjectId, SuperpositionVariantKind,
};
use converge_server::{FsObjectStore, MergeInput, ObjectKind, ObjectStore, merge_window};

fn tree(objects: &FsObjectStore, files: &[(&str, &[u8])]) -> Result<ObjectId> {
    let mut entries = Vec::new();
    for (name, content) in files {
        let blob = objects.put(ObjectKind::Blob, content)?;
        entries.push(ManifestEntry {
            name: (*name).into(),
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
    objects.put(
        ObjectKind::Manifest,
        &converge_model::encoding::encode_manifest(&manifest),
    )
}

fn manifest(objects: &FsObjectStore, id: &ObjectId) -> Result<Manifest> {
    converge_model::encoding::decode_manifest(&objects.get(ObjectKind::Manifest, id)?)
}

fn entry_kind(manifest: &Manifest, name: &str) -> Option<ManifestEntryKind> {
    manifest
        .entries
        .iter()
        .find(|e| e.name == name)
        .map(|e| e.kind.clone())
}

fn file_bytes(objects: &FsObjectStore, kind: &ManifestEntryKind) -> Result<Vec<u8>> {
    match kind {
        ManifestEntryKind::File { blob, .. } => objects.get(ObjectKind::Blob, blob),
        other => anyhow::bail!("expected plain file, got {other:?}"),
    }
}

fn input(lane: &str, base: Option<&ObjectId>, tree: &ObjectId) -> MergeInput {
    MergeInput {
        lane: lane.into(),
        base: base.cloned(),
        tree: tree.clone(),
    }
}

fn store() -> Result<(tempfile::TempDir, FsObjectStore)> {
    let dir = tempfile::tempdir()?;
    let objects = FsObjectStore::new(dir.path());
    Ok((dir, objects))
}

#[test]
fn untouched_path_passes_w_through() -> Result<()> {
    let (_dir, objects) = store()?;
    let w = tree(&objects, &[("a.txt", b"one"), ("b.txt", b"keep")])?;
    let t = tree(&objects, &[("a.txt", b"two"), ("b.txt", b"keep")])?;
    let root = merge_window(
        &objects,
        Some(&w),
        &[input("l1", Some(&w), &t)],
        "whole-file",
    )?;
    let m = manifest(&objects, &root)?;
    let kept = entry_kind(&m, "b.txt").expect("untouched path present");
    assert_eq!(file_bytes(&objects, &kept)?, b"keep");
    Ok(())
}

#[test]
fn single_modify_takes_that_value() -> Result<()> {
    let (_dir, objects) = store()?;
    let w = tree(&objects, &[("a.txt", b"one")])?;
    let t = tree(&objects, &[("a.txt", b"two")])?;
    let root = merge_window(
        &objects,
        Some(&w),
        &[input("l1", Some(&w), &t)],
        "whole-file",
    )?;
    let m = manifest(&objects, &root)?;
    let kind = entry_kind(&m, "a.txt").expect("present");
    assert_eq!(file_bytes(&objects, &kind)?, b"two");
    Ok(())
}

#[test]
fn identical_sets_dedup_to_one_value() -> Result<()> {
    let (_dir, objects) = store()?;
    let w = tree(&objects, &[("a.txt", b"one")])?;
    let t = tree(&objects, &[("a.txt", b"two")])?;
    let root = merge_window(
        &objects,
        Some(&w),
        &[input("l1", Some(&w), &t), input("l2", Some(&w), &t)],
        "whole-file",
    )?;
    let m = manifest(&objects, &root)?;
    let kind = entry_kind(&m, "a.txt").expect("present");
    assert_eq!(file_bytes(&objects, &kind)?, b"two", "no superposition");
    Ok(())
}

#[test]
fn divergent_sets_superpose_per_lane() -> Result<()> {
    let (_dir, objects) = store()?;
    let w = tree(&objects, &[("a.txt", b"one")])?;
    let t1 = tree(&objects, &[("a.txt", b"two")])?;
    let t2 = tree(&objects, &[("a.txt", b"three")])?;
    let root = merge_window(
        &objects,
        Some(&w),
        &[input("l1", Some(&w), &t1), input("l2", Some(&w), &t2)],
        "whole-file",
    )?;
    let m = manifest(&objects, &root)?;
    match entry_kind(&m, "a.txt").expect("present") {
        ManifestEntryKind::Superposition { variants } => {
            assert_eq!(variants.len(), 2);
            let sources: Vec<&str> = variants.iter().map(|v| v.source.as_str()).collect();
            assert_eq!(sources, ["l1", "l2"]);
        }
        other => panic!("expected superposition, got {other:?}"),
    }
    Ok(())
}

#[test]
fn lone_delete_removes_path() -> Result<()> {
    let (_dir, objects) = store()?;
    let w = tree(&objects, &[("a.txt", b"one"), ("b.txt", b"keep")])?;
    let t = tree(&objects, &[("b.txt", b"keep")])?;
    let root = merge_window(
        &objects,
        Some(&w),
        &[input("l1", Some(&w), &t)],
        "whole-file",
    )?;
    let m = manifest(&objects, &root)?;
    assert!(entry_kind(&m, "a.txt").is_none(), "clean deletion");
    assert!(entry_kind(&m, "b.txt").is_some());
    Ok(())
}

#[test]
fn delete_vs_modify_superposes_with_tombstone() -> Result<()> {
    let (_dir, objects) = store()?;
    let w = tree(&objects, &[("a.txt", b"one")])?;
    let deleted = tree(&objects, &[])?;
    let modified = tree(&objects, &[("a.txt", b"two")])?;
    let root = merge_window(
        &objects,
        Some(&w),
        &[
            input("deleter", Some(&w), &deleted),
            input("modifier", Some(&w), &modified),
        ],
        "whole-file",
    )?;
    let m = manifest(&objects, &root)?;
    match entry_kind(&m, "a.txt").expect("present") {
        ManifestEntryKind::Superposition { variants } => {
            assert_eq!(variants.len(), 2);
            assert!(
                variants
                    .iter()
                    .any(|v| v.kind == SuperpositionVariantKind::Tombstone
                        && v.source == "deleter")
            );
            assert!(variants.iter().any(|v| v.source == "modifier"));
        }
        other => panic!("expected superposition, got {other:?}"),
    }
    Ok(())
}

#[test]
fn unchanged_input_expresses_no_opinion() -> Result<()> {
    let (_dir, objects) = store()?;
    let w = tree(&objects, &[("a.txt", b"one")])?;
    let modified = tree(&objects, &[("a.txt", b"two")])?;
    let root = merge_window(
        &objects,
        Some(&w),
        &[
            input("bystander", Some(&w), &w),
            input("modifier", Some(&w), &modified),
        ],
        "whole-file",
    )?;
    let m = manifest(&objects, &root)?;
    let kind = entry_kind(&m, "a.txt").expect("present");
    assert_eq!(
        file_bytes(&objects, &kind)?,
        b"two",
        "bystander creates no variant"
    );
    Ok(())
}

/// The fixed cell (audit H4): a publisher who explicitly sets the path
/// back to W's value is contesting the delete, not agreeing with it.
#[test]
fn modify_back_to_w_vs_delete_superposes_instead_of_deleting() -> Result<()> {
    let (_dir, objects) = store()?;
    let w = tree(&objects, &[("a.txt", b"one")])?;
    // Both publishers built on an older base holding "nine" — concurrent
    // opinions, no causal ordering. The restorer explicitly set the file
    // back to W's "one"; the deleter removed it. (A deleter whose base
    // already contains "one" is causally newer and wins cleanly per the
    // supersession rule — that is not this cell.)
    let old_base = tree(&objects, &[("a.txt", b"nine")])?;
    let restored = tree(&objects, &[("a.txt", b"one")])?;
    let deleted = tree(&objects, &[])?;
    let root = merge_window(
        &objects,
        Some(&w),
        &[
            input("restorer", Some(&old_base), &restored),
            input("deleter", Some(&old_base), &deleted),
        ],
        "whole-file",
    )?;
    let m = manifest(&objects, &root)?;
    match entry_kind(&m, "a.txt").expect("path must survive as a superposition") {
        ManifestEntryKind::Superposition { variants } => {
            assert_eq!(variants.len(), 2);
            let restore = variants
                .iter()
                .find(|v| v.source == "restorer")
                .expect("restorer's keep opinion survives");
            match &restore.kind {
                SuperpositionVariantKind::File { blob, .. } => {
                    assert_eq!(objects.get(ObjectKind::Blob, blob)?, b"one");
                }
                other => panic!("expected file variant, got {other:?}"),
            }
            assert!(
                variants
                    .iter()
                    .any(|v| v.kind == SuperpositionVariantKind::Tombstone
                        && v.source == "deleter")
            );
        }
        other => panic!("expected superposition, got {other:?}"),
    }
    Ok(())
}

/// All opinions restating W with no deleter still collapse into W.
#[test]
fn restating_w_without_delete_collapses_cleanly() -> Result<()> {
    let (_dir, objects) = store()?;
    let w = tree(&objects, &[("a.txt", b"one")])?;
    let old_base = tree(&objects, &[("a.txt", b"nine")])?;
    let restored = tree(&objects, &[("a.txt", b"one")])?;
    let root = merge_window(
        &objects,
        Some(&w),
        &[input("restorer", Some(&old_base), &restored)],
        "whole-file",
    )?;
    let m = manifest(&objects, &root)?;
    let kind = entry_kind(&m, "a.txt").expect("present");
    assert_eq!(
        file_bytes(&objects, &kind)?,
        b"one",
        "plain entry, no superposition"
    );
    Ok(())
}
