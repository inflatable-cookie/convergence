use super::*;

pub(super) fn put_recipe(store: &LocalStore, recipe: &FileRecipe) -> Result<ObjectId> {
    let bytes = serde_json::to_vec(recipe).context("serialize recipe")?;
    let id = hash_bytes(&bytes);
    let path = store
        .root
        .join("objects/recipes")
        .join(format!("{}.json", id.as_str()));
    write_if_absent(&path, &bytes).context("store recipe")?;
    Ok(id)
}

pub(super) fn put_recipe_bytes(store: &LocalStore, id: &ObjectId, bytes: &[u8]) -> Result<()> {
    let actual = blake3::hash(bytes).to_hex().to_string();
    if actual != id.0 {
        return Err(anyhow!(
            "recipe hash mismatch (expected {}, got {})",
            id.as_str(),
            actual
        ));
    }
    let path = store
        .root
        .join("objects/recipes")
        .join(format!("{}.json", id.as_str()));
    write_if_absent(&path, bytes).context("store recipe bytes")?;
    Ok(())
}

pub(super) fn has_recipe(store: &LocalStore, id: &ObjectId) -> bool {
    store
        .root
        .join("objects/recipes")
        .join(format!("{}.json", id.as_str()))
        .exists()
}

pub(super) fn get_recipe_bytes(store: &LocalStore, id: &ObjectId) -> Result<Vec<u8>> {
    let path = store
        .root
        .join("objects/recipes")
        .join(format!("{}.json", id.as_str()));
    let bytes = fs::read(&path).with_context(|| format!("read recipe {}", id.as_str()))?;
    let actual = blake3::hash(&bytes).to_hex().to_string();
    if actual != id.0 {
        return Err(anyhow!(
            "recipe integrity check failed for {} (expected {}, got {})",
            path.display(),
            id.as_str(),
            actual
        ));
    }
    Ok(bytes)
}

pub(super) fn get_recipe(store: &LocalStore, id: &ObjectId) -> Result<FileRecipe> {
    let bytes = get_recipe_bytes(store, id)?;
    let r: FileRecipe =
        serde_json::from_slice(&bytes).with_context(|| format!("parse recipe {}", id.as_str()))?;
    Ok(r)
}
