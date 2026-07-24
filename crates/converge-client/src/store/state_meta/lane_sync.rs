use anyhow::Result;

use crate::model::LaneSyncRecord;

use super::LocalStore;

impl LocalStore {
    pub fn set_lane_sync(&self, lane_id: &str, snap_id: &str, synced_at: &str) -> Result<()> {
        self.mutate_state(|st| {
            st.lane_sync.insert(
                lane_id.to_string(),
                LaneSyncRecord {
                    snap_id: snap_id.to_string(),
                    synced_at: synced_at.to_string(),
                },
            );
            Ok(())
        })
    }
}
