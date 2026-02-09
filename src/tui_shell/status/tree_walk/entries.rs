use crate::model::{Manifest, ManifestEntryKind};

pub(super) fn entries_by_name(
    m: &Manifest,
) -> std::collections::BTreeMap<String, ManifestEntryKind> {
    let mut out = std::collections::BTreeMap::new();
    for e in &m.entries {
        out.insert(e.name.clone(), e.kind.clone());
    }
    out
}

pub(super) fn merged_entry_names(
    base_entries: &std::collections::BTreeMap<String, ManifestEntryKind>,
    cur_entries: &std::collections::BTreeMap<String, ManifestEntryKind>,
) -> std::collections::BTreeSet<String> {
    let mut names = std::collections::BTreeSet::new();
    for k in base_entries.keys() {
        names.insert(k.clone());
    }
    for k in cur_entries.keys() {
        names.insert(k.clone());
    }
    names
}

pub(super) fn join_path(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{}/{}", prefix, name)
    }
}
