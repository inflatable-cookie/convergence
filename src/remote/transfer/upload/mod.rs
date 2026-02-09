use anyhow::Result;

use crate::model::SnapRecord;
use crate::store::LocalStore;

use super::super::fetch::{collect_objects, manifest_postorder};
use super::super::{
    LaneHead, MissingObjectsRequest, MissingObjectsResponse, RemoteClient, with_retries,
};

mod missing;
mod upload_objects;

impl RemoteClient {
    pub fn upload_snap_objects(&self, store: &LocalStore, snap: &SnapRecord) -> Result<()> {
        // Reuse the publish upload path but skip publication creation.
        let (blobs, manifests, recipes) = collect_objects(store, &snap.root_manifest)?;
        let manifest_order = manifest_postorder(store, &snap.root_manifest)?;

        let missing = missing::query_missing_objects(self, snap, &blobs, &manifests, &recipes)?;
        upload_objects::upload_blobs(self, store, &missing.missing_blobs)?;
        upload_objects::upload_recipes(self, store, &missing.missing_recipes)?;
        upload_objects::upload_manifests(self, store, manifest_order, missing.missing_manifests)?;
        upload_objects::upload_snap_if_needed(self, snap, &missing.missing_snaps)?;

        Ok(())
    }

    pub fn sync_snap(
        &self,
        store: &LocalStore,
        snap: &SnapRecord,
        lane_id: &str,
        client_id: Option<String>,
    ) -> Result<LaneHead> {
        self.upload_snap_objects(store, snap)?;
        self.update_lane_head_me(lane_id, &snap.id, client_id)
    }
}
