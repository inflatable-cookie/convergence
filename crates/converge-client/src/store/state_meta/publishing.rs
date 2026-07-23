use anyhow::Result;

use crate::model::RemoteConfig;

use super::LocalStore;

impl LocalStore {
    fn publish_key(&self, remote: &RemoteConfig, scope: &str, gate: &str) -> String {
        format!("{}#{}#{}#{}", remote.base_url, remote.repo_id, scope, gate)
    }

    pub fn get_last_published(
        &self,
        remote: &RemoteConfig,
        scope: &str,
        gate: &str,
    ) -> Result<Option<String>> {
        let st = self.read_state()?;
        if st.version != 1 {
            anyhow::bail!("unsupported workspace state version {}", st.version);
        }
        Ok(st
            .last_published
            .get(&self.publish_key(remote, scope, gate))
            .cloned())
    }

    pub fn set_last_published(
        &self,
        remote: &RemoteConfig,
        scope: &str,
        gate: &str,
        snap_id: &str,
    ) -> Result<()> {
        let mut st = self.read_state()?;
        if st.version != 1 {
            anyhow::bail!("unsupported workspace state version {}", st.version);
        }
        st.last_published
            .insert(self.publish_key(remote, scope, gate), snap_id.to_string());
        self.write_state(&st)
    }
}
