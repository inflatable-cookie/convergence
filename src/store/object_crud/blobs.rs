use super::*;

pub(super) fn put_blob(store: &LocalStore, bytes: &[u8]) -> Result<ObjectId> {
    let id = hash_bytes(bytes);
    let path = store.root.join("objects/blobs").join(id.as_str());
    write_if_absent(&path, bytes).context("store blob")?;
    Ok(id)
}

pub(super) fn has_blob(store: &LocalStore, id: &ObjectId) -> bool {
    store.root.join("objects/blobs").join(id.as_str()).exists()
}

pub(super) fn get_blob(store: &LocalStore, id: &ObjectId) -> Result<Vec<u8>> {
    let path = store.root.join("objects/blobs").join(id.as_str());
    let bytes = fs::read(&path).with_context(|| format!("read blob {}", id.as_str()))?;
    let actual = blake3::hash(&bytes).to_hex().to_string();
    if actual != id.0 {
        return Err(anyhow!(
            "blob integrity check failed for {} (expected {}, got {})",
            path.display(),
            id.as_str(),
            actual
        ));
    }
    Ok(bytes)
}
