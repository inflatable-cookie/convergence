use super::super::*;

mod entry_validation;
mod readers;
mod writers;

pub(super) fn validate_manifest_entry_refs(
    state: &AppState,
    repo_id: &str,
    kind: &converge::model::ManifestEntryKind,
    allow_missing_blobs: bool,
) -> Result<(), Response> {
    entry_validation::validate_manifest_entry_refs(state, repo_id, kind, allow_missing_blobs)
}

pub(super) fn read_recipe(
    state: &AppState,
    repo_id: &str,
    recipe_id: &str,
) -> Result<converge::model::FileRecipe, Response> {
    readers::read_recipe(state, repo_id, recipe_id)
}

pub(super) fn read_snap(
    state: &AppState,
    repo_id: &str,
    snap_id: &str,
) -> Result<converge::model::SnapRecord, Response> {
    readers::read_snap(state, repo_id, snap_id)
}

pub(super) fn read_manifest(
    state: &AppState,
    repo_id: &str,
    manifest_id: &str,
) -> Result<converge::model::Manifest, Response> {
    readers::read_manifest(state, repo_id, manifest_id)
}

pub(super) fn store_manifest(
    state: &AppState,
    repo_id: &str,
    manifest: &converge::model::Manifest,
) -> Result<String, Response> {
    writers::store_manifest(state, repo_id, manifest)
}
