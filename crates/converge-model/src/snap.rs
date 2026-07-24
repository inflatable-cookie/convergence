use serde::{Deserialize, Serialize};

use crate::ids::ObjectId;

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
    /// Metadata only — never part of identity (arch doc 17 §1).
    pub created_at: String,
    pub root_manifest: ObjectId,
    /// Ordered, deduplicated; first parent is the primary lineage.
    #[serde(default)]
    pub parents: Vec<String>,
    /// Provenance edge when the tree came from a fetched bundle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derived_from_bundle: Option<String>,
    pub message: Option<String>,
    /// Why captured: "explicit" (user verb) or "automatic" (watcher).
    /// Metadata only — never part of identity.
    #[serde(default = "default_trigger")]
    pub trigger: String,
    pub stats: SnapStats,
}

fn default_trigger() -> String {
    "explicit".to_string()
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
    // Absent on version-1 (fixed-block) recipes from the g01 era.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<crate::chunk::ChunkParams>,
    pub chunks: Vec<FileRecipeChunk>,
}

/// Identity = content + lineage (arch doc 17 §1). Timestamp and message
/// are metadata: recapturing an unchanged tree over the same head yields
/// the same id, and messages stay editable after capture.
pub fn compute_snap_id(
    root_manifest: &ObjectId,
    parents: &[String],
    derived_from_bundle: Option<&str>,
) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"converge-snap-v2\n");
    hasher.update(root_manifest.as_str().as_bytes());
    hasher.update(b"\n");
    hasher.update(parents.join(",").as_bytes());
    hasher.update(b"\n");
    hasher.update(derived_from_bundle.unwrap_or("").as_bytes());
    hasher.finalize().to_hex().to_string()
}
