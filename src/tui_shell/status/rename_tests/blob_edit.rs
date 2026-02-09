use super::*;

#[test]
fn detects_rename_with_small_edit_for_blobs() -> anyhow::Result<()> {
    let (_dir, store) = setup_store()?;

    let blob_old = store.put_blob(b"hello world\n")?;
    let blob_new = store.put_blob(b"hello world!\n")?;

    let base_manifest = manifest_with_file("a.txt", &blob_old, 12);
    let base_root = store.put_manifest(&base_manifest)?;

    let cur_manifest = manifest_with_file("b.txt", &blob_new, 13);
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
            assert_eq!(from, "a.txt");
            assert_eq!(to, "b.txt");
            assert!(*modified);
        }
        other => anyhow::bail!("unexpected diff: {:?}", other),
    }
    Ok(())
}
