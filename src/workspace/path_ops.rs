use std::fs;
use std::path::Path;

use anyhow::{Context, Result, anyhow};

use super::Workspace;

impl Workspace {
    /// Move/rename a path within the workspace.
    ///
    /// This is meant to be a safe helper for case-insensitive filesystems where
    /// case-only renames can be awkward (we force a two-step rename when needed).
    pub fn move_path(&self, from: &Path, to: &Path) -> Result<()> {
        fn reject_path(p: &Path) -> Result<()> {
            if p.is_absolute() {
                anyhow::bail!("path must be relative to the workspace root");
            }
            for c in p.components() {
                match c {
                    std::path::Component::ParentDir | std::path::Component::RootDir => {
                        anyhow::bail!("path may not contain '..' or be rooted")
                    }
                    _ => {}
                }
            }
            Ok(())
        }

        reject_path(from)?;
        reject_path(to)?;

        if from == to {
            return Ok(());
        }

        // Disallow messing with internal dirs.
        if from.starts_with(".converge") || to.starts_with(".converge") {
            anyhow::bail!("refusing to move .converge");
        }
        if from.starts_with(".git") || to.starts_with(".git") {
            anyhow::bail!("refusing to move .git");
        }

        let from_abs = self.root.join(from);
        let to_abs = self.root.join(to);

        if !from_abs.exists() {
            anyhow::bail!("source does not exist: {}", from.display());
        }
        if to_abs.exists() {
            anyhow::bail!("destination already exists: {}", to.display());
        }

        if let Some(parent) = to_abs.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create parent dirs {}", parent.display()))?;
        }

        // For case-only renames on case-insensitive FS, force a two-step rename so
        // the directory entry actually changes.
        let from_s = from.to_string_lossy();
        let to_s = to.to_string_lossy();
        let is_case_only = from_s.to_lowercase() == to_s.to_lowercase() && from_s != to_s;

        if is_case_only {
            let parent = from_abs
                .parent()
                .ok_or_else(|| anyhow!("missing parent for {}", from_abs.display()))?;
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let tmp_name = format!(".converge_tmp_{}_{}", std::process::id(), nanos);
            let tmp_abs = parent.join(tmp_name);
            fs::rename(&from_abs, &tmp_abs)
                .with_context(|| format!("rename {} -> tmp", from_abs.display()))?;
            fs::rename(&tmp_abs, &to_abs)
                .with_context(|| format!("rename tmp -> {}", to_abs.display()))?;
        } else {
            fs::rename(&from_abs, &to_abs).with_context(|| {
                format!("rename {} -> {}", from_abs.display(), to_abs.display())
            })?;
        }

        Ok(())
    }
}
