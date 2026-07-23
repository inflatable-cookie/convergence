use std::collections::BTreeMap;

use anyhow::Result;

use crate::model::{Manifest, ManifestEntryKind, ObjectId};

use super::signatures::{EntrySig, sig_for_kind};

pub(super) fn join_path(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{}/{}", prefix, name)
    }
}

pub(super) fn walk_manifest(
    m: &Manifest,
    prefix: &str,
    out: &mut BTreeMap<String, EntrySig>,
    mut push_dir: impl FnMut((ObjectId, String)),
) -> Result<()> {
    for e in &m.entries {
        let path = join_path(prefix, &e.name);
        match &e.kind {
            ManifestEntryKind::Dir { manifest } => push_dir((manifest.clone(), path)),
            other => {
                out.insert(path, sig_for_kind(other)?);
            }
        }
    }
    Ok(())
}
