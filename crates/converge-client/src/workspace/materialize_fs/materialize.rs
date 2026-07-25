use std::fs;
use std::io::{BufWriter, Write};
use std::path::Path;

use anyhow::{Context, Result, anyhow};

use crate::model::{ManifestEntryKind, ObjectId, SuperpositionVariantKind};
use crate::store::LocalStore;

use super::platform::{create_symlink, set_file_mode};

/// Manifest entry names become filesystem paths, and manifests can come
/// from a remote — treat every name as untrusted (batch 12.1, audit D2).
fn validate_entry_name(name: &str) -> Result<()> {
    let mut components = Path::new(name).components();
    let single_normal = matches!(
        (components.next(), components.next()),
        (Some(std::path::Component::Normal(_)), None)
    );
    // `\` is a separator on Windows and an ordinary filename character
    // everywhere else (batch 18.3). Banning it outright made a snap
    // capturable but not restorable on the very platform that produced
    // it; banning it on Windows only keeps the traversal defence exactly
    // where the risk is. `components()` already rejects `..`, absolute
    // paths, and anything that is not one plain component.
    let windows_separator = cfg!(windows) && name.contains('\\');
    if !single_normal || name.contains('/') || windows_separator || name.contains('\0') {
        return Err(anyhow!(
            "manifest entry name {name:?} is not a single path component"
        ));
    }
    if name == ".converge" || name == ".git" {
        return Err(anyhow!("manifest entry name {name:?} is reserved"));
    }
    Ok(())
}

/// Symlink targets may not be absolute and may not climb above the
/// materialized root: `depth` is how many directories deep the link
/// itself sits.
fn validate_symlink_target(target: &str, depth: usize) -> Result<()> {
    let path = Path::new(target);
    if path.is_absolute() {
        return Err(anyhow!("symlink target {target:?} is absolute"));
    }
    let mut remaining = depth as i64;
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                remaining -= 1;
                if remaining < 0 {
                    return Err(anyhow!(
                        "symlink target {target:?} escapes the materialized root"
                    ));
                }
            }
            std::path::Component::Normal(_) | std::path::Component::CurDir => {}
            _ => return Err(anyhow!("symlink target {target:?} is not relative")),
        }
    }
    Ok(())
}

pub(super) fn materialize_manifest(
    store: &LocalStore,
    manifest_id: &ObjectId,
    out_dir: &Path,
) -> Result<()> {
    materialize_manifest_at_depth(store, manifest_id, out_dir, 0)
}

fn materialize_manifest_at_depth(
    store: &LocalStore,
    manifest_id: &ObjectId,
    out_dir: &Path,
    depth: usize,
) -> Result<()> {
    let manifest = store.get_manifest(manifest_id)?;
    let mut seen = std::collections::HashSet::new();
    for entry in manifest.entries {
        validate_entry_name(&entry.name)?;
        if !seen.insert(entry.name.clone()) {
            return Err(anyhow!(
                "manifest {} names {} twice",
                manifest_id.as_str(),
                entry.name
            ));
        }
        let path = out_dir.join(&entry.name);
        match entry.kind {
            ManifestEntryKind::Dir { manifest } => {
                fs::create_dir_all(&path)
                    .with_context(|| format!("create dir {}", path.display()))?;
                materialize_manifest_at_depth(store, &manifest, &path, depth + 1)?;
            }
            ManifestEntryKind::File { blob, mode, .. } => {
                let bytes = store.get_blob(&blob)?;
                fs::write(&path, &bytes)
                    .with_context(|| format!("write file {}", path.display()))?;
                set_file_mode(&path, mode)?;
            }
            ManifestEntryKind::FileChunks { recipe, mode, size } => {
                materialize_chunked_file(store, &path, &recipe, mode, size)?;
            }
            ManifestEntryKind::Symlink { target } => {
                validate_symlink_target(&target, depth)?;
                create_symlink(&target, &path)?
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

fn materialize_chunked_file(
    store: &LocalStore,
    path: &Path,
    recipe: &ObjectId,
    mode: u32,
    size: u64,
) -> Result<()> {
    let r = store.get_recipe(recipe)?;
    if r.size != size {
        return Err(anyhow!(
            "recipe size mismatch for {} (recipe {}, entry {})",
            path.display(),
            r.size,
            size
        ));
    }

    let f = fs::File::create(path).with_context(|| format!("create file {}", path.display()))?;
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
    set_file_mode(path, mode)?;
    Ok(())
}
