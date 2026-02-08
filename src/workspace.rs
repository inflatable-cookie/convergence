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
pub use self::gc::GcReport;
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
