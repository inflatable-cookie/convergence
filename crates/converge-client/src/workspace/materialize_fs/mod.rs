mod clear;
mod materialize;
mod platform;

use std::path::Path;

use anyhow::Result;

use crate::model::ObjectId;
use crate::store::LocalStore;

pub(super) fn is_empty_except_converge_and_git(root: &Path) -> Result<bool> {
    clear::is_empty_except_converge_and_git(root)
}

pub(super) fn is_empty_dir(root: &Path) -> Result<bool> {
    clear::is_empty_dir(root)
}

/// Materialize with destruction deferred until success (batch 12.1,
/// audit D1): the full tree lands in a temp dir inside `dest` first —
/// same filesystem, so the swap is renames — and only then is the
/// destination cleared (preserving `preserve` entries) and the new tree
/// moved in. A failed materialize leaves `dest` untouched.
pub(super) fn materialize_via_temp(
    store: &LocalStore,
    manifest_id: &ObjectId,
    dest: &Path,
    preserve: &[&str],
) -> Result<()> {
    use anyhow::Context;

    std::fs::create_dir_all(dest).with_context(|| format!("create dir {}", dest.display()))?;
    let temp_name = format!(".converge-materialize-{}", std::process::id());
    // Stage inside `.converge` when there is one (batch 18.2): a process
    // killed mid-materialize used to leave the staging tree sitting in
    // the workspace, where the scan counts it as pending changes and the
    // next `snap` captures it. `.converge` is excluded from the scan by
    // construction, so a kill now leaves nothing a user has to notice.
    let internal = dest.join(".converge");
    let temp = if internal.is_dir() {
        internal.join(&temp_name)
    } else {
        dest.join(&temp_name)
    };
    if temp.exists() {
        std::fs::remove_dir_all(&temp)
            .with_context(|| format!("clear stale temp {}", temp.display()))?;
    }
    std::fs::create_dir(&temp).with_context(|| format!("create temp {}", temp.display()))?;

    if let Err(err) = materialize::materialize_manifest(store, manifest_id, &temp) {
        let _ = std::fs::remove_dir_all(&temp);
        return Err(err);
    }

    // Success: destroy-and-swap. Preserve internal dirs and the temp
    // tree itself.
    for entry in std::fs::read_dir(dest).with_context(|| format!("read dir {}", dest.display()))? {
        let entry = entry?;
        let name = entry.file_name();
        if name == temp_name.as_str() || preserve.iter().any(|p| name == *p) {
            continue;
        }
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            std::fs::remove_dir_all(&path)
                .with_context(|| format!("remove dir {}", path.display()))?;
        } else {
            std::fs::remove_file(&path)
                .with_context(|| format!("remove file {}", path.display()))?;
        }
    }
    for entry in
        std::fs::read_dir(&temp).with_context(|| format!("read temp {}", temp.display()))?
    {
        let entry = entry?;
        let target = dest.join(entry.file_name());
        std::fs::rename(entry.path(), &target)
            .with_context(|| format!("move {} into place", target.display()))?;
    }
    std::fs::remove_dir_all(&temp).with_context(|| format!("remove temp {}", temp.display()))?;
    Ok(())
}
