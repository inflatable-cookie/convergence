use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

pub(super) fn set_file_mode(path: &Path, mode: u32) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perm = fs::Permissions::from_mode(mode);
        fs::set_permissions(path, perm)
            .with_context(|| format!("set permissions {}", path.display()))?;
        Ok(())
    }

    #[cfg(not(unix))]
    {
        let _ = (path, mode);
        Ok(())
    }
}

pub(super) fn create_symlink(target: &str, link_path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        symlink(target, link_path)
            .with_context(|| format!("create symlink {} -> {}", link_path.display(), target))?;
        Ok(())
    }

    #[cfg(not(unix))]
    {
        let _ = (target, link_path);
        Err(anyhow::anyhow!(
            "symlinks are not supported on this platform"
        ))
    }
}
