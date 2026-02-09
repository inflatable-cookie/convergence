use anyhow::Result;

use crate::model::{Manifest, ObjectId};
use crate::store::LocalStore;

use self::handlers::{handle_added, handle_changed, handle_deleted};
use super::StatusDelta;
use super::entries::{entries_by_name, join_path, merged_entry_names};

mod handlers;

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
