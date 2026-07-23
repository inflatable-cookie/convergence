use std::fs;
use std::path::Path;

use anyhow::{Context, Result, anyhow};

use crate::model::{Manifest, ManifestEntry, ManifestEntryKind, ObjectId, SnapStats};

use super::super::Workspace;
use super::super::chunk_io::chunk_file_to_recipe_store;
use super::super::chunking::ChunkingPolicy;
use super::common::{file_mode, read_dir_sorted, should_ignore_name, symlink_target};

pub(super) fn build_manifest_store_impl(
    workspace: &Workspace,
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
            let manifest = build_manifest_store_impl(workspace, &path, stats, policy)?;
            ManifestEntryKind::Dir { manifest }
        } else if file_type.is_file() {
            let mode = file_mode(&path)?;
            let meta =
                fs::symlink_metadata(&path).with_context(|| format!("stat {}", path.display()))?;
            let size = meta.len();

            let kind = if size >= policy.threshold {
                let recipe =
                    chunk_file_to_recipe_store(&workspace.store, &path, size, policy.chunk_size)?;
                ManifestEntryKind::FileChunks { recipe, mode, size }
            } else {
                let bytes =
                    fs::read(&path).with_context(|| format!("read file {}", path.display()))?;
                let blob = workspace.store.put_blob(&bytes)?;
                ManifestEntryKind::File { blob, mode, size }
            };

            stats.files += 1;
            stats.bytes += size;
            kind
        } else if file_type.is_symlink() {
            let target = symlink_target(&path)?;
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
    workspace.store.put_manifest(&manifest)
}
