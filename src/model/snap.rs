use serde::{Deserialize, Serialize};

use super::ids::ObjectId;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SnapStats {
    pub files: u64,
    pub dirs: u64,
    pub symlinks: u64,
    pub bytes: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SnapRecord {
    pub version: u32,
    pub id: String,
    pub created_at: String,
    pub root_manifest: ObjectId,
    pub message: Option<String>,
    pub stats: SnapStats,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileRecipeChunk {
    pub blob: ObjectId,
    pub size: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileRecipe {
    pub version: u32,
    pub size: u64,
    pub chunks: Vec<FileRecipeChunk>,
}

pub fn compute_snap_id(created_at: &str, root_manifest: &ObjectId) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(created_at.as_bytes());
    hasher.update(b"\n");
    hasher.update(root_manifest.as_str().as_bytes());
    hasher.finalize().to_hex().to_string()
}
