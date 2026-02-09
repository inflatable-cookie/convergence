use std::collections::HashSet;

use anyhow::{Context, Result};

use crate::model::ObjectId;
use crate::store::LocalStore;

use super::super::{RemoteClient, with_retries};

mod object_fetch;
mod traversal;

pub(super) fn fetch_manifest_tree(
    store: &LocalStore,
    remote: &RemoteClient,
    repo: &str,
    root: &ObjectId,
) -> Result<()> {
    let mut visited = HashSet::new();
    traversal::fetch_manifest_tree_inner(store, remote, repo, root, &mut visited)
}
