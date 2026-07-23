mod clear;
mod materialize;
mod platform;

use std::path::Path;

use anyhow::Result;

use crate::model::ObjectId;
use crate::store::LocalStore;

pub(super) fn clear_workspace_except_converge_and_git(root: &Path) -> Result<()> {
    clear::clear_workspace_except_converge_and_git(root)
}

pub(super) fn is_empty_except_converge_and_git(root: &Path) -> Result<bool> {
    clear::is_empty_except_converge_and_git(root)
}

pub(super) fn is_empty_dir(root: &Path) -> Result<bool> {
    clear::is_empty_dir(root)
}

pub(super) fn clear_dir(root: &Path) -> Result<()> {
    clear::clear_dir(root)
}

pub(super) fn materialize_manifest(
    store: &LocalStore,
    manifest_id: &ObjectId,
    out_dir: &Path,
) -> Result<()> {
    materialize::materialize_manifest(store, manifest_id, out_dir)
}
