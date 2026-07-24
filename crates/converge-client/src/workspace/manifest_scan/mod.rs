use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use crate::model::{Manifest, ObjectId, SnapStats};

use super::Workspace;
use super::chunking::ChunkingPolicy;

pub(in crate::workspace) mod common;
mod scan_memory;
mod scan_store;

use self::scan_memory::build_manifest_in_memory_impl;
use self::scan_store::build_manifest_store_impl;

impl Workspace {
    /// Build a manifest of an arbitrary directory into this workspace's
    /// store (git import extracts historical trees this way).
    pub fn build_manifest_of(&self, dir: &Path, stats: &mut SnapStats) -> Result<ObjectId> {
        let cfg = self.store.read_config()?;
        let policy = super::chunking::chunking_policy_from_config(cfg.chunking.as_ref())?;
        self.build_manifest(dir, stats, policy)
    }

    pub(super) fn build_manifest(
        &self,
        dir: &Path,
        stats: &mut SnapStats,
        policy: ChunkingPolicy,
    ) -> Result<ObjectId> {
        let ignores = common::load_root_ignores(dir);
        build_manifest_store_impl(self, dir, dir, &ignores, stats, policy)
    }
}

pub(super) fn build_manifest_in_memory(
    dir: &Path,
    stats: &mut SnapStats,
    manifests: &mut HashMap<ObjectId, Manifest>,
    policy: ChunkingPolicy,
) -> Result<ObjectId> {
    let ignores = common::load_root_ignores(dir);
    build_manifest_in_memory_impl(dir, dir, &ignores, stats, manifests, policy)
}
