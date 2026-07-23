use std::fs;
use std::io::{BufWriter, Write};
use std::path::Path;

use anyhow::{Context, Result, anyhow};

use crate::model::{ManifestEntryKind, ObjectId, SuperpositionVariantKind};
use crate::store::LocalStore;

use super::platform::{create_symlink, set_file_mode};

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
                materialize_chunked_file(store, &path, &recipe, mode, size)?;
            }
            ManifestEntryKind::Symlink { target } => create_symlink(&target, &path)?,
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
