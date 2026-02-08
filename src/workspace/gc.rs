use anyhow::Result;

use crate::model::{ManifestEntryKind, ObjectId, SuperpositionVariant, SuperpositionVariantKind};
use crate::store::LocalStore;

use super::Workspace;

#[derive(Clone, Debug, Default)]
pub struct GcReport {
    pub kept_snaps: usize,
    pub pruned_snaps: usize,
    pub deleted_blobs: usize,
    pub deleted_manifests: usize,
    pub deleted_recipes: usize,
}

impl Workspace {
    pub fn gc_local(&self, dry_run: bool) -> Result<GcReport> {
        let cfg = self.store.read_config()?;
        let retention = cfg.retention.unwrap_or_default();

        let mut snaps = self.store.list_snaps()?;
        snaps.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        let head = self.store.get_head()?;
        let now = time::OffsetDateTime::now_utc();
        let keep_days = retention.keep_days;
        let keep_last = retention.keep_last;

        let mut keep = std::collections::HashSet::new();
        for s in &retention.pinned {
            keep.insert(s.clone());
        }
        if let Some(h) = head {
            keep.insert(h);
        }
        if let Some(n) = keep_last {
            for s in snaps.iter().take(n as usize) {
                keep.insert(s.id.clone());
            }
        }
        if let Some(days) = keep_days {
            let cutoff = now - time::Duration::days(days as i64);
            for s in &snaps {
                if let Ok(ts) = time::OffsetDateTime::parse(
                    &s.created_at,
                    &time::format_description::well_known::Rfc3339,
                ) && ts >= cutoff
                {
                    keep.insert(s.id.clone());
                }
            }
        }
        if keep.is_empty() {
            // Safety: keep the newest snap if nothing else matches.
            if let Some(s) = snaps.first() {
                keep.insert(s.id.clone());
            }
        }

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

        for id in self.store.list_blob_ids()? {
            if !keep_blobs.contains(id.as_str()) {
                report.deleted_blobs += 1;
                if !dry_run {
                    self.store.delete_blob(&id)?;
                }
            }
        }
        for id in self.store.list_manifest_ids()? {
            if !keep_manifests.contains(id.as_str()) {
                report.deleted_manifests += 1;
                if !dry_run {
                    self.store.delete_manifest(&id)?;
                }
            }
        }
        for id in self.store.list_recipe_ids()? {
            if !keep_recipes.contains(id.as_str()) {
                report.deleted_recipes += 1;
                if !dry_run {
                    self.store.delete_recipe(&id)?;
                }
            }
        }

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

pub(super) fn collect_reachable_objects(
    store: &LocalStore,
    root: &ObjectId,
    blobs: &mut std::collections::HashSet<String>,
    manifests: &mut std::collections::HashSet<String>,
    recipes: &mut std::collections::HashSet<String>,
) -> Result<()> {
    let mut stack = vec![root.clone()];
    while let Some(mid) = stack.pop() {
        if !manifests.insert(mid.as_str().to_string()) {
            continue;
        }
        let m = store.get_manifest(&mid)?;
        for e in m.entries {
            match e.kind {
                ManifestEntryKind::Dir { manifest } => {
                    stack.push(manifest);
                }
                ManifestEntryKind::File { blob, .. } => {
                    blobs.insert(blob.as_str().to_string());
                }
                ManifestEntryKind::FileChunks { recipe, .. } => {
                    let rid = recipe.as_str().to_string();
                    if recipes.insert(rid) {
                        let r = store.get_recipe(&recipe)?;
                        for c in r.chunks {
                            blobs.insert(c.blob.as_str().to_string());
                        }
                    }
                }
                ManifestEntryKind::Symlink { .. } => {}
                ManifestEntryKind::Superposition { variants } => {
                    for v in variants {
                        collect_variant_objects(store, &v, blobs, manifests, recipes, &mut stack)?;
                    }
                }
            }
        }
    }
    Ok(())
}

fn collect_variant_objects(
    store: &LocalStore,
    v: &SuperpositionVariant,
    blobs: &mut std::collections::HashSet<String>,
    _manifests: &mut std::collections::HashSet<String>,
    recipes: &mut std::collections::HashSet<String>,
    stack: &mut Vec<ObjectId>,
) -> Result<()> {
    match &v.kind {
        SuperpositionVariantKind::File { blob, .. } => {
            blobs.insert(blob.as_str().to_string());
        }
        SuperpositionVariantKind::FileChunks { recipe, .. } => {
            let rid = recipe.as_str().to_string();
            if recipes.insert(rid) {
                let r = store.get_recipe(recipe)?;
                for c in r.chunks {
                    blobs.insert(c.blob.as_str().to_string());
                }
            }
        }
        SuperpositionVariantKind::Dir { manifest } => {
            stack.push(manifest.clone());
        }
        SuperpositionVariantKind::Symlink { .. } => {}
        SuperpositionVariantKind::Tombstone => {}
    }
    Ok(())
}
