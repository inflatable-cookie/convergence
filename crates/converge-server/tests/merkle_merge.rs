//! Batch 15.1 (audit 2.1, 2.2): merge cost tracks *changed* paths, the
//! promise doc 17 §2 makes. Proven structurally — untouched subtrees are
//! neither read nor rewritten — rather than by wall-clock timing.

use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::Result;

use converge_model::{Manifest, ManifestEntry, ManifestEntryKind, ObjectId};
use converge_server::{FsObjectStore, MergeInput, ObjectKind, ObjectStore, merge_window};

/// Counts reads per object kind so a test can assert what the merge did
/// *not* touch.
struct CountingStore<'a> {
    inner: &'a dyn ObjectStore,
    manifest_reads: AtomicUsize,
}

impl<'a> CountingStore<'a> {
    fn new(inner: &'a dyn ObjectStore) -> Self {
        Self {
            inner,
            manifest_reads: AtomicUsize::new(0),
        }
    }
    fn manifest_reads(&self) -> usize {
        self.manifest_reads.load(Ordering::Relaxed)
    }
}

impl ObjectStore for CountingStore<'_> {
    fn put(&self, kind: ObjectKind, bytes: &[u8]) -> Result<ObjectId> {
        self.inner.put(kind, bytes)
    }
    fn put_bytes(&self, kind: ObjectKind, id: &ObjectId, bytes: &[u8]) -> Result<()> {
        self.inner.put_bytes(kind, id, bytes)
    }
    fn get(&self, kind: ObjectKind, id: &ObjectId) -> Result<Vec<u8>> {
        if kind == ObjectKind::Manifest {
            self.manifest_reads.fetch_add(1, Ordering::Relaxed);
        }
        self.inner.get(kind, id)
    }
    fn has(&self, kind: ObjectKind, id: &ObjectId) -> bool {
        self.inner.has(kind, id)
    }
    fn list(&self, kind: ObjectKind) -> Result<Vec<(ObjectId, u64, std::time::SystemTime)>> {
        self.inner.list(kind)
    }
    fn delete(&self, kind: ObjectKind, id: &ObjectId) -> Result<()> {
        self.inner.delete(kind, id)
    }
}

fn put_manifest(objects: &dyn ObjectStore, entries: Vec<ManifestEntry>) -> Result<ObjectId> {
    let mut entries = entries;
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    objects.put(
        ObjectKind::Manifest,
        &converge_model::encoding::encode_manifest(&Manifest {
            version: 1,
            entries,
        }),
    )
}

fn file_entry(objects: &dyn ObjectStore, name: &str, content: &[u8]) -> Result<ManifestEntry> {
    let blob = objects.put(ObjectKind::Blob, content)?;
    Ok(ManifestEntry {
        name: name.into(),
        kind: ManifestEntryKind::File {
            blob,
            mode: 0o644,
            size: content.len() as u64,
        },
    })
}

/// `dirs` directories of `files` files each, plus a root manifest.
fn wide_tree(objects: &dyn ObjectStore, dirs: usize, files: usize) -> Result<ObjectId> {
    let mut root = Vec::new();
    for d in 0..dirs {
        let mut children = Vec::new();
        for f in 0..files {
            children.push(file_entry(
                objects,
                &format!("f{f}.txt"),
                format!("dir {d} file {f}").as_bytes(),
            )?);
        }
        root.push(ManifestEntry {
            name: format!("d{d}"),
            kind: ManifestEntryKind::Dir {
                manifest: put_manifest(objects, children)?,
            },
        });
    }
    put_manifest(objects, root)
}

fn dir_id(objects: &dyn ObjectStore, root: &ObjectId, name: &str) -> Result<ObjectId> {
    let manifest: Manifest =
        converge_model::encoding::decode_manifest(&objects.get(ObjectKind::Manifest, root)?)?;
    match &manifest
        .entries
        .iter()
        .find(|e| e.name == name)
        .expect("directory present")
        .kind
    {
        ManifestEntryKind::Dir { manifest } => Ok(manifest.clone()),
        other => anyhow::bail!("expected dir, got {other:?}"),
    }
}

/// Change one file inside `d0`, leaving every other directory byte-identical.
fn tree_with_one_edit(objects: &dyn ObjectStore, base: &ObjectId) -> Result<ObjectId> {
    tree_with_edit_in(objects, base, 0)
}

/// The same, in an arbitrary directory, so a window can be built from
/// publishes that each touch their own subtree.
fn tree_with_edit_in(objects: &dyn ObjectStore, base: &ObjectId, dir: usize) -> Result<ObjectId> {
    let name = format!("d{dir}");
    let root: Manifest =
        converge_model::encoding::decode_manifest(&objects.get(ObjectKind::Manifest, base)?)?;
    let mut entries = root.entries.clone();
    let target = dir_id(objects, base, &name)?;
    let mut children: Manifest =
        converge_model::encoding::decode_manifest(&objects.get(ObjectKind::Manifest, &target)?)?;
    children.entries[0] = file_entry(
        objects,
        &children.entries[0].name,
        format!("edited content in {name}").as_bytes(),
    )?;
    let rewritten = put_manifest(objects, children.entries)?;
    for entry in &mut entries {
        if entry.name == name {
            entry.kind = ManifestEntryKind::Dir {
                manifest: rewritten.clone(),
            };
        }
    }
    put_manifest(objects, entries)
}

#[test]
fn untouched_subtrees_are_reused_not_rebuilt() -> Result<()> {
    const DIRS: usize = 20;
    let tmp = tempfile::tempdir()?;
    let fs = FsObjectStore::new(tmp.path());

    let w = wide_tree(&fs, DIRS, 25)?;
    let edited = tree_with_one_edit(&fs, &w)?;

    let merged = merge_window(
        &fs,
        Some(&w),
        &[MergeInput {
            lane: "lane".into(),
            base: Some(w.clone()),
            tree: edited.clone(),
        }],
        "whole-file",
    )?;

    // Every directory the publisher did not touch keeps W's manifest id:
    // nothing was re-hashed or re-stored for it.
    for d in 1..DIRS {
        let name = format!("d{d}");
        assert_eq!(
            dir_id(&fs, &merged, &name)?,
            dir_id(&fs, &w, &name)?,
            "{name} must be reused from W"
        );
    }
    // The edited directory did change, and matches what was published.
    assert_ne!(dir_id(&fs, &merged, "d0")?, dir_id(&fs, &w, "d0")?);
    assert_eq!(dir_id(&fs, &merged, "d0")?, dir_id(&fs, &edited, "d0")?);
    Ok(())
}

#[test]
fn merge_reads_scale_with_changed_paths_not_tree_size() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let fs = FsObjectStore::new(tmp.path());

    // Same single-file edit against a small tree and a 10x wider one.
    let small_w = wide_tree(&fs, 5, 25)?;
    let small_edit = tree_with_one_edit(&fs, &small_w)?;
    let wide_w = wide_tree(&fs, 50, 25)?;
    let wide_edit = tree_with_one_edit(&fs, &wide_w)?;

    let measure = |w: &ObjectId, tree: &ObjectId| -> Result<usize> {
        let counting = CountingStore::new(&fs);
        merge_window(
            &counting,
            Some(w),
            &[MergeInput {
                lane: "lane".into(),
                base: Some(w.clone()),
                tree: tree.clone(),
            }],
            "whole-file",
        )?;
        Ok(counting.manifest_reads())
    };

    let small = measure(&small_w, &small_edit)?;
    let wide = measure(&wide_w, &wide_edit)?;

    // A flatten-everything merge would read every directory of both
    // trees, so the wide case would cost ~10x the small one. Bounded by
    // changed paths, the two are the same handful of manifests.
    assert_eq!(
        small, wide,
        "one-file edit costs the same on a 5-dir and a 50-dir tree \
         (small={small}, wide={wide})"
    );
    assert!(
        wide < 20,
        "reads stay proportional to the changed path, got {wide}"
    );
    Ok(())
}

#[test]
fn identical_tree_and_base_reads_almost_nothing() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let fs = FsObjectStore::new(tmp.path());
    let w = wide_tree(&fs, 30, 25)?;

    let counting = CountingStore::new(&fs);
    let merged = merge_window(
        &counting,
        Some(&w),
        &[MergeInput {
            lane: "lane".into(),
            base: Some(w.clone()),
            tree: w.clone(),
        }],
        "whole-file",
    )?;

    assert_eq!(merged, w, "no opinions, no change");
    assert!(
        counting.manifest_reads() <= 1,
        "equal subtree ids short-circuit the whole walk, got {}",
        counting.manifest_reads()
    );
    Ok(())
}

/// Fast guard for the quadratic the 15.4 benchmark caught: a window of
/// publishes must cost a flat number of reads *per publish*, not one walk
/// per (publish, contested path) pair. Cheap enough to run always — the
/// scale benchmark that found it is ignored by default.
#[test]
fn window_cost_stays_flat_per_publish() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let fs = FsObjectStore::new(tmp.path());
    let w = wide_tree(&fs, 20, 10)?;

    let measure = |count: usize| -> Result<usize> {
        let inputs: Vec<MergeInput> = (0..count)
            .map(|i| {
                Ok(MergeInput {
                    lane: format!("lane{i}"),
                    base: Some(w.clone()),
                    tree: tree_with_edit_in(&fs, &w, i)?,
                })
            })
            .collect::<Result<_>>()?;
        let counting = CountingStore::new(&fs);
        merge_window(&counting, Some(&w), &inputs, "whole-file")?;
        Ok(counting.manifest_reads())
    };

    let one = measure(1)?;
    let many = measure(16)?;
    assert!(
        many / 16 <= one,
        "16 publishes must not cost more per publish than 1 \
         (one={one}, many={many})"
    );
    Ok(())
}
