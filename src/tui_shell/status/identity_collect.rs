use std::collections::HashMap;

use anyhow::Result;

use crate::model::{Manifest, ManifestEntryKind, ObjectId};
use crate::store::LocalStore;

use super::rename_helpers::IdentityKey;

pub(super) fn collect_identities_current(
    prefix: &str,
    manifest_id: &ObjectId,
    cur_manifests: &HashMap<ObjectId, Manifest>,
    out: &mut HashMap<String, IdentityKey>,
) -> Result<()> {
    let m = cur_manifests
        .get(manifest_id)
        .ok_or_else(|| anyhow::anyhow!("missing current manifest {}", manifest_id.as_str()))?;
    for e in &m.entries {
        let path = if prefix.is_empty() {
            e.name.clone()
        } else {
            format!("{}/{}", prefix, e.name)
        };
        match &e.kind {
            ManifestEntryKind::Dir { manifest } => {
                collect_identities_current(&path, manifest, cur_manifests, out)?;
            }
            ManifestEntryKind::File { blob, .. } => {
                out.insert(path, IdentityKey::Blob(blob.as_str().to_string()));
            }
            ManifestEntryKind::FileChunks { recipe, .. } => {
                out.insert(path, IdentityKey::Recipe(recipe.as_str().to_string()));
            }
            ManifestEntryKind::Symlink { target } => {
                out.insert(path, IdentityKey::Symlink(target.clone()));
            }
            _ => {}
        }
    }
    Ok(())
}

pub(super) fn collect_identities_base(
    prefix: &str,
    store: &LocalStore,
    manifest_id: &ObjectId,
    out: &mut HashMap<String, IdentityKey>,
) -> Result<()> {
    let m = store.get_manifest(manifest_id)?;
    for e in &m.entries {
        let path = if prefix.is_empty() {
            e.name.clone()
        } else {
            format!("{}/{}", prefix, e.name)
        };
        match &e.kind {
            ManifestEntryKind::Dir { manifest } => {
                collect_identities_base(&path, store, manifest, out)?;
            }
            ManifestEntryKind::File { blob, .. } => {
                out.insert(path, IdentityKey::Blob(blob.as_str().to_string()));
            }
            ManifestEntryKind::FileChunks { recipe, .. } => {
                out.insert(path, IdentityKey::Recipe(recipe.as_str().to_string()));
            }
            ManifestEntryKind::Symlink { target } => {
                out.insert(path, IdentityKey::Symlink(target.clone()));
            }
            _ => {}
        }
    }
    Ok(())
}
