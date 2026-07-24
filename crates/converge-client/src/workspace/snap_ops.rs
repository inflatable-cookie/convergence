use std::collections::{HashMap, HashSet};

use super::*;

use time::format_description::well_known::Rfc3339;

use crate::model::{SnapRecord, compute_snap_id};

impl Workspace {
    pub fn create_snap(&self, message: Option<String>) -> Result<SnapRecord> {
        self.create_snap_with(message, "explicit")
    }

    pub fn create_snap_with(&self, message: Option<String>, trigger: &str) -> Result<SnapRecord> {
        // Validate store format early.
        let cfg = self.store.read_config()?;
        let policy = chunking::chunking_policy_from_config(cfg.chunking.as_ref())?;

        let mut stats = SnapStats::default();
        let root_manifest = self.build_manifest(&self.root, &mut stats, policy)?;

        let parents: Vec<String> = self.store.get_head()?.into_iter().collect();

        // Idempotent recapture (arch 17 §1): a tree identical to the head
        // snap's tree creates nothing — return the head record.
        if let Some(head_id) = parents.first() {
            let head = self.store.get_snap(head_id)?;
            if head.root_manifest == root_manifest {
                return Ok(head);
            }
        }

        let id = compute_snap_id(&root_manifest, &parents, None);

        let created_at = time::OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .context("format created_at")?;
        let snap = SnapRecord {
            version: 2,
            id,
            created_at,
            root_manifest,
            parents,
            derived_from_bundle: None,
            message,
            trigger: trigger.to_string(),
            stats,
        };
        self.store.put_snap(&snap)?;
        self.store.set_head(Some(&snap.id))?;
        Ok(snap)
    }

    /// Lineage order: head-first parent walk, then any snaps unreachable
    /// from head (parallel branches) newest-first by `created_at`.
    pub fn list_snaps(&self) -> Result<Vec<SnapRecord>> {
        let all = self.store.list_snaps()?;
        let by_id: HashMap<String, SnapRecord> =
            all.iter().map(|s| (s.id.clone(), s.clone())).collect();

        let mut out = Vec::new();
        let mut seen = HashSet::new();
        if let Some(head) = self.store.get_head()? {
            let mut stack = vec![head];
            while let Some(id) = stack.pop() {
                if !seen.insert(id.clone()) {
                    continue;
                }
                if let Some(snap) = by_id.get(&id) {
                    // First parent continues the primary lineage; push it
                    // last so it pops first.
                    for parent in snap.parents.iter().rev() {
                        stack.push(parent.clone());
                    }
                    out.push(snap.clone());
                }
            }
        }

        let mut rest: Vec<SnapRecord> = all.into_iter().filter(|s| !seen.contains(&s.id)).collect();
        rest.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        out.extend(rest);
        Ok(out)
    }

    pub fn show_snap(&self, snap_id: &str) -> Result<SnapRecord> {
        self.store.get_snap(snap_id)
    }
}
