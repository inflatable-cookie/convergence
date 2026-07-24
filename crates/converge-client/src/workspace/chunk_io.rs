use anyhow::Result;

use crate::model::{ObjectId, chunk_data};
use crate::store::LocalStore;
use crate::store::hash_bytes;

use super::chunking::ChunkingPolicy;

pub(super) fn chunk_bytes_to_recipe_store(
    store: &LocalStore,
    data: &[u8],
    policy: ChunkingPolicy,
) -> Result<ObjectId> {
    let (recipe, blobs) = chunk_data(data, policy.params);
    for (_, slice) in &blobs {
        store.put_blob(slice)?;
    }
    store.put_recipe(&recipe)
}

pub(super) fn chunk_bytes_to_recipe_id(data: &[u8], policy: ChunkingPolicy) -> Result<ObjectId> {
    let (recipe, _) = chunk_data(data, policy.params);
    let bytes = crate::model::encoding::encode_recipe(&recipe);
    Ok(hash_bytes(&bytes))
}
