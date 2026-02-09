use super::*;

pub(super) fn put_manifest(store: &LocalStore, manifest: &Manifest) -> Result<ObjectId> {
    let bytes = serde_json::to_vec(manifest).context("serialize manifest")?;
    let id = hash_bytes(&bytes);
    let path = store
        .root
        .join("objects/manifests")
        .join(format!("{}.json", id.as_str()));
    write_if_absent(&path, &bytes).context("store manifest")?;
    Ok(id)
}

pub(super) fn put_manifest_bytes(store: &LocalStore, id: &ObjectId, bytes: &[u8]) -> Result<()> {
    let actual = blake3::hash(bytes).to_hex().to_string();
    if actual != id.0 {
        return Err(anyhow!(
            "manifest hash mismatch (expected {}, got {})",
            id.as_str(),
            actual
        ));
    }
    let path = store
        .root
        .join("objects/manifests")
        .join(format!("{}.json", id.as_str()));
    write_if_absent(&path, bytes).context("store manifest bytes")?;
    Ok(())
}

pub(super) fn has_manifest(store: &LocalStore, id: &ObjectId) -> bool {
    store
        .root
        .join("objects/manifests")
        .join(format!("{}.json", id.as_str()))
        .exists()
}

pub(super) fn get_manifest_bytes(store: &LocalStore, id: &ObjectId) -> Result<Vec<u8>> {
    let path = store
        .root
        .join("objects/manifests")
        .join(format!("{}.json", id.as_str()));
    let bytes = fs::read(&path).with_context(|| format!("read manifest {}", id.as_str()))?;
    let actual = blake3::hash(&bytes).to_hex().to_string();
    if actual != id.0 {
        return Err(anyhow!(
            "manifest integrity check failed for {} (expected {}, got {})",
            path.display(),
            id.as_str(),
            actual
        ));
    }
    Ok(bytes)
}

pub(super) fn get_manifest(store: &LocalStore, id: &ObjectId) -> Result<Manifest> {
    let bytes = get_manifest_bytes(store, id)?;
    let m: Manifest = serde_json::from_slice(&bytes)
        .with_context(|| format!("parse manifest {}", id.as_str()))?;
    Ok(m)
}
