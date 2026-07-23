use std::fs;

use converge_client::model::ManifestEntryKind;
use converge_client::workspace::Workspace;

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

#[test]
fn large_file_snap_produces_v2_cdc_recipe_and_restores_exactly() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let root = dir.path();
    let ws = Workspace::init(root, false)?;

    let big = pseudo_random_bytes(12 * 1024 * 1024, 77);
    fs::write(root.join("asset.bin"), &big)?;
    fs::write(root.join("small.txt"), b"hello")?;

    let snap = ws.create_snap(Some("cdc roundtrip".into()))?;

    let manifest = ws.store.get_manifest(&snap.root_manifest)?;
    let recipe_id = manifest
        .entries
        .iter()
        .find_map(|e| match &e.kind {
            ManifestEntryKind::FileChunks { recipe, .. } if e.name == "asset.bin" => {
                Some(recipe.clone())
            }
            _ => None,
        })
        .expect("large file should be chunked");

    let recipe = ws.store.get_recipe(&recipe_id)?;
    assert_eq!(
        recipe.version,
        converge_client::model::chunk_recipe_version()
    );
    assert!(recipe.params.is_some(), "CDC params must be in the header");
    assert!(recipe.chunks.len() > 1);

    // Sharded fanout: blob lives at objects/blobs/ab/cd/<hash>.
    let first = &recipe.chunks[0].blob;
    let h = first.as_str();
    let sharded = root
        .join(".converge/objects/blobs")
        .join(&h[..2])
        .join(&h[2..4])
        .join(h);
    assert!(sharded.is_file(), "expected sharded path {sharded:?}");

    // Restore into a fresh directory and compare bytes.
    let out = tempfile::tempdir()?;
    ws.materialize_snap_to(&snap.id, out.path(), true)?;
    assert_eq!(fs::read(out.path().join("asset.bin"))?, big);
    assert_eq!(fs::read(out.path().join("small.txt"))?, b"hello");
    Ok(())
}

#[test]
fn identical_content_dedupes_across_files() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let root = dir.path();
    let ws = Workspace::init(root, false)?;

    let data = pseudo_random_bytes(9 * 1024 * 1024, 3);
    fs::write(root.join("a.bin"), &data)?;
    fs::write(root.join("b.bin"), &data)?;
    let snap = ws.create_snap(None)?;

    let manifest = ws.store.get_manifest(&snap.root_manifest)?;
    let recipes: Vec<_> = manifest
        .entries
        .iter()
        .filter_map(|e| match &e.kind {
            ManifestEntryKind::FileChunks { recipe, .. } => Some(recipe.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(recipes.len(), 2);
    assert_eq!(
        recipes[0], recipes[1],
        "identical content, identical recipe"
    );
    Ok(())
}
