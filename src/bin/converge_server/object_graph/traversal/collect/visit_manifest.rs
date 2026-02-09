use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn visit_manifest(
    state: &AppState,
    repo_id: &str,
    manifest_id: &str,
    blobs: &mut HashSet<String>,
    manifests: &mut HashSet<String>,
    recipes: &mut HashSet<String>,
    visited_manifests: &mut HashSet<String>,
    visited_recipes: &mut HashSet<String>,
) -> Result<(), Response> {
    if !visited_manifests.insert(manifest_id.to_string()) {
        return Ok(());
    }
    manifests.insert(manifest_id.to_string());

    let manifest = read_manifest(state, repo_id, manifest_id)?;
    for e in manifest.entries {
        match e.kind {
            converge::model::ManifestEntryKind::File { blob, .. } => {
                blobs.insert(blob.as_str().to_string());
            }
            converge::model::ManifestEntryKind::FileChunks { recipe, .. } => {
                super::visit_recipe::visit_recipe(
                    state,
                    repo_id,
                    recipe.as_str(),
                    blobs,
                    recipes,
                    visited_recipes,
                )?;
            }
            converge::model::ManifestEntryKind::Dir { manifest } => {
                visit_manifest(
                    state,
                    repo_id,
                    manifest.as_str(),
                    blobs,
                    manifests,
                    recipes,
                    visited_manifests,
                    visited_recipes,
                )?;
            }
            converge::model::ManifestEntryKind::Symlink { .. } => {}
            converge::model::ManifestEntryKind::Superposition { variants } => {
                for v in variants {
                    match v.kind {
                        converge::model::SuperpositionVariantKind::File { blob, .. } => {
                            blobs.insert(blob.as_str().to_string());
                        }
                        converge::model::SuperpositionVariantKind::FileChunks {
                            recipe, ..
                        } => {
                            super::visit_recipe::visit_recipe(
                                state,
                                repo_id,
                                recipe.as_str(),
                                blobs,
                                recipes,
                                visited_recipes,
                            )?;
                        }
                        converge::model::SuperpositionVariantKind::Dir { manifest } => {
                            visit_manifest(
                                state,
                                repo_id,
                                manifest.as_str(),
                                blobs,
                                manifests,
                                recipes,
                                visited_manifests,
                                visited_recipes,
                            )?;
                        }
                        converge::model::SuperpositionVariantKind::Symlink { .. } => {}
                        converge::model::SuperpositionVariantKind::Tombstone => {}
                    }
                }
            }
        }
    }

    Ok(())
}
