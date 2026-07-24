//! Batch 15.4: the scale proof behind doc 17 §2. Batch 15.1 showed the
//! merge is structurally sparse on toy trees; these run it at the size
//! the audit cared about — a 50k-path tree and a 100-publish window —
//! and pin the shape of the cost curve.
//!
//! Ignored by default (like the CBOR encoding benchmark): building the
//! fixtures costs seconds, and nothing here is needed to prove
//! correctness. Run with `effigy bench` or
//! `cargo nextest run --run-ignored ignored-only`.
//!
//! Assertions are on manifest reads, never wall-clock: read counts are
//! the thing doc 17 promises and the only thing a shared CI runner can
//! measure honestly. Timings are printed as information.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use anyhow::Result;

use converge_model::{Manifest, ManifestEntry, ManifestEntryKind, ObjectId};
use converge_server::{FsObjectStore, MergeInput, ObjectKind, ObjectStore, merge_window};

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

fn put_manifest(objects: &dyn ObjectStore, mut entries: Vec<ManifestEntry>) -> Result<ObjectId> {
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    objects.put(
        ObjectKind::Manifest,
        &converge_model::encoding::encode_manifest(&Manifest {
            version: 1,
            entries,
        }),
    )
}

/// File entries carry a synthesized blob id: the merge compares blob ids
/// and never reads blob bytes, so writing 50k blobs would only measure
/// the tmpdir. Manifests — the thing under test — are stored for real.
fn file_entry(name: &str, content: &str) -> ManifestEntry {
    ManifestEntry {
        name: name.into(),
        kind: ManifestEntryKind::File {
            blob: ObjectId(blake3::hash(content.as_bytes()).to_hex().to_string()),
            mode: 0o644,
            size: content.len() as u64,
        },
    }
}

/// `dirs` × `files` paths under one root.
fn wide_tree(objects: &dyn ObjectStore, dirs: usize, files: usize) -> Result<ObjectId> {
    let mut root = Vec::with_capacity(dirs);
    for d in 0..dirs {
        let children: Vec<ManifestEntry> = (0..files)
            .map(|f| file_entry(&format!("f{f}.txt"), &format!("dir {d} file {f}")))
            .collect();
        root.push(ManifestEntry {
            name: format!("d{d:04}"),
            kind: ManifestEntryKind::Dir {
                manifest: put_manifest(objects, children)?,
            },
        });
    }
    put_manifest(objects, root)
}

fn read_manifest(objects: &dyn ObjectStore, id: &ObjectId) -> Result<Manifest> {
    converge_model::encoding::decode_manifest(&objects.get(ObjectKind::Manifest, id)?)
}

fn dir_id(objects: &dyn ObjectStore, root: &ObjectId, name: &str) -> Result<ObjectId> {
    match &read_manifest(objects, root)?
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

/// A publish that edits one file in `d{dir:04}` and nothing else.
fn tree_with_edit_in(objects: &dyn ObjectStore, base: &ObjectId, dir: usize) -> Result<ObjectId> {
    let name = format!("d{dir:04}");
    let mut entries = read_manifest(objects, base)?.entries;
    let child_id = dir_id(objects, base, &name)?;
    let mut children = read_manifest(objects, &child_id)?.entries;
    children[0] = file_entry(&children[0].name, &format!("edited in {name}"));
    let rewritten = put_manifest(objects, children)?;
    for entry in &mut entries {
        if entry.name == name {
            entry.kind = ManifestEntryKind::Dir {
                manifest: rewritten.clone(),
            };
        }
    }
    put_manifest(objects, entries)
}

fn merge_reads(store: &dyn ObjectStore, w: &ObjectId, inputs: &[MergeInput]) -> Result<usize> {
    let counting = CountingStore::new(store);
    merge_window(&counting, Some(w), inputs, "whole-file")?;
    Ok(counting.manifest_reads())
}

fn one_edit_input(w: &ObjectId, tree: ObjectId) -> MergeInput {
    MergeInput {
        lane: "lane".into(),
        base: Some(w.clone()),
        tree,
    }
}

/// Exit criterion for roadmap 015: publish cost proportional to changed
/// paths on a 50k-path tree. A 10x wider tree with the same one-file
/// edit must cost the same reads, not 10x.
#[test]
#[ignore]
fn merge_cost_tracks_changed_paths_on_a_50k_tree() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let fs = FsObjectStore::new(tmp.path());

    let build = Instant::now();
    let small = wide_tree(&fs, 50, 100)?; // 5k paths
    let large = wide_tree(&fs, 500, 100)?; // 50k paths
    eprintln!("built 5k + 50k path trees in {:?}", build.elapsed());

    let small_edit = tree_with_edit_in(&fs, &small, 0)?;
    let large_edit = tree_with_edit_in(&fs, &large, 0)?;

    let started = Instant::now();
    let small_reads = merge_reads(&fs, &small, &[one_edit_input(&small, small_edit)])?;
    let small_time = started.elapsed();
    let started = Instant::now();
    let large_reads = merge_reads(&fs, &large, &[one_edit_input(&large, large_edit)])?;
    let large_time = started.elapsed();

    eprintln!(
        "one-file edit: 5k tree {small_reads} manifest reads ({small_time:?}), \
         50k tree {large_reads} manifest reads ({large_time:?})"
    );

    assert_eq!(
        small_reads, large_reads,
        "tree size must not enter the cost (5k={small_reads}, 50k={large_reads})"
    );
    assert!(
        large_reads < 16,
        "a one-path edit should read a handful of manifests, got {large_reads}"
    );
    Ok(())
}

/// The other half of the wall: a window of many publishes. Cost must
/// track the number of *changed paths in the window*, so it grows with
/// the window and stays flat in tree size.
#[test]
#[ignore]
fn merge_cost_tracks_window_size_not_tree_size() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let fs = FsObjectStore::new(tmp.path());

    // 100 dirs on the small tree so both shapes can host a 100-publish
    // window; the point of comparison is path count, not fanout.
    let small = wide_tree(&fs, 100, 50)?; // 5k paths
    let large = wide_tree(&fs, 500, 100)?; // 50k paths

    // Each publish touches its own directory, so the window has 100
    // distinct contested paths — the worst realistic case, not 100
    // publishes of the same file.
    let window = |w: &ObjectId, count: usize| -> Result<Vec<MergeInput>> {
        (0..count)
            .map(|i| {
                Ok(MergeInput {
                    lane: format!("lane{i}"),
                    base: Some(w.clone()),
                    tree: tree_with_edit_in(&fs, w, i)?,
                })
            })
            .collect()
    };

    let one = merge_reads(&fs, &large, &window(&large, 1)?)?;
    let started = Instant::now();
    let hundred = merge_reads(&fs, &large, &window(&large, 100)?)?;
    let hundred_time = started.elapsed();
    let hundred_small = merge_reads(&fs, &small, &window(&small, 100)?)?;

    eprintln!(
        "50k tree: window of 1 = {one} reads, window of 100 = {hundred} reads \
         ({hundred_time:?}); same window on a 5k tree = {hundred_small} reads"
    );

    assert_eq!(
        hundred, hundred_small,
        "window cost must not depend on tree size (50k={hundred}, 5k={hundred_small})"
    );
    // Per-publish cost must not grow with the window. Before batch 15.4
    // memoized the fold's path walks this read 20601 — every input asking
    // every other input's base for every contested path.
    assert!(
        hundred / 100 <= one,
        "per-publish cost must stay flat as the window grows \
         (one={one}, hundred={hundred}) — growth means the fold is \
         re-walking shared structure"
    );
    Ok(())
}

/// A quiet window — every publish already contained in W — is the shape
/// a busy repo hits constantly (re-publish, no-op sync). It must cost
/// essentially nothing regardless of how big the tree is.
#[test]
#[ignore]
fn quiet_window_on_a_50k_tree_reads_almost_nothing() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let fs = FsObjectStore::new(tmp.path());
    let w = wide_tree(&fs, 500, 100)?;

    let inputs: Vec<MergeInput> = (0..50)
        .map(|i| MergeInput {
            lane: format!("lane{i}"),
            base: Some(w.clone()),
            tree: w.clone(),
        })
        .collect();

    let started = Instant::now();
    let reads = merge_reads(&fs, &w, &inputs)?;
    eprintln!(
        "50-publish quiet window on 50k paths: {reads} manifest reads ({:?})",
        started.elapsed()
    );

    assert!(
        reads <= 1,
        "identical subtree ids short-circuit every input, got {reads}"
    );
    Ok(())
}
