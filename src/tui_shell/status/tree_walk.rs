use anyhow::Result;

use crate::model::{Manifest, ManifestEntryKind, ObjectId};
use crate::store::LocalStore;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StatusDelta {
    Added,
    Modified,
    Deleted,
}

pub(super) fn diff_trees(
    store: &LocalStore,
    base_root: Option<&ObjectId>,
    cur_root: &ObjectId,
    cur_manifests: &std::collections::HashMap<ObjectId, Manifest>,
) -> Result<Vec<(StatusDelta, String)>> {
    let mut out = Vec::new();
    diff_dir("", store, base_root, cur_root, cur_manifests, &mut out)?;
    out.sort_by(|a, b| a.1.cmp(&b.1));
    Ok(out)
}

fn diff_dir(
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

    let mut names = std::collections::BTreeSet::new();
    for k in base_entries.keys() {
        names.insert(k.clone());
    }
    for k in cur_entries.keys() {
        names.insert(k.clone());
    }

    for name in names {
        let b = base_entries.get(&name);
        let c = cur_entries.get(&name);
        let path = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{}/{}", prefix, name)
        };

        match (b, c) {
            (None, Some(kind)) => match kind {
                ManifestEntryKind::Dir { manifest } => {
                    collect_leaves_current(
                        &path,
                        manifest,
                        cur_manifests,
                        StatusDelta::Added,
                        out,
                    )?;
                }
                _ => out.push((StatusDelta::Added, path)),
            },
            (Some(kind), None) => match kind {
                ManifestEntryKind::Dir { manifest } => {
                    collect_leaves_base(&path, store, manifest, StatusDelta::Deleted, out)?;
                }
                _ => out.push((StatusDelta::Deleted, path)),
            },
            (Some(bk), Some(ck)) => match (bk, ck) {
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
                        out.push((StatusDelta::Modified, path));
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
                        out.push((StatusDelta::Modified, path));
                    }
                }
                (
                    ManifestEntryKind::Symlink { target: b_t },
                    ManifestEntryKind::Symlink { target: c_t },
                ) => {
                    if b_t != c_t {
                        out.push((StatusDelta::Modified, path));
                    }
                }
                (
                    ManifestEntryKind::Dir { manifest: b_m },
                    ManifestEntryKind::Dir { manifest: c_m },
                ) => {
                    if b_m != c_m {
                        diff_dir(&path, store, Some(b_m), c_m, cur_manifests, out)?;
                    }
                }
                _ => {
                    out.push((StatusDelta::Modified, path));
                }
            },
            (None, None) => {}
        }
    }

    Ok(())
}

fn entries_by_name(m: &Manifest) -> std::collections::BTreeMap<String, ManifestEntryKind> {
    let mut out = std::collections::BTreeMap::new();
    for e in &m.entries {
        out.insert(e.name.clone(), e.kind.clone());
    }
    out
}

fn collect_leaves_current(
    prefix: &str,
    manifest_id: &ObjectId,
    cur_manifests: &std::collections::HashMap<ObjectId, Manifest>,
    kind: StatusDelta,
    out: &mut Vec<(StatusDelta, String)>,
) -> Result<()> {
    let m = cur_manifests
        .get(manifest_id)
        .ok_or_else(|| anyhow::anyhow!("missing current manifest {}", manifest_id.as_str()))?;
    for e in &m.entries {
        let path = if prefix.is_empty() {
            e.name.clone()
        } else {
            format!("{}/{}", prefix, e.name)
        };
        match &e.kind {
            ManifestEntryKind::Dir { manifest } => {
                collect_leaves_current(&path, manifest, cur_manifests, kind, out)?;
            }
            _ => out.push((kind, path)),
        }
    }
    Ok(())
}

fn collect_leaves_base(
    prefix: &str,
    store: &LocalStore,
    manifest_id: &ObjectId,
    kind: StatusDelta,
    out: &mut Vec<(StatusDelta, String)>,
) -> Result<()> {
    let m = store.get_manifest(manifest_id)?;
    for e in &m.entries {
        let path = if prefix.is_empty() {
            e.name.clone()
        } else {
            format!("{}/{}", prefix, e.name)
        };
        match &e.kind {
            ManifestEntryKind::Dir { manifest } => {
                collect_leaves_base(&path, store, manifest, kind, out)?;
            }
            _ => out.push((kind, path)),
        }
    }
    Ok(())
}
