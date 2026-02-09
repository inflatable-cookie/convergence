use serde::{Deserialize, Serialize};

use super::ids::ObjectId;
use super::resolution::{VariantKey, VariantKeyKind};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub version: u32,
    pub entries: Vec<ManifestEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub name: String,

    #[serde(flatten)]
    pub kind: ManifestEntryKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ManifestEntryKind {
    File {
        blob: ObjectId,
        mode: u32,
        size: u64,
    },
    FileChunks {
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
    Superposition {
        variants: Vec<SuperpositionVariant>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuperpositionVariant {
    pub source: String,

    #[serde(flatten)]
    pub kind: SuperpositionVariantKind,
}

impl SuperpositionVariant {
    pub fn key(&self) -> VariantKey {
        let kind = match &self.kind {
            SuperpositionVariantKind::File { blob, mode, size } => VariantKeyKind::File {
                blob: blob.clone(),
                mode: *mode,
                size: *size,
            },
            SuperpositionVariantKind::FileChunks { recipe, mode, size } => {
                VariantKeyKind::ChunkedFile {
                    recipe: recipe.clone(),
                    mode: *mode,
                    size: *size,
                }
            }
            SuperpositionVariantKind::Dir { manifest } => VariantKeyKind::Dir {
                manifest: manifest.clone(),
            },
            SuperpositionVariantKind::Symlink { target } => VariantKeyKind::Symlink {
                target: target.clone(),
            },
            SuperpositionVariantKind::Tombstone => VariantKeyKind::Tombstone,
        };

        VariantKey {
            source: self.source.clone(),
            kind,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SuperpositionVariantKind {
    File {
        blob: ObjectId,
        mode: u32,
        size: u64,
    },
    FileChunks {
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
