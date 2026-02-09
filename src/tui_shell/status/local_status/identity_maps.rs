use anyhow::Result;

use crate::model::{Manifest, ObjectId, SnapRecord};
use crate::workspace::Workspace;

use super::super::identity_collect::{collect_identities_base, collect_identities_current};
use super::super::rename_helpers::IdentityKey;

pub(super) type IdentityMap = std::collections::HashMap<String, IdentityKey>;

pub(super) fn base_identities(
    ws: &Workspace,
    baseline: Option<&SnapRecord>,
) -> Result<Option<IdentityMap>> {
    if let Some(s) = baseline {
        let mut m = std::collections::HashMap::new();
        collect_identities_base("", &ws.store, &s.root_manifest, &mut m)?;
        Ok(Some(m))
    } else {
        Ok(None)
    }
}

pub(super) fn current_identities(
    cur_root: &ObjectId,
    cur_manifests: &std::collections::HashMap<ObjectId, Manifest>,
) -> Result<IdentityMap> {
    let mut cur_ids = std::collections::HashMap::new();
    collect_identities_current("", cur_root, cur_manifests, &mut cur_ids)?;
    Ok(cur_ids)
}
