//! Lanes and snap records: heads, readability, upload.

use anyhow::{Result, bail};

use converge_model::{LaneHead, SnapRecord};

use crate::authz::{AuthzContext, Capability};

use super::{now, require};

use super::Engine;

impl Engine<'_> {
    /// Push a lane head (unpublished sync). Snap records for the new head's
    /// lineage must already be uploaded; the move must fast-forward from
    /// the current head unless forced.
    pub fn set_lane_head(
        &self,
        authz: AuthzContext,
        lane_id: Option<String>,
        snap_id: &str,
        force: bool,
    ) -> Result<LaneHead> {
        require(&authz, Capability::SnapSync)?;
        let lane_id = self.resolve_writable_lane(&authz, &lane_id)?;

        if self
            .meta
            .get_snap_record(authz.repo_id(), snap_id)?
            .is_none()
        {
            bail!("snap {snap_id} has not been uploaded");
        }
        if let Some(current) = self.meta.get_lane_head(authz.repo_id(), &lane_id)?
            && !force
            && !self.is_ancestor(authz.repo_id(), &current.snap_id, snap_id)?
        {
            bail!(
                "non-fast-forward: {} is not an ancestor of {snap_id} (use force)",
                current.snap_id
            );
        }
        let head = LaneHead {
            lane_id,
            snap_id: snap_id.to_string(),
            updated_at: now(),
        };
        self.meta.set_lane_head(authz.repo_id(), &head)?;
        // The head lineage's trees are now referenced by a lane head:
        // release their upload pins (batch 12.2).
        let mut stack = vec![head.snap_id.clone()];
        let mut walked = std::collections::HashSet::new();
        while let Some(id) = stack.pop() {
            if !walked.insert(id.clone()) {
                continue;
            }
            if let Some(record) = self.meta.get_snap_record(authz.repo_id(), &id)? {
                self.unpin_tree(authz.repo_id(), &record.root_manifest)?;
                stack.extend(record.parents);
            }
        }
        self.meta
            .add_event(authz.repo_id(), "lane", &head.lane_id, &now())?;
        Ok(head)
    }

    /// Is `ancestor` reachable from `descendant` via uploaded snap records?
    fn is_ancestor(&self, repo_id: &str, ancestor: &str, descendant: &str) -> Result<bool> {
        let mut stack = vec![descendant.to_string()];
        let mut seen = std::collections::HashSet::new();
        while let Some(id) = stack.pop() {
            if id == ancestor {
                return Ok(true);
            }
            if !seen.insert(id.clone()) {
                continue;
            }
            if let Some(record) = self.meta.get_snap_record(repo_id, &id)? {
                stack.extend(record.parents.iter().cloned());
            }
        }
        Ok(false)
    }

    /// Read access to a lane: owner/members always; repo-visible lanes for
    /// any subject holding the read capability the caller already proved.
    pub fn check_lane_readable(&self, authz: &AuthzContext, lane_id: &str) -> Result<()> {
        let lane = self
            .meta
            .get_lane(authz.repo_id(), lane_id)?
            .ok_or_else(|| anyhow::anyhow!("lane {lane_id} is not registered"))?;
        let subject = authz.subject().to_string();
        if lane.visibility == "repo" || lane.owner == subject || lane.members.contains(&subject) {
            Ok(())
        } else {
            bail!("lane {lane_id} is private to its owner and members")
        }
    }

    pub fn upload_snap_record(&self, authz: &AuthzContext, snap: &SnapRecord) -> Result<()> {
        // Verify declared identity before storing (mirrors object stores'
        // verify-on-write).
        let expected = converge_model::compute_snap_id(
            &snap.root_manifest,
            &snap.parents,
            snap.derived_from_candidate.as_deref(),
        );
        if expected != snap.id {
            bail!("snap record identity mismatch (expected {expected})");
        }
        // The snap's tree must be present (batch 12.2, audit M4): otherwise
        // a lane head fast-forwarded to it would dangle and never
        // materialize. Thinned *ancestors* may be absent, but the head's own
        // root manifest may not.
        if !self
            .objects
            .has(crate::storage::ObjectKind::Manifest, &snap.root_manifest)
        {
            bail!(
                "snap {} root manifest {} not uploaded",
                snap.id,
                snap.root_manifest.as_str()
            );
        }
        self.meta.put_snap_record(authz.repo_id(), snap)
    }
}
