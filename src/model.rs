use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ObjectId(pub String);

impl ObjectId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    pub version: u32,

    #[serde(default)]
    pub remote: Option<RemoteConfig>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RemoteConfig {
    pub base_url: String,
    pub token: String,
    pub repo_id: String,
    pub scope: String,
    pub gate: String,
}

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

pub fn compute_snap_id(
    created_at: &str,
    root_manifest: &ObjectId,
    message: Option<&str>,
) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(created_at.as_bytes());
    hasher.update(b"\n");
    hasher.update(root_manifest.as_str().as_bytes());
    if let Some(message) = message {
        hasher.update(b"\n");
        hasher.update(message.as_bytes());
    }
    hasher.finalize().to_hex().to_string()
}

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
    Dir {
        manifest: ObjectId,
    },
    Symlink {
        target: String,
    },
    Tombstone,
}
