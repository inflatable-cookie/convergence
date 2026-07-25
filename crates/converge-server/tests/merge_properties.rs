//! Batch 18.3: properties over generated windows, not hand-picked cases.
//!
//! The decision table (batch 13.3) pins each cell with an example. These
//! run the same fold over many generated trees and assert the invariants
//! that must hold for *every* window: determinism, idempotence, and a
//! resolution that does not depend on how the variants were ordered.
//!
//! Generation is a seeded xorshift rather than a fuzzing crate, matching
//! the chunking properties: a failure names its seed, so it reproduces
//! exactly instead of "sometimes".

use anyhow::Result;
use converge_client::model::{ResolutionDecision, VariantKey};
use converge_client::resolve::{apply_resolution, superposition_variants};
use converge_client::store::LocalStore;
use converge_model::{Manifest, ManifestEntry, ManifestEntryKind, ObjectId};
use converge_server::{FsObjectStore, MergeInput, ObjectKind, ObjectStore, merge_window};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
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

fn file(objects: &dyn ObjectStore, name: &str, content: &str) -> Result<ManifestEntry> {
    let blob = objects.put(ObjectKind::Blob, content.as_bytes())?;
    Ok(ManifestEntry {
        name: name.into(),
        kind: ManifestEntryKind::File {
            blob,
            mode: 0o100644,
            size: content.len() as u64,
        },
    })
}

/// A tree of `paths` files, some inside one subdirectory, with content
/// drawn from `rng` so different generations diverge on different paths.
fn tree(objects: &dyn ObjectStore, rng: &mut Rng, paths: usize, tag: &str) -> Result<ObjectId> {
    let mut root = Vec::new();
    let mut nested = Vec::new();
    for i in 0..paths {
        let content = format!("{tag}-{}", rng.below(3));
        if i % 3 == 0 {
            nested.push(file(objects, &format!("n{i}.txt"), &content)?);
        } else {
            root.push(file(objects, &format!("f{i}.txt"), &content)?);
        }
    }
    root.push(ManifestEntry {
        name: "sub".into(),
        kind: ManifestEntryKind::Dir {
            manifest: put_manifest(objects, nested)?,
        },
    });
    put_manifest(objects, root)
}

fn fold(objects: &dyn ObjectStore, w: &ObjectId, inputs: &[MergeInput]) -> Result<ObjectId> {
    merge_window(objects, Some(w), inputs, "whole-file")
}

fn inputs_for(w: &ObjectId, trees: &[ObjectId]) -> Vec<MergeInput> {
    trees
        .iter()
        .enumerate()
        .map(|(i, tree)| MergeInput {
            lane: format!("lane{i}"),
            base: Some(w.clone()),
            tree: tree.clone(),
        })
        .collect()
}

/// The fold is a function of its inputs: same window, same result, every
/// time. Determinism is what makes `bundle_id` meaningful — a bundle id
/// that did not pin the merged tree would be a label, not an identity.
#[test]
fn merge_is_deterministic_and_idempotent_over_generated_windows() -> Result<()> {
    for seed in [1u64, 7, 99, 4242, 987_654_321] {
        let tmp = tempfile::tempdir()?;
        let fs = FsObjectStore::new(tmp.path());
        let mut rng = Rng(seed);

        let w = tree(&fs, &mut rng, 8, "base")?;
        let trees: Vec<ObjectId> = (0..4)
            .map(|i| tree(&fs, &mut rng, 8, &format!("pub{i}")))
            .collect::<Result<_>>()?;
        let inputs = inputs_for(&w, &trees);

        let first = fold(&fs, &w, &inputs)?;
        for _ in 0..3 {
            assert_eq!(
                fold(&fs, &w, &inputs)?,
                first,
                "seed {seed}: merge is not deterministic"
            );
        }

        // Folding the result again with no new opinions is a fixed
        // point: re-building a window that nothing has changed must not
        // invent a new tree (doc 17 §3 rebuilds windows constantly).
        let empty: Vec<MergeInput> = trees
            .iter()
            .enumerate()
            .map(|(i, _)| MergeInput {
                lane: format!("lane{i}"),
                base: Some(first.clone()),
                tree: first.clone(),
            })
            .collect();
        assert_eq!(
            fold(&fs, &first, &empty)?,
            first,
            "seed {seed}: re-folding an unchanged window moved the tree"
        );
    }
    Ok(())
}

/// Resolving by *variant key* must not depend on the order the variants
/// happen to appear in. Keys carry provenance precisely so a decision
/// survives a re-merge that reorders them.
#[test]
fn resolution_by_key_is_independent_of_variant_order() -> Result<()> {
    for seed in [3u64, 31, 314, 27_182] {
        let tmp = tempfile::tempdir()?;
        let fs = FsObjectStore::new(tmp.path());
        let store = LocalStore::init(tmp.path(), true)?;
        let mut rng = Rng(seed);

        let w = tree(&fs, &mut rng, 6, "base")?;
        let a = tree(&fs, &mut rng, 6, "alice")?;
        let b = tree(&fs, &mut rng, 6, "bob")?;

        // Same two publishers, opposite input order. Lanes travel with
        // the tree, not the position — a key names provenance, so the
        // test has to keep provenance stable to be testing anything.
        let named = |lane: &str, tree: &ObjectId| MergeInput {
            lane: lane.to_string(),
            base: Some(w.clone()),
            tree: tree.clone(),
        };
        let forward = fold(&fs, &w, &[named("alice", &a), named("bob", &b)])?;
        let reverse = fold(&fs, &w, &[named("bob", &b), named("alice", &a)])?;

        // Copy both merged trees into a client store so the resolve API
        // (which reads a LocalStore) can see them.
        copy_tree(&fs, &store, &forward)?;
        copy_tree(&fs, &store, &reverse)?;

        let forward_variants = superposition_variants(&store, &forward)?;
        if forward_variants.is_empty() {
            continue; // this seed produced no divergence; nothing to prove
        }
        let reverse_variants = superposition_variants(&store, &reverse)?;
        assert_eq!(
            forward_variants.len(),
            reverse_variants.len(),
            "seed {seed}: input order changed which paths are contested"
        );

        // Decide every path by the *key* of the alice-sourced variant.
        let pick_by_key = |variants: &std::collections::BTreeMap<String, Vec<_>>| {
            variants
                .iter()
                .map(
                    |(path, vs): (&String, &Vec<converge_model::SuperpositionVariant>)| {
                        let chosen: VariantKey = vs
                            .iter()
                            .find(|v| v.source == "alice")
                            .unwrap_or(&vs[0])
                            .key();
                        (path.clone(), ResolutionDecision::Key(chosen))
                    },
                )
                .collect::<std::collections::BTreeMap<String, ResolutionDecision>>()
        };
        let forward_map: std::collections::BTreeMap<String, Vec<_>> =
            forward_variants.clone().into_iter().collect();
        let decisions = pick_by_key(&forward_map);

        let resolved_forward = apply_resolution(&store, &forward, &decisions)?;

        // The same keys applied to the reverse-ordered tree: keys name
        // content and provenance, not positions, so this must be legal
        // and must land the same content.
        let reverse_map: std::collections::BTreeMap<String, Vec<_>> =
            reverse_variants.into_iter().collect();
        let reverse_decisions: std::collections::BTreeMap<String, ResolutionDecision> = reverse_map
            .iter()
            .map(|(path, vs)| {
                let wanted = decisions.get(path).expect("same paths contested");
                let ResolutionDecision::Key(key) = wanted else {
                    unreachable!("decisions were built as keys")
                };
                let chosen = vs
                    .iter()
                    .find(|v| v.key() == *key)
                    .unwrap_or_else(|| panic!("seed {seed}: key missing after reordering"))
                    .key();
                (path.clone(), ResolutionDecision::Key(chosen))
            })
            .collect();
        let resolved_reverse = apply_resolution(&store, &reverse, &reverse_decisions)?;

        assert_eq!(
            resolved_forward, resolved_reverse,
            "seed {seed}: the same keyed decisions produced different trees"
        );
    }
    Ok(())
}

/// Copy every object reachable from `root` between stores.
fn copy_tree(from: &dyn ObjectStore, to: &LocalStore, root: &ObjectId) -> Result<()> {
    let bytes = from.get(ObjectKind::Manifest, root)?;
    to.put_manifest_bytes(root, &bytes)?;
    let manifest = converge_model::encoding::decode_manifest(&bytes)?;
    for entry in manifest.entries {
        match entry.kind {
            ManifestEntryKind::Dir { manifest } => copy_tree(from, to, &manifest)?,
            ManifestEntryKind::File { blob, .. } => {
                to.put_blob(&from.get(ObjectKind::Blob, &blob)?)?;
            }
            ManifestEntryKind::FileChunks { recipe, .. } => {
                let bytes = from.get(ObjectKind::Recipe, &recipe)?;
                to.put_recipe_bytes(&recipe, &bytes)?;
            }
            ManifestEntryKind::Superposition { variants } => {
                for variant in variants {
                    match variant.kind {
                        converge_model::SuperpositionVariantKind::File { blob, .. } => {
                            to.put_blob(&from.get(ObjectKind::Blob, &blob)?)?;
                        }
                        converge_model::SuperpositionVariantKind::Dir { manifest } => {
                            copy_tree(from, to, &manifest)?
                        }
                        _ => {}
                    }
                }
            }
            ManifestEntryKind::Symlink { .. } => {}
        }
    }
    Ok(())
}
