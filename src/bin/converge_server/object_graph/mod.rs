//! Manifest/snap object graph traversal, validation, and merge helpers.

use super::*;

mod merge;
mod store;
mod traversal;

pub(super) fn validate_manifest_entry_refs(
    state: &AppState,
    repo_id: &str,
    kind: &converge::model::ManifestEntryKind,
    allow_missing_blobs: bool,
) -> Result<(), Response> {
    store::validate_manifest_entry_refs(state, repo_id, kind, allow_missing_blobs)
}

pub(super) fn read_snap(
    state: &AppState,
    repo_id: &str,
    snap_id: &str,
) -> Result<converge::model::SnapRecord, Response> {
    store::read_snap(state, repo_id, snap_id)
}

pub(super) fn collect_objects_from_manifest_tree(
    state: &AppState,
    repo_id: &str,
    root_manifest_id: &str,
    blobs: &mut HashSet<String>,
    manifests: &mut HashSet<String>,
    recipes: &mut HashSet<String>,
) -> Result<(), Response> {
    traversal::collect_objects_from_manifest_tree(
        state,
        repo_id,
        root_manifest_id,
        blobs,
        manifests,
        recipes,
    )
}

pub(super) fn validate_manifest_tree_availability(
    state: &AppState,
    repo_id: &str,
    root_manifest_id: &str,
    require_blobs: bool,
) -> Result<(), Response> {
    traversal::validate_manifest_tree_availability(state, repo_id, root_manifest_id, require_blobs)
}

pub(super) fn coalesce_root_manifest(
    state: &AppState,
    repo_id: &str,
    inputs: &[(String, String)],
) -> Result<String, Response> {
    merge::coalesce_root_manifest(state, repo_id, inputs)
}

pub(super) fn manifest_has_superpositions(
    state: &AppState,
    repo_id: &str,
    root_manifest_id: &str,
) -> Result<bool, Response> {
    traversal::manifest_has_superpositions(state, repo_id, root_manifest_id)
}

pub(super) fn compute_promotability(
    gate: &GateDef,
    has_superpositions: bool,
    approval_count: usize,
) -> (bool, Vec<String>) {
    merge::compute_promotability(gate, has_superpositions, approval_count)
}
