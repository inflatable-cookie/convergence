use std::collections::{HashMap, HashSet};

use super::*;

use time::format_description::well_known::Rfc3339;

use crate::model::{SnapRecord, compute_snap_id};

impl Workspace {
    pub fn create_snap(&self, message: Option<String>) -> Result<SnapRecord> {
        self.create_snap_with(message, "explicit")
    }

    /// Capture a tree that is already in the store — a fetched bundle or
    /// the output of a resolution — as a snap (batch 16.1).
    ///
    /// Doc 17 §1: the bundle is a provenance edge (`derived_from_bundle`),
    /// never a parent; the first parent is the workspace head, because
    /// this is the workspace continuing, not a new history.
    ///
    /// Head does **not** move: doc 17 §1 ties head to what the workspace
    /// actually holds, and this records a tree that may not be checked
    /// out. `adopt_tree` materializes first and then moves head.
    /// Recapturing the head's own tree with the same provenance returns
    /// the head record, matching `create_snap`'s idempotence.
    pub fn capture_tree(
        &self,
        root_manifest: &ObjectId,
        message: Option<String>,
        derived_from_bundle: Option<&str>,
    ) -> Result<SnapRecord> {
        let parents: Vec<String> = self.store.get_head()?.into_iter().collect();
        let message = message
            .map(|m| m.trim().to_string())
            .filter(|m| !m.is_empty());

        if let Some(head_id) = parents.first() {
            let head = self.store.get_snap(head_id)?;
            if &head.root_manifest == root_manifest
                && head.derived_from_bundle.as_deref() == derived_from_bundle
            {
                return Ok(head);
            }
        }

        let id = compute_snap_id(root_manifest, &parents, derived_from_bundle);
        let snap = SnapRecord {
            version: 2,
            id,
            created_at: time::OffsetDateTime::now_utc()
                .format(&Rfc3339)
                .context("format created_at")?,
            root_manifest: root_manifest.clone(),
            parents,
            derived_from_bundle: derived_from_bundle.map(str::to_string),
            message,
            trigger: "explicit".to_string(),
            stats: self.stats_for_root(root_manifest)?,
        };
        self.store.put_snap(&snap)?;
        Ok(snap)
    }

    /// Stats for a stored tree. The scan path counts as it hashes; a tree
    /// that arrived from the wire has to be walked once to say the same
    /// thing. Superpositions count as one entry — an unresolved path is
    /// one thing in the tree, whatever it holds.
    fn stats_for_root(&self, root: &ObjectId) -> Result<SnapStats> {
        let mut stats = SnapStats::default();
        let mut stack = vec![root.clone()];
        while let Some(id) = stack.pop() {
            stats.dirs += 1;
            for entry in self.store.get_manifest(&id)?.entries {
                match entry.kind {
                    crate::model::ManifestEntryKind::Dir { manifest } => stack.push(manifest),
                    crate::model::ManifestEntryKind::File { size, .. }
                    | crate::model::ManifestEntryKind::FileChunks { size, .. } => {
                        stats.files += 1;
                        stats.bytes += size;
                    }
                    crate::model::ManifestEntryKind::Symlink { .. } => stats.symlinks += 1,
                    crate::model::ManifestEntryKind::Superposition { .. } => stats.files += 1,
                }
            }
        }
        // The root itself is not a directory entry.
        stats.dirs -= 1;
        Ok(stats)
    }

    pub fn create_snap_with(&self, message: Option<String>, trigger: &str) -> Result<SnapRecord> {
        // Validate store format early.
        let cfg = self.store.read_config()?;
        let policy = chunking::chunking_policy_from_config(cfg.chunking.as_ref())?;

        let mut stats = SnapStats::default();
        let root_manifest = self.build_manifest(&self.root, &mut stats, policy)?;

        let parents: Vec<String> = self.store.get_head()?.into_iter().collect();

        // Idempotent recapture (arch 17 §1): a tree identical to the head
        // snap's tree creates nothing — return the head record. An
        // explicit message still lands on it rather than being silently
        // dropped (batch 13.4, audit C3); messages are editable metadata,
        // so this needs no new lineage node.
        let message = message
            .map(|m| m.trim().to_string())
            .filter(|m| !m.is_empty());
        if let Some(head_id) = parents.first() {
            let mut head = self.store.get_snap(head_id)?;
            if head.root_manifest == root_manifest {
                if let Some(message) = message
                    && head.message.as_ref() != Some(&message)
                {
                    self.store
                        .update_snap_message(&head.id, Some(message.as_str()))?;
                    head.message = Some(message);
                }
                return Ok(head);
            }
        } else if message.is_none() {
            // No HEAD (fresh or detached workspace): dedup against an
            // existing parentless capture of the same tree, so repeated
            // auto-captures do not pile up identical records.
            if let Some(existing) = self
                .store
                .list_snaps()?
                .into_iter()
                .filter(|s| s.parents.is_empty() && s.root_manifest == root_manifest)
                .min_by(|a, b| a.created_at.cmp(&b.created_at))
            {
                self.store.set_head(Some(&existing.id))?;
                return Ok(existing);
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
