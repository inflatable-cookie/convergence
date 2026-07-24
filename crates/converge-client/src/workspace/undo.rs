use super::*;

use crate::model::SnapRecord;

/// What `unsnap` did, so the caller can report it precisely.
pub struct Unsnapped {
    pub removed: SnapRecord,
    /// New head — `None` when the undone snap was the first capture.
    pub head: Option<String>,
    /// The record was deleted (not merely unreferenced).
    pub deleted: bool,
}

impl Workspace {
    /// Undo the head capture (batch 16.2, UX spec §4.5 / audit P4.19).
    ///
    /// Undoing a *capture*, not the work: head moves to the first parent
    /// and the working tree is left exactly as it is, so the content
    /// reappears as pending changes. Nothing a user typed is at risk —
    /// which is why this needs no `--force` for the common case.
    ///
    /// Refused when the snap is not a leaf (something was built on it) or
    /// when it has been published (it is part of a shared history now);
    /// `force` overrides the published check only. Anything else would be
    /// rewriting lineage other records already point at.
    pub fn unsnap(&self, keep_record: bool, force: bool) -> Result<Unsnapped> {
        let head_id = self
            .store
            .get_head()?
            .ok_or_else(|| anyhow!("no head snap to undo"))?;
        let head = self.store.get_snap(&head_id)?;

        let all = self.store.list_snaps()?;
        if let Some(child) = all.iter().find(|s| s.parents.contains(&head_id)) {
            anyhow::bail!(
                "refusing to unsnap {}: {} builds on it",
                short(&head_id),
                short(&child.id)
            );
        }

        if !force && self.is_published(&head_id)? {
            anyhow::bail!(
                "refusing to unsnap {}: already published (use --force to undo locally anyway)",
                short(&head_id)
            );
        }

        let parent = head.parents.first().cloned();
        self.store.set_head(parent.as_deref())?;

        // The record is deleted by default: an undone capture that stays
        // in the store shows up in history as an orphan branch, which is
        // the opposite of undo. Objects it referenced stay — they are
        // content-addressed and the working tree still holds that content.
        let deleted = !keep_record;
        if deleted {
            self.store.delete_snap(&head_id)?;
        }

        Ok(Unsnapped {
            removed: head,
            head: parent,
            deleted,
        })
    }

    /// Has this snap been published to any configured remote target?
    fn is_published(&self, snap_id: &str) -> Result<bool> {
        let cfg = self.store.read_config()?;
        let Some(remote) = cfg.remote else {
            return Ok(false);
        };
        // State is keyed per (remote, scope, gate); the configured pair is
        // the one this workspace publishes to.
        Ok(self
            .store
            .get_last_published(&remote, &remote.scope, &remote.gate)?
            .as_deref()
            == Some(snap_id))
    }
}

fn short(id: &str) -> String {
    id.chars().take(8).collect()
}
