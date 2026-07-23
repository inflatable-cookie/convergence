use anyhow::{Result, anyhow};

use crate::model::ChunkingConfig;

const DEFAULT_CHUNK_SIZE: u64 = 4 * 1024 * 1024;
const DEFAULT_CHUNK_THRESHOLD: u64 = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug)]
pub(super) struct ChunkingPolicy {
    pub(super) chunk_size: usize,
    pub(super) threshold: u64,
}

pub(super) fn chunking_policy_from_config(cfg: Option<&ChunkingConfig>) -> Result<ChunkingPolicy> {
    let chunk_size = cfg
        .map(|c| c.chunk_size)
        .unwrap_or(DEFAULT_CHUNK_SIZE)
        .max(64 * 1024);
    let threshold = cfg.map(|c| c.threshold).unwrap_or(DEFAULT_CHUNK_THRESHOLD);

    let chunk_size_usize =
        usize::try_from(chunk_size).map_err(|_| anyhow!("chunk_size too large: {}", chunk_size))?;

    Ok(ChunkingPolicy {
        chunk_size: chunk_size_usize,
        threshold,
    })
}
