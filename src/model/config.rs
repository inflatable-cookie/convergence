use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    pub version: u32,

    #[serde(default)]
    pub remote: Option<RemoteConfig>,

    #[serde(default)]
    pub chunking: Option<ChunkingConfig>,

    #[serde(default)]
    pub retention: Option<RetentionConfig>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChunkingConfig {
    /// Chunk size in bytes.
    pub chunk_size: u64,
    /// Chunking threshold in bytes. Files with size >= threshold are chunked.
    pub threshold: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RetentionConfig {
    /// Keep at least the most recent N snaps.
    #[serde(default)]
    pub keep_last: Option<u64>,

    /// Keep snaps newer than N days.
    #[serde(default)]
    pub keep_days: Option<u64>,

    /// Snap ids that are always kept.
    #[serde(default)]
    pub pinned: Vec<String>,

    /// If true, `gc` will delete snap records that are not kept.
    #[serde(default)]
    pub prune_snaps: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RemoteConfig {
    pub base_url: String,

    // Token is stored in workspace state, not config.json.
    // Kept as an optional field for backwards-compatible parsing of older config files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,

    pub repo_id: String,
    pub scope: String,
    pub gate: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WorkspaceState {
    pub version: u32,

    #[serde(default)]
    pub lane_sync: std::collections::HashMap<String, LaneSyncRecord>,

    #[serde(default)]
    pub remote_tokens: std::collections::HashMap<String, String>,

    /// Tracks the last snap published for a given remote+scope+gate.
    #[serde(default)]
    pub last_published: std::collections::HashMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LaneSyncRecord {
    pub snap_id: String,
    pub synced_at: String,
}
