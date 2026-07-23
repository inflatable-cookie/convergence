use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::model::{ObjectId, chunk_data};
use crate::store::LocalStore;
use crate::store::hash_bytes;

use super::chunking::ChunkingPolicy;

pub(super) fn chunk_file_to_recipe_store(
    store: &LocalStore,
    path: &Path,
    size: u64,
    policy: ChunkingPolicy,
) -> Result<ObjectId> {
    let data = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    if data.len() as u64 != size {
        anyhow::bail!(
            "size mismatch while chunking {} (expected {}, got {})",
            path.display(),
            size,
            data.len()
        );
    }
    let (recipe, blobs) = chunk_data(&data, policy.params);
    for (_, slice) in &blobs {
        store.put_blob(slice)?;
    }
    store.put_recipe(&recipe)
}

pub(super) fn chunk_file_to_recipe_id(
    path: &Path,
    size: u64,
    policy: ChunkingPolicy,
) -> Result<ObjectId> {
    let data = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    if data.len() as u64 != size {
        anyhow::bail!(
            "size mismatch while chunking {} (expected {}, got {})",
            path.display(),
            size,
            data.len()
        );
    }
    let (recipe, _) = chunk_data(&data, policy.params);
    let bytes = serde_json::to_vec(&recipe).context("serialize recipe")?;
    Ok(hash_bytes(&bytes))
}
