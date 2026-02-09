use super::super::*;
use super::store::{read_manifest, store_manifest, validate_manifest_entry_refs};

mod manifest_merge;
mod promotability;

pub(super) use self::promotability::compute_promotability;

pub(super) fn coalesce_root_manifest(
    state: &AppState,
    repo_id: &str,
    inputs: &[(String, String)],
) -> Result<String, Response> {
    let mut sorted_inputs = inputs.to_vec();
    sorted_inputs.sort_by(|a, b| a.0.cmp(&b.0));
    manifest_merge::merge_dir_manifests(state, repo_id, &sorted_inputs)
}
