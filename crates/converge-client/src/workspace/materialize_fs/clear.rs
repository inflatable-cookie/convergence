use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

pub(super) fn is_empty_except_converge_and_git(root: &Path) -> Result<bool> {
    for entry in fs::read_dir(root).with_context(|| format!("read dir {}", root.display()))? {
        let entry = entry?;
        let name = entry.file_name();
        if name == ".converge" || name == ".git" {
            continue;
        }
        return Ok(false);
    }
    Ok(true)
}

pub(super) fn is_empty_dir(root: &Path) -> Result<bool> {
    let mut it = fs::read_dir(root).with_context(|| format!("read dir {}", root.display()))?;
    if let Some(entry) = it.next() {
        let _ = entry?;
        return Ok(false);
    }
    Ok(true)
}
