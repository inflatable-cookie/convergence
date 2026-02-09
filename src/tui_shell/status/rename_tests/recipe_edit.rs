use super::fixtures::{manifest_with_chunked_file, new_recipe, old_recipe, recipe_ids};
use super::*;

#[test]
fn detects_rename_with_small_edit_for_recipes() -> anyhow::Result<()> {
    let (_dir, store) = setup_store()?;

    let ids = recipe_ids();
    let rid_old = store.put_recipe(&old_recipe(&ids))?;
    let rid_new = store.put_recipe(&new_recipe(&ids))?;

    let base_manifest = manifest_with_chunked_file("a.bin", &rid_old, 40);
    let base_root = store.put_manifest(&base_manifest)?;

    let cur_manifest = manifest_with_chunked_file("b.bin", &rid_new, 40);
    let cur_root = store.put_manifest(&cur_manifest)?;
    let mut cur_manifests = std::collections::HashMap::new();
    cur_manifests.insert(cur_root.clone(), cur_manifest);

    let out = diff_trees_with_renames(
        &store,
        Some(&base_root),
        &cur_root,
        &cur_manifests,
        None,
        default_chunk_size_bytes(),
    )?;
    assert_eq!(out.len(), 1);
    match &out[0] {
        StatusChange::Renamed { from, to, modified } => {
            assert_eq!(from, "a.bin");
            assert_eq!(to, "b.bin");
            assert!(*modified);
        }
        other => anyhow::bail!("unexpected diff: {:?}", other),
    }

    Ok(())
}
