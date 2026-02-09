use anyhow::Result;

use super::GcReport;
use super::prune::prune_unreferenced_objects;
use super::reachability::collect_reachable_objects;
use super::retention_plan::compute_keep_set;
use crate::workspace::Workspace;

impl Workspace {
    pub(crate) fn gc_local(&self, dry_run: bool) -> Result<GcReport> {
        let cfg = self.store.read_config()?;
        let retention = cfg.retention.unwrap_or_default();

        let mut snaps = self.store.list_snaps()?;
        snaps.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        let head = self.store.get_head()?;
        let now = time::OffsetDateTime::now_utc();
        let keep = compute_keep_set(&snaps, &retention, head, now);

        // Walk reachable objects.
        let mut keep_blobs = std::collections::HashSet::new();
        let mut keep_manifests = std::collections::HashSet::new();
        let mut keep_recipes = std::collections::HashSet::new();
        for s in &snaps {
            if !keep.contains(&s.id) {
                continue;
            }
            collect_reachable_objects(
                &self.store,
                &s.root_manifest,
                &mut keep_blobs,
                &mut keep_manifests,
                &mut keep_recipes,
            )?;
        }

        // Delete unreferenced objects.
        let mut report = GcReport {
            kept_snaps: keep.len(),
            pruned_snaps: snaps.len().saturating_sub(keep.len()),
            ..GcReport::default()
        };

        prune_unreferenced_objects(
            &self.store,
            dry_run,
            &keep_blobs,
            &keep_manifests,
            &keep_recipes,
            &mut report,
        )?;

        if retention.prune_snaps && !dry_run {
            for s in &snaps {
                if !keep.contains(&s.id) {
                    self.store.delete_snap(&s.id)?;
                }
            }
        }

        Ok(report)
    }
}
