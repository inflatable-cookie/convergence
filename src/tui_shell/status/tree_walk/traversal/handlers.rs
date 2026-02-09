use anyhow::Result;

use crate::model::{Manifest, ManifestEntryKind, ObjectId};
use crate::store::LocalStore;

use super::super::StatusDelta;
use super::super::leaves::{collect_leaves_base, collect_leaves_current};
use super::diff_dir;

pub(super) fn handle_added(
    path: &str,
    kind: &ManifestEntryKind,
    cur_manifests: &std::collections::HashMap<ObjectId, Manifest>,
    out: &mut Vec<(StatusDelta, String)>,
) -> Result<()> {
    match kind {
        ManifestEntryKind::Dir { manifest } => {
            collect_leaves_current(path, manifest, cur_manifests, StatusDelta::Added, out)?;
        }
        _ => out.push((StatusDelta::Added, path.to_string())),
    }
    Ok(())
}

pub(super) fn handle_deleted(
    path: &str,
    kind: &ManifestEntryKind,
    store: &LocalStore,
    out: &mut Vec<(StatusDelta, String)>,
) -> Result<()> {
    match kind {
        ManifestEntryKind::Dir { manifest } => {
            collect_leaves_base(path, store, manifest, StatusDelta::Deleted, out)?;
        }
        _ => out.push((StatusDelta::Deleted, path.to_string())),
    }
    Ok(())
}

pub(super) fn handle_changed(
    path: &str,
    before: &ManifestEntryKind,
    after: &ManifestEntryKind,
    store: &LocalStore,
    cur_manifests: &std::collections::HashMap<ObjectId, Manifest>,
    out: &mut Vec<(StatusDelta, String)>,
) -> Result<()> {
    match (before, after) {
        (
            ManifestEntryKind::File {
                blob: b_blob,
                mode: b_mode,
                ..
            },
            ManifestEntryKind::File {
                blob: c_blob,
                mode: c_mode,
                ..
            },
        ) => {
            if b_blob != c_blob || b_mode != c_mode {
                out.push((StatusDelta::Modified, path.to_string()));
            }
        }
        (
            ManifestEntryKind::FileChunks {
                recipe: b_r,
                mode: b_mode,
                ..
            },
            ManifestEntryKind::FileChunks {
                recipe: c_r,
                mode: c_mode,
                ..
            },
        ) => {
            if b_r != c_r || b_mode != c_mode {
                out.push((StatusDelta::Modified, path.to_string()));
            }
        }
        (
            ManifestEntryKind::Symlink { target: b_t },
            ManifestEntryKind::Symlink { target: c_t },
        ) => {
            if b_t != c_t {
                out.push((StatusDelta::Modified, path.to_string()));
            }
        }
        (ManifestEntryKind::Dir { manifest: b_m }, ManifestEntryKind::Dir { manifest: c_m }) => {
            if b_m != c_m {
                diff_dir(path, store, Some(b_m), c_m, cur_manifests, out)?;
            }
        }
        _ => out.push((StatusDelta::Modified, path.to_string())),
    }
    Ok(())
}
