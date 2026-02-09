use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use crate::model::{Manifest, ObjectId, SnapStats};

use super::Workspace;
use super::chunking::ChunkingPolicy;

mod common;
mod scan_memory;
mod scan_store;

use self::scan_memory::build_manifest_in_memory_impl;
use self::scan_store::build_manifest_store_impl;

impl Workspace {
    pub(super) fn build_manifest(
        &self,
        dir: &Path,
        stats: &mut SnapStats,
        policy: ChunkingPolicy,
    ) -> Result<ObjectId> {
        build_manifest_store_impl(self, dir, stats, policy)
    }
}

pub(super) fn build_manifest_in_memory(
    dir: &Path,
    stats: &mut SnapStats,
    manifests: &mut HashMap<ObjectId, Manifest>,
    policy: ChunkingPolicy,
) -> Result<ObjectId> {
    build_manifest_in_memory_impl(dir, stats, manifests, policy)
}
