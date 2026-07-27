use std::fs;
use std::path::Path;

use anyhow::{Context, Result, anyhow};

/// Root-level ignore patterns from `.convergeignore` (arch doc 18 §3):
/// exact names, `dir/` forms. No negations or nesting — documented.
pub(in crate::workspace) fn load_root_ignores(
    root: &std::path::Path,
) -> std::collections::HashSet<String> {
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

pub(in crate::workspace) fn should_ignore_name(name: &str) -> bool {
    matches!(name, ".converge" | ".git")
}

/// Does `.convergeignore` exclude this entry?
///
/// A bare name matches **at any depth**, the way `.gitignore` does. A rule
/// containing a slash is anchored to the workspace root.
///
/// Batch 22.4 found why this matters, on the first real project: rules
/// were matched only against the top level, so `target` excluded a root
/// build directory and silently captured `crates/todo-core/target` — 18 MB
/// and some seventeen hundred files, in a project with about forty real
/// ones. Every Rust workspace with nested crates and every JS monorepo
/// hits that immediately.
pub(in crate::workspace) fn is_ignored(
    ignores: &std::collections::HashSet<String>,
    relative: &Path,
    name: &str,
) -> bool {
    if ignores.contains(name) {
        return true;
    }
    // Anchored rules: compare against the path from the workspace root,
    // with `/` separators so a rule reads the same on every platform.
    let rel = relative
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    ignores
        .iter()
        .any(|rule| rule.contains('/') && rule == &rel)
}

pub(in crate::workspace) fn read_dir_sorted(dir: &Path) -> Result<Vec<fs::DirEntry>> {
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
