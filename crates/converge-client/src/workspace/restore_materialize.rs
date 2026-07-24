use super::*;

impl Workspace {
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
            } else if materialize_fs::is_empty_except_converge_and_git(&self.root)? {
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

        // Destruction deferred until the target fully materializes
        // (batch 12.1): a superposed or unfetchable target must not
        // cost the current tree.
        materialize_fs::materialize_via_temp(
            &self.store,
            &snap.root_manifest,
            &self.root,
            &[".converge", ".git"],
        )?;
        self.store.set_head(Some(&snap.id))?;
        Ok(())
    }

    /// Materialize a snap into a separate directory (does not create a workspace).
    pub fn materialize_snap_to(&self, snap_id: &str, out_dir: &Path, force: bool) -> Result<()> {
        let snap = self.store.get_snap(snap_id)?;
        ensure_output_dir_ready(out_dir, force)?;
        materialize_fs::materialize_via_temp(&self.store, &snap.root_manifest, out_dir, &[])?;
        Ok(())
    }

    /// Materialize a manifest tree into a separate directory (does not create a workspace).
    pub fn materialize_manifest_to(
        &self,
        root_manifest: &ObjectId,
        out_dir: &Path,
        force: bool,
    ) -> Result<()> {
        ensure_output_dir_ready(out_dir, force)?;
        materialize_fs::materialize_via_temp(&self.store, root_manifest, out_dir, &[])?;
        Ok(())
    }
}

/// Refuse a non-empty destination without `--force`; the clear itself is
/// deferred to the post-materialize swap.
fn ensure_output_dir_ready(out_dir: &Path, force: bool) -> Result<()> {
    if out_dir.exists() && !force && !materialize_fs::is_empty_dir(out_dir)? {
        anyhow::bail!(
            "destination is not empty: {} (use --force)",
            out_dir.display()
        );
    }
    if !out_dir.exists() {
        fs::create_dir_all(out_dir).with_context(|| format!("create dir {}", out_dir.display()))?;
    }
    Ok(())
}
