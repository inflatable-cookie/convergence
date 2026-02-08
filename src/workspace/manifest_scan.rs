use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, anyhow};

use crate::model::{Manifest, ManifestEntry, ManifestEntryKind, ObjectId, SnapStats};
use crate::store::hash_bytes;

use super::Workspace;
use super::chunk_io::{chunk_file_to_recipe_id, chunk_file_to_recipe_store};
use super::chunking::ChunkingPolicy;

impl Workspace {
    pub(super) fn build_manifest(
        &self,
        dir: &Path,
        stats: &mut SnapStats,
        policy: ChunkingPolicy,
    ) -> Result<ObjectId> {
        let mut entries = Vec::new();
        let children = read_dir_sorted(dir)?;

        for child in children {
            let file_name = child
                .file_name()
                .into_string()
                .map_err(|_| anyhow!("non-utf8 filename in {}", dir.display()))?;

            if should_ignore_name(&file_name) {
                continue;
            }

            let path = child.path();
            let file_type = child.file_type().context("read file type")?;

            let kind = if file_type.is_dir() {
                stats.dirs += 1;
                let manifest = self.build_manifest(&path, stats, policy)?;
                ManifestEntryKind::Dir { manifest }
            } else if file_type.is_file() {
                let mode = file_mode(&path)?;
                let meta = fs::symlink_metadata(&path)
                    .with_context(|| format!("stat {}", path.display()))?;
                let size = meta.len();

                let kind = if size >= policy.threshold {
                    let recipe =
                        chunk_file_to_recipe_store(&self.store, &path, size, policy.chunk_size)?;
                    ManifestEntryKind::FileChunks { recipe, mode, size }
                } else {
                    let bytes =
                        fs::read(&path).with_context(|| format!("read file {}", path.display()))?;
                    let blob = self.store.put_blob(&bytes)?;
                    ManifestEntryKind::File { blob, mode, size }
                };

                stats.files += 1;
                stats.bytes += size;
                kind
            } else if file_type.is_symlink() {
                let target = fs::read_link(&path)
                    .with_context(|| format!("read symlink {}", path.display()))?;
                let target = target
                    .to_str()
                    .ok_or_else(|| anyhow!("non-utf8 symlink target for {}", path.display()))?
                    .to_string();
                stats.symlinks += 1;
                ManifestEntryKind::Symlink { target }
            } else {
                continue;
            };

            entries.push(ManifestEntry {
                name: file_name,
                kind,
            });
        }

        entries.sort_by(|a, b| a.name.cmp(&b.name));
        let manifest = Manifest {
            version: 1,
            entries,
        };
        self.store.put_manifest(&manifest)
    }
}

pub(super) fn build_manifest_in_memory(
    dir: &Path,
    stats: &mut SnapStats,
    manifests: &mut HashMap<ObjectId, Manifest>,
    policy: ChunkingPolicy,
) -> Result<ObjectId> {
    let mut entries = Vec::new();
    let children = read_dir_sorted(dir)?;

    for child in children {
        let file_name = child
            .file_name()
            .into_string()
            .map_err(|_| anyhow!("non-utf8 filename in {}", dir.display()))?;

        if should_ignore_name(&file_name) {
            continue;
        }

        let path = child.path();
        let file_type = child.file_type().context("read file type")?;

        let kind = if file_type.is_dir() {
            stats.dirs += 1;
            let manifest = build_manifest_in_memory(&path, stats, manifests, policy)?;
            ManifestEntryKind::Dir { manifest }
        } else if file_type.is_file() {
            let mode = file_mode(&path)?;
            let meta =
                fs::symlink_metadata(&path).with_context(|| format!("stat {}", path.display()))?;
            let size = meta.len();

            let kind = if size >= policy.threshold {
                let recipe = chunk_file_to_recipe_id(&path, size, policy.chunk_size)?;
                ManifestEntryKind::FileChunks { recipe, mode, size }
            } else {
                let bytes =
                    fs::read(&path).with_context(|| format!("read file {}", path.display()))?;
                let blob = hash_bytes(&bytes);
                ManifestEntryKind::File { blob, mode, size }
            };

            stats.files += 1;
            stats.bytes += size;
            kind
        } else if file_type.is_symlink() {
            let target =
                fs::read_link(&path).with_context(|| format!("read symlink {}", path.display()))?;
            let target = target
                .to_str()
                .ok_or_else(|| anyhow!("non-utf8 symlink target for {}", path.display()))?
                .to_string();
            stats.symlinks += 1;
            ManifestEntryKind::Symlink { target }
        } else {
            continue;
        };

        entries.push(ManifestEntry {
            name: file_name,
            kind,
        });
    }

    entries.sort_by(|a, b| a.name.cmp(&b.name));
    let manifest = Manifest {
        version: 1,
        entries,
    };
    let bytes = serde_json::to_vec(&manifest).context("serialize manifest")?;
    let id = hash_bytes(&bytes);
    manifests.insert(id.clone(), manifest);
    Ok(id)
}

fn should_ignore_name(name: &str) -> bool {
    matches!(name, ".converge" | ".git")
}

fn read_dir_sorted(dir: &Path) -> Result<Vec<fs::DirEntry>> {
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

#[cfg(unix)]
fn os_str_bytes(s: &std::ffi::OsStr) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    s.as_bytes().to_vec()
}

#[cfg(not(unix))]
fn os_str_bytes(s: &std::ffi::OsStr) -> Vec<u8> {
    s.to_string_lossy().as_bytes().to_vec()
}

fn file_mode(path: &Path) -> Result<u32> {
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
