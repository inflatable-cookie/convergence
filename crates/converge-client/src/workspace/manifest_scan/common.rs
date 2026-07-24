use std::fs;
use std::path::Path;

use anyhow::{Context, Result, anyhow};

/// Root-level ignore patterns from `.convergeignore` (arch doc 18 §3):
/// exact names, `dir/` forms. No negations or nesting — documented.
pub(super) fn load_root_ignores(root: &std::path::Path) -> std::collections::HashSet<String> {
    let mut out = std::collections::HashSet::new();
    if let Ok(text) = std::fs::read_to_string(root.join(".convergeignore")) {
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with('!') {
                continue;
            }
            out.insert(line.trim_end_matches('/').to_string());
        }
    }
    out
}

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

/// Stat → read → re-stat (audit D3): a file changing during capture
/// would otherwise snap torn bytes (silent small-file truncation) or
/// record a stale size. Bounded retries, then a loud failure instead
/// of a torn snapshot.
pub(super) fn read_file_stable(path: &Path) -> Result<(Vec<u8>, u64)> {
    const ATTEMPTS: u32 = 3;
    for _ in 0..ATTEMPTS {
        let before =
            fs::symlink_metadata(path).with_context(|| format!("stat {}", path.display()))?;
        let bytes = fs::read(path).with_context(|| format!("read file {}", path.display()))?;
        let after =
            fs::symlink_metadata(path).with_context(|| format!("stat {}", path.display()))?;
        if before.len() == after.len()
            && bytes.len() as u64 == after.len()
            && before.modified().ok() == after.modified().ok()
        {
            return Ok((bytes, after.len()));
        }
    }
    anyhow::bail!(
        "{} kept changing during capture — retry once writes settle",
        path.display()
    )
}
