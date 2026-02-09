use std::fs;
use std::path::Path;

use anyhow::{Context, Result, anyhow};

pub(super) fn should_ignore_name(name: &str) -> bool {
    matches!(name, ".converge" | ".git")
}

pub(super) fn read_dir_sorted(dir: &Path) -> Result<Vec<fs::DirEntry>> {
    let mut entries: Vec<fs::DirEntry> = fs::read_dir(dir)
        .with_context(|| format!("read dir {}", dir.display()))?
        .collect::<std::result::Result<_, _>>()
        .with_context(|| format!("collect dir entries for {}", dir.display()))?;

    entries.sort_by(|a, b| {
        let a = a.file_name();
        let b = b.file_name();
        os_str_bytes(&a).cmp(&os_str_bytes(&b))
    });
    Ok(entries)
}

pub(super) fn symlink_target(path: &Path) -> Result<String> {
    let target = fs::read_link(path).with_context(|| format!("read symlink {}", path.display()))?;
    target
        .to_str()
        .ok_or_else(|| anyhow!("non-utf8 symlink target for {}", path.display()))
        .map(ToString::to_string)
}

pub(super) fn file_mode(path: &Path) -> Result<u32> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let meta =
            fs::symlink_metadata(path).with_context(|| format!("stat {}", path.display()))?;
        Ok(meta.permissions().mode())
    }

    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(0)
    }
}

#[cfg(unix)]
fn os_str_bytes(s: &std::ffi::OsStr) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    s.as_bytes().to_vec()
}

#[cfg(not(unix))]
fn os_str_bytes(s: &std::ffi::OsStr) -> Vec<u8> {
    s.to_string_lossy().as_bytes().to_vec()
}
