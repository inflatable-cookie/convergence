use anyhow::{Result, anyhow};

use crate::model::{ChunkParams, ChunkingConfig};

const DEFAULT_CHUNK_THRESHOLD: u64 = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug)]
pub(super) struct ChunkingPolicy {
    pub(super) params: ChunkParams,
    pub(super) threshold: u64,
}

// Config carries a single `chunk_size`; it maps to the FastCDC average with
// min = avg/4 and max = avg*4 (arch 16).
pub(super) fn chunking_policy_from_config(cfg: Option<&ChunkingConfig>) -> Result<ChunkingPolicy> {
    let threshold = cfg.map(|c| c.threshold).unwrap_or(DEFAULT_CHUNK_THRESHOLD);
    let params = match cfg.map(|c| c.chunk_size) {
        None => ChunkParams::default(),
        Some(avg) => {
            let avg =
                u32::try_from(avg.max(64 * 1024)).map_err(|_| anyhow!("chunk_size too large"))?;
            ChunkParams {
                min_size: avg / 4,
                avg_size: avg,
                max_size: avg.saturating_mul(4),
            }
        }
    };
    Ok(ChunkingPolicy { params, threshold })
}
