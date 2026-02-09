use std::collections::HashMap;

use anyhow::Result;

use crate::model::{ObjectId, ResolutionDecision};
use crate::store::LocalStore;

mod decisions;
mod precheck;
mod rewrite;

pub fn apply_resolution(
    store: &LocalStore,
    root: &ObjectId,
    decisions: &std::collections::BTreeMap<String, ResolutionDecision>,
) -> Result<ObjectId> {
    precheck::ensure_resolution_valid(store, root, decisions)?;

    let mut memo = HashMap::new();
    rewrite::rewrite_manifest(store, root, "", decisions, &mut memo)
}
