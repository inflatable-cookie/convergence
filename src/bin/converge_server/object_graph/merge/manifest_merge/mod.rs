use super::*;

mod variants;

pub(super) fn merge_dir_manifests(
    state: &AppState,
    repo_id: &str,
    inputs: &[(String, String)],
) -> Result<String, Response> {
    use std::collections::{BTreeMap, BTreeSet};

    let mut input_maps: Vec<(String, BTreeMap<String, converge::model::ManifestEntryKind>)> =
        Vec::new();
    for (pub_id, mid) in inputs {
        let manifest = read_manifest(state, repo_id, mid)?;
        let mut map = BTreeMap::new();
        for entry in manifest.entries {
            map.insert(entry.name, entry.kind);
        }
        input_maps.push((pub_id.clone(), map));
    }

    let mut names = BTreeSet::new();
    for (_, map) in &input_maps {
        for key in map.keys() {
            names.insert(key.clone());
        }
    }

    let mut out_entries = Vec::new();
    for name in names {
        let mut kinds: Vec<(String, Option<converge::model::ManifestEntryKind>)> = Vec::new();
        for (pub_id, map) in &input_maps {
            kinds.push((pub_id.clone(), map.get(&name).cloned()));
        }

        if let Some(entry) = try_merge_present_dir(state, repo_id, &name, &kinds)? {
            out_entries.push(entry);
            continue;
        }
        if let Some(entry) = try_merge_identical_scalar(&name, &kinds) {
            out_entries.push(entry);
            continue;
        }

        out_entries.push(variants::superposition_entry(name, kinds));
    }

    let merged = converge::model::Manifest {
        version: 1,
        entries: out_entries,
    };

    for entry in &merged.entries {
        validate_manifest_entry_refs(state, repo_id, &entry.kind, true)?;
    }

    store_manifest(state, repo_id, &merged)
}

fn try_merge_present_dir(
    state: &AppState,
    repo_id: &str,
    name: &str,
    kinds: &[(String, Option<converge::model::ManifestEntryKind>)],
) -> Result<Option<converge::model::ManifestEntry>, Response> {
    let all_present = kinds.iter().all(|(_, k)| k.is_some());
    if !all_present {
        return Ok(None);
    }

    let all_dirs = kinds
        .iter()
        .all(|(_, k)| matches!(k, Some(converge::model::ManifestEntryKind::Dir { .. })));
    if !all_dirs {
        return Ok(None);
    }

    let child_inputs = kinds
        .iter()
        .map(|(pub_id, k)| {
            let converge::model::ManifestEntryKind::Dir { manifest } =
                k.clone().expect("checked all_present")
            else {
                unreachable!();
            };
            (pub_id.clone(), manifest.as_str().to_string())
        })
        .collect::<Vec<_>>();
    let merged_child = merge_dir_manifests(state, repo_id, &child_inputs)?;
    Ok(Some(converge::model::ManifestEntry {
        name: name.to_string(),
        kind: converge::model::ManifestEntryKind::Dir {
            manifest: converge::model::ObjectId(merged_child),
        },
    }))
}

fn try_merge_identical_scalar(
    name: &str,
    kinds: &[(String, Option<converge::model::ManifestEntryKind>)],
) -> Option<converge::model::ManifestEntry> {
    let all_present = kinds.iter().all(|(_, k)| k.is_some());
    if !all_present {
        return None;
    }

    let first = kinds[0].1.clone().expect("checked all_present");
    let identical = kinds
        .iter()
        .all(|(_, k)| match k.clone().expect("checked all_present") {
            converge::model::ManifestEntryKind::File { .. } => {
                k.clone().expect("checked all_present") == first
            }
            converge::model::ManifestEntryKind::FileChunks { .. } => {
                k.clone().expect("checked all_present") == first
            }
            converge::model::ManifestEntryKind::Symlink { .. } => {
                k.clone().expect("checked all_present") == first
            }
            _ => false,
        });
    if !identical {
        return None;
    }

    Some(converge::model::ManifestEntry {
        name: name.to_string(),
        kind: first,
    })
}
