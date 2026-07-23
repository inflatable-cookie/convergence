use super::*;

use time::format_description::well_known::Rfc3339;

use crate::model::{SnapRecord, compute_snap_id};

impl Workspace {
    pub fn create_snap(&self, message: Option<String>) -> Result<SnapRecord> {
        // Validate store format early.
        let cfg = self.store.read_config()?;
        let policy = chunking::chunking_policy_from_config(cfg.chunking.as_ref())?;

        let mut stats = SnapStats::default();
        let root_manifest = self.build_manifest(&self.root, &mut stats, policy)?;
        let created_at = time::OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .context("format created_at")?;

        let id = compute_snap_id(&created_at, &root_manifest);
        let snap = SnapRecord {
            version: 1,
            id,
            created_at,
            root_manifest,
            message,
            stats,
        };
        self.store.put_snap(&snap)?;
        self.store.set_head(Some(&snap.id))?;
        Ok(snap)
    }

    pub fn list_snaps(&self) -> Result<Vec<SnapRecord>> {
        let mut snaps = self.store.list_snaps()?;
        snaps.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(snaps)
    }

    pub fn show_snap(&self, snap_id: &str) -> Result<SnapRecord> {
        self.store.get_snap(snap_id)
    }
}
