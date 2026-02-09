use anyhow::Result;

use crate::model::{Manifest, ManifestEntryKind, ObjectId};
use crate::store::LocalStore;

use super::StatusDelta;
use super::entries::{entries_by_name, join_path, merged_entry_names};
use super::leaves::{collect_leaves_base, collect_leaves_current};

pub(super) fn diff_dir(
    prefix: &str,
    store: &LocalStore,
    base_id: Option<&ObjectId>,
    cur_id: &ObjectId,
    cur_manifests: &std::collections::HashMap<ObjectId, Manifest>,
    out: &mut Vec<(StatusDelta, String)>,
) -> Result<()> {
    let base_entries = if let Some(id) = base_id {
        let m = store.get_manifest(id)?;
        entries_by_name(&m)
    } else {
        std::collections::BTreeMap::new()
    };

    let cur_manifest = cur_manifests
        .get(cur_id)
        .ok_or_else(|| anyhow::anyhow!("missing current manifest {}", cur_id.as_str()))?;
    let cur_entries = entries_by_name(cur_manifest);
    let names = merged_entry_names(&base_entries, &cur_entries);

    for name in names {
        let b = base_entries.get(&name);
        let c = cur_entries.get(&name);
        let path = join_path(prefix, &name);

        match (b, c) {
            (None, Some(kind)) => handle_added(&path, kind, cur_manifests, out)?,
            (Some(kind), None) => handle_deleted(&path, kind, store, out)?,
            (Some(bk), Some(ck)) => handle_changed(&path, bk, ck, store, cur_manifests, out)?,
            (None, None) => {}
        }
    }

    Ok(())
}

fn handle_added(
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

fn handle_deleted(
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

fn handle_changed(
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
