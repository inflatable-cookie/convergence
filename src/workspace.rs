use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use time::format_description::well_known::Rfc3339;

use crate::model::{Manifest, ObjectId, SnapRecord, SnapStats, compute_snap_id};
use crate::store::LocalStore;

mod chunk_io;
mod chunking;
mod gc;
use self::chunking::chunking_policy_from_config;
use self::gc::collect_reachable_objects;
mod manifest_scan;
use self::manifest_scan::build_manifest_in_memory;
mod materialize_fs;
mod path_ops;
use self::materialize_fs::{
    clear_dir, clear_workspace_except_converge_and_git, is_empty_dir,
    is_empty_except_converge_and_git, materialize_manifest,
};

#[derive(Clone)]
pub struct Workspace {
    pub root: PathBuf,
    pub store: LocalStore,
}

impl Workspace {
    pub fn init(root: &Path, force: bool) -> Result<Self> {
        let store = LocalStore::init(root, force)?;
        Ok(Self {
            root: root.to_path_buf(),
            store,
        })
    }

    pub fn discover(start: &Path) -> Result<Self> {
        let start = start
            .canonicalize()
            .with_context(|| format!("canonicalize {}", start.display()))?;
        for dir in start.ancestors() {
            let converge_dir = LocalStore::converge_dir(dir);
            if converge_dir.is_dir() {
                let store = LocalStore::open(dir)?;
                return Ok(Self {
                    root: dir.to_path_buf(),
                    store,
                });
            }
        }
        Err(anyhow!(
            "No .converge directory found (run `converge init`)"
        ))
    }

    pub fn create_snap(&self, message: Option<String>) -> Result<SnapRecord> {
        // Validate store format early.
        let cfg = self.store.read_config()?;
        let policy = chunking_policy_from_config(cfg.chunking.as_ref())?;

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

    pub fn restore_snap(&self, snap_id: &str, force: bool) -> Result<()> {
        let snap = self.store.get_snap(snap_id)?;

        if !force {
            let (cur_root, _cur_manifests, _stats) = self.current_manifest_tree()?;

            if let Some(head_id) = self.store.get_head()? {
                let head_snap = self.store.get_snap(&head_id)?;
                if cur_root != head_snap.root_manifest {
                    let short = head_id.chars().take(8).collect::<String>();
                    return Err(anyhow!(
                        "Refusing to restore: workspace has changes since {} (use --force)",
                        short
                    ));
                }
            } else if is_empty_except_converge_and_git(&self.root)? {
                // Empty workspace: allow restore.
            } else {
                // No HEAD: try to infer one from the current workspace state.
                let snaps = self.list_snaps()?;
                let matching = snaps
                    .into_iter()
                    .find(|s| s.root_manifest == cur_root)
                    .map(|s| s.id);
                let Some(head_id) = matching else {
                    return Err(anyhow!(
                        "No HEAD snap and workspace doesn't match any known snap (use --force)"
                    ));
                };
                self.store.set_head(Some(&head_id))?;
            }
        }

        clear_workspace_except_converge_and_git(&self.root)?;

        materialize_manifest(&self.store, &snap.root_manifest, &self.root)?;
        self.store.set_head(Some(&snap.id))?;
        Ok(())
    }

    /// Materialize a snap into a separate directory (does not create a workspace).
    pub fn materialize_snap_to(&self, snap_id: &str, out_dir: &Path, force: bool) -> Result<()> {
        let snap = self.store.get_snap(snap_id)?;

        if out_dir.exists() {
            if !force {
                if !is_empty_dir(out_dir)? {
                    anyhow::bail!(
                        "destination is not empty: {} (use --force)",
                        out_dir.display()
                    );
                }
            } else {
                clear_dir(out_dir)?;
            }
        } else {
            fs::create_dir_all(out_dir)
                .with_context(|| format!("create dir {}", out_dir.display()))?;
        }

        materialize_manifest(&self.store, &snap.root_manifest, out_dir)?;
        Ok(())
    }

    /// Materialize a manifest tree into a separate directory (does not create a workspace).
    pub fn materialize_manifest_to(
        &self,
        root_manifest: &ObjectId,
        out_dir: &Path,
        force: bool,
    ) -> Result<()> {
        if out_dir.exists() {
            if !force {
                if !is_empty_dir(out_dir)? {
                    anyhow::bail!(
                        "destination is not empty: {} (use --force)",
                        out_dir.display()
                    );
                }
            } else {
                clear_dir(out_dir)?;
            }
        } else {
            fs::create_dir_all(out_dir)
                .with_context(|| format!("create dir {}", out_dir.display()))?;
        }

        materialize_manifest(&self.store, root_manifest, out_dir)?;
        Ok(())
    }

    /// Compute a manifest tree for the current working directory without writing a snap.
    ///
    /// Note: this still reads file contents to compute stable blob ids.
    pub fn current_manifest_tree(
        &self,
    ) -> Result<(ObjectId, HashMap<ObjectId, Manifest>, SnapStats)> {
        let cfg = self.store.read_config()?;
        let policy = chunking_policy_from_config(cfg.chunking.as_ref())?;
        let mut stats = SnapStats::default();
        let mut manifests: HashMap<ObjectId, Manifest> = HashMap::new();
        let root_manifest =
            build_manifest_in_memory(&self.root, &mut stats, &mut manifests, policy)?;
        Ok((root_manifest, manifests, stats))
    }
}

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
