use super::*;

impl Workspace {
    pub fn restore_snap(&self, snap_id: &str, force: bool) -> Result<()> {
        let snap = self.store.get_snap(snap_id)?;
        self.ensure_safe_to_overwrite(force)?;

        // Destruction deferred until the target fully materializes
        // (batch 12.1): a superposed or unfetchable target must not
        // cost the current tree.
        let preserve = self.preserved_entries();
        let preserve: Vec<&str> = preserve.iter().map(String::as_str).collect();
        materialize_fs::materialize_via_temp(
            &self.store,
            &snap.root_manifest,
            &self.root,
            &preserve,
        )?;
        self.store.set_head(Some(&snap.id))?;
        Ok(())
    }

    /// Materialize a stored tree into the workspace and capture it as a
    /// snap (batch 16.1) — the "continue from this tree" move that
    /// `resolve apply` and bundle checkout both need.
    ///
    /// Doc 17 §1 splits the two halves deliberately: materializing alone
    /// does not move head, so head moves here only because the capture
    /// happens too, and the workspace genuinely holds that tree.
    pub fn adopt_tree(
        &self,
        root_manifest: &ObjectId,
        message: Option<String>,
        derived_from_bundle: Option<&str>,
        force: bool,
    ) -> Result<crate::model::SnapRecord> {
        self.ensure_safe_to_overwrite(force)?;
        let preserve = self.preserved_entries();
        let preserve: Vec<&str> = preserve.iter().map(String::as_str).collect();
        materialize_fs::materialize_via_temp(&self.store, root_manifest, &self.root, &preserve)?;
        let snap = self.capture_tree(root_manifest, message, derived_from_bundle)?;
        self.store.set_head(Some(&snap.id))?;
        Ok(snap)
    }

    /// Entries a workspace materialize must leave alone: the internals,
    /// plus everything `.convergeignore` claims (batch 18.4).
    ///
    /// Ignored paths are build output, caches, and local scratch — the
    /// user has said they are not project content, so a snap never held
    /// them and a restore has nothing to put back. Deleting them anyway
    /// destroys expensive local state to no purpose, and it is not what
    /// checking out a revision means anywhere else.
    fn preserved_entries(&self) -> Vec<String> {
        let mut preserve = vec![".converge".to_string(), ".git".to_string()];
        preserve.extend(super::manifest_scan::common::load_root_ignores(&self.root));
        preserve
    }

    /// Refuse to overwrite a workspace carrying uncaptured work.
    fn ensure_safe_to_overwrite(&self, force: bool) -> Result<()> {
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
