use std::path::Path;

use anyhow::{Context, Result, anyhow};

use super::Workspace;
use super::manifest_scan::common::{
    is_ignored, load_root_ignores, read_dir_sorted, should_ignore_name,
};

/// Cheap change detector for the working tree (batch 15.3).
///
/// Walks the same set of paths `current_manifest_tree` would, but stats
/// instead of reading: name, kind, mode, size, and mtime. A long-lived
/// front-end (the TUI) compares stamps to decide whether a full
/// content-hashing rescan is worth doing at all.
///
/// Not a substitute for the scan: mtime granularity means a write that
/// lands in the same tick as the last stamp and keeps the size identical
/// is invisible here. That is why the stamp only ever gates a *cache*
/// whose miss path is the real scan — snap and publish always rescan.
impl Workspace {
    pub fn dirstamp(&self) -> Result<String> {
        let ignores = load_root_ignores(&self.root);
        let mut hasher = blake3::Hasher::new();
        stamp_dir(&self.root, &self.root, &ignores, &mut hasher)?;
        Ok(hasher.finalize().to_hex().to_string())
    }
}

fn stamp_dir(
    scan_root: &Path,
    dir: &Path,
    root_ignores: &std::collections::HashSet<String>,
    hasher: &mut blake3::Hasher,
) -> Result<()> {
    for child in read_dir_sorted(dir)? {
        let name = child
            .file_name()
            .into_string()
            .map_err(|_| anyhow!("non-utf8 filename in {}", dir.display()))?;
        if should_ignore_name(&name) {
            continue;
        }
        let path = child.path();
        // The same rule the scan uses (batch 22.4). A dirstamp that
        // disagreed with the scan would either miss changes or force a
        // rescan on every tick — the cache would be worse than none.
        if is_ignored(
            root_ignores,
            path.strip_prefix(scan_root).unwrap_or(&path),
            &name,
        ) {
            continue;
        }

        let file_type = child.file_type().context("read file type")?;
        hasher.update(name.as_bytes());

        if file_type.is_dir() {
            hasher.update(b"d");
            stamp_dir(scan_root, &path, root_ignores, hasher)?;
            hasher.update(b"/");
            continue;
        }
        if file_type.is_symlink() {
            hasher.update(b"l");
            hasher.update(std::fs::read_link(&path)?.to_string_lossy().as_bytes());
            continue;
        }
        if !file_type.is_file() {
            continue;
        }

        let meta =
            std::fs::symlink_metadata(&path).with_context(|| format!("stat {}", path.display()))?;
        hasher.update(b"f");
        hasher.update(&meta.len().to_le_bytes());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            hasher.update(&meta.permissions().mode().to_le_bytes());
        }
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        hasher.update(&mtime.to_le_bytes());
    }
    Ok(())
}
