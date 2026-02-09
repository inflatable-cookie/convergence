use serde::{Deserialize, Serialize};

use super::ids::ObjectId;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VariantKey {
    pub source: String,

    #[serde(flatten)]
    pub kind: VariantKeyKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum VariantKeyKind {
    File {
        blob: ObjectId,
        mode: u32,
        size: u64,
    },
    ChunkedFile {
        recipe: ObjectId,
        mode: u32,
        size: u64,
    },
    Dir {
        manifest: ObjectId,
    },
    Symlink {
        target: String,
    },
    Tombstone,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResolutionDecision {
    /// Legacy decision: 0-based variant index.
    Index(u32),
    /// Stable decision: a key derived from variant content.
    Key(VariantKey),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Resolution {
    pub version: u32,
    pub bundle_id: String,
    pub root_manifest: ObjectId,
    pub created_at: String,

    /// Path -> selected decision (v1 index or v2 key)
    pub decisions: std::collections::BTreeMap<String, ResolutionDecision>,
}
