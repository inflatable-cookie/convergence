use super::object_fetch::{fetch_blob_if_missing, fetch_recipe_and_chunks};
use super::*;

pub(super) fn fetch_manifest_tree_inner(
    store: &LocalStore,
    remote: &RemoteClient,
    repo: &str,
    manifest_id: &ObjectId,
    visited: &mut HashSet<String>,
) -> Result<()> {
    if !visited.insert(manifest_id.as_str().to_string()) {
        return Ok(());
    }

    if !store.has_manifest(manifest_id) {
        let resp = remote
            .client
            .get(remote.url(&format!(
                "/repos/{}/objects/manifests/{}",
                repo,
                manifest_id.as_str()
            )))
            .header(reqwest::header::AUTHORIZATION, remote.auth())
            .send()
            .context("fetch manifest")?;
        let bytes = remote
            .ensure_ok(resp, "fetch manifest")?
            .bytes()
            .context("read manifest bytes")?;

        store.put_manifest_bytes(manifest_id, &bytes)?;
    }

    let manifest = store.get_manifest(manifest_id)?;
    for e in manifest.entries {
        match e.kind {
            crate::model::ManifestEntryKind::Dir { manifest } => {
                fetch_manifest_tree_inner(store, remote, repo, &manifest, visited)?;
            }
            crate::model::ManifestEntryKind::File { blob, .. } => {
                fetch_blob_if_missing(store, remote, repo, &blob)?;
            }
            crate::model::ManifestEntryKind::FileChunks { recipe, .. } => {
                fetch_recipe_and_chunks(store, remote, repo, &recipe)?;
            }
            crate::model::ManifestEntryKind::Symlink { .. } => {}
            crate::model::ManifestEntryKind::Superposition { variants } => {
                for v in variants {
                    match v.kind {
                        crate::model::SuperpositionVariantKind::File { blob, .. } => {
                            fetch_blob_if_missing(store, remote, repo, &blob)?;
                        }
                        crate::model::SuperpositionVariantKind::Dir { manifest } => {
                            fetch_manifest_tree_inner(store, remote, repo, &manifest, visited)?;
                        }
                        crate::model::SuperpositionVariantKind::Symlink { .. } => {}
                        crate::model::SuperpositionVariantKind::Tombstone => {}
                        crate::model::SuperpositionVariantKind::FileChunks { recipe, .. } => {
                            fetch_recipe_and_chunks(store, remote, repo, &recipe)?;
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
