use anyhow::Result;

use crate::model::{Manifest, ObjectId};
use crate::store::LocalStore;

mod entries;
mod leaves;
mod traversal;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StatusDelta {
    Added,
    Modified,
    Deleted,
}

pub(super) fn diff_trees(
    store: &LocalStore,
    base_root: Option<&ObjectId>,
    cur_root: &ObjectId,
    cur_manifests: &std::collections::HashMap<ObjectId, Manifest>,
) -> Result<Vec<(StatusDelta, String)>> {
    let mut out = Vec::new();
    traversal::diff_dir("", store, base_root, cur_root, cur_manifests, &mut out)?;
    out.sort_by(|a, b| a.1.cmp(&b.1));
    Ok(out)
}
