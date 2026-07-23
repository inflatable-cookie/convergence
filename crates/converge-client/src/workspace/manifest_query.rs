use super::*;

use crate::model::Manifest;

impl Workspace {
    /// Compute a manifest tree for the current working directory without writing a snap.
    ///
    /// Note: this still reads file contents to compute stable blob ids.
    pub fn current_manifest_tree(
        &self,
    ) -> Result<(ObjectId, HashMap<ObjectId, Manifest>, SnapStats)> {
        let cfg = self.store.read_config()?;
        let policy = chunking::chunking_policy_from_config(cfg.chunking.as_ref())?;
        let mut stats = SnapStats::default();
        let mut manifests: HashMap<ObjectId, Manifest> = HashMap::new();
        let root_manifest = manifest_scan::build_manifest_in_memory(
            &self.root,
            &mut stats,
            &mut manifests,
            policy,
        )?;
        Ok((root_manifest, manifests, stats))
    }
}
