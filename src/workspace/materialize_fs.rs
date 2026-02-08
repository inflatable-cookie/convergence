use std::fs;
use std::io::{BufWriter, Write};
use std::path::Path;

use anyhow::{Context, Result, anyhow};

use crate::model::{ManifestEntryKind, ObjectId, SuperpositionVariantKind};
use crate::store::LocalStore;
pub(super) fn clear_workspace_except_converge_and_git(root: &Path) -> Result<()> {
    for entry in fs::read_dir(root).with_context(|| format!("read dir {}", root.display()))? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        if name == ".converge" || name == ".git" {
            continue;
        }
        let ft = entry.file_type()?;
        if ft.is_dir() {
            fs::remove_dir_all(&path).with_context(|| format!("remove dir {}", path.display()))?;
        } else {
            fs::remove_file(&path).with_context(|| format!("remove file {}", path.display()))?;
        }
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
        let ft = entry.file_type()?;
        if ft.is_dir() {
            fs::remove_dir_all(&path).with_context(|| format!("remove dir {}", path.display()))?;
        } else {
            fs::remove_file(&path).with_context(|| format!("remove file {}", path.display()))?;
        }
    }
    Ok(())
}

pub(super) fn materialize_manifest(
    store: &LocalStore,
    manifest_id: &ObjectId,
    out_dir: &Path,
) -> Result<()> {
    let manifest = store.get_manifest(manifest_id)?;
    for entry in manifest.entries {
        let path = out_dir.join(&entry.name);
        match entry.kind {
            ManifestEntryKind::Dir { manifest } => {
                fs::create_dir_all(&path)
                    .with_context(|| format!("create dir {}", path.display()))?;
                materialize_manifest(store, &manifest, &path)?;
            }
            ManifestEntryKind::File { blob, mode, .. } => {
                let bytes = store.get_blob(&blob)?;
                fs::write(&path, &bytes)
                    .with_context(|| format!("write file {}", path.display()))?;
                set_file_mode(&path, mode)?;
            }
            ManifestEntryKind::FileChunks { recipe, mode, size } => {
                let r = store.get_recipe(&recipe)?;
                if r.size != size {
                    return Err(anyhow!(
                        "recipe size mismatch for {} (recipe {}, entry {})",
                        path.display(),
                        r.size,
                        size
                    ));
                }

                let f = fs::File::create(&path)
                    .with_context(|| format!("create file {}", path.display()))?;
                let mut w = BufWriter::new(f);
                for c in r.chunks {
                    let bytes = store.get_blob(&c.blob)?;
                    if bytes.len() != c.size as usize {
                        return Err(anyhow!(
                            "chunk size mismatch for {} (chunk {} expected {}, got {})",
                            path.display(),
                            c.blob.as_str(),
                            c.size,
                            bytes.len()
                        ));
                    }
                    w.write_all(&bytes)
                        .with_context(|| format!("write {}", path.display()))?;
                }
                w.flush()
                    .with_context(|| format!("flush {}", path.display()))?;
                set_file_mode(&path, mode)?;
            }
            ManifestEntryKind::Symlink { target } => {
                create_symlink(&target, &path)?;
            }
            ManifestEntryKind::Superposition { variants } => {
                let mut sources = Vec::new();
                for v in variants {
                    sources.push(match v.kind {
                        SuperpositionVariantKind::Tombstone => format!("{}: tombstone", v.source),
                        SuperpositionVariantKind::File { .. } => format!("{}: file", v.source),
                        SuperpositionVariantKind::FileChunks { .. } => {
                            format!("{}: chunked_file", v.source)
                        }
                        SuperpositionVariantKind::Dir { .. } => format!("{}: dir", v.source),
                        SuperpositionVariantKind::Symlink { .. } => {
                            format!("{}: symlink", v.source)
                        }
                    });
                }
                return Err(anyhow!(
                    "cannot materialize superposition at {} (variants: {})",
                    path.display(),
                    sources.join(", ")
                ));
            }
        }
    }
    Ok(())
}

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
        Err(anyhow!("symlinks are not supported on this platform"))
    }
}
