use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

pub(super) fn clear_workspace_except_converge_and_git(root: &Path) -> Result<()> {
    for entry in fs::read_dir(root).with_context(|| format!("read dir {}", root.display()))? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        if name == ".converge" || name == ".git" {
            continue;
        }
        remove_entry(&path, entry.file_type()?)?;
    }
    Ok(())
}

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

pub(super) fn clear_dir(root: &Path) -> Result<()> {
    for entry in fs::read_dir(root).with_context(|| format!("read dir {}", root.display()))? {
        let entry = entry?;
        let path = entry.path();
        remove_entry(&path, entry.file_type()?)?;
    }
    Ok(())
}

fn remove_entry(path: &Path, file_type: fs::FileType) -> Result<()> {
    if file_type.is_dir() {
        fs::remove_dir_all(path).with_context(|| format!("remove dir {}", path.display()))?;
    } else {
        fs::remove_file(path).with_context(|| format!("remove file {}", path.display()))?;
    }
    Ok(())
}
