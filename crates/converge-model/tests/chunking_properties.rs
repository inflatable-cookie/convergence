use std::collections::HashSet;

use converge_model::{ChunkParams, chunk_data};

// Deterministic pseudo-random bytes (no external RNG dep; Date/random-free).
fn pseudo_random_bytes(len: usize, mut seed: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(len);
    while out.len() < len {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        out.extend_from_slice(&seed.to_le_bytes());
    }
    out.truncate(len);
    out
}

fn chunk_ids(data: &[u8]) -> Vec<String> {
    let (recipe, _) = chunk_data(data, ChunkParams::default());
    recipe.chunks.iter().map(|c| c.blob.0.clone()).collect()
}

#[test]
fn identical_content_identical_chunks() {
    let data = pseudo_random_bytes(16 * 1024 * 1024, 42);
    assert_eq!(chunk_ids(&data), chunk_ids(&data.clone()));
}

#[test]
fn recipe_reassembles_exactly() {
    let data = pseudo_random_bytes(10 * 1024 * 1024 + 137, 7);
    let (recipe, blobs) = chunk_data(&data, ChunkParams::default());
    assert_eq!(recipe.size, data.len() as u64);
    let reassembled: Vec<u8> = blobs.iter().flat_map(|(_, s)| s.iter().copied()).collect();
    assert_eq!(reassembled, data);
    let sum: u64 = recipe.chunks.iter().map(|c| c.size as u64).sum();
    assert_eq!(sum, recipe.size);
}

#[test]
fn insert_edit_shifts_bounded_chunks() {
    let data = pseudo_random_bytes(16 * 1024 * 1024, 99);
    let mut edited = data.clone();
    // Insert 1 KiB in the middle: with CDC, chunks after the edit re-align.
    let mid = edited.len() / 2;
    let insert = pseudo_random_bytes(1024, 5);
    edited.splice(mid..mid, insert);

    let before: HashSet<_> = chunk_ids(&data).into_iter().collect();
    let after: HashSet<_> = chunk_ids(&edited).into_iter().collect();
    let changed = after.difference(&before).count();

    // Fixed-block chunking would change every chunk after the midpoint
    // (~8 of ~16). CDC must keep the damage local.
    assert!(changed <= 4, "insert changed {changed} chunks");
    assert!(before.intersection(&after).count() >= before.len() - 5);
}

#[test]
fn delete_edit_shifts_bounded_chunks() {
    let data = pseudo_random_bytes(16 * 1024 * 1024, 123);
    let mut edited = data.clone();
    let mid = edited.len() / 2;
    edited.drain(mid..mid + 2048);

    let before: HashSet<_> = chunk_ids(&data).into_iter().collect();
    let after: HashSet<_> = chunk_ids(&edited).into_iter().collect();
    let changed = after.difference(&before).count();
    assert!(changed <= 4, "delete changed {changed} chunks");
}

#[test]
fn params_recorded_in_recipe_header() {
    let params = ChunkParams {
        min_size: 64 * 1024,
        avg_size: 256 * 1024,
        max_size: 1024 * 1024,
    };
    let (recipe, _) = chunk_data(&pseudo_random_bytes(2 * 1024 * 1024, 1), params);
    assert_eq!(recipe.params, Some(params));
    assert_eq!(recipe.version, converge_model::chunk_recipe_version());
}

/// Dev-only comparison (run with `cargo test -- --ignored`): canonical
/// CBOR vs JSON on a synthetic 10k-entry manifest.
#[test]
#[ignore]
fn encoding_benchmark_10k_entries() {
    use converge_model::{Manifest, ManifestEntry, ManifestEntryKind, ObjectId};
    let manifest = Manifest {
        version: 1,
        entries: (0..10_000)
            .map(|i| ManifestEntry {
                name: format!("file-{i:05}.bin"),
                kind: ManifestEntryKind::File {
                    blob: ObjectId(format!("{i:064x}")),
                    mode: 0o644,
                    size: i as u64,
                },
            })
            .collect(),
    };
    let start = std::time::Instant::now();
    let cbor = converge_model::encoding::encode_manifest(&manifest);
    let cbor_encode = start.elapsed();
    let start = std::time::Instant::now();
    let json = serde_json::to_vec(&manifest).unwrap();
    let json_encode = start.elapsed();
    let start = std::time::Instant::now();
    let _ = converge_model::encoding::decode_manifest(&cbor).unwrap();
    let cbor_decode = start.elapsed();
    eprintln!(
        "10k entries: cbor {} bytes ({cbor_encode:?} enc, {cbor_decode:?} dec) vs json {} bytes ({json_encode:?} enc)",
        cbor.len(),
        json.len()
    );
    assert!(cbor.len() < json.len(), "canonical form is smaller");
}
