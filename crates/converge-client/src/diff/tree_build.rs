use std::collections::{BTreeMap, HashMap};

use anyhow::{Context, Result};

use crate::model::{Manifest, ObjectId};
use crate::store::LocalStore;

use super::signatures::EntrySig;
use super::walk::walk_manifest;

pub fn tree_from_store(store: &LocalStore, root: &ObjectId) -> Result<BTreeMap<String, EntrySig>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![(root.clone(), String::new())];
    while let Some((mid, prefix)) = stack.pop() {
        let m = store.get_manifest(&mid)?;
        walk_manifest(&m, &prefix, &mut out, |child| {
            stack.push(child);
        })?;
    }
    Ok(out)
}

pub fn tree_from_memory(
    manifests: &HashMap<ObjectId, Manifest>,
    root: &ObjectId,
) -> Result<BTreeMap<String, EntrySig>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![(root.clone(), String::new())];
    while let Some((mid, prefix)) = stack.pop() {
        let m = manifests
            .get(&mid)
            .with_context(|| format!("manifest missing: {}", mid.as_str()))?;
        walk_manifest(m, &prefix, &mut out, |child| {
            stack.push(child);
        })?;
    }
    Ok(out)
}
