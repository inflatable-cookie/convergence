use super::*;
use crate::model::{Manifest, ManifestEntry, ManifestEntryKind, ObjectId};
use crate::store::LocalStore;
use tempfile::tempdir;

mod blob_edit;
mod fixtures;
mod recipe_edit;
mod same_blob;

fn setup_store() -> anyhow::Result<(tempfile::TempDir, LocalStore)> {
    let dir = tempdir()?;
    let store = LocalStore::init(dir.path(), false)?;
    Ok((dir, store))
}

fn manifest_with_file(name: &str, blob: &ObjectId, size: u64) -> Manifest {
    Manifest {
        version: 1,
        entries: vec![ManifestEntry {
            name: name.to_string(),
            kind: ManifestEntryKind::File {
                blob: blob.clone(),
                mode: 0o100644,
                size,
            },
        }],
    }
}
