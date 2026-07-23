use crate::model::ManifestEntryKind;

use anyhow::Result;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind")]
pub enum EntrySig {
    File {
        blob: String,
        mode: u32,
        size: u64,
    },
    FileChunks {
        recipe: String,
        mode: u32,
        size: u64,
    },
    Symlink {
        target: String,
    },
    Superposition {
        variants: usize,
    },
}

pub(super) fn sig_for_kind(kind: &ManifestEntryKind) -> Result<EntrySig> {
    match kind {
        ManifestEntryKind::File { blob, mode, size } => Ok(EntrySig::File {
            blob: blob.as_str().to_string(),
            mode: *mode,
            size: *size,
        }),
        ManifestEntryKind::FileChunks { recipe, mode, size } => Ok(EntrySig::FileChunks {
            recipe: recipe.as_str().to_string(),
            mode: *mode,
            size: *size,
        }),
        ManifestEntryKind::Symlink { target } => Ok(EntrySig::Symlink {
            target: target.clone(),
        }),
        ManifestEntryKind::Superposition { variants } => Ok(EntrySig::Superposition {
            variants: variants.len(),
        }),
        ManifestEntryKind::Dir { .. } => anyhow::bail!("dir entry should be handled by traversal"),
    }
}
